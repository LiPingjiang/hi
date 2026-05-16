//! Markdown preview: convert buffer content to styled HTML and open in browser.
//!
//! Uses `pulldown-cmark` for Markdown→HTML conversion and embeds a light-themed
//! CSS stylesheet (GitHub-light inspired) for beautiful rendering.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use pulldown_cmark::{html, Options, Parser};
use crate::locale::Locale;

/// Generate a complete HTML document from Markdown source with embedded CSS.
fn markdown_to_html(markdown: &str, title: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, opts);
    let mut html_body = String::new();
    html::push_html(&mut html_body, parser);

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
{css}
</style>
</head>
<body>
<article class="markdown-body">
{body}
</article>
</body>
</html>"#,
        title = html_escape(title),
        css = CSS_THEME,
        body = html_body,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Open the current buffer as a Markdown preview in the default browser.
///
/// Returns a user-facing message string (success or error).
pub fn open_preview(buffer_content: &str, file_path: Option<&Path>, locale: &Locale) -> String {
    let title = file_path
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled.md".to_string());

    // Check if the file looks like Markdown
    let is_md = file_path
        .and_then(|p| p.extension())
        .map(|ext| {
            let e = ext.to_string_lossy().to_lowercase();
            e == "md" || e == "markdown" || e == "mkd" || e == "mdx"
        })
        .unwrap_or(false);

    if !is_md {
        return locale.messages.preview_not_markdown.clone();
    }

    let html = markdown_to_html(buffer_content, &title);

    // Write to temp file
    let tmp_name = format!("hi-preview-{}.html", sanitize_filename(&title));
    let tmp_path = std::env::temp_dir().join(&tmp_name);

    match std::fs::File::create(&tmp_path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(html.as_bytes()) {
                return locale.messages.preview_write_failed.replace("{err}", &e.to_string());
            }
        }
        Err(e) => return locale.messages.preview_write_failed.replace("{err}", &e.to_string()),
    }

    // Open in browser
    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg(&tmp_path).spawn()
    } else if cfg!(target_os = "linux") {
        Command::new("xdg-open").arg(&tmp_path).spawn()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start"]).arg(&tmp_path).spawn()
    } else {
        return "Unsupported platform for preview".to_string();
    };

    match result {
        Ok(_) => locale.messages.preview_opened.replace("{path}", &tmp_path.display().to_string()),
        Err(e) => locale.messages.preview_open_failed.replace("{err}", &e.to_string()),
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

// ── Embedded CSS ─────────────────────────────────────────────────────────────

const CSS_THEME: &str = r#"
:root {
    --bg: #ffffff;
    --fg: #1f2328;
    --fg-muted: #656d76;
    --border: #d0d7de;
    --link: #0969da;
    --code-bg: #f6f8fa;
    --code-border: #d0d7de;
    --blockquote-border: #d0d7de;
    --heading-color: #1f2328;
    --table-border: #d0d7de;
    --table-row-alt: #f6f8fa;
    --accent: #0969da;
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

html {
    font-size: 16px;
    -webkit-font-smoothing: antialiased;
}

body {
    background: var(--bg);
    color: var(--fg);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans",
                 Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji";
    line-height: 1.7;
    padding: 2rem;
    max-width: 900px;
    margin: 0 auto;
}

.markdown-body {
    padding: 2rem 0;
}

/* ── Headings ─────────────────────────────────────────── */
h1, h2, h3, h4, h5, h6 {
    color: var(--heading-color);
    font-weight: 600;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
    line-height: 1.3;
}

h1 {
    font-size: 2em;
    padding-bottom: 0.3em;
    border-bottom: 1px solid var(--border);
}

h2 {
    font-size: 1.5em;
    padding-bottom: 0.25em;
    border-bottom: 1px solid var(--border);
}

h3 { font-size: 1.25em; }
h4 { font-size: 1em; }
h5 { font-size: 0.875em; }
h6 { font-size: 0.85em; color: var(--fg-muted); }

/* ── Paragraphs & text ────────────────────────────────── */
p {
    margin-bottom: 1em;
}

a {
    color: var(--link);
    text-decoration: none;
}
a:hover {
    text-decoration: underline;
}

strong { font-weight: 600; }
em { font-style: italic; }
del { text-decoration: line-through; color: var(--fg-muted); }

/* ── Code ─────────────────────────────────────────────── */
code {
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 0.875em;
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-radius: 6px;
    padding: 0.2em 0.4em;
}

pre {
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-radius: 8px;
    padding: 1em 1.2em;
    overflow-x: auto;
    margin-bottom: 1em;
    line-height: 1.5;
}

pre code {
    background: none;
    border: none;
    padding: 0;
    font-size: 0.875em;
}

/* ── Blockquotes ──────────────────────────────────────── */
blockquote {
    border-left: 4px solid var(--blockquote-border);
    padding: 0.5em 1em;
    margin: 0 0 1em 0;
    color: var(--fg-muted);
    background: var(--code-bg);
    border-radius: 0 6px 6px 0;
}

blockquote p {
    margin-bottom: 0.5em;
}
blockquote p:last-child {
    margin-bottom: 0;
}

/* ── Lists ────────────────────────────────────────────── */
ul, ol {
    padding-left: 2em;
    margin-bottom: 1em;
}

li {
    margin-bottom: 0.25em;
}

li > p {
    margin-bottom: 0.5em;
}

/* Task lists */
li input[type="checkbox"] {
    margin-right: 0.5em;
}

/* ── Tables ───────────────────────────────────────────── */
table {
    border-collapse: collapse;
    width: 100%;
    margin-bottom: 1em;
    overflow-x: auto;
    display: block;
}

th, td {
    border: 1px solid var(--table-border);
    padding: 0.6em 1em;
    text-align: left;
}

th {
    background: var(--code-bg);
    font-weight: 600;
}

tr:nth-child(even) {
    background: var(--table-row-alt);
}

/* ── Horizontal rules ─────────────────────────────────── */
hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 2em 0;
}

/* ── Images ───────────────────────────────────────────── */
img {
    max-width: 100%;
    height: auto;
    border-radius: 8px;
    margin: 1em 0;
}

/* ── Footnotes ────────────────────────────────────────── */
.footnote-definition {
    font-size: 0.875em;
    color: var(--fg-muted);
    margin-top: 2em;
    padding-top: 1em;
    border-top: 1px solid var(--border);
}
"#;
