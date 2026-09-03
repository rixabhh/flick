// Flick - config.rs
// Per PRD §8.8: Non-sensitive settings stored as JSON in the app data directory.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const CONFIG_FILENAME: &str = "config.json";

/// A user-defined custom command - per §8.5.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomCommand {
    #[serde(default)]
    pub id: String,
    pub trigger: String,
    pub prompt: String,
}

/// Metadata for a locally installed speech model. Model binaries deliberately
/// live outside this config file; this record is only used to present the
/// user's installed models and safely recover after an interrupted download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalModel {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextCorrection {
    pub find: String,
    pub replace: String,
}

/// Application configuration - per §8.8.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlickConfig {
    #[serde(default = "default_config_version")]
    pub version: u32,
    pub enabled: bool,
    pub launch_at_login: bool,
    pub show_done_toast: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    #[serde(default)]
    pub onboarding_complete: bool,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub custom_base_url: String,
    pub custom_commands: Vec<CustomCommand>,
    #[serde(default = "default_composer_shortcut")]
    pub composer_shortcut: String,
    #[serde(default = "default_copy_last_result_shortcut")]
    pub copy_last_result_shortcut: String,
    #[serde(default = "default_paste_plain_text_shortcut")]
    pub paste_plain_text_shortcut: String,
    #[serde(default = "default_dictation_shortcut")]
    pub dictation_shortcut: String,
    #[serde(default = "default_dictation_mode")]
    pub dictation_mode: String,
    #[serde(default)]
    pub dictation_device_id: String,
    #[serde(default = "default_dictation_model_id")]
    pub dictation_model_id: String,
    #[serde(default = "default_dictation_language")]
    pub dictation_language: String,
    #[serde(default)]
    pub dictation_translate_to_english: bool,
    #[serde(default = "default_true")]
    pub dictation_filler_cleanup: bool,
    #[serde(default)]
    pub dictation_llm_post_process: bool,
    #[serde(default)]
    pub dictation_corrections: Vec<TextCorrection>,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_true")]
    pub history_enabled: bool,
    #[serde(default)]
    pub retain_recordings: bool,
    #[serde(default = "default_recording_retention_count")]
    pub recording_retention_count: usize,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default)]
    pub local_models: Vec<LocalModel>,
    #[serde(default)]
    pub disabled_apps: Vec<String>,
    #[serde(default)]
    pub auto_submit_apps: Vec<String>,
}

fn default_config_version() -> u32 {
    2
}
fn default_theme() -> String {
    "system".to_string()
}
fn default_app_language() -> String {
    "en".to_string()
}
fn default_composer_shortcut() -> String {
    #[cfg(target_os = "macos")]
    {
        return "Cmd+Shift+Space".to_string();
    }
    #[cfg(not(target_os = "macos"))]
    "Ctrl+Shift+Space".to_string()
}
fn default_dictation_shortcut() -> String {
    #[cfg(target_os = "macos")]
    {
        return "Cmd+Space".to_string();
    }
    #[cfg(not(target_os = "macos"))]
    "Ctrl+Space".to_string()
}
fn default_copy_last_result_shortcut() -> String {
    #[cfg(target_os = "macos")]
    {
        return "Cmd+Alt+C".to_string();
    }
    #[cfg(not(target_os = "macos"))]
    "Ctrl+Alt+C".to_string()
}
fn default_paste_plain_text_shortcut() -> String {
    #[cfg(target_os = "macos")]
    {
        return "Cmd+Alt+V".to_string();
    }
    #[cfg(not(target_os = "macos"))]
    "Ctrl+Alt+V".to_string()
}
fn default_dictation_mode() -> String {
    "hold-or-toggle".to_string()
}
fn default_dictation_model_id() -> String {
    "whisper-tiny-en".to_string()
}
fn default_dictation_language() -> String {
    "en".to_string()
}
fn default_true() -> bool {
    true
}
fn default_history_limit() -> usize {
    100
}
fn default_recording_retention_count() -> usize {
    20
}

