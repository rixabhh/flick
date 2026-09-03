// Flick - replacer.rs
// Per PRD §8.3: Text replacement flow using clipboard strategy.
// The 12-step pipeline: save clipboard, select-all, copy, strip trigger,
// call AI, paste result, restore clipboard.

use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;

use crate::ai_client;

const KEY_SETTLE_DELAY: Duration = Duration::from_millis(25);
const CLIPBOARD_COPY_DELAY: Duration = Duration::from_millis(60);
const CLIPBOARD_RESTORE_DELAY: Duration = Duration::from_millis(50);
const PASTE_DELAY: Duration = Duration::from_millis(35);

#[cfg(target_os = "macos")]
fn platform_modifier() -> Key {
    Key::Meta
}

#[cfg(not(target_os = "macos"))]
fn platform_modifier() -> Key {
    Key::Control
}

/// Execute the full text replacement pipeline - per §8.3.
#[allow(clippy::too_many_arguments)] // Existing public trigger pipeline; refactor follows its v2 command boundary.
pub async fn execute_replacement(
    app: &AppHandle,
    api_key: &str,
    provider: &str,
    model: &str,
    custom_base_url: &str,
    command: &str,
    param: Option<&str>,
    trigger: &str,
    show_done_toast: bool,
) -> Result<()> {
    let started_at = Instant::now();

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
    let clipboard_ms = started_at.elapsed().as_millis();

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
            let _ = app.emit(
                "flick://error",
                serde_json::json!({"message": "Unknown command"}),
            );
            restore_clipboard(&original_clipboard);
            bail!("Unknown command: {}", command);
        }
    };

    let transformed =
        match transform_with_provider(api_key, provider, model, custom_base_url, &prompt).await {
            Ok(text) => text,
            Err(e) => {
                // Per §8.3: If API call fails, restore clipboard and show error toast
                let _ = app.emit(
                    "flick://error",
                    serde_json::json!({"message": format!("API error: {}", e)}),
                );
                restore_clipboard(&original_clipboard);
                bail!("API transform failed: {}", e);
            }
        };
    let ai_ms = started_at
        .elapsed()
        .as_millis()
        .saturating_sub(clipboard_ms);

    // Step 8: Set transformed text as clipboard content
    {
        let mut cb = match Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(error) => {
                restore_clipboard(&original_clipboard);
                return Err(error).context("Failed to access clipboard for transformed text");
            }
        };
        if let Err(error) = cb.set_text(transformed) {
            restore_clipboard(&original_clipboard);
            return Err(error).context("Failed to set transformed text to clipboard");
        }
    }

    // Step 9: Simulate Ctrl+V / Cmd+V → paste transformed text
    if let Err(e) = simulate_paste().await {
        restore_clipboard(&original_clipboard);
        let _ = app.emit(
            "flick://error",
            serde_json::json!({"message": "Failed to paste"}),
        );
        bail!("Failed to paste: {}", e);
    }

    // Step 10: Wait briefly so the paste completes before restoring clipboard.
    sleep(CLIPBOARD_RESTORE_DELAY).await;

    // Step 11: Restore original clipboard content
    restore_clipboard(&original_clipboard);

    // Step 12: Dismiss toast / show done
    if show_done_toast {
        let _ = app.emit("flick://done", ());
    }

    log::info!(
        "Replacement completed in {}ms (clipboard: {}ms, ai: {}ms)",
        started_at.elapsed().as_millis(),
        clipboard_ms,
        ai_ms
    );

    Ok(())
}

