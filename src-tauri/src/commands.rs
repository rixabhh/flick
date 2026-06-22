// Flick - commands.rs
// Tauri IPC command handlers - per PRD §8.5.
// These bridge the Svelte frontend to the Rust backend.

use crate::{ai_client, config, keychain};
use tauri::{AppHandle, Manager};

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
    config::save_config(&app, &config).map_err(|e| e.to_string())
}

/// Toggle the enabled/disabled state of Flick.
#[tauri::command]
pub async fn toggle_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    cfg.enabled = enabled;
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;

    // Update the global enabled state
    if let Some(state) = app.try_state::<crate::AppState>() {
        let mut guard = state.enabled.lock().unwrap();
        *guard = enabled;
    }

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
    cfg.custom_commands.push(config::CustomCommand {
        trigger: trigger.clone(),
        prompt,
    });
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;

    // Update the shared custom triggers list
    if let Some(state) = app.try_state::<crate::AppState>() {
        let mut guard = state.custom_triggers.lock().unwrap();
        *guard = config::get_custom_trigger_names(&cfg);
    }

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
    cfg.custom_commands[index] = config::CustomCommand {
        trigger: trigger.clone(),
        prompt,
    };
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;

    if let Some(state) = app.try_state::<crate::AppState>() {
        let mut guard = state.custom_triggers.lock().unwrap();
        *guard = config::get_custom_trigger_names(&cfg);
    }

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

    if let Some(state) = app.try_state::<crate::AppState>() {
        let mut guard = state.custom_triggers.lock().unwrap();
        *guard = config::get_custom_trigger_names(&cfg);
    }

    log::info!("Custom command deleted: !{}", removed.trigger);
    Ok(())
}
