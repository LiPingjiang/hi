use std::path::PathBuf;
use anyhow::Result;
use super::Config;

/// Alias used by main.rs — load config from ~/.hirc (or $HI_CONFIG).
pub fn load_config() -> anyhow::Result<Config> {
    Ok(load())
}

/// Load config from ~/.hirc (or $HI_CONFIG), falling back to defaults.
/// Never panics — if the file is missing or malformed, returns defaults.
pub fn load() -> Config {
    let path = config_path();
    let mut config = read_from_path(&path).unwrap_or_default();
    apply_env_overrides(&mut config);
    config
}

fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("HI_CONFIG") {
        return PathBuf::from(p);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".hirc")
}

fn read_from_path(path: &PathBuf) -> Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

fn apply_env_overrides(config: &mut Config) {
    if let Ok(key) = std::env::var("HI_API_KEY") {
        if !key.is_empty() {
            config.ai.api_key = key;
        }
    }
    if let Ok(model) = std::env::var("HI_MODEL") {
        if !model.is_empty() {
            config.ai.model = model;
        }
    }
}

/// Persist the chosen theme into `~/.hirc` so it survives restarts.
///
/// Strategy: read the existing file as a raw `toml::Value` table, update
/// (or create) the `[theme]` section with `editor_theme` and `chat_theme`
/// both set to `name`, then serialise the whole table back.  This preserves
/// every other section (`[ai]`, `[general]`, …) and their comments are lost
/// only because the `toml` crate doesn't preserve comments — acceptable
/// trade-off to avoid adding `toml_edit` as a dependency.
pub fn save_theme(name: &str) -> Result<()> {
    let path = config_path();

    // Read existing content or start with an empty table.
    let mut table: toml::value::Table = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content).unwrap_or_default()
    } else {
        toml::value::Table::new()
    };

    // Upsert [theme] section
    let theme_section = table
        .entry("theme".to_string())
        .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    if let toml::Value::Table(ref mut t) = theme_section {
        t.insert("editor_theme".to_string(), toml::Value::String(name.to_string()));
        t.insert("chat_theme".to_string(), toml::Value::String(name.to_string()));
    }

    let serialised = toml::to_string_pretty(&table)?;
    std::fs::write(&path, serialised)?;
    Ok(())
}
