use std::path::Path;

use anyhow::Context;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
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

    /// Path to a Markdown speaker notes file to use as the notes source.
    #[arg(long)]
    notes: Option<String>,

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
    /// Export presentation assets.
    Export(ExportCli),
}

#[derive(Parser, Debug)]
struct ExportCli {
    /// Path to the PDF file to export.
    pdf_path: String,

    /// Output file path for PDF, or output directory for SVG/PNG.
    #[arg(long)]
    out: String,

    /// Output format. Defaults to the --out extension when possible, otherwise PDF.
    #[arg(long, value_enum)]
    format: Option<ExportFormatArg>,

    /// Content layers to include.
    #[arg(long, value_enum)]
    layers: Option<ExportLayersArg>,

    /// Export one page per logical slide by using the final build page of each group.
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_handout")]
    handout: bool,

    /// Disable handout export even when it is enabled in config.
    #[arg(long = "no-handout", action = ArgAction::SetTrue)]
    no_handout: bool,

    /// Whiteboard export behavior.
    #[arg(long, value_enum)]
    whiteboard: Option<WhiteboardArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormatArg {
    Pdf,
    Svg,
    Png,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportLayersArg {
    Background,
    Ink,
    Text,
    Overlays,
    All,
}

impl std::fmt::Display for ExportLayersArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum WhiteboardArg {
    None,
    Append,
    Only,
}

impl std::fmt::Display for WhiteboardArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
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

    if let Some(CliCommand::Export(export)) = &cli.command {
        return run_export_cli(&cli, export);
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
    let (engine, shared_state) =
        create_engine(page_count, &metadata, &config, receiver, Path::new(&pdf_path), &cli)?;

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

fn create_engine(
    page_count: usize,
    metadata: &dais_sidecar::types::PresentationMetadata,
    config: &dais_core::config::Config,
    receiver: dais_core::bus::CommandReceiver,
    pdf_path: &Path,
    cli: &Cli,
) -> anyhow::Result<(
    dais_engine::engine::PresentationEngine,
    std::sync::Arc<std::sync::RwLock<dais_core::state::PresentationState>>,
)> {
    if let Some(notes_path) = &cli.notes {
        return dais_engine::engine::PresentationEngine::new_with_notes(
            page_count,
            metadata,
            config,
            receiver,
            pdf_path.to_path_buf(),
            Path::new(notes_path),
        )
        .map_err(|e| anyhow::anyhow!("Failed to load Markdown notes: {e}"));
    }

    Ok(dais_engine::engine::PresentationEngine::new(
        page_count,
        metadata,
        config,
        receiver,
        pdf_path.to_path_buf(),
    ))
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

fn run_export_cli(cli: &Cli, export: &ExportCli) -> anyhow::Result<()> {
    run_export_annotated(cli, export)
}

fn run_export_annotated(cli: &Cli, export: &ExportCli) -> anyhow::Result<()> {
    use dais_document::source::DocumentSource;

    let pdf_path = Path::new(&export.pdf_path);
    let out_path = Path::new(&export.out);
    tracing::info!("Exporting presentation: {} -> {}", pdf_path.display(), out_path.display());
    let config = load_effective_config(cli, pdf_path);

    let doc = dais_document::pdf_hayro::HayroDocument::open(pdf_path)?;
    let embedded_pdfpc = doc.embedded_metadata().and_then(|m| m.pdfpc_data);
    let (metadata, meta_source) =
        dais_sidecar::metadata::load_metadata(pdf_path, embedded_pdfpc.as_deref());
    tracing::info!("Metadata source: {meta_source:?}");

    let settings = resolve_export_settings(export, out_path, &config)?;
    let artifacts = dais_document::typst_export::export_annotated(
        dais_document::typst_export::AnnotatedExport {
            pdf_path,
            metadata: &metadata,
            format: settings.format.into(),
            layers: settings.layers.into(),
            handout: settings.handout,
            whiteboard: settings.whiteboard.into(),
        },
    )?;
    write_export_artifacts(out_path, settings.format, &artifacts)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ExportSettings {
    format: ExportFormatArg,
    layers: ExportLayersArg,
    handout: bool,
    whiteboard: WhiteboardArg,
}

fn resolve_export_settings(
    export: &ExportCli,
    out_path: &Path,
    config: &dais_core::config::Config,
) -> anyhow::Result<ExportSettings> {
    Ok(ExportSettings {
        format: resolve_export_format(export.format, out_path, &config.export.format)?,
        layers: match export.layers {
            Some(layers) => layers,
            None => parse_export_layers(&config.export.layers)?,
        },
        handout: if export.handout {
            true
        } else if export.no_handout {
            false
        } else {
            config.export.handout
        },
        whiteboard: match export.whiteboard {
            Some(whiteboard) => whiteboard,
            None => parse_whiteboard_export(&config.export.whiteboard)?,
        },
    })
}

fn resolve_export_format(
    format: Option<ExportFormatArg>,
    out_path: &Path,
    configured_format: &str,
) -> anyhow::Result<ExportFormatArg> {
    if let Some(format) = format {
        return Ok(format);
    }
    match out_path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("pdf") => Ok(ExportFormatArg::Pdf),
        Some(ext) if ext.eq_ignore_ascii_case("svg") => Ok(ExportFormatArg::Svg),
        Some(ext) if ext.eq_ignore_ascii_case("png") => Ok(ExportFormatArg::Png),
        _ => parse_export_format(configured_format),
    }
}

fn parse_export_format(value: &str) -> anyhow::Result<ExportFormatArg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pdf" => Ok(ExportFormatArg::Pdf),
        "svg" => Ok(ExportFormatArg::Svg),
        "png" => Ok(ExportFormatArg::Png),
        other => anyhow::bail!("Unsupported export.format value '{other}'; use pdf, svg, or png"),
    }
}

fn parse_export_layers(value: &str) -> anyhow::Result<ExportLayersArg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "background" => Ok(ExportLayersArg::Background),
        "ink" => Ok(ExportLayersArg::Ink),
        "text" => Ok(ExportLayersArg::Text),
        "overlays" => Ok(ExportLayersArg::Overlays),
        "all" => Ok(ExportLayersArg::All),
        other => anyhow::bail!(
            "Unsupported export.layers value '{other}'; use background, ink, text, overlays, or all"
        ),
    }
}

