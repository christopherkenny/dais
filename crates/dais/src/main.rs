use std::path::Path;

use clap::{Parser, Subcommand};
use dais_ui::display_mode::{DisplayHints, DisplayMode};

/// Dais — A native PDF presenter console.
#[derive(Parser, Debug)]
#[command(name = "dais", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,

    /// Path to the PDF file to present.
    pdf_path: Option<String>,

    /// Path to a custom config file (overrides default location).
    #[arg(long)]
    config: Option<String>,

    /// Skip the OS user config directory for a portable, folder-local run.
    #[arg(long)]
    portable: bool,

    /// Force single-monitor mode.
    #[arg(long)]
    single: bool,

    /// Start in screen-share mode (audience window as normal window).
    #[arg(long, alias = "screen-share")]
    screen_share: bool,

    /// Open the slide grouping editor instead of presenting.
    #[arg(long)]
    edit: bool,

    /// Open a diagnostic window that shows raw key events and their mapped actions.
    #[arg(long)]
    test_input: bool,

    /// Do not update per-slide timing data when saving sidecars.
    #[arg(long)]
    time_ignore: bool,

    /// Start the local remote-control HTTP API with the presentation.
    #[arg(long)]
    remote: bool,

    /// Start the remote-control HTTP API for phone/tablet access on the local network.
    #[arg(long)]
    remote_lan: bool,

    /// Override the remote-control HTTP API bind host.
    #[arg(long)]
    remote_host: Option<String>,

    /// Override the remote-control HTTP API bind port.
    #[arg(long)]
    remote_port: Option<u16>,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Send commands to a running Dais remote API.
    Remote(RemoteCli),
}

#[derive(Parser, Debug)]
struct RemoteCli {
    /// Remote API host.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Remote API port.
    #[arg(long, default_value_t = 4317)]
    port: u16,

    /// Bearer token for authenticated remote APIs.
    #[arg(long)]
    token: Option<String>,

    #[command(subcommand)]
    command: RemoteCommand,
}

#[derive(Subcommand, Debug)]
enum RemoteCommand {
    /// Print the current presentation state as JSON.
    State,
    /// Dispatch a public Dais action name, such as `next_slide`.
    Action { action_name: String },
    /// Jump to a 1-based logical slide number.
    Goto { slide_number: usize },
    /// Set normalized pointer coordinates in the current slide area.
    Pointer { x: f32, y: f32 },
    /// Control the presentation timer.
    Timer {
        #[command(subcommand)]
        command: RemoteTimerCommand,
    },
    /// Set the speaker notes for the current slide.
    Notes { text: String },
}

#[derive(Subcommand, Debug)]
enum RemoteTimerCommand {
    Start,
    Pause,
    Toggle,
    Reset,
}

