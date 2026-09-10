// Flick - commands.rs
// Tauri IPC command handlers - per PRD §8.5.
// These bridge the Svelte frontend to the Rust backend.

use crate::{ai_client, config, keychain};
use serde::Deserialize;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, PhysicalPosition, Position};

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
    Ok(())
}

#[tauri::command]
pub async fn update_config_fields(
    app: AppHandle,
    patch: serde_json::Value,
) -> Result<config::FlickConfig, String> {
    config::update_config(&app, |current| {
        *current = config::merge_fields(current, patch)?;
        Ok(())
    })
    .map_err(|error| error.to_string())
}

/// Apply the user's shared pill placement to the hidden floating windows. We
/// compute physical coordinates against the current/primary monitor, so this
/// remains correct on DPI-scaled and multi-monitor desktops.
#[tauri::command]
pub async fn apply_floating_pill_position(app: AppHandle) -> Result<(), String> {
    position_floating_pills(&app).map_err(|error| error.to_string())
}

pub fn position_floating_pills(app: &AppHandle) -> anyhow::Result<()> {
    let position = config::load_config(app)?.floating_pill_position;
    for label in ["dictation", "toast"] {
        let Some(window) = app.get_webview_window(label) else {
            continue;
        };
        let monitor = window
            .current_monitor()?
            .or(app.primary_monitor()?)
            .ok_or_else(|| {
                anyhow::anyhow!("Could not determine a display for Flick's floating pill")
            })?;
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let window_size = window.outer_size()?;
        let margin = 24_i32;
        let left = monitor_position.x;
        let top = monitor_position.y;
        let right = left + monitor_size.width as i32;
        let bottom = top + monitor_size.height as i32;
        let width = window_size.width as i32;
        let height = window_size.height as i32;
        let (x, y) = match position.as_str() {
            "bottom-left" => (left + margin, bottom - height - margin),
            "bottom-right" => (right - width - margin, bottom - height - margin),
            "top-center" => (left + (monitor_size.width as i32 - width) / 2, top + margin),
            _ => (
                left + (monitor_size.width as i32 - width) / 2,
                bottom - height - margin,
            ),
        };
        window.set_position(Position::Physical(PhysicalPosition::new(x, y)))?;
    }
    Ok(())
}

/// Toggle the enabled/disabled state of Flick.
#[tauri::command]
pub async fn toggle_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    config::update_config(&app, |cfg| {
        cfg.enabled = enabled;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

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
    let trigger = normalize_trigger(&trigger);
    let prompt = prompt.trim().to_string();
    let command = config::CustomCommand {
        id: format!("cmd-{}", trigger),
        trigger: trigger.clone(),
        prompt,
    };
    config::update_config(&app, |cfg| {
        validate_custom_command(cfg, &trigger, &command.prompt, None)
            .map_err(anyhow::Error::msg)?;
        cfg.custom_commands.push(command.clone());
        Ok(())
    })
    .map_err(|e| e.to_string())?;

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
    config::update_config(&app, |cfg| {
        let index = cfg
            .custom_commands
            .iter()
            .position(|command| command.id == id)
            .ok_or_else(|| anyhow::anyhow!("Unknown command"))?;
        let trigger = normalize_trigger(&trigger);
        let prompt = prompt.trim().to_string();
        validate_custom_command(cfg, &trigger, &prompt, Some(&id)).map_err(anyhow::Error::msg)?;
        let id = cfg.custom_commands[index].id.clone();
        cfg.custom_commands[index] = config::CustomCommand {
            id,
            trigger: trigger.clone(),
            prompt,
        };
        log::info!("Custom command updated: !{}", trigger);
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete a custom command by its stable ID.
#[tauri::command]
pub async fn delete_custom_command(app: AppHandle, id: String) -> Result<(), String> {
    config::update_config(&app, |cfg| {
        let index = cfg
            .custom_commands
            .iter()
            .position(|command| command.id == id)
            .ok_or_else(|| anyhow::anyhow!("Unknown command"))?;
        let removed = cfg.custom_commands.remove(index);
        log::info!("Custom command deleted: !{}", removed.trigger);
        Ok(())
    })
    .map_err(|e| e.to_string())?;
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
    let mut added = 0;
    config::update_config(&app, |config| {
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
            validate_custom_command(config, &trigger, &prompt, None).map_err(anyhow::Error::msg)?;
            config.custom_commands.push(config::CustomCommand {
                id: format!("cmd-{trigger}"),
                trigger,
                prompt,
            });
            added += 1;
        }
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    Ok(added)
}
