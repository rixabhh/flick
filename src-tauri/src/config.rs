// Flick — config.rs
// Per PRD §8.8: Non-sensitive settings stored as JSON in the app data directory.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const CONFIG_FILENAME: &str = "config.json";

/// A user-defined custom command — per §8.5.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomCommand {
    pub trigger: String,
    pub prompt: String,
}

/// Application configuration — per §8.8.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlickConfig {
    pub enabled: bool,
    pub launch_at_login: bool,
    pub show_done_toast: bool,
    pub custom_commands: Vec<CustomCommand>,
}

impl Default for FlickConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            launch_at_login: false,
            show_done_toast: true,
            custom_commands: Vec::new(),
        }
    }
}

/// Get the config file path using Tauri's app data directory.
fn config_path(app: &AppHandle) -> Result<PathBuf> {
    let app_data = app
        .path()
        .app_data_dir()
        .context("Failed to resolve app data directory")?;
    fs::create_dir_all(&app_data)
        .context("Failed to create app data directory")?;
    Ok(app_data.join(CONFIG_FILENAME))
}

/// Load configuration from disk. Returns default config if file doesn't exist.
pub fn load_config(app: &AppHandle) -> Result<FlickConfig> {
    let path = config_path(app)?;
    if !path.exists() {
        let default = FlickConfig::default();
        save_config(app, &default)?;
        return Ok(default);
    }
    let contents = fs::read_to_string(&path)
        .context("Failed to read config file")?;
    let config: FlickConfig = serde_json::from_str(&contents)
        .unwrap_or_else(|_| {
            log::warn!("Config file corrupted, using defaults");
            FlickConfig::default()
        });
    Ok(config)
}

/// Save configuration to disk.
pub fn save_config(app: &AppHandle, config: &FlickConfig) -> Result<()> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(config)
        .context("Failed to serialize config")?;
    fs::write(&path, json)
        .context("Failed to write config file")?;
    Ok(())
}

/// Extract custom trigger names from the config for trigger detection.
pub fn get_custom_trigger_names(config: &FlickConfig) -> Vec<String> {
    config.custom_commands.iter().map(|c| c.trigger.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FlickConfig::default();
        assert!(config.enabled);
        assert!(!config.launch_at_login);
        assert!(config.show_done_toast);
        assert!(config.custom_commands.is_empty());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let config = FlickConfig {
            enabled: true,
            launch_at_login: true,
            show_done_toast: false,
            custom_commands: vec![
                CustomCommand {
                    trigger: "summarize".to_string(),
                    prompt: "Summarize: {{text}}".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: FlickConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(deserialized.custom_commands.len(), 1);
        assert_eq!(deserialized.custom_commands[0].trigger, "summarize");
    }

    #[test]
    fn test_get_custom_trigger_names() {
        let config = FlickConfig {
            custom_commands: vec![
                CustomCommand { trigger: "tldr".into(), prompt: "TLDR: {{text}}".into() },
                CustomCommand { trigger: "poem".into(), prompt: "Write as poem: {{text}}".into() },
            ],
            ..FlickConfig::default()
        };
        let names = get_custom_trigger_names(&config);
        assert_eq!(names, vec!["tldr".to_string(), "poem".to_string()]);
    }
}
