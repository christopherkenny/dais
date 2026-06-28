use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use dais_core::bus::CommandSender;
use dais_core::commands::Command;
use dais_core::config::RemoteConfig;
use dais_core::keybindings::Action;
use dais_core::state::{PresentationState, TimerPhase};
use serde::{Deserialize, Serialize};

const READ_TIMEOUT: Duration = Duration::from_secs(3);
const SSE_INTERVAL: Duration = Duration::from_millis(500);

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
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
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
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
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

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    peer: SocketAddr,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

pub fn start_server(
    mut settings: ServerSettings,
    sender: CommandSender,
    shared_state: Arc<RwLock<PresentationState>>,
) -> Result<RemoteServer> {
    let listener =
        TcpListener::bind((settings.host.as_str(), settings.port)).with_context(|| {
            format!("failed to bind remote API to {}:{}", settings.host, settings.port)
        })?;
    listener.set_nonblocking(true).context("failed to set remote API listener nonblocking")?;
    let addr = listener.local_addr().context("failed to read remote API address")?;
    settings.port = addr.port();
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let token = settings.token.clone();
    let allow_unauthenticated_loopback = server_settings_allow_loopback(&settings);
    let server_settings = Arc::new(settings);
    let thread_settings = Arc::clone(&server_settings);

    let handle = thread::spawn(move || {
        while !thread_shutdown.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    let sender = sender.clone();
                    let shared_state = Arc::clone(&shared_state);
                    let settings = Arc::clone(&thread_settings);
                    let shutdown = Arc::clone(&thread_shutdown);
                    thread::spawn(move || {
                        if let Err(error) = handle_stream(
                            stream,
                            peer,
                            &settings,
                            &sender,
                            &shared_state,
                            &shutdown,
                        ) {
                            tracing::debug!("remote request failed: {error:#}");
                        }
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => {
                    tracing::warn!("remote API accept failed: {error}");
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });

    Ok(RemoteServer { addr, token, allow_unauthenticated_loopback, shutdown, handle: Some(handle) })
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
    let response = client_request(endpoint, "GET", "/api/v1/state", None)?;
    if response.status != 200 {
        return Err(anyhow!("remote API returned {} {}", response.status, response.reason));
    }
    serde_json::from_slice(&response.body).context("failed to parse remote state")
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
    let body = serde_json::to_vec(body).context("failed to serialize remote request")?;
    let response = client_request(endpoint, method, path, Some(&body))?;
    if !(200..300).contains(&response.status) {
        let text = String::from_utf8_lossy(&response.body);
        return Err(anyhow!("remote API returned {} {}: {text}", response.status, response.reason));
    }
    serde_json::from_slice(&response.body).context("failed to parse remote command response")
}

fn client_request(
    endpoint: &RemoteEndpoint,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
) -> Result<HttpResponse> {
    let mut stream =
        TcpStream::connect((endpoint.host.as_str(), endpoint.port)).with_context(|| {
            format!("failed to connect to remote API at {}:{}", endpoint.host, endpoint.port)
        })?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .context("failed to set remote client read timeout")?;
    let body = body.unwrap_or(&[]);
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}:{}\r\nContent-Length: {}\r\nConnection: close\r\n",
        endpoint.host,
        endpoint.port,
        body.len()
    );
    if !body.is_empty() {
        request.push_str("Content-Type: application/json\r\n");
    }
    if let Some(token) = &endpoint.token {
        let _ = write!(request, "Authorization: Bearer {token}\r\n");
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).context("failed to write remote request")?;
    stream.write_all(body).context("failed to write remote request body")?;
    parse_client_response(&mut stream)
}

fn handle_stream(
    mut stream: TcpStream,
    peer: SocketAddr,
    settings: &ServerSettings,
    sender: &CommandSender,
    shared_state: &Arc<RwLock<PresentationState>>,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let request = parse_request(&mut stream, peer)?;

    if !host_header_allowed(&request, settings) || !origin_allowed(&request) {
        return write_response(&mut stream, &forbidden());
    }

    if !is_authorized(&request, settings) {
        return write_response(&mut stream, &unauthorized());
    }

    if request.method == "GET" && request.path == "/api/v1/events" {
        return stream_events(stream, shared_state, shutdown);
    }

    let response = route_request(&request, sender, shared_state);
    write_response(&mut stream, &response)
}

fn route_request(
    request: &HttpRequest,
    sender: &CommandSender,
    shared_state: &Arc<RwLock<PresentationState>>,
) -> HttpResponse {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/v1/state") => match shared_state.read() {
            Ok(state) => json_response(200, "OK", &remote_state(&state)),
            Err(_) => text_response(500, "Internal Server Error", "state lock poisoned"),
        },
        ("POST", path) if path.starts_with("/api/v1/actions/") => {
            let action = path.trim_start_matches("/api/v1/actions/");
            match send_remote_action(sender, action) {
                Ok(()) => json_response(200, "OK", &ok_response("action dispatched")),
                Err(error) => text_response(400, "Bad Request", &error.to_string()),
            }
        }
        ("POST", "/api/v1/commands/goto") => handle_json(request, |body: GoToRequest| {
            if body.slide == 0 {
                return Err(anyhow!("slide must be 1 or greater"));
            }
            sender
                .send(Command::GoToSlide(body.slide - 1))
                .map_err(|_| anyhow!("presentation engine is not accepting commands"))?;
            Ok(ok_response("goto dispatched"))
        }),
        ("POST", "/api/v1/commands/pointer") => handle_json(request, |body: PointerRequest| {
            sender
                .send(Command::SetPointerPosition(body.x, body.y))
                .map_err(|_| anyhow!("presentation engine is not accepting commands"))?;
            Ok(ok_response("pointer dispatched"))
        }),
        ("POST", "/api/v1/commands/timer") => handle_json(request, |body: TimerRequest| {
            let command = match body.action {
                TimerRemoteAction::Start => Command::StartTimer,
                TimerRemoteAction::Pause => Command::PauseTimer,
                TimerRemoteAction::Toggle => Command::ToggleTimer,
                TimerRemoteAction::Reset => Command::ResetTimer,
            };
            sender
                .send(command)
                .map_err(|_| anyhow!("presentation engine is not accepting commands"))?;
            Ok(ok_response("timer command dispatched"))
        }),
        _ => text_response(404, "Not Found", "unknown remote API endpoint"),
    }
}

