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