impl Default for FlickConfig {
    fn default() -> Self {
        Self {
            version: 2,
            enabled: true,
            launch_at_login: false,
            show_done_toast: true,
            theme: default_theme(),
            app_language: default_app_language(),
            onboarding_complete: false,
            provider: "gemini".to_string(),
            model: "gemini-2.5-flash-lite".to_string(),
            custom_base_url: String::new(),
            custom_commands: Vec::new(),
            composer_shortcut: default_composer_shortcut(),
            copy_last_result_shortcut: default_copy_last_result_shortcut(),
            paste_plain_text_shortcut: default_paste_plain_text_shortcut(),
            dictation_shortcut: default_dictation_shortcut(),
            dictation_mode: default_dictation_mode(),
            dictation_device_id: String::new(),
            dictation_model_id: default_dictation_model_id(),
            dictation_language: default_dictation_language(),
            dictation_translate_to_english: false,
            dictation_filler_cleanup: true,
            dictation_llm_post_process: false,
            dictation_corrections: Vec::new(),
            append_trailing_space: false,
            history_enabled: true,
            retain_recordings: false,
            recording_retention_count: default_recording_retention_count(),
            history_limit: default_history_limit(),
            local_models: Vec::new(),
            disabled_apps: Vec::new(),
            auto_submit_apps: Vec::new(),
        }
    }
}

/// Get the config file path using Tauri's app data directory.
fn config_path(app: &AppHandle) -> Result<PathBuf> {
    let app_data = app
        .path()
        .app_data_dir()
        .context("Failed to resolve app data directory")?;
    fs::create_dir_all(&app_data).context("Failed to create app data directory")?;
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
    let contents = fs::read_to_string(&path).context("Failed to read config file")?;
    let config: FlickConfig = serde_json::from_str(&contents).unwrap_or_else(|_| {
        log::warn!("Config file corrupted, using defaults");
        FlickConfig::default()
    });
    let (config, migrated) = migrate_config(config);
    if migrated {
        save_config(app, &config)?;
    }
    Ok(config)
}

/// Upgrade an in-memory configuration without discarding any Flick 1.x
/// settings. Keeping this pure lets upgrades be tested without an app-data
/// directory and makes future version transitions explicit.
fn migrate_config(mut config: FlickConfig) -> (FlickConfig, bool) {
    // Flick 1.x command entries did not have stable IDs. Fill them on load so
    // callers can stop relying on array positions without breaking old users.
    let mut migrated = config.version < 2;
    config.version = 2;
    for command in &mut config.custom_commands {
        if command.id.is_empty() {
            command.id = format!("cmd-{}", command.trigger);
            migrated = true;
        }
    }
    (config, migrated)
}

/// Save configuration to disk.
pub fn save_config(app: &AppHandle, config: &FlickConfig) -> Result<()> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(&path, json).context("Failed to write config file")?;
    Ok(())
}

/// Extract custom trigger names from the config for trigger detection.
pub fn get_custom_trigger_names(config: &FlickConfig) -> Vec<String> {
    config
        .custom_commands
        .iter()
        .map(|c| c.trigger.clone())
        .collect()
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
        assert_eq!(config.provider, "gemini");
        assert_eq!(config.model, "gemini-2.5-flash-lite");
        assert_eq!(config.dictation_model_id, "whisper-tiny-en");
        assert!(config.custom_commands.is_empty());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let config = FlickConfig {
            enabled: true,
            launch_at_login: true,
            show_done_toast: false,
            provider: "openrouter".to_string(),
            model: "openai/gpt-4o-mini".to_string(),
            custom_commands: vec![CustomCommand {
                id: "cmd-summarize".to_string(),
                trigger: "summarize".to_string(),
                prompt: "Summarize: {{text}}".to_string(),
            }],
            ..FlickConfig::default()
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
                CustomCommand {
                    id: "cmd-tldr".into(),
                    trigger: "tldr".into(),
                    prompt: "TLDR: {{text}}".into(),
                },
                CustomCommand {
                    id: "cmd-poem".into(),
                    trigger: "poem".into(),
                    prompt: "Write as poem: {{text}}".into(),
                },
            ],
            ..FlickConfig::default()
        };
        let names = get_custom_trigger_names(&config);
        assert_eq!(names, vec!["tldr".to_string(), "poem".to_string()]);
    }

    #[test]
    fn migrates_v1_without_losing_existing_preferences() {
        let legacy = FlickConfig {
            version: 1,
            enabled: false,
            launch_at_login: true,
            show_done_toast: false,
            provider: "openrouter".into(),
            model: "openai/gpt-4o-mini".into(),
            custom_commands: vec![CustomCommand {
                id: String::new(),
                trigger: "tldr".into(),
                prompt: "Summarize {{text}}".into(),
            }],
            ..FlickConfig::default()
        };

        let (migrated, changed) = migrate_config(legacy);
        assert!(changed);
        assert_eq!(migrated.version, 2);
        assert!(!migrated.enabled);
        assert!(migrated.launch_at_login);
        assert!(!migrated.show_done_toast);
        assert_eq!(migrated.provider, "openrouter");
        assert_eq!(migrated.model, "openai/gpt-4o-mini");
        assert_eq!(migrated.custom_commands[0].id, "cmd-tldr");
    }
}
