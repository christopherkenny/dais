use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::header::{AUTHORIZATION, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use dais_core::bus::CommandSender;
use dais_core::commands::Command;
use dais_core::config::RemoteConfig;
use dais_core::keybindings::Action;
use dais_core::state::{PresentationState, TimerPhase};
use dais_document::page::RenderSize;
use dais_document::source::DocumentSource;
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;

const SSE_INTERVAL: Duration = Duration::from_millis(500);
const REMOTE_SLIDE_SIZE: RenderSize = RenderSize { width: 960, height: 540 };
const X_DAIS_TOKEN: HeaderName = HeaderName::from_static("x-dais-token");

/// CLI-friendly remote endpoint selection.
#[derive(Debug, Clone)]
pub struct RemoteEndpoint {
    pub host: String,
    pub port: u16,
    pub token: Option<String>,
}

impl Default for RemoteEndpoint {
    fn default() -> Self {
        Self { host: "127.0.0.1".to_string(), port: 4317, token: None }
    }
}

/// Runtime override values supplied by CLI flags.
#[derive(Debug, Clone, Default)]
pub struct RemoteOverrides {
    pub enabled: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
}

/// Effective server settings after config and CLI overrides.
#[derive(Debug, Clone)]
pub struct ServerSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub allow_unauthenticated_loopback: bool,
}

impl ServerSettings {
    pub fn from_config(config: &RemoteConfig, overrides: &RemoteOverrides) -> Self {
        let host = overrides.host.clone().unwrap_or_else(|| config.host.clone());
        let port = overrides.port.unwrap_or(config.port);
        let enabled = overrides.enabled || config.enabled;
        let token = if config.token.is_empty() { generate_token() } else { config.token.clone() };
        Self {
            enabled,
            host,
            port,
            token,
            allow_unauthenticated_loopback: config.allow_unauthenticated_loopback,
        }
    }
}

/// Handle for the background remote API server.
pub struct RemoteServer {
    addr: SocketAddr,
    token: String,
    allow_unauthenticated_loopback: bool,
    urls: Vec<String>,
    status: RemoteStatusHandle,
    shutdown: Option<oneshot::Sender<()>>,
    _handle: thread::JoinHandle<()>,
}

impl RemoteServer {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn requires_token_for_non_loopback(&self) -> bool {
        !self.allow_unauthenticated_loopback || !self.addr.ip().is_loopback()
    }

    pub fn urls(&self) -> &[String] {
        &self.urls
    }

    pub fn status(&self) -> RemoteStatus {
        self.status.snapshot()
    }

    pub fn status_handle(&self) -> RemoteStatusHandle {
        self.status.clone()
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        // Do not join here: active browser remotes can hold SSE connections open,
        // and the presenter console must remain authoritative over app shutdown.
    }
}

