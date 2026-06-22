// Flick - commands.rs
// Tauri IPC command handlers - per PRD §8.5.
// These bridge the Svelte frontend to the Rust backend.

use crate::{ai_client, config, keychain};
use tauri::{AppHandle, Manager};

const BUILTIN_TRIGGERS: &[&str] = &[
    "fix",
    "formal",
    "casual",
    "shorter",
    "longer",
    "improve",
    "rephrase",
    "bullet",
    "explain",
    "translate",
];

fn sync_config_state(app: &AppHandle, cfg: &config::FlickConfig) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        *state.config.lock().unwrap() = cfg.clone();
        *state.enabled.lock().unwrap() = cfg.enabled;
        *state.custom_triggers.lock().unwrap() = config::get_custom_trigger_names(cfg);
    }
}

fn normalize_trigger(trigger: &str) -> String {
    trigger.trim().trim_start_matches('!').to_lowercase()
}

fn validate_custom_command(
    cfg: &config::FlickConfig,
    trigger: &str,
    prompt: &str,
    current_index: Option<usize>,
) -> Result<(), String> {
    if trigger.len() < 2 || trigger.len() > 32 {
        return Err("Trigger must be 2-32 characters.".to_string());
    }
    if !trigger
        .chars()
        .enumerate()
        .all(|(i, c)| c.is_ascii_lowercase() || c.is_ascii_digit() && i > 0 || c == '-' || c == '_')
        || !trigger
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
    {
        return Err("Trigger must start with a letter and use lowercase letters, numbers, dashes, or underscores.".to_string());
    }
    if BUILTIN_TRIGGERS.contains(&trigger) {
        return Err(format!("!{} is already a built-in command.", trigger));
    }
    if !prompt.contains("{{text}}") {
        return Err("Prompt template must include {{text}}.".to_string());
    }
    if let Some((i, _)) = cfg
        .custom_commands
        .iter()
        .enumerate()
        .find(|(i, cmd)| cmd.trigger == trigger && Some(*i) != current_index)
    {
        return Err(format!("!{} already exists at position {}.", trigger, i + 1));
    }
    Ok(())
}

/// Save the configured API key to the OS keychain.
#[tauri::command]
pub async fn save_api_key(key: String) -> Result<(), String> {
    keychain::save_api_key(&key).map_err(|e| e.to_string())
}

/// Load the configured API key from the OS keychain.
#[tauri::command]
pub async fn load_api_key() -> Result<String, String> {
    keychain::load_api_key().map_err(|e| e.to_string())
}

/// Test the selected provider/model connection with the provided key.
#[tauri::command]
pub async fn test_api_connection(
    key: String,
    provider: String,
    model: String,
) -> Result<(), String> {
    ai_client::test_connection(&key, &provider, &model)
        .await
        .map_err(|e| e.to_string())
}

/// Get the current application configuration.
#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<config::FlickConfig, String> {
    config::load_config(&app).map_err(|e| e.to_string())
}

/// Save the full application configuration.
#[tauri::command]
pub async fn save_config(app: AppHandle, config: config::FlickConfig) -> Result<(), String> {
    config::save_config(&app, &config).map_err(|e| e.to_string())?;
    sync_config_state(&app, &config);
    Ok(())
}

/// Toggle the enabled/disabled state of Flick.
#[tauri::command]
pub async fn toggle_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    cfg.enabled = enabled;
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    sync_config_state(&app, &cfg);

    log::info!("Flick {}", if enabled { "enabled" } else { "disabled" });
    Ok(())
}

/// Add a new custom command.
#[tauri::command]
pub async fn add_custom_command(
    app: AppHandle,
    trigger: String,
    prompt: String,
) -> Result<(), String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    let trigger = normalize_trigger(&trigger);
    let prompt = prompt.trim().to_string();
    validate_custom_command(&cfg, &trigger, &prompt, None)?;
    cfg.custom_commands.push(config::CustomCommand {
        trigger: trigger.clone(),
        prompt,
    });
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    sync_config_state(&app, &cfg);

    log::info!("Custom command added: !{}", trigger);
    Ok(())
}

/// Update an existing custom command by index.
#[tauri::command]
pub async fn update_custom_command(
    app: AppHandle,
    index: usize,
    trigger: String,
    prompt: String,
) -> Result<(), String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    if index >= cfg.custom_commands.len() {
        return Err("Invalid command index".to_string());
    }
    let trigger = normalize_trigger(&trigger);
    let prompt = prompt.trim().to_string();
    validate_custom_command(&cfg, &trigger, &prompt, Some(index))?;
    cfg.custom_commands[index] = config::CustomCommand {
        trigger: trigger.clone(),
        prompt,
    };
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    sync_config_state(&app, &cfg);

    log::info!("Custom command updated at index {}: !{}", index, trigger);
    Ok(())
}

/// Delete a custom command by index.
#[tauri::command]
pub async fn delete_custom_command(app: AppHandle, index: usize) -> Result<(), String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    if index >= cfg.custom_commands.len() {
        return Err("Invalid command index".to_string());
    }
    let removed = cfg.custom_commands.remove(index);
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    sync_config_state(&app, &cfg);

    log::info!("Custom command deleted: !{}", removed.trigger);
    Ok(())
}