fn parse_whiteboard_export(value: &str) -> anyhow::Result<WhiteboardArg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(WhiteboardArg::None),
        "append" => Ok(WhiteboardArg::Append),
        "only" => Ok(WhiteboardArg::Only),
        other => {
            anyhow::bail!(
                "Unsupported export.whiteboard value '{other}'; use none, append, or only"
            )
        }
    }
}

fn write_export_artifacts(
    out_path: &Path,
    format: ExportFormatArg,
    artifacts: &[dais_document::typst_export::ExportArtifact],
) -> anyhow::Result<()> {
    match format {
        ExportFormatArg::Pdf => {
            let artifact = artifacts
                .first()
                .ok_or_else(|| anyhow::anyhow!("PDF export produced no output"))?;
            std::fs::write(out_path, &artifact.bytes)
                .with_context(|| format!("Failed to write PDF to {}", out_path.display()))?;
            println!("Wrote PDF to {}", out_path.display());
        }
        ExportFormatArg::Svg | ExportFormatArg::Png => {
            std::fs::create_dir_all(out_path).with_context(|| {
                format!("Failed to create output directory {}", out_path.display())
            })?;
            for artifact in artifacts {
                let path = out_path.join(&artifact.name);
                std::fs::write(&path, &artifact.bytes)
                    .with_context(|| format!("Failed to write {}", path.display()))?;
            }
            println!("Wrote {} files to {}", artifacts.len(), out_path.display());
        }
    }
    Ok(())
}

impl From<ExportFormatArg> for dais_document::typst_export::ExportFormat {
    fn from(value: ExportFormatArg) -> Self {
        match value {
            ExportFormatArg::Pdf => Self::Pdf,
            ExportFormatArg::Svg => Self::Svg,
            ExportFormatArg::Png => Self::Png,
        }
    }
}

impl From<ExportLayersArg> for dais_document::typst_export::ExportLayers {
    fn from(value: ExportLayersArg) -> Self {
        match value {
            ExportLayersArg::Background => Self::Background,
            ExportLayersArg::Ink => Self::Ink,
            ExportLayersArg::Text => Self::Text,
            ExportLayersArg::Overlays => Self::Overlays,
            ExportLayersArg::All => Self::All,
        }
    }
}

impl From<WhiteboardArg> for dais_document::typst_export::WhiteboardExport {
    fn from(value: WhiteboardArg) -> Self {
        match value {
            WhiteboardArg::None => Self::None,
            WhiteboardArg::Append => Self::Append,
            WhiteboardArg::Only => Self::Only,
        }
    }
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
    fn parses_export_subcommand() {
        let cli =
            Cli::try_parse_from(["dais", "export", "slides.pdf", "--out", "slides-annotated.pdf"])
                .unwrap();

        let Some(CliCommand::Export(export)) = cli.command else {
            panic!("expected export subcommand");
        };
        assert_eq!(export.pdf_path, "slides.pdf");
        assert_eq!(export.out, "slides-annotated.pdf");
        assert!(export.format.is_none());
        assert!(export.layers.is_none());
        assert!(!export.handout);
        assert!(!export.no_handout);
        assert!(export.whiteboard.is_none());
    }