/// Stable state DTO returned by the remote API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteState {
    pub current_page: usize,
    pub current_logical_slide: usize,
    pub current_overlay_within_group: usize,
    pub total_pages: usize,
    pub total_logical_slides: usize,
    pub audience_page: usize,
    pub frozen: bool,
    pub blacked_out: bool,
    pub whiteboard_active: bool,
    pub screen_share_mode: bool,
    pub presentation_mode: bool,
    pub laser_active: bool,
    pub pointer_position: Option<(f32, f32)>,
    pub ink_active: bool,
    pub spotlight_active: bool,
    pub spotlight_position: Option<(f32, f32)>,
    pub zoom_active: bool,
    pub overview_visible: bool,
    pub notes_visible: bool,
    pub notes_editing: bool,
    pub current_notes: Option<String>,
    pub timer: RemoteTimerState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteTimerState {
    pub running: bool,
    pub elapsed_seconds: u64,
    pub display_seconds: u64,
    pub phase: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteStatus {
    pub active_event_clients: usize,
    pub last_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GoToRequest {
    slide: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct PointerRequest {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct TimerRequest {
    action: TimerRemoteAction,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TimerRemoteAction {
    Start,
    Pause,
    Toggle,
    Reset,
}

#[derive(Clone)]
struct HandlerContext {
    settings: Arc<ServerSettings>,
    sender: CommandSender,
    shared_state: Arc<RwLock<PresentationState>>,
    doc: Arc<dyn DocumentSource>,
    status: RemoteStatusHandle,
    png_cache: Arc<Mutex<HashMap<usize, Vec<u8>>>>,
}

#[derive(Debug, Clone, Default)]
pub struct RemoteStatusHandle {
    inner: Arc<RwLock<RemoteStatus>>,
}

impl RemoteStatusHandle {
    pub fn snapshot(&self) -> RemoteStatus {
        self.inner.read().map_or_else(|_| RemoteStatus::default(), |status| status.clone())
    }

    fn set_last_command(&self, command: impl Into<String>) {
        if let Ok(mut status) = self.inner.write() {
            status.last_command = Some(command.into());
        }
    }

    fn add_event_client(&self) {
        if let Ok(mut status) = self.inner.write() {
            status.active_event_clients += 1;
        }
    }

    fn remove_event_client(&self) {
        if let Ok(mut status) = self.inner.write() {
            status.active_event_clients = status.active_event_clients.saturating_sub(1);
        }
    }
}

pub fn start_server(
    mut settings: ServerSettings,
    sender: CommandSender,
    shared_state: Arc<RwLock<PresentationState>>,
    doc: Arc<dyn DocumentSource>,
) -> Result<RemoteServer> {
    let std_listener = std::net::TcpListener::bind((settings.host.as_str(), settings.port))
        .with_context(|| {
            format!("failed to bind remote API to {}:{}", settings.host, settings.port)
        })?;
    std_listener.set_nonblocking(true).context("failed to set remote API listener nonblocking")?;
    let addr = std_listener.local_addr().context("failed to read remote API address")?;
    settings.port = addr.port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let token = settings.token.clone();
    let allow_unauthenticated_loopback = server_settings_allow_loopback(&settings);
    let urls = remote_urls(addr);
    let status = RemoteStatusHandle::default();
    let context = HandlerContext {
        settings: Arc::new(settings),
        sender,
        shared_state,
        doc,
        status: status.clone(),
        png_cache: Arc::new(Mutex::new(HashMap::new())),
    };

    let handle = thread::spawn(move || match tokio::runtime::Runtime::new() {
        Ok(runtime) => {
            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::from_std(std_listener) {
                    Ok(listener) => listener,
                    Err(error) => {
                        tracing::error!("failed to create remote API listener: {error}");
                        return;
                    }
                };
                let app = remote_router(context);
                if let Err(error) =
                    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
                        .with_graceful_shutdown(async {
                            let _ = shutdown_rx.await;
                        })
                        .await
                {
                    tracing::warn!("remote API server stopped with error: {error}");
                }
            });
        }
        Err(error) => tracing::error!("failed to start remote API runtime: {error}"),
    });

    Ok(RemoteServer {
        addr,
        token,
        allow_unauthenticated_loopback,
        urls,
        status,
        shutdown: Some(shutdown_tx),
        _handle: handle,
    })
}

fn remote_router(context: HandlerContext) -> Router {
    Router::new()
        .route("/", get(web_remote))
        .route("/remote", get(web_remote))
        .route("/api/v1/state", get(get_state))
        .route("/api/v1/events", get(events))
        .route("/api/v1/remote-status", get(remote_status))
        .route("/api/v1/actions/{action}", post(action))
        .route("/api/v1/commands/goto", post(goto))
        .route("/api/v1/commands/pointer", post(pointer))
        .route("/api/v1/commands/timer", post(timer))
        .route("/api/v1/slides/current.png", get(current_slide_png))
        .route("/api/v1/slides/next.png", get(next_slide_png))
        .route("/api/v1/slides/{slide}/thumbnail.png", get(thumbnail_png))
        .layer(SetSensitiveRequestHeadersLayer::new([AUTHORIZATION, X_DAIS_TOKEN]))
        .route_layer(middleware::from_fn_with_state(context.clone(), request_guard))
        .with_state(context)
}

async fn request_guard(
    State(context): State<HandlerContext>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let query = request.uri().query();
    if !host_header_allowed(&headers, &context.settings) || !origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    if !is_authorized(&headers, query, peer, &context.settings) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    next.run(request).await
}

async fn web_remote() -> Html<&'static str> {
    Html(remote_html())
}

async fn get_state(State(context): State<HandlerContext>) -> Response {
    match remote_state_snapshot(&context.shared_state) {
        Ok((state, pages)) => {
            preload_pages(&context, pages);
            Json(state).into_response()
        }
        Err(error) => server_error(&error),
    }
}

async fn remote_status(State(context): State<HandlerContext>) -> Json<RemoteStatus> {
    Json(context.status.snapshot())
}

async fn current_slide_png(State(context): State<HandlerContext>) -> Response {
    match current_page(&context.shared_state).and_then(|page| cached_png(&context, page)) {
        Ok(png) => png_response(png),
        Err(error) => server_error(&error),
    }
}

async fn next_slide_png(State(context): State<HandlerContext>) -> Response {
    match next_page(&context.shared_state).and_then(|page| cached_png(&context, page)) {
        Ok(png) => png_response(png),
        Err(error) => server_error(&error),
    }
}

async fn thumbnail_png(
    State(context): State<HandlerContext>,
    Path(slide): Path<usize>,
) -> Response {
    if slide == 0 {
        return bad_request(&anyhow!("slide must be 1 or greater"));
    }

    match logical_slide_page(&context.shared_state, slide - 1)
        .and_then(|page| cached_png(&context, page))
    {
        Ok(png) => png_response(png),
        Err(error) => bad_request(&error),
    }
}

async fn action(State(context): State<HandlerContext>, Path(action): Path<String>) -> Response {
    match send_remote_action(&context.sender, &action) {
        Ok(()) => {
            context.status.set_last_command(action);
            preload_from_shared_state(&context);
            Json(ok_response("action dispatched")).into_response()
        }
        Err(error) => bad_request(&error),
    }
}

async fn goto(State(context): State<HandlerContext>, Json(body): Json<GoToRequest>) -> Response {
    if body.slide == 0 {
        return bad_request(&anyhow!("slide must be 1 or greater"));
    }

    match context.sender.send(Command::GoToSlide(body.slide - 1)) {
        Ok(()) => {
            context.status.set_last_command(format!("goto {}", body.slide));
            preload_from_shared_state(&context);
            Json(ok_response("goto dispatched")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn pointer(
    State(context): State<HandlerContext>,
    Json(body): Json<PointerRequest>,
) -> Response {
    match context.sender.send(Command::SetPointerPosition(body.x, body.y)) {
        Ok(()) => {
            context.status.set_last_command("pointer");
            preload_from_shared_state(&context);
            Json(ok_response("pointer dispatched")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn timer(State(context): State<HandlerContext>, Json(body): Json<TimerRequest>) -> Response {
    let command = match body.action {
        TimerRemoteAction::Start => Command::StartTimer,
        TimerRemoteAction::Pause => Command::PauseTimer,
        TimerRemoteAction::Toggle => Command::ToggleTimer,
        TimerRemoteAction::Reset => Command::ResetTimer,
    };

    match context.sender.send(command) {
        Ok(()) => {
            context.status.set_last_command(format!("timer {:?}", body.action));
            preload_from_shared_state(&context);
            Json(ok_response("timer command dispatched")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn events(
    State(context): State<HandlerContext>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    context.status.add_event_client();
    let status = context.status.clone();
    let shared_state = Arc::clone(&context.shared_state);
    let stream = async_stream::stream! {
        let _guard = EventClientGuard { status };
        let mut interval = tokio::time::interval(SSE_INTERVAL);
        loop {
            interval.tick().await;
            match remote_state_snapshot(&shared_state) {
                Ok((state, _pages)) => {
                    let json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
                    yield Ok(Event::default().data(json));
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

struct EventClientGuard {
    status: RemoteStatusHandle,
}

impl Drop for EventClientGuard {
    fn drop(&mut self) {
        self.status.remove_event_client();
    }
}

fn png_response(png: Vec<u8>) -> Response {
    ([(axum::http::header::CONTENT_TYPE, "image/png")], png).into_response()
}

fn bad_request(error: &anyhow::Error) -> Response {
    (StatusCode::BAD_REQUEST, error.to_string()).into_response()
}

fn server_error(error: &anyhow::Error) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
}

fn server_settings_allow_loopback(settings: &ServerSettings) -> bool {
    settings.allow_unauthenticated_loopback && host_is_loopback(&settings.host)
}

pub fn remote_state(state: &PresentationState) -> RemoteState {
    RemoteState {
        current_page: state.current_page,
        current_logical_slide: state.current_logical_slide,
        current_overlay_within_group: state.current_overlay_within_group,
        total_pages: state.total_pages,
        total_logical_slides: state.total_logical_slides,
        audience_page: state.audience_page(),
        frozen: state.frozen,
        blacked_out: state.blacked_out,
        whiteboard_active: state.whiteboard_active,
        screen_share_mode: state.screen_share_mode,
        presentation_mode: state.presentation_mode,
        laser_active: state.laser_active,
        pointer_position: state.pointer_position,
        ink_active: state.ink_active,
        spotlight_active: state.spotlight_active,
        spotlight_position: state.spotlight_position,
        zoom_active: state.zoom_active,
        overview_visible: state.overview_visible,
        notes_visible: state.notes_visible,
        notes_editing: state.notes_editing,
        current_notes: state.current_notes.clone(),
        timer: RemoteTimerState {
            running: state.timer.running,
            elapsed_seconds: state.timer.elapsed.as_secs(),
            display_seconds: state.timer.display_time().as_secs(),
            phase: timer_phase_name(state.timer.phase()).to_string(),
        },
    }
}

pub fn command_for_action_name(name: &str) -> Option<Command> {
    action_to_command(Action::from_config_name(name)?)
}

pub fn send_remote_action(sender: &CommandSender, name: &str) -> Result<()> {
    let command =
        command_for_action_name(name).ok_or_else(|| anyhow!("unknown remote action '{name}'"))?;
    sender.send(command).map_err(|_| anyhow!("presentation engine is not accepting commands"))
}

pub fn client_get_state(endpoint: &RemoteEndpoint) -> Result<RemoteState> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .context("failed to create remote API client")?;
    let request = client.get(endpoint_url(endpoint, "/api/v1/state"));
    let request =
        if let Some(token) = &endpoint.token { request.bearer_auth(token) } else { request };
    let response = request.send().with_context(|| {
        format!("failed to connect to remote API at {}:{}", endpoint.host, endpoint.port)
    })?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().unwrap_or_default();
        return Err(anyhow!("remote API returned {}: {text}", status.as_u16()));
    }
    response.json().context("failed to parse remote state")
}

pub fn client_action(endpoint: &RemoteEndpoint, action: &str) -> Result<CommandResponse> {
    client_json_request(endpoint, "POST", &format!("/api/v1/actions/{action}"), &())
}

pub fn client_goto(endpoint: &RemoteEndpoint, slide: usize) -> Result<CommandResponse> {
    client_json_request(
        endpoint,
        "POST",
        "/api/v1/commands/goto",
        &serde_json::json!({ "slide": slide }),
    )
}

pub fn client_pointer(endpoint: &RemoteEndpoint, x: f32, y: f32) -> Result<CommandResponse> {
    client_json_request(
        endpoint,
        "POST",
        "/api/v1/commands/pointer",
        &serde_json::json!({ "x": x, "y": y }),
    )
}

pub fn client_timer(endpoint: &RemoteEndpoint, action: &str) -> Result<CommandResponse> {
    client_json_request(
        endpoint,
        "POST",
        "/api/v1/commands/timer",
        &serde_json::json!({ "action": action }),
    )
}

fn client_json_request<T: Serialize>(
    endpoint: &RemoteEndpoint,
    method: &str,
    path: &str,
    body: &T,
) -> Result<CommandResponse> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .context("failed to create remote API client")?;
    let request = client.request(method.parse()?, endpoint_url(endpoint, path)).json(body);
    let request =
        if let Some(token) = &endpoint.token { request.bearer_auth(token) } else { request };
    let response = request.send().with_context(|| {
        format!("failed to connect to remote API at {}:{}", endpoint.host, endpoint.port)
    })?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().unwrap_or_default();
        return Err(anyhow!("remote API returned {}: {text}", status.as_u16()));
    }
    response.json().context("failed to parse remote command response")
}

fn endpoint_url(endpoint: &RemoteEndpoint, path: &str) -> String {
    format!("http://{}:{}{path}", endpoint.host, endpoint.port)
}

fn remote_state_snapshot(
    shared_state: &Arc<RwLock<PresentationState>>,
) -> Result<(RemoteState, Vec<usize>)> {
    let state = shared_state.read().map_err(|_| anyhow!("state lock poisoned"))?;
    let dto = remote_state(&state);
    let pages = preloaded_pages(&state);
    Ok((dto, pages))
}

fn ok_response(message: &str) -> CommandResponse {
    CommandResponse { ok: true, message: message.to_string() }
}

fn is_authorized(
    headers: &HeaderMap,
    query: Option<&str>,
    peer: SocketAddr,
    settings: &ServerSettings,
) -> bool {
    if settings.allow_unauthenticated_loopback && peer.ip().is_loopback() {
        return true;
    }

    request_token(headers, query).is_some_and(|token| token == settings.token)
}

fn request_token<'a>(headers: &'a HeaderMap, query: Option<&'a str>) -> Option<&'a str> {
    if let Some(token) = query_token(query) {
        return Some(token);
    }
    if let Some(auth) = header_str(headers, AUTHORIZATION) {
        return auth.strip_prefix("Bearer ").or(Some(auth));
    }
    header_str(headers, &X_DAIS_TOKEN)
}

fn query_token(query: Option<&str>) -> Option<&str> {
    query?
        .split('&')
        .filter_map(|part| part.split_once('='))
        .find_map(|(key, value)| (key == "token").then_some(value))
}

fn header_str(headers: &HeaderMap, name: impl axum::http::header::AsHeaderName) -> Option<&str> {
    headers.get(name)?.to_str().ok()
}

fn host_header_allowed(headers: &HeaderMap, settings: &ServerSettings) -> bool {
    let Some(host) = header_str(headers, HOST) else {
        return false;
    };
    let Some((name, port)) = split_host_port(host) else {
        return false;
    };
    port == effective_port(settings) && host_name_allowed(name, settings)
}

fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = header_str(headers, ORIGIN) else {
        return true;
    };
    let Some(host) = header_str(headers, HOST) else {
        return false;
    };
    origin == format!("http://{host}") || origin == format!("https://{host}")
}

fn split_host_port(host: &str) -> Option<(&str, u16)> {
    let host = host.trim();
    let (name, port) = host.rsplit_once(':')?;
    Some((name.trim_matches(['[', ']']), port.parse().ok()?))
}

fn effective_port(settings: &ServerSettings) -> u16 {
    settings.port
}

fn host_name_allowed(name: &str, settings: &ServerSettings) -> bool {
    if name.eq_ignore_ascii_case("localhost") {
        return true;
    }

    let Ok(host_ip) = name.parse::<IpAddr>() else {
        return false;
    };

    if host_ip.is_loopback() {
        return true;
    }

    let Ok(bind_ip) = settings.host.parse::<IpAddr>() else {
        return false;
    };

    host_ip == bind_ip
        || bind_ip.is_unspecified() && likely_lan_ip().is_some_and(|lan_ip| host_ip == lan_ip)
}

fn host_is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback())
}

fn generate_token() -> String {
    let code = fastrand::u32(0..100_000_000);
    format!("{:04}-{:04}", code / 10_000, code % 10_000)
}

fn current_page(shared_state: &Arc<RwLock<PresentationState>>) -> Result<usize> {
    Ok(shared_state.read().map_err(|_| anyhow!("state lock poisoned"))?.current_page)
}

fn next_page(shared_state: &Arc<RwLock<PresentationState>>) -> Result<usize> {
    let state = shared_state.read().map_err(|_| anyhow!("state lock poisoned"))?;
    let group = state.current_logical_slide.saturating_add(1);
    if let Some(next) = state.slide_groups.get(group).and_then(|group| group.pages.first()) {
        Ok(*next)
    } else {
        Ok(state.current_page)
    }
}

fn logical_slide_page(
    shared_state: &Arc<RwLock<PresentationState>>,
    logical_slide: usize,
) -> Result<usize> {
    let state = shared_state.read().map_err(|_| anyhow!("state lock poisoned"))?;
    state
        .slide_groups
        .get(logical_slide)
        .and_then(|group| group.pages.first())
        .copied()
        .ok_or_else(|| anyhow!("logical slide out of range"))
}

fn cached_png(context: &HandlerContext, page_index: usize) -> Result<Vec<u8>> {
    if let Ok(cache) = context.png_cache.lock()
        && let Some(png) = cache.get(&page_index)
    {
        return Ok(png.clone());
    }

    let png = render_png(&context.doc, page_index)?;
    if let Ok(mut cache) = context.png_cache.lock() {
        cache.insert(page_index, png.clone());
        if cache.len() > 12
            && let Some(oldest) = cache.keys().copied().next()
        {
            cache.remove(&oldest);
        }
    }
    Ok(png)
}

fn preload_from_shared_state(context: &HandlerContext) {
    if let Ok((_state, pages)) = remote_state_snapshot(&context.shared_state) {
        preload_pages(context, pages);
    }
}

fn preloaded_pages(state: &PresentationState) -> Vec<usize> {
    [
        Some(state.current_page),
        state
            .slide_groups
            .get(state.current_logical_slide.saturating_add(1))
            .and_then(|group| group.pages.first())
            .copied(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn preload_pages(context: &HandlerContext, pages: Vec<usize>) {
    for page in pages {
        let _ = cached_png(context, page);
    }
}

fn render_png(doc: &Arc<dyn DocumentSource>, page_index: usize) -> Result<Vec<u8>> {
    let page = doc
        .render_page(page_index, REMOTE_SLIDE_SIZE)
        .with_context(|| format!("failed to render page {}", page_index + 1))?;
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&page.data, page.width, page.height, ColorType::Rgba8.into())
        .context("failed to encode slide PNG")?;
    Ok(png)
}

fn remote_urls(addr: SocketAddr) -> Vec<String> {
    let mut urls = Vec::new();
    if addr.ip().is_unspecified() {
        urls.push(format!("http://127.0.0.1:{}/remote", addr.port()));
        if let Some(ip) = likely_lan_ip() {
            urls.push(format!("http://{}:{}/remote", ip, addr.port()));
        }
    } else {
        urls.push(format!("http://{addr}/remote"));
    }

    if !addr.ip().is_loopback()
        && !addr.ip().is_unspecified()
        && let Some(ip) = likely_lan_ip()
    {
        urls.push(format!("http://{}:{}/remote", ip, addr.port()));
    }
    urls.sort();
    urls.dedup();
    urls
}

fn likely_lan_ip() -> Option<IpAddr> {
    let socket = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("8.8.8.8", 80)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback()).then_some(ip)
}

fn remote_html() -> &'static str {
    include_str!("../assets/remote.html")
}

fn timer_phase_name(phase: TimerPhase) -> &'static str {
    match phase {
        TimerPhase::Normal => "normal",
        TimerPhase::Warning => "warning",
        TimerPhase::Overrun => "overrun",
    }
}

fn action_to_command(action: Action) -> Option<Command> {
    match action {
        Action::NextSlide => Some(Command::NextSlide),
        Action::PreviousSlide => Some(Command::PreviousSlide),
        Action::NextOverlay => Some(Command::NextOverlay),
        Action::PreviousOverlay => Some(Command::PreviousOverlay),
        Action::FirstSlide => Some(Command::FirstSlide),
        Action::LastSlide => Some(Command::LastSlide),
        Action::ToggleFreeze => Some(Command::ToggleFreeze),
        Action::ToggleBlackout => Some(Command::ToggleBlackout),
        Action::ToggleWhiteboard => Some(Command::ToggleWhiteboard),
        Action::ToggleLaser => Some(Command::ToggleLaser),
        Action::CycleLaserStyle => Some(Command::CycleLaserStyle),
        Action::ToggleInk => Some(Command::ToggleInk),
        Action::ClearInk => Some(Command::ClearInk),
        Action::CycleInkColor => Some(Command::CycleInkColor),
        Action::CycleInkWidth => Some(Command::CycleInkWidth),
        Action::ToggleSpotlight => Some(Command::ToggleSpotlight),
        Action::ToggleZoom => Some(Command::ToggleZoom),
        Action::ToggleOverview => Some(Command::ToggleSlideOverview),
        Action::ToggleNotes => Some(Command::ToggleNotesPanel),
        Action::ToggleNotesEdit => None,
        Action::StartPauseTimer => Some(Command::ToggleTimer),
        Action::ResetTimer => Some(Command::ResetTimer),
        Action::IncrementNotesFont => None,
        Action::DecrementNotesFont => None,
        Action::ToggleScreenShare => Some(Command::ToggleScreenShareMode),
        Action::TogglePresentationMode => Some(Command::TogglePresentationMode),
        Action::ToggleTextBoxMode => None,
        Action::Quit => None,
        Action::SaveSidecar => None,
        Action::GoToSlide => None,
    }
}

#[cfg(test)]
mod tests {
    use dais_core::bus::CommandBus;
    use dais_core::slide_group::SlideGroup;
    use dais_document::page::{PageDimensions, RenderedPage};
    use dais_document::source::{DocumentError, EmbeddedMetadata, OutlineEntry};

    use super::*;

    fn test_state() -> PresentationState {
        let mut state = PresentationState::new(
            3,
            vec![
                SlideGroup { logical_index: 0, pages: vec![0], notes: Some("hello".to_string()) },
                SlideGroup { logical_index: 1, pages: vec![1], notes: None },
                SlideGroup { logical_index: 2, pages: vec![2], notes: None },
            ],
        );
        state.current_page = 1;
        state.current_logical_slide = 1;
        state.blacked_out = true;
        state.timer.running = true;
        state
    }

    struct TestDoc;

    impl DocumentSource for TestDoc {
        fn page_count(&self) -> usize {
            3
        }

        fn page_dimensions(&self, _page_index: usize) -> PageDimensions {
            PageDimensions { width_pts: 16.0, height_pts: 9.0 }
        }

        fn render_page(
            &self,
            _page_index: usize,
            _target_size: RenderSize,
        ) -> std::result::Result<RenderedPage, DocumentError> {
            Ok(RenderedPage { data: vec![255; 4], width: 1, height: 1 })
        }

        fn embedded_metadata(&self) -> Option<EmbeddedMetadata> {
            None
        }

        fn outline(&self) -> Option<Vec<OutlineEntry>> {
            None
        }
    }

    fn test_doc() -> Arc<dyn DocumentSource> {
        Arc::new(TestDoc)
    }

    fn test_server(sender: CommandSender) -> RemoteServer {
        start_server(
            ServerSettings {
                enabled: true,
                host: "127.0.0.1".to_string(),
                port: 0,
                token: "secret".to_string(),
                allow_unauthenticated_loopback: true,
            },
            sender,
            Arc::new(RwLock::new(test_state())),
            test_doc(),
        )
        .unwrap()
    }

    fn endpoint(server: &RemoteServer) -> RemoteEndpoint {
        RemoteEndpoint { host: "127.0.0.1".to_string(), port: server.addr().port(), token: None }
    }

    fn headers(host: &str) -> HeaderMap {
        HeaderMap::from_iter([(HOST, host.parse().unwrap())])
    }

    #[test]
    fn action_names_map_to_commands() {
        assert_eq!(command_for_action_name("next_slide"), Some(Command::NextSlide));
        assert_eq!(command_for_action_name("start_pause_timer"), Some(Command::ToggleTimer));
        assert_eq!(command_for_action_name("go_to_slide"), None);
        assert_eq!(command_for_action_name("quit"), None);
        assert_eq!(command_for_action_name("save_sidecar"), None);
        assert_eq!(command_for_action_name("toggle_notes_edit"), None);
        assert_eq!(command_for_action_name("missing"), None);
    }

    #[test]
    fn state_dto_uses_stable_fields() {
        let dto = remote_state(&test_state());
        assert_eq!(dto.current_page, 1);
        assert_eq!(dto.current_logical_slide, 1);
        assert!(dto.blacked_out);
        assert!(dto.timer.running);
        assert_eq!(dto.current_notes.as_deref(), Some("hello"));
    }

    #[test]
    fn valid_action_dispatches_command() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();

        send_remote_action(&sender, "next_slide").unwrap();

        assert_eq!(receiver.try_recv(), Some(Command::NextSlide));
    }

    #[test]
    fn unknown_action_returns_error() {
        let bus = CommandBus::new();
        let sender = bus.sender();

        assert!(send_remote_action(&sender, "not_real").is_err());
    }

    #[test]
    fn http_action_route_dispatches_command() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        let response = client_action(&endpoint(&server), "next_slide").unwrap();

        assert!(response.ok);
        assert_eq!(receiver.try_recv(), Some(Command::NextSlide));
    }

    #[test]
    fn http_goto_rejects_zero_slide() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        let error = client_goto(&endpoint(&server), 0).unwrap_err().to_string();

        assert!(error.contains("400"));
        assert_eq!(receiver.try_recv(), None);
    }

    #[test]
    fn web_remote_route_serves_html() {
        let bus = CommandBus::new();
        let server = test_server(bus.sender());

        let response = reqwest::blocking::get(format!("http://{}/remote", server.addr())).unwrap();
        let content_type =
            response.headers().get("content-type").unwrap().to_str().unwrap().to_string();
        let body = response.text().unwrap();

        assert!(content_type.starts_with("text/html"));
        assert!(body.contains("Dais Remote"));
    }

    #[test]
    fn slide_png_route_renders_image() {
        let bus = CommandBus::new();
        let server = test_server(bus.sender());

        let response =
            reqwest::blocking::get(format!("http://{}/api/v1/slides/current.png", server.addr()))
                .unwrap();
        let content_type =
            response.headers().get("content-type").unwrap().to_str().unwrap().to_string();
        let body = response.bytes().unwrap();

        assert_eq!(content_type, "image/png");
        assert!(body.starts_with(b"\x89PNG"));
    }

    #[test]
    fn query_token_authorizes_request() {
        let settings = ServerSettings {
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 4317,
            token: "secret".to_string(),
            allow_unauthenticated_loopback: false,
        };
        let headers = HeaderMap::new();

        assert!(is_authorized(
            &headers,
            Some("token=secret"),
            "127.0.0.1:50000".parse().unwrap(),
            &settings
        ));
    }

    #[test]
    fn loopback_peer_can_use_loopback_exemption_on_wildcard_bind() {
        let settings = ServerSettings {
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 4317,
            token: "secret".to_string(),
            allow_unauthenticated_loopback: true,
        };
        let headers = HeaderMap::new();

        assert!(is_authorized(&headers, None, "127.0.0.1:50000".parse().unwrap(), &settings));
    }

    #[test]
    fn non_loopback_peer_requires_token_even_when_loopback_exemption_is_enabled() {
        let settings = ServerSettings {
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 4317,
            token: "secret".to_string(),
            allow_unauthenticated_loopback: true,
        };
        let headers = HeaderMap::new();

        assert!(!is_authorized(&headers, None, "192.168.1.50:50000".parse().unwrap(), &settings));
    }

    #[test]
    fn wildcard_bind_urls_do_not_advertise_unspecified_address() {
        let urls = remote_urls("0.0.0.0:4317".parse().unwrap());

        assert!(urls.iter().any(|url| url == "http://127.0.0.1:4317/remote"));
        assert!(!urls.iter().any(|url| url.contains("0.0.0.0")));
    }

    #[test]
    fn host_header_rejects_unknown_hostname_even_on_matching_port() {
        let settings = ServerSettings {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 4317,
            token: "secret".to_string(),
            allow_unauthenticated_loopback: true,
        };
        let headers = headers("attacker.example:4317");

        assert!(!host_header_allowed(&headers, &settings));
    }

    #[test]
    fn host_header_allows_loopback_names_on_matching_port() {
        let settings = ServerSettings {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 4317,
            token: "secret".to_string(),
            allow_unauthenticated_loopback: true,
        };
        let headers = headers("localhost:4317");

        assert!(host_header_allowed(&headers, &settings));
    }

    #[test]
    fn generated_token_is_human_typeable() {
        let token = generate_token();

        assert_eq!(token.len(), 9);
        assert_eq!(token.as_bytes()[4], b'-');
        assert!(token.chars().filter(char::is_ascii_digit).count() == 8);
    }
}