/// Execute replacement with a custom instruction prompt.
#[allow(clippy::too_many_arguments)] // Public trigger boundary keeps provider configuration explicit.
pub async fn execute_custom_replacement(
    app: &AppHandle,
    api_key: &str,
    provider: &str,
    model: &str,
    custom_base_url: &str,
    system_prompt: &str,
    trigger: &str,
    show_done_toast: bool,
) -> Result<()> {
    let started_at = Instant::now();

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
    let clipboard_ms = started_at.elapsed().as_millis();

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

    // Step 6-7: Build the custom prompt and call API.
    let prompt = ai_client::get_custom_prompt(system_prompt, &clean_text);

    let transformed =
        match transform_with_provider(api_key, provider, model, custom_base_url, &prompt).await {
            Ok(text) => text,
            Err(e) => {
                let _ = app.emit(
                    "flick://error",
                    serde_json::json!({"message": format!("API error: {}", e)}),
                );
                restore_clipboard(&original_clipboard);
                bail!("API transform failed: {}", e);
            }
        };
    let ai_ms = started_at
        .elapsed()
        .as_millis()
        .saturating_sub(clipboard_ms);

    // Step 8: Set clipboard
    {
        let mut cb = match Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(error) => {
                restore_clipboard(&original_clipboard);
                return Err(error).context("Failed to access clipboard for transformed text");
            }
        };
        if let Err(error) = cb.set_text(transformed) {
            restore_clipboard(&original_clipboard);
            return Err(error).context("Failed to set transformed text to clipboard");
        }
    }

    // Step 9: Paste
    if let Err(e) = simulate_paste().await {
        restore_clipboard(&original_clipboard);
        let _ = app.emit(
            "flick://error",
            serde_json::json!({"message": "Failed to paste"}),
        );
        bail!("Failed to paste: {}", e);
    }

    // Step 10: Wait briefly so the paste completes before restoring clipboard.
    sleep(CLIPBOARD_RESTORE_DELAY).await;

    // Step 11: Restore clipboard
    restore_clipboard(&original_clipboard);

    // Step 12: Done
    if show_done_toast {
        let _ = app.emit("flick://done", ());
    }

    log::info!(
        "Custom replacement completed in {}ms (clipboard: {}ms, ai: {}ms)",
        started_at.elapsed().as_millis(),
        clipboard_ms,
        ai_ms
    );

    Ok(())
}

async fn transform_with_provider(
    api_key: &str,
    provider: &str,
    model: &str,
    custom_base_url: &str,
    prompt: &str,
) -> Result<String> {
    if provider == "custom" {
        if custom_base_url.trim().is_empty() {
            bail!("Add a base URL for the OpenAI-compatible provider in Settings");
        }
        ai_client::transform_openai_compatible(api_key, custom_base_url, model, prompt).await
    } else {
        ai_client::transform_text(api_key, provider, model, prompt).await
    }
}

/// Copy the currently selected text without altering the active selection.
pub async fn capture_selected_text() -> Result<String> {
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    let original_clipboard = clipboard.get_text().unwrap_or_default();
    // Composer privacy contract: capture an explicit selection only. Unlike
    // the rewrite pipeline, do not select the entire field here.
    let selected = copy_existing_selection().await;
    if let Ok(mut restore) = Clipboard::new() {
        let _ = restore.set_text(original_clipboard);
    }
    selected
}

async fn copy_existing_selection() -> Result<String> {
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    // A sentinel lets us distinguish “copy did not change the clipboard” from
    // a valid selection that happens to match what was copied previously.
    let sentinel = format!("__flick_selection_probe_{}__", std::process::id());
    clipboard
        .set_text(sentinel.clone())
        .context("Failed to prepare clipboard")?;
    simulate_copy().await?;
    sleep(CLIPBOARD_COPY_DELAY).await;
    let copied = Clipboard::new()
        .context("Failed to read copied selection")?
        .get_text()
        .context("The selected content is not text")?;
    if copied == sentinel || copied.trim().is_empty() {
        bail!("No text selection was copied");
    }
    Ok(copied)
}

/// Place text in the previously active target and restore the user's clipboard.
/// Callers must ask for explicit user confirmation before invoking this.
pub async fn paste_text_transaction(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    let original_clipboard = clipboard.get_text().unwrap_or_default();
    clipboard
        .set_text(text.to_string())
        .context("Failed to set clipboard text")?;
    let paste_result = simulate_paste().await;
    sleep(CLIPBOARD_RESTORE_DELAY).await;
    if let Ok(mut restore) = Clipboard::new() {
        let _ = restore.set_text(original_clipboard);
    }
    paste_result
}