    #[test]
    fn parses_export_options() {
        let cli = Cli::try_parse_from([
            "dais",
            "export",
            "slides.pdf",
            "--out",
            "exported",
            "--format",
            "svg",
            "--layers",
            "ink",
            "--handout",
            "--whiteboard",
            "append",
        ])
        .unwrap();

        let Some(CliCommand::Export(export)) = cli.command else {
            panic!("expected export subcommand");
        };
        assert!(matches!(export.format, Some(ExportFormatArg::Svg)));
        assert!(matches!(export.layers, Some(ExportLayersArg::Ink)));
        assert!(export.handout);
        assert!(!export.no_handout);
        assert!(matches!(export.whiteboard, Some(WhiteboardArg::Append)));
    }

    #[test]
    fn resolves_export_config_defaults_with_cli_overrides() {
        let export = ExportCli {
            pdf_path: "slides.pdf".to_string(),
            out: "exported".to_string(),
            format: None,
            layers: Some(ExportLayersArg::Ink),
            handout: false,
            no_handout: true,
            whiteboard: None,
        };
        let mut config = dais_core::config::Config::default();
        config.export.format = "svg".to_string();
        config.export.layers = "all".to_string();
        config.export.handout = true;
        config.export.whiteboard = "append".to_string();

        let settings = resolve_export_settings(&export, Path::new(&export.out), &config).unwrap();

        assert!(matches!(settings.format, ExportFormatArg::Svg));
        assert!(matches!(settings.layers, ExportLayersArg::Ink));
        assert!(!settings.handout);
        assert!(matches!(settings.whiteboard, WhiteboardArg::Append));
    }

    #[test]
    fn export_annotated_writes_pdf() {
        use dais_sidecar::dais_format::DaisFormat;
        use dais_sidecar::format::SidecarFormat;

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source_pdf = root.join("tests/fixtures/test.pdf");
        let unique =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("dais_export_test_{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf_path = dir.join("talk.pdf");
        let out_path = dir.join("talk-annotated.pdf");
        std::fs::copy(&source_pdf, &pdf_path).unwrap();

        let mut annotations = std::collections::HashMap::new();
        annotations.insert(
            0,
            vec![dais_sidecar::types::InkStrokeMeta {
                points: vec![(0.1, 0.1), (0.9, 0.9)],
                color: [255, 0, 0, 255],
                width: 3.0,
            }],
        );
        let mut text_boxes = std::collections::HashMap::new();
        text_boxes.insert(
            0,
            vec![dais_sidecar::types::TextBoxMeta {
                id: 1,
                rect: (0.2, 0.2, 0.4, 0.2),
                content: "Exported".to_string(),
                font_size: 18.0,
                color: [0, 0, 0, 255],
                background: Some([255, 255, 255, 180]),
                typst_prelude: String::new(),
            }],
        );
        let metadata = dais_sidecar::types::PresentationMetadata {
            slide_annotations: annotations,
            slide_text_boxes: text_boxes,
            ..Default::default()
        };
        DaisFormat.write(&pdf_path.with_extension("dais"), &metadata).unwrap();

        let export = ExportCli {
            pdf_path: pdf_path.to_string_lossy().into_owned(),
            out: out_path.to_string_lossy().into_owned(),
            format: None,
            layers: None,
            handout: false,
            no_handout: false,
            whiteboard: None,
        };
        let cli = Cli {
            command: None,
            pdf_path: None,
            config: None,
            notes: None,
            portable: false,
            single: false,
            screen_share: false,
            edit: false,
            test_input: false,
            time_ignore: false,
            remote: false,
            remote_lan: false,
            remote_host: None,
            remote_port: None,
        };
        run_export_annotated(&cli, &export).unwrap();

        let bytes = std::fs::read(&out_path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 1000);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_portable_flag() {
        let cli = Cli::try_parse_from(["dais", "--portable", "slides.pdf"]).unwrap();

        assert!(cli.portable);
        assert_eq!(cli.pdf_path.as_deref(), Some("slides.pdf"));
    }

    #[test]
    fn parses_notes_flag() {
        let cli = Cli::try_parse_from(["dais", "--notes", "talk_notes.md", "slides.pdf"]).unwrap();

        assert_eq!(cli.notes.as_deref(), Some("talk_notes.md"));
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