fn handle_json<T, F>(request: &HttpRequest, handler: F) -> HttpResponse
where
    T: for<'de> Deserialize<'de>,
    F: FnOnce(T) -> Result<CommandResponse>,
{
    match serde_json::from_slice::<T>(&request.body)
        .context("invalid JSON request body")
        .and_then(handler)
    {
        Ok(response) => json_response(200, "OK", &response),
        Err(error) => text_response(400, "Bad Request", &error.to_string()),
    }
}

fn stream_events(
    mut stream: TcpStream,
    shared_state: &Arc<RwLock<PresentationState>>,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n",
    )?;
    while !shutdown.load(Ordering::SeqCst) {
        let state = shared_state
            .read()
            .map_err(|_| anyhow!("state lock poisoned"))
            .map(|state| remote_state(&state))?;
        let json =
            serde_json::to_string(&state).context("failed to serialize remote state event")?;
        if stream.write_all(format!("data: {json}\n\n").as_bytes()).is_err() {
            break;
        }
        thread::sleep(SSE_INTERVAL);
    }
    Ok(())
}

fn parse_request(stream: &mut TcpStream, peer: SocketAddr) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];
    let header_end;
    loop {
        let n = stream.read(&mut temp).context("failed to read remote request")?;
        if n == 0 {
            return Err(anyhow!("empty remote request"));
        }
        buffer.extend_from_slice(&temp[..n]);
        if let Some(pos) = find_header_end(&buffer) {
            header_end = pos;
            break;
        }
        if buffer.len() > 64 * 1024 {
            return Err(anyhow!("remote request headers too large"));
        }
    }

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or_else(|| anyhow!("missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or_else(|| anyhow!("missing method"))?.to_string();
    let path = parts.next().ok_or_else(|| anyhow!("missing path"))?.to_string();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<HashMap<_, _>>();
    let content_length =
        headers.get("content-length").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut temp).context("failed to read remote request body")?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&temp[..n]);
    }
    body.truncate(content_length);

    Ok(HttpRequest { method, path, headers, body, peer })
}

