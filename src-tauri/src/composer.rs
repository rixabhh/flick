//! Privacy-first conversational reply drafting.
//!
//! The composer deliberately captures only an explicit selection. It stores no
//! context or drafts: the Svelte window owns that short-lived state.

use arboard::Clipboard;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

use crate::{ai_client, key_hook, keychain, replacer, AppState, ComposerTargetState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetIdentity {
    app_name: String,
    process_path: String,
}

fn foreground_target() -> Result<TargetIdentity, String> {
    let window = active_win_pos_rs::get_active_window()
        .map_err(|_| "Flick could not verify the active app".to_string())?;
    Ok(TargetIdentity {
        app_name: window.app_name.to_ascii_lowercase(),
        process_path: window.process_path.to_string_lossy().to_ascii_lowercase(),
    })
}

fn same_target(expected: &TargetIdentity, current: &TargetIdentity) -> bool {
    !expected.app_name.is_empty()
        && expected.app_name == current.app_name
        && (expected.process_path.is_empty()
            || current.process_path.is_empty()
            || expected.process_path == current.process_path)
}

fn remember_target(app: &AppHandle, target: Option<TargetIdentity>) {
    if let Some(state) = app.try_state::<ComposerTargetState>() {
        if let Ok(mut remembered) = state.target.lock() {
            *remembered = target;
        }
    }
}

fn target_is_still_appropriate(app: &AppHandle) -> Result<bool, String> {
    let expected = app
        .try_state::<ComposerTargetState>()
        .and_then(|state| state.target.lock().ok().and_then(|target| target.clone()))
        .ok_or_else(|| {
            "Flick could not verify the original target. Copy the draft instead.".to_string()
        })?;
    let current = foreground_target()?;
    Ok(same_target(&expected, &current))
}

fn current_target_is_protected(app: &AppHandle) -> bool {
    let disabled_apps = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .config
                .lock()
                .ok()
                .map(|config| config.disabled_apps.clone())
        })
        .unwrap_or_default();
    key_hook::active_app_is_protected(&disabled_apps)
}

/// Captures an explicit selection before the composer window is shown. Drafts
/// and context remain in the renderer only for the lifetime of that window.
pub async fn open_from_shortcut(app: &AppHandle) {
    // Capture identity before Flick's own window is visible. The selected text
    // itself remains renderer-only and is deliberately not stored here. This
    // check belongs here—not only in the global hook—because CLI and tray
    // actions can enter the composer without passing through that hook.
    let context = if current_target_is_protected(app) {
        remember_target(app, None);
        let _ = app.emit(
            "flick://error",
            serde_json::json!({"message": "Flick will not capture text from a protected app or password field."}),
        );
        String::new()
    } else {
        remember_target(app, foreground_target().ok());
        replacer::capture_selected_text().await.unwrap_or_default()
    };
    if let Some(window) = app.get_webview_window("composer") {
        let _ = app.emit("flick://composer-context", context);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub async fn capture_reply_context(app: AppHandle) -> Result<String, String> {
    if current_target_is_protected(&app) {
        return Err("Flick will not capture text from a protected app or password field.".into());
    }
    // This command can be invoked after the composer has already been open.
    // Bind insertion to the target that supplied this new explicit selection,
    // not an older target remembered when the window first appeared.
    remember_target(&app, foreground_target().ok());
    let selection = replacer::capture_selected_text()
        .await
        .map_err(|error| error.to_string())?;
    if selection.trim().is_empty() {
        return Err(
            "No selected text was found. Select a message first, or add context manually.".into(),
        );
    }
    Ok(selection)
}

#[tauri::command]
pub async fn generate_reply(
    app: AppHandle,
    context: String,
    tone: String,
    instruction: String,
) -> Result<String, String> {
    if context.trim().is_empty() {
        return Err("Add conversation context before generating a reply.".into());
    }
    if instruction.trim().is_empty() {
        return Err("Describe what you want to say before generating a reply.".into());
    }

    let (provider, model, custom_base_url) = app
        .try_state::<AppState>()
        .map(|state| {
            let config = state
                .config
                .lock()
                .map_err(|_| "Settings are temporarily unavailable")?;
            Ok::<_, String>((
                config.provider.clone(),
                config.model.clone(),
                config.custom_base_url.clone(),
            ))
        })
        .transpose()?
        .ok_or_else(|| "Flick is still starting. Please try again.".to_string())?;

    let api_key = if provider == "custom" {
        // Local OpenAI-compatible servers commonly do not need credentials.
        keychain::load_api_key(&provider).unwrap_or_default()
    } else {
        keychain::load_api_key(&provider).map_err(|error| error.to_string())?
    };
    let prompt = ai_client::get_reply_prompt(&context, &tone, &instruction);
    let response = if provider == "custom" {
        if custom_base_url.trim().is_empty() {
            return Err("Add a base URL for the OpenAI-compatible provider in Settings.".into());
        }
        ai_client::transform_openai_compatible(&api_key, &custom_base_url, &model, &prompt).await
    } else {
        ai_client::transform_text(&api_key, &provider, &model, &prompt).await
    };
    response.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn insert_reply(app: AppHandle, draft: String) -> Result<(), String> {
    if draft.trim().is_empty() {
        return Err("There is no draft to insert.".into());
    }
    let append_space = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .config
                .lock()
                .ok()
                .map(|config| config.append_trailing_space)
        })
        .unwrap_or(false);
    let text = if append_space {
        format!("{} ", draft.trim())
    } else {
        draft
    };
    // Release the composer before pasting. On supported desktop window
    // managers, hiding restores the previously active target; pasting while
    // the composer owns focus would incorrectly insert into Flick itself.
    if let Some(window) = app.get_webview_window("composer") {
        let _ = window.hide();
    }
    sleep(Duration::from_millis(120)).await;
    if current_target_is_protected(&app) {
        restore_composer(&app);
        return Err("Flick will not insert into a protected app. Copy the draft instead.".into());
    }
    match target_is_still_appropriate(&app) {
        Ok(true) => {}
        Ok(false) => {
            restore_composer(&app);
            return Err("The original app is no longer active. Copy the draft instead.".into());
        }
        Err(error) => {
            restore_composer(&app);
            return Err(error);
        }
    }
    if let Err(error) = replacer::paste_text_transaction(&text).await {
        restore_composer(&app);
        return Err(format!("Could not insert the draft: {error}"));
    }
    Ok(())
}

fn restore_composer(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("composer") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_match_requires_the_same_foreground_application() {
        let expected = TargetIdentity {
            app_name: "slack".into(),
            process_path: "c:/apps/slack.exe".into(),
        };
        assert!(same_target(&expected, &expected));
        assert!(!same_target(
            &expected,
            &TargetIdentity {
                app_name: "discord".into(),
                process_path: "c:/apps/discord.exe".into(),
            }
        ));
    }
}

#[tauri::command]
pub async fn copy_reply(draft: String) -> Result<(), String> {
    if draft.trim().is_empty() {
        return Err("There is no draft to copy.".into());
    }
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(draft))
        .map_err(|error| format!("Could not copy draft: {error}"))
}