fn main() -> anyhow::Result<()> {
    init_logging();

    let cli = Cli::parse();

    if let Some(CliCommand::Remote(remote)) = &cli.command {
        return run_remote_cli(remote);
    }

    if cli.test_input {
        return run_test_input(&cli);
    }

    if cli.pdf_path.is_none() {
        anyhow::bail!("Usage: dais <file.pdf>");
    }

    let pdf_path = cli.pdf_path.as_deref().unwrap();
    tracing::info!("Opening: {pdf_path}");

    let config = load_effective_config(&cli, Path::new(&pdf_path));
    tracing::debug!("Config loaded: {config:?}");

    let doc = dais_document::pdf_hayro::HayroDocument::open(Path::new(&pdf_path))?;

    let page_count = page_count(&doc);
    tracing::info!("Document has {page_count} pages");

    if cli.edit {
        return run_grouping_editor(doc, pdf_path, config.normalized_sidecar_format());
    }

    let embedded_pdfpc = {
        use dais_document::source::DocumentSource;
        doc.embedded_metadata().and_then(|m| m.pdfpc_data)
    };
    let (metadata, meta_source) =
        dais_sidecar::metadata::load_metadata(Path::new(&pdf_path), embedded_pdfpc.as_deref());
    tracing::info!("Metadata source: {meta_source:?}");

    // Detect monitors and determine display mode
    let monitor_mgr = dais_platform::create_monitor_manager();
    let hints = DisplayHints { force_single: cli.single, force_screen_share: cli.screen_share };
    let display_result =
        dais_ui::display_mode::determine_display_mode(hints, &config, &monitor_mgr);
    let display_warnings = display_result.warnings;
    let audience_reassignment = display_result.audience_reassignment;
    let display_mode = display_result.mode;
    tracing::info!("Display mode: {display_mode:?}");

    // Set screen-share mode in engine if needed
    let is_screen_share = matches!(display_mode, DisplayMode::ScreenShare);

    // Create command bus
    let bus = dais_core::bus::CommandBus::new();
    let sender = bus.sender();
    let receiver = bus.into_receiver();

    // Create presentation engine
    let (engine, shared_state) = dais_engine::engine::PresentationEngine::new(
        page_count,
        &metadata,
        &config,
        receiver,
        Path::new(&pdf_path).to_path_buf(),
    );

    // Sync engine state for screen-share
    if is_screen_share {
        let _ = sender.send(dais_core::commands::Command::ToggleScreenShareMode);
    }
    if matches!(display_mode, DisplayMode::Single) {
        let _ = sender.send(dais_core::commands::Command::TogglePresentationMode);
    }

    // Create and run the eframe application
    let doc_arc: std::sync::Arc<dyn dais_document::source::DocumentSource> =
        std::sync::Arc::new(doc);

    let remote_server =
        start_remote_server_if_enabled(&cli, &config, &sender, &shared_state, doc_arc.clone())?;
    let remote_toasts = remote_toasts(remote_server.as_ref());
    let remote_info = remote_ui_info(remote_server.as_ref());

    tracing::info!("Dais v{} starting", env!("CARGO_PKG_VERSION"));

    let presenter_window_size = egui::vec2(1400.0, 850.0);
    let native_options = eframe::NativeOptions {
        viewport: dais_ui::display_mode::presenter_viewport_builder(
            &config,
            &monitor_mgr,
            presenter_window_size,
        ),
        ..Default::default()
    };

    let config_clone = config.clone();
    eframe::run_native(
        "Dais",
        native_options,
        Box::new(move |_cc| {
            let mut app = dais_ui::app::DaisApp::new(
                engine,
                shared_state,
                doc_arc,
                sender,
                &config_clone,
                display_mode,
            );
            app.set_audience_reassignment(audience_reassignment.clone());
            for warning in &display_warnings {
                app.toast_manager_mut()
                    .push(dais_ui::widgets::toast::ToastLevel::Warning, warning.clone());
            }
            for message in &remote_toasts {
                app.toast_manager_mut()
                    .push(dais_ui::widgets::toast::ToastLevel::Info, message.clone());
            }
            app.set_remote_info(remote_info.clone());
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

fn load_effective_config(cli: &Cli, pdf_path: &Path) -> dais_core::config::Config {
    let explicit_config = cli.config.as_deref().map(Path::new);
    let options = dais_core::config::ConfigLoadOptions { portable: cli.portable };
    let mut config =
        dais_core::config::load_config_for_with_options(pdf_path, explicit_config, options);
    if cli.time_ignore {
        config.save_slide_timings = false;
    }
    config
}

fn remote_toasts(server: Option<&dais_remote::RemoteServer>) -> Vec<String> {
    server
        .map(|server| {
            server.urls().iter().map(|url| format!("Remote control: {url}")).collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn page_count(doc: &dais_document::pdf_hayro::HayroDocument) -> usize {
    use dais_document::source::DocumentSource;
    doc.page_count()
}

fn remote_ui_info(
    server: Option<&dais_remote::RemoteServer>,
) -> Option<dais_ui::app::RemoteUiInfo> {
    server.map(|server| dais_ui::app::RemoteUiInfo {
        urls: server.urls().to_vec(),
        token: server.token().to_string(),
        requires_token: server.requires_token_for_non_loopback(),
        status: server.status_handle(),
    })
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

fn start_remote_server_if_enabled(
    cli: &Cli,
    config: &dais_core::config::Config,
    sender: &dais_core::bus::CommandSender,
    shared_state: &std::sync::Arc<std::sync::RwLock<dais_core::state::PresentationState>>,
    doc: std::sync::Arc<dyn dais_document::source::DocumentSource>,
) -> anyhow::Result<Option<dais_remote::RemoteServer>> {
    let remote_overrides = dais_remote::RemoteOverrides {
        enabled: cli.remote || cli.remote_lan,
        host: remote_host_override(cli),
        port: cli.remote_port,
    };
    let remote_settings =
        dais_remote::ServerSettings::from_config(&config.remote, &remote_overrides);
    if !remote_settings.enabled {
        return Ok(None);
    }

    let server =
        dais_remote::start_server(remote_settings, sender.clone(), shared_state.clone(), doc)?;
    tracing::info!("Dais remote API listening at http://{}", server.addr());
    for url in server.urls() {
        tracing::info!("Dais web remote: {url}");
    }
    if server.requires_token_for_non_loopback() {
        tracing::info!("Dais remote API token: {}", server.token());
        tracing::info!("Open the web remote with ?token={} when using a browser", server.token());
    } else {
        tracing::info!(
            "Dais remote API allows unauthenticated loopback requests; token for other clients: {}",
            server.token()
        );
    }
    Ok(Some(server))
}

fn remote_host_override(cli: &Cli) -> Option<String> {
    cli.remote_host.clone().or_else(|| cli.remote_lan.then(|| "0.0.0.0".to_string()))
}

fn run_remote_cli(remote: &RemoteCli) -> anyhow::Result<()> {
    let endpoint = dais_remote::RemoteEndpoint {
        host: remote.host.clone(),
        port: remote.port,
        token: remote.token.clone(),
    };

    match &remote.command {
        RemoteCommand::State => {
            let state = dais_remote::client_get_state(&endpoint)?;
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        RemoteCommand::Action { action_name } => {
            let response = dais_remote::client_action(&endpoint, action_name)?;
            println!("{}", response.message);
        }
        RemoteCommand::Goto { slide_number } => {
            let response = dais_remote::client_goto(&endpoint, *slide_number)?;
            println!("{}", response.message);
        }
        RemoteCommand::Pointer { x, y } => {
            let response = dais_remote::client_pointer(&endpoint, *x, *y)?;
            println!("{}", response.message);
        }
        RemoteCommand::Timer { command } => {
            let action = match command {
                RemoteTimerCommand::Start => "start",
                RemoteTimerCommand::Pause => "pause",
                RemoteTimerCommand::Toggle => "toggle",
                RemoteTimerCommand::Reset => "reset",
            };
            let response = dais_remote::client_timer(&endpoint, action)?;
            println!("{}", response.message);
        }
        RemoteCommand::Notes { text } => {
            let response = dais_remote::client_notes(&endpoint, text)?;
            println!("{}", response.message);
        }
    }

    Ok(())
}

/// Run the grouping editor as a standalone eframe app.
fn run_grouping_editor(
    doc: dais_document::pdf_hayro::HayroDocument,
    pdf_path: &str,
    sidecar_format: &str,
) -> anyhow::Result<()> {
    use dais_document::source::DocumentSource;

    tracing::info!("Opening grouping editor");

    // Load existing sidecar metadata (if any)
    let embedded_pdfpc = doc.embedded_metadata().and_then(|m| m.pdfpc_data);
    let (metadata, meta_source) =
        dais_sidecar::metadata::load_metadata(Path::new(pdf_path), embedded_pdfpc.as_deref());
    tracing::info!("Metadata source: {meta_source:?}");

    let doc_box: Box<dyn DocumentSource> = Box::new(doc);
    let path = Path::new(pdf_path);
    let sidecar_format = sidecar_format.to_string();

    let native_options = eframe::NativeOptions {
        viewport: dais_ui::display_mode::with_app_icon(egui::ViewportBuilder::default())
            .with_title("Dais — Grouping Editor")
            .with_inner_size(egui::vec2(1200.0, 320.0)),
        ..Default::default()
    };

    eframe::run_native(
        "Dais Grouping Editor",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(dais_ui::grouping_editor::GroupingEditor::new(
                doc_box,
                path,
                metadata,
                &sidecar_format,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

/// Run the test-input diagnostic window.
fn run_test_input(cli: &Cli) -> anyhow::Result<()> {
    use dais_core::keybindings::KeybindingMap;

    tracing::info!("Opening test-input diagnostic mode");

    // Load config if a PDF path or explicit config was provided, otherwise use defaults.
    let config = if let Some(ref pdf_path) = cli.pdf_path {
        let explicit_config = cli.config.as_deref().map(Path::new);
        dais_core::config::load_config_for_with_options(
            Path::new(pdf_path),
            explicit_config,
            dais_core::config::ConfigLoadOptions { portable: cli.portable },
        )
    } else if let Some(ref config_path) = cli.config {
        dais_core::config::load_config_for_with_options(
            Path::new("."),
            Some(Path::new(config_path)),
            dais_core::config::ConfigLoadOptions { portable: cli.portable },
        )
    } else {
        dais_core::config::Config::default()
    };

    let keybindings = KeybindingMap::from_full_config(&config);

    let native_options = eframe::NativeOptions {
        viewport: dais_ui::display_mode::with_app_icon(egui::ViewportBuilder::default())
            .with_title("Dais — Test Input")
            .with_inner_size(egui::vec2(600.0, 500.0)),
        ..Default::default()
    };

    eframe::run_native(
        "Dais Test Input",
        native_options,
        Box::new(move |_cc| Ok(Box::new(dais_ui::test_input::TestInputApp::new(keybindings)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_remote_action_subcommand() {
        let cli = Cli::try_parse_from(["dais", "remote", "action", "next_slide"]).unwrap();

        let Some(CliCommand::Remote(remote)) = cli.command else {
            panic!("expected remote subcommand");
        };
        let RemoteCommand::Action { action_name } = remote.command else {
            panic!("expected action command");
        };
        assert_eq!(action_name, "next_slide");
        assert_eq!(remote.host, "127.0.0.1");
        assert_eq!(remote.port, 4317);
    }

    #[test]
    fn parses_remote_timer_subcommand() {
        let cli =
            Cli::try_parse_from(["dais", "remote", "--port", "4318", "timer", "start"]).unwrap();

        let Some(CliCommand::Remote(remote)) = cli.command else {
            panic!("expected remote subcommand");
        };
        let RemoteCommand::Timer { command } = remote.command else {
            panic!("expected timer command");
        };
        assert!(matches!(command, RemoteTimerCommand::Start));
        assert_eq!(remote.port, 4318);
    }

    #[test]
    fn parses_time_ignore_flag() {
        let cli = Cli::try_parse_from(["dais", "--time-ignore", "slides.pdf"]).unwrap();

        assert!(cli.time_ignore);
        assert_eq!(cli.pdf_path.as_deref(), Some("slides.pdf"));
    }

    #[test]
    fn parses_portable_flag() {
        let cli = Cli::try_parse_from(["dais", "--portable", "slides.pdf"]).unwrap();

        assert!(cli.portable);
        assert_eq!(cli.pdf_path.as_deref(), Some("slides.pdf"));
    }

    #[test]
    fn remote_lan_enables_wildcard_remote_host() {
        let cli = Cli::try_parse_from(["dais", "--remote-lan", "slides.pdf"]).unwrap();

        assert!(cli.remote_lan);
        assert_eq!(remote_host_override(&cli).as_deref(), Some("0.0.0.0"));
    }

    #[test]
    fn explicit_remote_host_overrides_remote_lan_host() {
        let cli = Cli::try_parse_from([
            "dais",
            "--remote-lan",
            "--remote-host",
            "192.168.1.5",
            "slides.pdf",
        ])
        .unwrap();

        assert_eq!(remote_host_override(&cli).as_deref(), Some("192.168.1.5"));
    }
}
