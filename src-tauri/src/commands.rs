// Flick - commands.rs
// Tauri IPC command handlers - per PRD §8.5.
// These bridge the Svelte frontend to the Rust backend.

use crate::{ai_client, config, keychain};
use serde::Deserialize;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
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
    current_id: Option<&str>,
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
    if prompt.trim().is_empty() {
        return Err("System prompt is required.".to_string());
    }
    if prompt.len() > 2000 {
        return Err("System prompt must be 2000 characters or fewer.".to_string());
    }
    if let Some((i, _)) = cfg
        .custom_commands
        .iter()
        .enumerate()
        .find(|(_, cmd)| cmd.trigger == trigger && Some(cmd.id.as_str()) != current_id)
    {
        return Err(format!(
            "!{} already exists at position {}.",
            trigger,
            i + 1
        ));
    }
    Ok(())
}

/// Save the configured API key to the OS keychain.
#[tauri::command]
pub async fn save_api_key(key: String, provider: Option<String>) -> Result<(), String> {
    keychain::save_api_key(provider.as_deref().unwrap_or("gemini"), &key).map_err(|e| e.to_string())
}

/// Load the configured API key from the OS keychain.
#[tauri::command]
pub async fn load_api_key(provider: Option<String>) -> Result<String, String> {
    keychain::load_api_key(provider.as_deref().unwrap_or("gemini")).map_err(|e| e.to_string())
}

/// Remove the provider credential from the operating system keychain.
#[tauri::command]
pub async fn delete_api_key(provider: Option<String>) -> Result<(), String> {
    keychain::delete_api_key(provider.as_deref().unwrap_or("gemini")).map_err(|e| e.to_string())
}

/// Test the selected provider/model connection with the provided key.
#[tauri::command]
pub async fn test_api_connection(
    key: String,
    provider: String,
    model: String,
    custom_base_url: Option<String>,
) -> Result<(), String> {
    ai_client::test_connection(&key, &provider, &model, custom_base_url.as_deref())
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
) -> Result<config::CustomCommand, String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    let trigger = normalize_trigger(&trigger);
    let prompt = prompt.trim().to_string();
    validate_custom_command(&cfg, &trigger, &prompt, None)?;
    let command = config::CustomCommand {
        id: format!("cmd-{}", trigger),
        trigger: trigger.clone(),
        prompt,
    };
    cfg.custom_commands.push(command.clone());
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    sync_config_state(&app, &cfg);

    log::info!("Custom command added: !{}", trigger);
    Ok(command)
}

/// Update an existing custom command by its stable ID.
#[tauri::command]
pub async fn update_custom_command(
    app: AppHandle,
    id: String,
    trigger: String,
    prompt: String,
) -> Result<(), String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    let index = cfg
        .custom_commands
        .iter()
        .position(|command| command.id == id)
        .ok_or_else(|| "Unknown command".to_string())?;
    let trigger = normalize_trigger(&trigger);
    let prompt = prompt.trim().to_string();
    validate_custom_command(&cfg, &trigger, &prompt, Some(&id))?;
    let id = cfg.custom_commands[index].id.clone();
    cfg.custom_commands[index] = config::CustomCommand {
        id,
        trigger: trigger.clone(),
        prompt,
    };
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    sync_config_state(&app, &cfg);

    log::info!("Custom command updated: !{}", trigger);
    Ok(())
}

/// Delete a custom command by its stable ID.
#[tauri::command]
pub async fn delete_custom_command(app: AppHandle, id: String) -> Result<(), String> {
    let mut cfg = config::load_config(&app).map_err(|e| e.to_string())?;
    let index = cfg
        .custom_commands
        .iter()
        .position(|command| command.id == id)
        .ok_or_else(|| "Unknown command".to_string())?;
    let removed = cfg.custom_commands.remove(index);
    config::save_config(&app, &cfg).map_err(|e| e.to_string())?;
    sync_config_state(&app, &cfg);

    log::info!("Custom command deleted: !{}", removed.trigger);
    Ok(())
}

#[derive(Deserialize)]
struct ImportedCommand {
    #[serde(default)]
    trigger: String,
    #[serde(default)]
    prompt: String,
}

/// Write non-secret custom command templates to an explicit local JSON file.
#[tauri::command]
pub async fn export_command_templates(app: AppHandle) -> Result<String, String> {
    let config = config::load_config(&app).map_err(|error| error.to_string())?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("exports");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = directory.join(format!("flick-command-templates-{timestamp}.json"));
    let export: Vec<ImportedCommandExport<'_>> = config
        .custom_commands
        .iter()
        .map(|command| ImportedCommandExport {
            trigger: &command.trigger,
            prompt: &command.prompt,
        })
        .collect();
    let contents = serde_json::to_string_pretty(&export).map_err(|error| error.to_string())?;
    fs::write(&path, contents).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[derive(serde::Serialize)]
struct ImportedCommandExport<'a> {
    trigger: &'a str,
    prompt: &'a str,
}

/// Add valid templates from a user-provided JSON file. Existing trigger names
/// are retained, so importing never overwrites a user's local command.
#[tauri::command]
pub async fn import_command_templates(app: AppHandle, path: String) -> Result<usize, String> {
    let metadata =
        fs::metadata(&path).map_err(|error| format!("Could not read import file: {error}"))?;
    if metadata.len() > 256 * 1024 {
        return Err("Template import files must be 256 KB or smaller.".into());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read import file: {error}"))?;
    let imported: Vec<ImportedCommand> = serde_json::from_str(&contents)
        .map_err(|error| format!("Invalid template JSON: {error}"))?;
    let mut config = config::load_config(&app).map_err(|error| error.to_string())?;
    let mut added = 0;
    for template in imported {
        let trigger = normalize_trigger(&template.trigger);
        let prompt = template.prompt.trim().to_string();
        if config
            .custom_commands
            .iter()
            .any(|command| command.trigger == trigger)
        {
            continue;
        }
        validate_custom_command(&config, &trigger, &prompt, None)?;
        config.custom_commands.push(config::CustomCommand {
            id: format!("cmd-{trigger}"),
            trigger,
            prompt,
        });
        added += 1;
    }
    if added > 0 {
        crate::config::save_config(&app, &config).map_err(|error| error.to_string())?;
        sync_config_state(&app, &config);
    }
    Ok(added)
}
