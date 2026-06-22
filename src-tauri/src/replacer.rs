// Flick - replacer.rs
// Per PRD §8.3: Text replacement flow using clipboard strategy.
// The 12-step pipeline: save clipboard, select-all, copy, strip trigger,
// call AI, paste result, restore clipboard.

use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;

use crate::ai_client;

/// Execute the full text replacement pipeline - per §8.3.
pub async fn execute_replacement(
    app: &AppHandle,
    api_key: &str,
    provider: &str,
    model: &str,
    command: &str,
    param: Option<&str>,
    trigger: &str,
    show_done_toast: bool,
) -> Result<()> {
    // Step 1: Save current clipboard content
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    let original_clipboard = clipboard.get_text().unwrap_or_default();

    // Helper closure to restore clipboard on failure
    let restore_clipboard = |text: &str| {
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(text.to_string());
        }
    };

    // Step 2-3: Select all text and copy it
    // We use Ctrl+A to select all in the active field, then Ctrl+C to copy
    let selected_text = match select_and_copy().await {
        Ok(text) => text,
        Err(e) => {
            restore_clipboard(&original_clipboard);
            bail!("Failed to select and copy text: {}", e);
        }
    };

    // Abort if clipboard is empty after copy - per §8.3 failure handling
    if selected_text.trim().is_empty() {
        restore_clipboard(&original_clipboard);
        bail!("No text found to transform");
    }

    // Step 4: Strip the trigger word from the end of the text
    let clean_text = selected_text
        .trim_end()
        .strip_suffix(trigger)
        .unwrap_or(&selected_text)
        .trim()
        .to_string();

    if clean_text.is_empty() {
        restore_clipboard(&original_clipboard);
        bail!("No text found after stripping trigger");
    }

    // Step 5: Emit "transforming" event to UI → show floating toast
    let _ = app.emit("flick://transforming", ());

    // Step 6-7: Build prompt and call Gemini Flash API
    // Check for custom command prompt first, then built-in
    let prompt = match ai_client::get_prompt(command, param, &clean_text) {
        Some(p) => p,
        None => {
            // This must be a custom command - the prompt will be resolved by the caller
            // For now, bail if we can't find a prompt
            let _ = app.emit("flick://error", serde_json::json!({"message": "Unknown command"}));
            restore_clipboard(&original_clipboard);
            bail!("Unknown command: {}", command);
        }
    };

    let transformed = match ai_client::transform_text(api_key, provider, model, &prompt).await {
        Ok(text) => text,
        Err(e) => {
            // Per §8.3: If API call fails, restore clipboard and show error toast
            let _ = app.emit("flick://error", serde_json::json!({"message": format!("API error: {}", e)}));
            restore_clipboard(&original_clipboard);
            bail!("API transform failed: {}", e);
        }
    };

    // Step 8: Set transformed text as clipboard content
    {
        let mut cb = Clipboard::new().context("Failed to access clipboard")?;
        cb.set_text(transformed)
            .context("Failed to set transformed text to clipboard")?;
    }

    // Step 9: Simulate Ctrl+V / Cmd+V → paste transformed text
    if let Err(e) = simulate_paste().await {
        restore_clipboard(&original_clipboard);
        let _ = app.emit("flick://error", serde_json::json!({"message": "Failed to paste"}));
        bail!("Failed to paste: {}", e);
    }

    // Step 10: Wait 100ms - per §8.3
    sleep(Duration::from_millis(100)).await;

    // Step 11: Restore original clipboard content
    restore_clipboard(&original_clipboard);

    // Step 12: Dismiss toast / show done
    if show_done_toast {
        let _ = app.emit("flick://done", ());
    }

    Ok(())
}

/// Execute replacement with a custom prompt template.
pub async fn execute_custom_replacement(
    app: &AppHandle,
    api_key: &str,
    provider: &str,
    model: &str,
    prompt_template: &str,
    trigger: &str,
    show_done_toast: bool,
) -> Result<()> {
    // Step 1: Save current clipboard
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    let original_clipboard = clipboard.get_text().unwrap_or_default();

    let restore_clipboard = |text: &str| {
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(text.to_string());
        }
    };

    // Step 2-3: Select and copy
    let selected_text = match select_and_copy().await {
        Ok(text) => text,
        Err(e) => {
            restore_clipboard(&original_clipboard);
            bail!("Failed to select and copy text: {}", e);
        }
    };

    if selected_text.trim().is_empty() {
        restore_clipboard(&original_clipboard);
        bail!("No text found to transform");
    }

    // Step 4: Strip trigger
    let clean_text = selected_text
        .trim_end()
        .strip_suffix(trigger)
        .unwrap_or(&selected_text)
        .trim()
        .to_string();

    if clean_text.is_empty() {
        restore_clipboard(&original_clipboard);
        bail!("No text found after stripping trigger");
    }

    // Step 5: Emit transforming
    let _ = app.emit("flick://transforming", ());

    // Step 6-7: Substitute {{text}} in prompt template and call API
    let prompt = prompt_template.replace("{{text}}", &clean_text);

    let transformed = match ai_client::transform_text(api_key, provider, model, &prompt).await {
        Ok(text) => text,
        Err(e) => {
            let _ = app.emit("flick://error", serde_json::json!({"message": format!("API error: {}", e)}));
            restore_clipboard(&original_clipboard);
            bail!("API transform failed: {}", e);
        }
    };

    // Step 8: Set clipboard
    {
        let mut cb = Clipboard::new().context("Failed to access clipboard")?;
        cb.set_text(transformed)
            .context("Failed to set transformed text to clipboard")?;
    }

    // Step 9: Paste
    if let Err(e) = simulate_paste().await {
        restore_clipboard(&original_clipboard);
        let _ = app.emit("flick://error", serde_json::json!({"message": "Failed to paste"}));
        bail!("Failed to paste: {}", e);
    }

    // Step 10: Wait
    sleep(Duration::from_millis(100)).await;

    // Step 11: Restore clipboard
    restore_clipboard(&original_clipboard);

    // Step 12: Done
    if show_done_toast {
        let _ = app.emit("flick://done", ());
    }

    Ok(())
}

/// Simulate Ctrl+A (select all) then Ctrl+C (copy), read clipboard.
async fn select_and_copy() -> Result<String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Failed to create Enigo instance: {:?}", e))?;

    // Small delay to ensure previous key events are processed
    sleep(Duration::from_millis(50)).await;

    // Ctrl+A - select all text in the active input field
    enigo.key(Key::Control, Direction::Press)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo.key(Key::Unicode('a'), Direction::Click)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo.key(Key::Control, Direction::Release)
        .map_err(|e| anyhow::anyhow!("Key release failed: {:?}", e))?;

    sleep(Duration::from_millis(50)).await;

    // Ctrl+C - copy selected text
    enigo.key(Key::Control, Direction::Press)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo.key(Key::Unicode('c'), Direction::Click)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo.key(Key::Control, Direction::Release)
        .map_err(|e| anyhow::anyhow!("Key release failed: {:?}", e))?;

    // Wait for clipboard to update
    sleep(Duration::from_millis(100)).await;

    // Read clipboard content
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    let text = clipboard.get_text().unwrap_or_default();
    Ok(text)
}

/// Simulate Ctrl+V (paste).
async fn simulate_paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Failed to create Enigo instance: {:?}", e))?;

    enigo.key(Key::Control, Direction::Press)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo.key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo.key(Key::Control, Direction::Release)
        .map_err(|e| anyhow::anyhow!("Key release failed: {:?}", e))?;

    sleep(Duration::from_millis(50)).await;
    Ok(())
}
