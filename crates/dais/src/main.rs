use clap::Parser;

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

    if cli.pdf_path.is_none() {
        // TODO: Show a file picker or usage help via GUI
        anyhow::bail!("Usage: dais <file.pdf>");
    }

    let pdf_path = cli.pdf_path.unwrap();
    tracing::info!("Opening: {pdf_path}");

    // Load config
    let config = dais_core::config::load_config();
    tracing::debug!("Config loaded: {config:?}");

    // TODO: Phase 5.1 — full startup sequence:
    // 1. Load document via DocumentSource
    // 2. Load sidecar metadata (priority chain)
    // 3. Build slide groups
    // 4. Initialize CommandBus
    // 5. Initialize PresentationEngine
    // 6. Detect monitors
    // 7. Create windows and enter render loop

    tracing::info!("Dais v{} starting", env!("CARGO_PKG_VERSION"));
    println!("Dais — PDF presenter console");
    println!("Opening: {pdf_path}");
    println!("(Full UI not yet implemented — see Phase 5 in the implementation plan)");

    Ok(())
}