fn parse_client_response(stream: &mut TcpStream) -> Result<HttpResponse> {
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).context("failed to read remote response")?;
    let header_end = find_header_end(&buffer).ok_or_else(|| anyhow!("invalid remote response"))?;
    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let status_line = lines.next().ok_or_else(|| anyhow!("missing status line"))?;
    let mut parts = status_line.split_whitespace();
    let _http = parts.next();
    let status = parts
        .next()
        .ok_or_else(|| anyhow!("missing status code"))?
        .parse::<u16>()
        .context("invalid status code")?;
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    Ok(HttpResponse {
        status,
        reason,
        content_type: "application/octet-stream",
        body: buffer[header_end + 4..].to_vec(),
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> Result<()> {
    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(&response.body)?;
    Ok(())
}

fn json_response<T: Serialize>(status: u16, reason: &'static str, body: &T) -> HttpResponse {
    let body = serde_json::to_vec(body).unwrap_or_else(|_| b"{\"ok\":false}".to_vec());
    HttpResponse { status, reason, content_type: "application/json", body }
}

fn text_response(status: u16, reason: &'static str, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        reason,
        content_type: "text/plain; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

fn forbidden() -> HttpResponse {
    text_response(403, "Forbidden", "forbidden")
}

fn unauthorized() -> HttpResponse {
    text_response(401, "Unauthorized", "unauthorized")
}

fn ok_response(message: &str) -> CommandResponse {
    CommandResponse { ok: true, message: message.to_string() }
}

fn is_authorized(request: &HttpRequest, settings: &ServerSettings) -> bool {
    if settings.allow_unauthenticated_loopback
        && host_is_loopback(&settings.host)
        && request.peer.ip().is_loopback()
    {
        return true;
    }

    request_token(request).is_some_and(|token| token == settings.token)
}

fn request_token(request: &HttpRequest) -> Option<&str> {
    if let Some(auth) = request.headers.get("authorization") {
        return auth.strip_prefix("Bearer ").or(Some(auth.as_str()));
    }
    request.headers.get("x-dais-token").map(String::as_str)
}

fn host_header_allowed(request: &HttpRequest, settings: &ServerSettings) -> bool {
    let Some(host) = request.headers.get("host") else {
        return false;
    };
    let Some((_, port)) = split_host_port(host) else {
        return false;
    };
    port == effective_port(settings)
}

fn origin_allowed(request: &HttpRequest) -> bool {
    let Some(origin) = request.headers.get("origin") else {
        return true;
    };
    let Some(host) = request.headers.get("host") else {
        return false;
    };
    origin == &format!("http://{host}") || origin == &format!("https://{host}")
}

fn split_host_port(host: &str) -> Option<(&str, u16)> {
    let host = host.trim();
    let (name, port) = host.rsplit_once(':')?;
    Some((name.trim_matches(['[', ']']), port.parse().ok()?))
}

fn effective_port(settings: &ServerSettings) -> u16 {
    settings.port
}

fn host_is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback())
}

fn generate_token() -> String {
    let mut token = String::with_capacity(32);
    for _ in 0..4 {
        let _ = write!(token, "{:016x}", fastrand::u64(..));
    }
    token.truncate(32);
    token
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
        Action::ToggleNotesEdit => Some(Command::ToggleNotesEdit),
        Action::StartPauseTimer => Some(Command::ToggleTimer),
        Action::ResetTimer => Some(Command::ResetTimer),
        Action::IncrementNotesFont => Some(Command::IncrementNotesFontSize),
        Action::DecrementNotesFont => Some(Command::DecrementNotesFontSize),
        Action::ToggleScreenShare => Some(Command::ToggleScreenShareMode),
        Action::TogglePresentationMode => Some(Command::TogglePresentationMode),
        Action::ToggleTextBoxMode => Some(Command::ToggleTextBoxMode),
        Action::Quit => Some(Command::Quit),
        Action::SaveSidecar => Some(Command::SaveSidecar),
        Action::GoToSlide => None,
    }
}

#[cfg(test)]
mod tests {
    use dais_core::bus::CommandBus;
    use dais_core::slide_group::SlideGroup;

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

    #[test]
    fn action_names_map_to_commands() {
        assert_eq!(command_for_action_name("next_slide"), Some(Command::NextSlide));
        assert_eq!(command_for_action_name("start_pause_timer"), Some(Command::ToggleTimer));
        assert_eq!(command_for_action_name("go_to_slide"), None);
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
        let state = Arc::new(RwLock::new(test_state()));
        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/api/v1/actions/next_slide".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
            peer: "127.0.0.1:50000".parse().unwrap(),
        };

        let response = route_request(&request, &sender, &state);

        assert_eq!(response.status, 200);
        assert_eq!(receiver.try_recv(), Some(Command::NextSlide));
    }

    #[test]
    fn http_goto_rejects_zero_slide() {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let state = Arc::new(RwLock::new(test_state()));
        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/api/v1/commands/goto".to_string(),
            headers: HashMap::new(),
            body: br#"{"slide":0}"#.to_vec(),
            peer: "127.0.0.1:50000".parse().unwrap(),
        };

        let response = route_request(&request, &sender, &state);

        assert_eq!(response.status, 400);
        assert_eq!(receiver.try_recv(), None);
    }

    #[test]
    fn token_required_for_non_loopback_when_loopback_exemption_does_not_apply() {
        let settings = ServerSettings {
            enabled: true,
            host: "0.0.0.0".to_string(),
            port: 4317,
            token: "secret".to_string(),
            allow_unauthenticated_loopback: true,
        };
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/api/v1/state".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
            peer: "127.0.0.1:50000".parse().unwrap(),
        };

        assert!(!is_authorized(&request, &settings));
    }
}
