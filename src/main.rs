use std::path::PathBuf;
use anyhow::Result;
use clap::Parser;
use crossterm::style::Color;

use hi::app::App;
use hi::config::loader::load_config;
use hi::locale::Locale;

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

    /// Render a Markdown file to the terminal using the glow-dark theme and exit.
    /// Usage: hi --render path/to/file.md
    /// Useful for visually comparing output against `glow path/to/file.md`.
    #[arg(long, value_name = "FILE")]
    render: Option<PathBuf>,

    /// Theme to use with --render (default: glow-dark).
    /// Options: glow-dark, dark, dracula, tokyo-night, monokai-pro,
    ///          github-dark, one-dark-pro, electric, synthwave
    #[arg(long, default_value = "glow-dark")]
    theme: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── --render mode: print a Markdown file to stdout and exit ──────────────
    if let Some(md_path) = cli.render {
        return render_to_terminal(&md_path, &cli.theme);
    }

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

    // Load locale: "auto" means detect from LANG/LC_ALL env var.
    let locale = if config.general.language == "auto" {
        Locale::auto()
    } else {
        Locale::load(&config.general.language)
    };

    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let mut app = App::new(config, locale, cli.file.as_deref(), width, height)?;
    app.run()
}

/// Render a Markdown file to the terminal using ANSI escape codes, then exit.
/// This mirrors what `glow <file>` does, allowing direct visual comparison.
fn render_to_terminal(path: &std::path::Path, theme_name: &str) -> Result<()> {
    use hi::ui::mdrender::{MdRenderer, MdTheme};

    let markdown = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Cannot read {:?}: {}", path, e))?;

    let theme = match theme_name {
        "dark"                => MdTheme::dark(),
        "dracula"             => MdTheme::dracula(),
        "tokyo-night"         => MdTheme::tokyo_night(),
        "monokai-pro"         => MdTheme::monokai_pro(),
        "github-dark"         => MdTheme::github_dark(),
        "one-dark-pro"        => MdTheme::one_dark_pro(),
        "electric"            => MdTheme::electric_impressionism(),
        "synthwave"           => MdTheme::synthwave(),
        _                     => MdTheme::glow_dark(),   // default / "glow-dark"
    };

    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    // glow uses a 2-column margin on each side → effective width = term_width - 4
    // We replicate that by passing (term_width - 4) as the render width, then
    // printing a 2-space left margin before each line.
    let render_width = (term_width as usize).saturating_sub(4).max(20);
    let renderer = MdRenderer::new(theme);
    let lines = renderer.render(&markdown, render_width);

    // Print a leading blank line (glow's document block_prefix = "\n")
    println!();

    for line in &lines {
        // 2-space left margin (glow document margin = 2)
        print!("  ");

        // Indent (blockquote, list nesting, code block padding)
        if line.indent > 0 {
            print!("{}", " ".repeat(line.indent));
        }

        // Optional left border (blockquote │), stored as (text, color)
        if let Some((ref text, color)) = line.border {
            let fg = color_to_ansi_fg(color);
            print!("\x1b[{}m{}\x1b[0m", fg, text);
        }

        // Spans
        for span in &line.spans {
            print_ansi_span(span);
        }

        println!();
    }

    // Trailing blank line (glow's document block_suffix = "\n")
    println!();

    Ok(())
}

/// Print a single StyledSpan using raw ANSI escape codes.
fn print_ansi_span(span: &hi::ui::mdrender::StyledSpan) {
    if span.text.is_empty() {
        return;
    }

    let mut codes: Vec<u8> = Vec::new();

    if span.bold          { codes.push(1); }
    if span.italic        { codes.push(3); }
    if span.underline     { codes.push(4); }
    if span.strikethrough { codes.push(9); }
    if span.dim           { codes.push(2); }

    let fg_code = span.fg.map(color_to_ansi_fg);
    let bg_code = span.bg.map(color_to_ansi_bg);

    let has_style = !codes.is_empty() || fg_code.is_some() || bg_code.is_some();

    if has_style {
        let mut parts: Vec<String> = codes.iter().map(|c| c.to_string()).collect();
        if let Some(ref fg) = fg_code { parts.push(fg.clone()); }
        if let Some(ref bg) = bg_code { parts.push(bg.clone()); }
        print!("\x1b[{}m{}\x1b[0m", parts.join(";"), span.text);
    } else {
        print!("{}", span.text);
    }
}

fn color_to_ansi_fg(c: crossterm::style::Color) -> String {
    match c {
        Color::Rgb { r, g, b } => format!("38;2;{r};{g};{b}"),
        Color::AnsiValue(n)    => format!("38;5;{n}"),
        Color::Black           => "30".into(),
        Color::DarkRed         => "31".into(),
        Color::DarkGreen       => "32".into(),
        Color::DarkYellow      => "33".into(),
        Color::DarkBlue        => "34".into(),
        Color::DarkMagenta     => "35".into(),
        Color::DarkCyan        => "36".into(),
        Color::Grey            => "37".into(),
        Color::DarkGrey        => "90".into(),
        Color::Red             => "91".into(),
        Color::Green           => "92".into(),
        Color::Yellow          => "93".into(),
        Color::Blue            => "94".into(),
        Color::Magenta         => "95".into(),
        Color::Cyan            => "96".into(),
        Color::White           => "97".into(),
        _                      => "39".into(),
    }
}

fn color_to_ansi_bg(c: crossterm::style::Color) -> String {
    match c {
        Color::Rgb { r, g, b } => format!("48;2;{r};{g};{b}"),
        Color::AnsiValue(n)    => format!("48;5;{n}"),
        Color::Black           => "40".into(),
        Color::DarkRed         => "41".into(),
        Color::DarkGreen       => "42".into(),
        Color::DarkYellow      => "43".into(),
        Color::DarkBlue        => "44".into(),
        Color::DarkMagenta     => "45".into(),
        Color::DarkCyan        => "46".into(),
        Color::Grey            => "47".into(),
        Color::DarkGrey        => "100".into(),
        Color::Red             => "101".into(),
        Color::Green           => "102".into(),
        Color::Yellow          => "103".into(),
        Color::Blue            => "104".into(),
        Color::Magenta         => "105".into(),
        Color::Cyan            => "106".into(),
        Color::White           => "107".into(),
        _                      => "49".into(),
    }
}
