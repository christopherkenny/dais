use std::path::Path;

use clap::Parser;
use dais_ui::display_mode::{DisplayHints, DisplayMode};

/// Dais — A native PDF presenter console.
#[derive(Parser, Debug)]
#[command(name = "dais", version, about)]
struct Cli {
    /// Path to the PDF file to present.
    pdf_path: Option<String>,

    /// Path to a custom config file (overrides default location).
    #[arg(long)]
    config: Option<String>,

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
}

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // --- Test-input diagnostic mode (no PDF required) ---
    if cli.test_input {
        return run_test_input(&cli);
    }

    if cli.pdf_path.is_none() {
        anyhow::bail!("Usage: dais <file.pdf>");
    }

    let pdf_path = cli.pdf_path.unwrap();
    tracing::info!("Opening: {pdf_path}");

    // Load config
    let explicit_config = cli.config.as_deref().map(Path::new);
    let config = dais_core::config::load_config_for(Path::new(&pdf_path), explicit_config);
    tracing::debug!("Config loaded: {config:?}");

    // Open document
    let doc = dais_document::pdf_hayro::HayroDocument::open(Path::new(&pdf_path))?;

    let page_count = {
        use dais_document::source::DocumentSource;
        doc.page_count()
    };
    tracing::info!("Document has {page_count} pages");

    // --- Grouping editor mode ---
    if cli.edit {
        return run_grouping_editor(doc, &pdf_path, config.normalized_sidecar_format());
    }

    // --- Presentation mode ---

    // Load sidecar metadata
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

    tracing::info!("Dais v{} starting", env!("CARGO_PKG_VERSION"));

    // Create and run the eframe application
    // Create and run the eframe application
    let doc_arc: std::sync::Arc<dyn dais_document::source::DocumentSource> =
        std::sync::Arc::new(doc);

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
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

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
        dais_core::config::load_config_for(Path::new(pdf_path), explicit_config)
    } else if let Some(ref config_path) = cli.config {
        dais_core::config::load_config_for(Path::new("."), Some(Path::new(config_path)))
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