/// Re-paste the text representation currently on the clipboard. This is a
/// deliberate formatting conversion: rich clipboard formats are not pasted.
pub async fn paste_plain_text_from_clipboard() -> Result<()> {
    let text = Clipboard::new()
        .context("Failed to access clipboard")?
        .get_text()
        .context("Clipboard does not contain text to paste")?;
    if text.is_empty() {
        bail!("Clipboard does not contain text to paste");
    }
    paste_text_transaction(&text).await
}

/// Submit the focused target only after an explicit per-app opt-in.
pub async fn submit_current_target() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| anyhow::anyhow!("Failed to create keyboard input: {error:?}"))?;
    enigo
        .key(Key::Return, Direction::Click)
        .map_err(|error| anyhow::anyhow!("Failed to submit target: {error:?}"))?;
    sleep(PASTE_DELAY).await;
    Ok(())
}

async fn select_and_copy() -> Result<String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Failed to create Enigo instance: {:?}", e))?;

    // Small delay to ensure previous key events are processed
    sleep(KEY_SETTLE_DELAY).await;

    // Ctrl+A - select all text in the active input field
    enigo
        .key(platform_modifier(), Direction::Press)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo
        .key(Key::Unicode('a'), Direction::Click)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo
        .key(platform_modifier(), Direction::Release)
        .map_err(|e| anyhow::anyhow!("Key release failed: {:?}", e))?;

    sleep(KEY_SETTLE_DELAY).await;

    // Ctrl+C - copy selected text
    enigo
        .key(platform_modifier(), Direction::Press)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo
        .key(Key::Unicode('c'), Direction::Click)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo
        .key(platform_modifier(), Direction::Release)
        .map_err(|e| anyhow::anyhow!("Key release failed: {:?}", e))?;

    // Wait for clipboard to update
    sleep(CLIPBOARD_COPY_DELAY).await;

    // Read clipboard content
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    let text = clipboard.get_text().unwrap_or_default();
    Ok(text)
}

/// Simulate Ctrl+V (paste).
async fn simulate_paste() -> Result<()> {
    #[cfg(target_os = "linux")]
    if try_linux_key_chord("v") {
        sleep(PASTE_DELAY).await;
        return Ok(());
    }
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Failed to create Enigo instance: {:?}", e))?;

    enigo
        .key(platform_modifier(), Direction::Press)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| anyhow::anyhow!("Key press failed: {:?}", e))?;
    enigo
        .key(platform_modifier(), Direction::Release)
        .map_err(|e| anyhow::anyhow!("Key release failed: {:?}", e))?;

    sleep(PASTE_DELAY).await;
    Ok(())
}

async fn simulate_copy() -> Result<()> {
    #[cfg(target_os = "linux")]
    if try_linux_key_chord("c") {
        sleep(KEY_SETTLE_DELAY).await;
        return Ok(());
    }
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| anyhow::anyhow!("Failed to create keyboard input: {error:?}"))?;
    enigo
        .key(platform_modifier(), Direction::Press)
        .map_err(|error| anyhow::anyhow!("Failed to hold copy modifier: {error:?}"))?;
    enigo
        .key(Key::Unicode('c'), Direction::Click)
        .map_err(|error| anyhow::anyhow!("Failed to copy selection: {error:?}"))?;
    enigo
        .key(platform_modifier(), Direction::Release)
        .map_err(|error| anyhow::anyhow!("Failed to release copy modifier: {error:?}"))?;
    sleep(KEY_SETTLE_DELAY).await;
    Ok(())
}

/// Prefer the standard display-server helpers on Linux when they are
/// installed. They are more reliable than synthetic input under restrictive
/// Wayland compositors. A failed/missing helper is deliberately non-fatal: the
/// cross-platform input backend remains the fallback.
#[cfg(target_os = "linux")]
fn try_linux_key_chord(key: &str) -> bool {
    use std::process::Command;

    let chord = format!("ctrl+{key}");
    let run = |program: &str, args: &[&str]| {
        Command::new(program)
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if run("wtype", &["-M", "ctrl", "-P", key, "-m", "ctrl"]) || run("dotool", &["key", &chord])
        {
            return true;
        }
    }
    if std::env::var_os("DISPLAY").is_some() && run("xdotool", &["key", "--clearmodifiers", &chord])
    {
        return true;
    }
    false
}
