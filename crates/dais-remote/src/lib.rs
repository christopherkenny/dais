use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::extract::{ConnectInfo, Path, Query, Request, State};
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
use dais_document::render_pipeline::FALLBACK_RENDER_SIZE;
use dais_document::source::DocumentSource;
use dais_document::typst_renderer::render_text_box_svg;
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;

const SSE_INTERVAL: Duration = Duration::from_millis(500);
const REMOTE_SLIDE_SIZE: RenderSize = RenderSize { width: 960, height: 540 };
const REMOTE_THUMBNAIL_SIZE: RenderSize = RenderSize { width: 320, height: 180 };
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
    pub generated_token: bool,
    pub allow_unauthenticated_loopback: bool,
}

impl ServerSettings {
    pub fn from_config(config: &RemoteConfig, overrides: &RemoteOverrides) -> Self {
        let host = overrides.host.clone().unwrap_or_else(|| config.host.clone());
        let port = overrides.port.unwrap_or(config.port);
        let enabled = overrides.enabled || config.enabled;
        let generated_token = config.token.is_empty();
        let token = if generated_token { generate_token() } else { config.token.clone() };
        Self {
            enabled,
            host,
            port,
            token,
            generated_token,
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
    pub draw_tool: String,
    pub eraser_radius: f32,
    pub spotlight_active: bool,
    pub spotlight_position: Option<(f32, f32)>,
    pub zoom_active: bool,
    pub overview_visible: bool,
    pub notes_visible: bool,
    pub notes_editing: bool,
    pub current_notes: Option<String>,
    pub ink_pen_color: [u8; 4],
    pub ink_pen_width: f32,
    pub ink_color_presets: Vec<[u8; 4]>,
    pub ink_highlighter_color: [u8; 4],
    pub ink_highlighter_width: f32,
    pub ink_highlighter_color_presets: Vec<[u8; 4]>,
    pub ink_strokes: Vec<RemoteInkStroke>,
    pub text_box_mode: bool,
    pub selected_text_box: Option<u64>,
    pub text_box_editing: bool,
    pub text_boxes: Vec<RemoteTextBox>,
    pub timer: RemoteTimerState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteInkStroke {
    pub points: Vec<[f32; 2]>,
    pub color: [u8; 4],
    pub width: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteTextBox {
    pub id: u64,
    pub rect: [f32; 4],
    pub content: String,
    pub font_size: f32,
    pub color: [u8; 4],
    pub background: Option<[u8; 4]>,
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

#[derive(Debug, Clone, Deserialize)]
struct NotesRequest {
    notes: String,
}

#[derive(Debug, Clone, Deserialize)]
struct InkStrokeRequest {
    points: Vec<[f32; 2]>,
    tool: Option<String>,
    color: Option<[u8; 4]>,
    width: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
struct InkEraseRequest {
    points: Vec<[f32; 2]>,
    radius: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
struct InkSetToolRequest {
    tool: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TextBoxPlaceRequest {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct TextBoxIdRequest {
    id: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct TextBoxContentRequest {
    id: u64,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TextBoxMoveRequest {
    id: u64,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct TextBoxResizeRequest {
    id: u64,
    w: f32,
    h: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct TextBoxSvgQuery {
    w: Option<u32>,
    h: Option<u32>,
    slide_w: Option<u32>,
    slide_h: Option<u32>,
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
    png_cache: Arc<Mutex<HashMap<PngCacheKey, Vec<u8>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PngCacheKey {
    page_index: usize,
    size: RenderSize,
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
    if settings.generated_token {
        if !generated_pairing_code_is_valid(&settings.token) {
            return Err(anyhow!("generated remote API pairing code is invalid"));
        }
    } else if !custom_token_is_valid(&settings.token) {
        return Err(anyhow!(
            "configured remote API token may only contain ASCII letters and digits"
        ));
    }

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
        .route("/api/v1/commands/notes", post(set_notes))
        .route("/api/v1/commands/ink/stroke", post(ink_stroke))
        .route("/api/v1/commands/ink/erase", post(ink_erase))
        .route("/api/v1/commands/ink/set_tool", post(ink_set_tool))
        .route("/api/v1/commands/ink/clear", post(ink_clear))
        .route("/api/v1/commands/text-boxes/place", post(text_box_place))
        .route("/api/v1/commands/text-boxes/select", post(text_box_select))
        .route("/api/v1/commands/text-boxes/content", post(text_box_content))
        .route("/api/v1/commands/text-boxes/move", post(text_box_move))
        .route("/api/v1/commands/text-boxes/resize", post(text_box_resize))
        .route("/api/v1/commands/text-boxes/delete", post(text_box_delete))
        .route("/api/v1/text-boxes/{id}/svg", get(text_box_svg))
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
    match current_page(&context.shared_state)
        .and_then(|page| cached_png(&context, page, REMOTE_SLIDE_SIZE))
    {
        Ok(png) => png_response(png),
        Err(error) => server_error(&error),
    }
}

async fn next_slide_png(State(context): State<HandlerContext>) -> Response {
    match next_page(&context.shared_state)
        .and_then(|page| cached_png(&context, page, REMOTE_SLIDE_SIZE))
    {
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
        .and_then(|page| cached_png(&context, page, REMOTE_THUMBNAIL_SIZE))
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

async fn set_notes(
    State(context): State<HandlerContext>,
    Json(body): Json<NotesRequest>,
) -> Response {
    let send_result = context
        .sender
        .send(Command::SetCurrentSlideNotes(body.notes))
        .and_then(|()| context.sender.send(Command::SaveSidecar));
    match send_result {
        Ok(()) => {
            context.status.set_last_command("set_notes");
            Json(ok_response("notes updated")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn ink_stroke(
    State(context): State<HandlerContext>,
    Json(body): Json<InkStrokeRequest>,
) -> Response {
    if body.points.len() < 2 {
        return bad_request(&anyhow!("stroke requires at least 2 points"));
    }

    let ink_was_active = context.shared_state.read().is_ok_and(|s| s.ink_active);
    let requested_tool = match body.tool.as_deref() {
        Some("pen") => Some(dais_core::state::DrawTool::Pen),
        Some("highlighter") => Some(dais_core::state::DrawTool::Highlighter),
        Some("eraser") => {
            return bad_request(&anyhow!("eraser strokes use /api/v1/commands/ink/erase"));
        }
        Some(tool) => return bad_request(&anyhow!("unknown tool: {tool}")),
        None => None,
    };

    let mut cmds = Vec::with_capacity(body.points.len() + 6);
    if !ink_was_active {
        cmds.push(Command::ToggleInk);
    }
    if let Some(tool) = requested_tool {
        cmds.push(Command::SetDrawTool(tool));
    }
    if let Some(color) = body.color {
        cmds.push(Command::SetInkColor(color));
    }
    if let Some(width) = body.width {
        cmds.push(Command::SetInkWidth(width));
    }
    for &[x, y] in &body.points {
        cmds.push(Command::AddInkPoint(x, y));
    }
    cmds.push(Command::FinishInkStroke);
    if !ink_was_active {
        cmds.push(Command::ToggleInk);
    }
    cmds.push(Command::SaveSidecar);

    for cmd in cmds {
        if context.sender.send(cmd).is_err() {
            return server_error(&anyhow!("presentation engine is not accepting commands"));
        }
    }

    context.status.set_last_command("ink_stroke");
    Json(ok_response("stroke added")).into_response()
}

async fn ink_set_tool(
    State(context): State<HandlerContext>,
    Json(body): Json<InkSetToolRequest>,
) -> Response {
    let tool = match body.tool.as_str() {
        "pen" => dais_core::state::DrawTool::Pen,
        "highlighter" => dais_core::state::DrawTool::Highlighter,
        "eraser" => dais_core::state::DrawTool::Eraser,
        _ => return bad_request(&anyhow::anyhow!("unknown tool: {}", body.tool)),
    };
    match context.sender.send(Command::SetDrawTool(tool)) {
        Ok(()) => {
            context.status.set_last_command("ink_set_tool");
            Json(ok_response("draw tool set")).into_response()
        }
        Err(_) => server_error(&anyhow::anyhow!("presentation engine is not accepting commands")),
    }
}

async fn ink_erase(
    State(context): State<HandlerContext>,
    Json(body): Json<InkEraseRequest>,
) -> Response {
    if body.points.is_empty() {
        return bad_request(&anyhow::anyhow!("erase requires at least one point"));
    }

    let radius = body.radius.unwrap_or(0.03).clamp(0.001, 0.5);

    for &[x, y] in &body.points {
        if context.sender.send(Command::EraseInkNear { x, y, radius }).is_err() {
            return server_error(&anyhow::anyhow!("presentation engine is not accepting commands"));
        }
    }

    if context.sender.send(Command::SaveSidecar).is_err() {
        return server_error(&anyhow::anyhow!("presentation engine is not accepting commands"));
    }

    context.status.set_last_command("ink_erase");
    Json(ok_response("erase applied")).into_response()
}

async fn ink_clear(State(context): State<HandlerContext>) -> Response {
    match context
        .sender
        .send(Command::ClearInk)
        .and_then(|()| context.sender.send(Command::SaveSidecar))
    {
        Ok(()) => {
            context.status.set_last_command("ink_clear");
            Json(ok_response("ink cleared")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn text_box_place(
    State(context): State<HandlerContext>,
    Json(body): Json<TextBoxPlaceRequest>,
) -> Response {
    if body.w <= 0.0 || body.h <= 0.0 {
        return bad_request(&anyhow!("text box size must be positive"));
    }

    match context
        .sender
        .send(Command::PlaceTextBox { x: body.x, y: body.y, w: body.w, h: body.h })
        .and_then(|()| context.sender.send(Command::SaveSidecar))
    {
        Ok(()) => {
            context.status.set_last_command("text_box_place");
            Json(ok_response("text box placed")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn text_box_select(
    State(context): State<HandlerContext>,
    Json(body): Json<TextBoxIdRequest>,
) -> Response {
    match context.sender.send(Command::SelectTextBox(body.id)) {
        Ok(()) => {
            context.status.set_last_command("text_box_select");
            Json(ok_response("text box selected")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn text_box_content(
    State(context): State<HandlerContext>,
    Json(body): Json<TextBoxContentRequest>,
) -> Response {
    match context
        .sender
        .send(Command::EditTextBoxContent { id: body.id, content: body.content })
        .and_then(|()| context.sender.send(Command::SaveSidecar))
    {
        Ok(()) => {
            context.status.set_last_command("text_box_content");
            Json(ok_response("text box content updated")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn text_box_move(
    State(context): State<HandlerContext>,
    Json(body): Json<TextBoxMoveRequest>,
) -> Response {
    match context
        .sender
        .send(Command::MoveTextBox { id: body.id, x: body.x, y: body.y })
        .and_then(|()| context.sender.send(Command::SaveSidecar))
    {
        Ok(()) => {
            context.status.set_last_command("text_box_move");
            Json(ok_response("text box moved")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn text_box_resize(
    State(context): State<HandlerContext>,
    Json(body): Json<TextBoxResizeRequest>,
) -> Response {
    if body.w <= 0.0 || body.h <= 0.0 {
        return bad_request(&anyhow!("text box size must be positive"));
    }

    match context
        .sender
        .send(Command::ResizeTextBox { id: body.id, w: body.w, h: body.h })
        .and_then(|()| context.sender.send(Command::SaveSidecar))
    {
        Ok(()) => {
            context.status.set_last_command("text_box_resize");
            Json(ok_response("text box resized")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn text_box_delete(
    State(context): State<HandlerContext>,
    Json(body): Json<TextBoxIdRequest>,
) -> Response {
    match context
        .sender
        .send(Command::DeleteTextBox { id: body.id })
        .and_then(|()| context.sender.send(Command::SaveSidecar))
    {
        Ok(()) => {
            context.status.set_last_command("text_box_delete");
            Json(ok_response("text box deleted")).into_response()
        }
        Err(_) => server_error(&anyhow!("presentation engine is not accepting commands")),
    }
}

async fn text_box_svg(
    State(context): State<HandlerContext>,
    Path(id): Path<u64>,
    Query(query): Query<TextBoxSvgQuery>,
) -> Response {
    let width = query.w.unwrap_or(320).clamp(1, 3840);
    let height = query.h.unwrap_or(120).clamp(1, 2160);
    let slide_width = query.slide_w.unwrap_or(REMOTE_SLIDE_SIZE.width).clamp(1, 3840);
    let slide_height = query.slide_h.unwrap_or(REMOTE_SLIDE_SIZE.height).clamp(1, 2160);
    let text_box = match current_text_box(&context.shared_state, id) {
        Ok(Some(text_box)) => text_box,
        Ok(None) => return (StatusCode::NOT_FOUND, "text box not found").into_response(),
        Err(error) => return server_error(&error),
    };
    let font_size = remote_text_box_font_size(&text_box, slide_width, slide_height);

    match render_text_box_svg(
        &text_box.content,
        &text_box.typst_prelude,
        width,
        height,
        font_size,
        text_box.color,
        text_box.background,
    ) {
        Some(svg) => ([(axum::http::header::CONTENT_TYPE, "image/svg+xml")], svg).into_response(),
        None => server_error(&anyhow!("failed to render text box SVG")),
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
        draw_tool: draw_tool_name(state.draw_tool).to_string(),
        eraser_radius: state.eraser_radius,
        spotlight_active: state.spotlight_active,
        spotlight_position: state.spotlight_position,
        zoom_active: state.zoom_active,
        overview_visible: state.overview_visible,
        notes_visible: state.notes_visible,
        notes_editing: state.notes_editing,
        current_notes: state.current_notes.clone(),
        ink_pen_color: state.active_pen.color,
        ink_pen_width: state.active_pen.width,
        ink_color_presets: state.ink_color_presets.clone(),
        ink_highlighter_color: state.active_highlighter.color,
        ink_highlighter_width: state.active_highlighter.width,
        ink_highlighter_color_presets: state.highlighter_color_presets.clone(),
        ink_strokes: remote_ink_strokes(state),
        text_box_mode: state.text_box_mode,
        selected_text_box: state.selected_text_box,
        text_box_editing: state.text_box_editing,
        text_boxes: remote_text_boxes(state),
        timer: RemoteTimerState {
            running: state.timer.running,
            elapsed_seconds: state.timer.elapsed.as_secs(),
            display_seconds: state.timer.display_time().as_secs(),
            phase: timer_phase_name(state.timer.phase()).to_string(),
        },
    }
}

fn remote_ink_strokes(state: &PresentationState) -> Vec<RemoteInkStroke> {
    let strokes = if state.whiteboard_active {
        state.whiteboard_strokes.as_slice()
    } else {
        state.current_page_ink()
    };
    strokes
        .iter()
        .filter(|stroke| stroke.points.len() >= 2)
        .map(|stroke| RemoteInkStroke {
            points: stroke.points.iter().map(|&(x, y)| [x, y]).collect(),
            color: stroke.color,
            width: stroke.width,
        })
        .collect()
}

fn remote_text_boxes(state: &PresentationState) -> Vec<RemoteTextBox> {
    if state.whiteboard_active {
        return Vec::new();
    }

    state
        .current_page_text_boxes()
        .iter()
        .map(|text_box| {
            let (x, y, w, h) = text_box.rect;
            RemoteTextBox {
                id: text_box.id,
                rect: [x, y, w, h],
                content: text_box.content.clone(),
                font_size: text_box.font_size,
                color: text_box.color,
                background: text_box.background,
            }
        })
        .collect()
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

pub fn client_notes(endpoint: &RemoteEndpoint, notes: &str) -> Result<CommandResponse> {
    client_json_request(
        endpoint,
        "POST",
        "/api/v1/commands/notes",
        &serde_json::json!({ "notes": notes }),
    )
}

pub fn client_ink_stroke(
    endpoint: &RemoteEndpoint,
    points: &[[f32; 2]],
    color: Option<[u8; 4]>,
    width: Option<f32>,
) -> Result<CommandResponse> {
    client_json_request(
        endpoint,
        "POST",
        "/api/v1/commands/ink/stroke",
        &serde_json::json!({ "points": points, "color": color, "width": width }),
    )
}

pub fn client_ink_clear(endpoint: &RemoteEndpoint) -> Result<CommandResponse> {
    client_json_request(endpoint, "POST", "/api/v1/commands/ink/clear", &serde_json::json!({}))
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

    host_ip == bind_ip || bind_ip.is_unspecified()
}

fn host_is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback())
}

fn generate_token() -> String {
    let code = fastrand::u32(0..100_000_000);
    format!("{:04}-{:04}", code / 10_000, code % 10_000)
}

fn custom_token_is_valid(token: &str) -> bool {
    !token.is_empty() && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn generated_pairing_code_is_valid(token: &str) -> bool {
    token.len() == 9
        && token.as_bytes().get(4) == Some(&b'-')
        && token
            .bytes()
            .enumerate()
            .all(|(index, byte)| if index == 4 { byte == b'-' } else { byte.is_ascii_digit() })
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

fn cached_png(context: &HandlerContext, page_index: usize, size: RenderSize) -> Result<Vec<u8>> {
    let key = PngCacheKey { page_index, size };
    if let Ok(cache) = context.png_cache.lock()
        && let Some(png) = cache.get(&key)
    {
        return Ok(png.clone());
    }

    let png = render_png(&context.doc, page_index, size)?;
    if let Ok(mut cache) = context.png_cache.lock() {
        cache.insert(key, png.clone());
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
        let _ = cached_png(context, page, REMOTE_SLIDE_SIZE);
    }
}

fn render_png(
    doc: &Arc<dyn DocumentSource>,
    page_index: usize,
    size: RenderSize,
) -> Result<Vec<u8>> {
    let page = doc
        .render_page(page_index, size)
        .with_context(|| format!("failed to render page {}", page_index + 1))?;
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&page.data, page.width, page.height, ColorType::Rgba8.into())
        .context("failed to encode slide PNG")?;
    Ok(png)
}

fn current_text_box(
    shared_state: &Arc<RwLock<PresentationState>>,
    id: u64,
) -> Result<Option<dais_core::state::TextBox>> {
    let state = shared_state.read().map_err(|_| anyhow!("state lock poisoned"))?;
    Ok(state.current_page_text_boxes().iter().find(|text_box| text_box.id == id).cloned())
}

#[allow(clippy::cast_precision_loss)]
fn remote_text_box_font_size(
    text_box: &dais_core::state::TextBox,
    slide_width: u32,
    slide_height: u32,
) -> f32 {
    let width_scale = slide_width as f32 / FALLBACK_RENDER_SIZE.width as f32;
    let height_scale = slide_height as f32 / FALLBACK_RENDER_SIZE.height as f32;
    let scale = width_scale.min(height_scale).max(0.05);
    (text_box.font_size.clamp(8.0, 72.0) * scale).max(1.0)
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

fn draw_tool_name(tool: dais_core::state::DrawTool) -> &'static str {
    match tool {
        dais_core::state::DrawTool::Pen => "pen",
        dais_core::state::DrawTool::Highlighter => "highlighter",
        dais_core::state::DrawTool::Eraser => "eraser",
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
        Action::SwapDisplays => Some(Command::SwapDisplays),
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
        state.slide_ink_by_page.insert(
            1,
            vec![dais_core::state::InkStroke {
                points: vec![(0.1, 0.2), (0.8, 0.9)],
                color: [0, 110, 255, 255],
                width: 4.0,
                finished: true,
            }],
        );
        state.slide_text_boxes_by_page.insert(
            1,
            vec![dais_core::state::TextBox {
                id: 42,
                rect: (0.2, 0.3, 0.4, 0.1),
                content: "Remote text".to_string(),
                font_size: 24.0,
                color: [10, 20, 30, 255],
                background: Some([255, 255, 255, 200]),
                typst_prelude: "#set align(center)".to_string(),
            }],
        );
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
                generated_token: false,
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
        assert_eq!(dto.ink_pen_color, [255, 0, 0, 255]);
        assert!((dto.ink_pen_width - 3.0).abs() < f32::EPSILON);
        assert_eq!(dto.ink_color_presets.len(), 0);
        assert_eq!(dto.ink_highlighter_color, [255, 220, 0, 100]);
        assert!((dto.ink_highlighter_width - 10.0).abs() < f32::EPSILON);
        assert_eq!(dto.ink_highlighter_color_presets.len(), 0);
        assert_eq!(dto.ink_strokes.len(), 1);
        assert_eq!(dto.ink_strokes[0].points, vec![[0.1, 0.2], [0.8, 0.9]]);
        assert_eq!(dto.ink_strokes[0].color, [0, 110, 255, 255]);
        assert!((dto.ink_strokes[0].width - 4.0).abs() < f32::EPSILON);
        assert_eq!(dto.text_boxes.len(), 1);
        assert_eq!(dto.text_boxes[0].id, 42);
        for (actual, expected) in dto.text_boxes[0].rect.iter().zip([0.2, 0.3, 0.4, 0.1]) {
            assert!((*actual - expected).abs() < f32::EPSILON);
        }
        assert_eq!(dto.text_boxes[0].content, "Remote text");
        assert!((dto.text_boxes[0].font_size - 24.0).abs() < f32::EPSILON);
        assert_eq!(dto.text_boxes[0].color, [10, 20, 30, 255]);
        assert_eq!(dto.text_boxes[0].background, Some([255, 255, 255, 200]));
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
    fn http_ink_stroke_dispatches_points_and_finishes() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        let response = client_ink_stroke(
            &endpoint(&server),
            &[[0.1, 0.2], [0.5, 0.5], [0.9, 0.8]],
            Some([255, 0, 0, 255]),
            Some(4.0),
        )
        .unwrap();

        assert!(response.ok);
        // ink_active is false in test state, so ToggleInk bookends the stroke
        assert_eq!(receiver.try_recv(), Some(Command::ToggleInk));
        assert_eq!(receiver.try_recv(), Some(Command::SetInkColor([255, 0, 0, 255])));
        assert_eq!(receiver.try_recv(), Some(Command::SetInkWidth(4.0)));
        assert_eq!(receiver.try_recv(), Some(Command::AddInkPoint(0.1, 0.2)));
        assert_eq!(receiver.try_recv(), Some(Command::AddInkPoint(0.5, 0.5)));
        assert_eq!(receiver.try_recv(), Some(Command::AddInkPoint(0.9, 0.8)));
        assert_eq!(receiver.try_recv(), Some(Command::FinishInkStroke));
        assert_eq!(receiver.try_recv(), Some(Command::ToggleInk));
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
    }

    #[test]
    fn http_ink_stroke_rejects_single_point() {
        let bus = CommandBus::new();
        let server = test_server(bus.sender());

        let error = client_ink_stroke(&endpoint(&server), &[[0.5, 0.5]], None, None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("400"));
    }

    #[test]
    fn http_ink_stroke_without_color_or_width_omits_set_commands() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        client_ink_stroke(&endpoint(&server), &[[0.1, 0.2], [0.9, 0.8]], None, None).unwrap();

        assert_eq!(receiver.try_recv(), Some(Command::ToggleInk));
        assert_eq!(receiver.try_recv(), Some(Command::AddInkPoint(0.1, 0.2)));
        assert_eq!(receiver.try_recv(), Some(Command::AddInkPoint(0.9, 0.8)));
        assert_eq!(receiver.try_recv(), Some(Command::FinishInkStroke));
        assert_eq!(receiver.try_recv(), Some(Command::ToggleInk));
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
    }

    #[test]
    fn http_ink_stroke_with_tool_dispatches_tool_before_style_and_points() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        let response = client_json_request(
            &endpoint(&server),
            "POST",
            "/api/v1/commands/ink/stroke",
            &serde_json::json!({
                "points": [[0.1, 0.2], [0.9, 0.8]],
                "tool": "highlighter",
                "color": [255, 220, 0, 100],
                "width": 12.0
            }),
        )
        .unwrap();

        assert!(response.ok);
        assert_eq!(receiver.try_recv(), Some(Command::ToggleInk));
        assert_eq!(
            receiver.try_recv(),
            Some(Command::SetDrawTool(dais_core::state::DrawTool::Highlighter))
        );
        assert_eq!(receiver.try_recv(), Some(Command::SetInkColor([255, 220, 0, 100])));
        assert_eq!(receiver.try_recv(), Some(Command::SetInkWidth(12.0)));
        assert_eq!(receiver.try_recv(), Some(Command::AddInkPoint(0.1, 0.2)));
        assert_eq!(receiver.try_recv(), Some(Command::AddInkPoint(0.9, 0.8)));
        assert_eq!(receiver.try_recv(), Some(Command::FinishInkStroke));
        assert_eq!(receiver.try_recv(), Some(Command::ToggleInk));
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
    }

    #[test]
    fn http_ink_clear_dispatches_command() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        let response = client_ink_clear(&endpoint(&server)).unwrap();

        assert!(response.ok);
        assert_eq!(receiver.try_recv(), Some(Command::ClearInk));
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
    }

    #[test]
    fn http_text_box_place_dispatches_command() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        let response = client_json_request(
            &endpoint(&server),
            "POST",
            "/api/v1/commands/text-boxes/place",
            &serde_json::json!({ "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4 }),
        )
        .unwrap();

        assert!(response.ok);
        assert_eq!(
            receiver.try_recv(),
            Some(Command::PlaceTextBox { x: 0.1, y: 0.2, w: 0.3, h: 0.4 })
        );
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
    }

    #[test]
    fn http_text_box_editing_routes_dispatch_commands() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let server = test_server(sender);

        client_json_request(
            &endpoint(&server),
            "POST",
            "/api/v1/commands/text-boxes/select",
            &serde_json::json!({ "id": 42 }),
        )
        .unwrap();
        client_json_request(
            &endpoint(&server),
            "POST",
            "/api/v1/commands/text-boxes/content",
            &serde_json::json!({ "id": 42, "content": "Updated" }),
        )
        .unwrap();
        client_json_request(
            &endpoint(&server),
            "POST",
            "/api/v1/commands/text-boxes/move",
            &serde_json::json!({ "id": 42, "x": 0.4, "y": 0.5 }),
        )
        .unwrap();
        client_json_request(
            &endpoint(&server),
            "POST",
            "/api/v1/commands/text-boxes/resize",
            &serde_json::json!({ "id": 42, "w": 0.2, "h": 0.3 }),
        )
        .unwrap();
        client_json_request(
            &endpoint(&server),
            "POST",
            "/api/v1/commands/text-boxes/delete",
            &serde_json::json!({ "id": 42 }),
        )
        .unwrap();

        assert_eq!(receiver.try_recv(), Some(Command::SelectTextBox(42)));
        assert_eq!(
            receiver.try_recv(),
            Some(Command::EditTextBoxContent { id: 42, content: "Updated".to_string() })
        );
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
        assert_eq!(receiver.try_recv(), Some(Command::MoveTextBox { id: 42, x: 0.4, y: 0.5 }));
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
        assert_eq!(receiver.try_recv(), Some(Command::ResizeTextBox { id: 42, w: 0.2, h: 0.3 }));
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
        assert_eq!(receiver.try_recv(), Some(Command::DeleteTextBox { id: 42 }));
        assert_eq!(receiver.try_recv(), Some(Command::SaveSidecar));
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
    fn text_box_svg_route_renders_typst_image() {
        let bus = CommandBus::new();
        let server = test_server(bus.sender());

        let response = reqwest::blocking::get(format!(
            "http://{}/api/v1/text-boxes/42/svg?w=180&h=60&slide_w=960&slide_h=540",
            server.addr()
        ))
        .unwrap();
        let content_type =
            response.headers().get("content-type").unwrap().to_str().unwrap().to_string();
        let body = response.text().unwrap();

        assert_eq!(content_type, "image/svg+xml");
        assert!(body.starts_with("<svg"));
    }

    #[test]
    fn query_token_authorizes_request() {
        let settings = ServerSettings {
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 4317,
            token: "secret".to_string(),
            generated_token: false,
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
            generated_token: false,
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
            generated_token: false,
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
            generated_token: false,
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
            generated_token: false,
            allow_unauthenticated_loopback: true,
        };
        let headers = headers("localhost:4317");

        assert!(host_header_allowed(&headers, &settings));
    }

    #[test]
    fn wildcard_bind_allows_ip_literal_host_on_matching_port() {
        let settings = ServerSettings {
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 4317,
            token: "secret".to_string(),
            generated_token: false,
            allow_unauthenticated_loopback: true,
        };
        let headers = headers("192.168.50.25:4317");

        assert!(host_header_allowed(&headers, &settings));
    }

    #[test]
    fn custom_token_must_be_alphanumeric() {
        assert!(custom_token_is_valid("abcDEF123"));
        assert!(!custom_token_is_valid(""));
        assert!(!custom_token_is_valid("abc-123"));
        assert!(!custom_token_is_valid("abc_123"));
        assert!(!custom_token_is_valid("abc+123"));
        assert!(!custom_token_is_valid("abc/123"));
        assert!(!custom_token_is_valid("abc&123"));
    }

    #[test]
    fn start_server_rejects_configured_token_that_is_not_alphanumeric() {
        let bus = CommandBus::new();

        let result = start_server(
            ServerSettings {
                enabled: true,
                host: "127.0.0.1".to_string(),
                port: 0,
                token: "not-a-normal-code".to_string(),
                generated_token: false,
                allow_unauthenticated_loopback: true,
            },
            bus.sender(),
            Arc::new(RwLock::new(test_state())),
            test_doc(),
        );
        let error = match result {
            Ok(_) => panic!("expected invalid token to be rejected"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("ASCII letters and digits"));
    }

    #[test]
    fn generated_token_is_human_typeable() {
        let token = generate_token();

        assert_eq!(token.len(), 9);
        assert_eq!(token.as_bytes()[4], b'-');
        assert!(token.chars().filter(char::is_ascii_digit).count() == 8);
        assert!(generated_pairing_code_is_valid(&token));
    }
}
