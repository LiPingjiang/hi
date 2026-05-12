use std::path::PathBuf;
use anyhow::Result;
use clap::Parser;

use hi::app::App;
use hi::config::loader::load_config;

/// hi — a modal text editor with native AI assistance
#[derive(Parser, Debug)]
#[command(
    name = "hi",
    version = env!("CARGO_PKG_VERSION"),
    about = "A modal text editor with native AI assistance",
)]
struct Cli {
    /// File to open (optional)
    file: Option<PathBuf>,

    /// Override AI model (e.g. gpt-4o, claude-3-5-sonnet-20241022)
    #[arg(long, env = "HI_MODEL")]
    model: Option<String>,

    /// Override API key
    #[arg(long, env = "HI_API_KEY")]
    api_key: Option<String>,

    /// Override API base URL
    #[arg(long, env = "HI_API_BASE")]
    api_base: Option<String>,

    /// Enable debug logging (writes to ~/.hi/ai.log)
    #[arg(long, env = "HI_DEBUG")]
    debug: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut config = load_config()?;
    if let Some(model) = cli.model {
        config.ai.model = model;
    }
    if let Some(key) = cli.api_key {
        config.ai.api_key = key;
    }
    if let Some(base) = cli.api_base {
        config.ai.api_base_url = base;
    }
    if cli.debug {
        config.ai.debug = true;
    }

    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let mut app = App::new(config, cli.file.as_deref(), width, height)?;
    app.run()
}
