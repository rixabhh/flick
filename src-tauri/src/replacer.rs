//! Clipboard-assisted capture and guarded insertion.
//!
//! Clipboard ownership is limited to the short copy/paste operation. Network
//! requests never retain a snapshot that could later overwrite a newer copy.

use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::sleep;

use crate::{active_target, ai_client};

const KEY_SETTLE_DELAY: Duration = Duration::from_millis(25);
const CLIPBOARD_COPY_DELAY: Duration = Duration::from_millis(30);
const CLIPBOARD_COPY_TIMEOUT: Duration = Duration::from_millis(750);
const CLIPBOARD_RESTORE_DELAY: Duration = Duration::from_millis(100);
const PASTE_DELAY: Duration = Duration::from_millis(35);

// One persistent handle also keeps restored/copied data available on Linux,
// where dropping the last arboard handle relinquishes clipboard ownership.
static CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::const_new(None);
static COPY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn clipboard_access() -> Result<MutexGuard<'static, Option<Clipboard>>> {
    let mut state = CLIPBOARD.try_lock().map_err(|_| {
        anyhow::anyhow!("Flick is finishing another copy or paste. Please try again in a moment.")
    })?;
    if state.is_none() {
        *state = Some(Clipboard::new().context("Failed to access clipboard")?);
    }
    Ok(state)
}

/// Explicit Copy actions share the same lock as temporary clipboard use.
/// Otherwise a delayed paste restoration could erase a freshly copied draft.
pub fn copy_text_to_clipboard(text: &str) -> Result<()> {
    clipboard_access()?
        .as_mut()
        .context("Clipboard is unavailable")?
        .set_text(text.to_owned())
        .context("Could not write the clipboard")
}

enum ClipboardSnapshot {
    Empty,
    Text(String),
    Image(arboard::ImageData<'static>),
}

fn snapshot_clipboard(clipboard: &mut Clipboard) -> Result<ClipboardSnapshot> {
    match clipboard.get_text() {
        Ok(text) => return Ok(ClipboardSnapshot::Text(text)),
        Err(arboard::Error::ContentNotAvailable) => {}
        Err(error) => return Err(error).context("Could not preserve the clipboard"),
    }
    match clipboard.get_image() {
        Ok(image) => return Ok(ClipboardSnapshot::Image(image)),
        Err(arboard::Error::ContentNotAvailable) => {}
        Err(error) => return Err(error).context("Could not preserve the clipboard image"),
    }
    // arboard deliberately uses the same error for an empty clipboard and an
    // unsupported format. Never treat copied files as an empty clipboard.
    if clipboard_is_empty() {
        return Ok(ClipboardSnapshot::Empty);
    }
    bail!("Flick could not safely preserve the current clipboard contents. Copy it as text or an image first, then try again.")
}

#[cfg(target_os = "windows")]
fn clipboard_is_empty() -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn CountClipboardFormats() -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn SetLastError(code: u32);
        fn GetLastError() -> u32;
    }
    // CountClipboardFormats also returns zero on failure. Only a successful
    // empty result is safe to clear when the temporary operation finishes.
    unsafe {
        SetLastError(0);
        CountClipboardFormats() == 0 && GetLastError() == 0
    }
}

#[cfg(target_os = "macos")]
fn clipboard_is_empty() -> bool {
    std::process::Command::new("osascript")
        .args(["-l", "JavaScript", "-e",
            "ObjC.import('AppKit'); Number($.NSPasteboard.generalPasteboard.pasteboardItems.count);"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "0")
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn clipboard_is_empty() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn clipboard_revision() -> Option<u64> {
    #[link(name = "user32")]
    extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
    }
    let revision = unsafe { GetClipboardSequenceNumber() };
    (revision != 0).then_some(u64::from(revision))
}

#[cfg(not(target_os = "windows"))]
fn clipboard_revision() -> Option<u64> {
    None
}

fn still_owns_clipboard(
    expected_text: &str,
    current_text: Option<&str>,
    expected_revision: Option<u64>,
    current_revision: Option<u64>,
) -> bool {
    current_text == Some(expected_text)
        && match expected_revision {
            Some(expected) => current_revision == Some(expected),
            None => true,
        }
}

struct ClipboardTransaction {
    state: MutexGuard<'static, Option<Clipboard>>,
    original: ClipboardSnapshot,
    owned_text: Option<String>,
    revision: Option<u64>,
}

impl ClipboardTransaction {
    fn begin() -> Result<Self> {
        let mut state = clipboard_access()?;
        let original = snapshot_clipboard(state.as_mut().context("Clipboard is unavailable")?)?;
        Ok(Self {
            state,
            original,
            owned_text: None,
            revision: None,
        })
    }

    fn set_text(&mut self, text: String) -> Result<()> {
        self.state
            .as_mut()
            .context("Clipboard is unavailable")?
            .set_text(text.clone())
            .context("Could not prepare the clipboard")?;
        self.claim_text(text);
        Ok(())
    }

    fn claim_text(&mut self, text: String) {
        self.owned_text = Some(text);
        self.revision = clipboard_revision();
    }

    fn get_text(&mut self) -> Result<String, arboard::Error> {
        match self.state.as_mut() {
            Some(clipboard) => clipboard.get_text(),
            None => Err(arboard::Error::ClipboardNotSupported),
        }
    }

    fn restore(&mut self) -> Result<()> {
        let Some(expected) = self.owned_text.as_deref() else {
            return Ok(());
        };
        let clipboard = self.state.as_mut().context("Clipboard is unavailable")?;
        let current = match clipboard.get_text() {
            Ok(text) => Some(text),
            Err(arboard::Error::ContentNotAvailable) => None,
            Err(error) => {
                return Err(error).context("Could not check the clipboard before restoring it")
            }
        };
        if still_owns_clipboard(
            expected,
            current.as_deref(),
            self.revision,
            clipboard_revision(),
        ) {
            match &self.original {
                ClipboardSnapshot::Empty => clipboard.clear(),
                ClipboardSnapshot::Text(text) => clipboard.set_text(text.clone()),
                ClipboardSnapshot::Image(image) => clipboard.set_image(image.clone()),
            }
            .context("Could not restore the original clipboard contents")?;
        }
        self.owned_text = None;
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        self.restore()
    }
}

impl Drop for ClipboardTransaction {
    fn drop(&mut self) {
        // Errors and cancelled futures take the same restoration path. A user
        // copy made since our last write is left intact.
        if let Err(error) = self.restore() {
            log::warn!("{error}");
        }
    }
}

fn verify_original_target(expected: &active_target::ActiveTarget) -> Result<()> {
    let current = active_target::get()
        .context("Flick could not verify the active app before replacing text")?;
    if active_target::matches_target(expected, &current) {
        Ok(())
    } else {
        bail!("The original app or window is no longer active. Flick did not paste into the new target; use Copy last result to recover the completed text.")
    }
}

fn active_target_is_protected(app: &AppHandle) -> bool {
    app.try_state::<crate::AppState>()
        .and_then(|state| {
            state
                .config
                .lock()
                .ok()
                .map(|config| config.disabled_apps.clone())
        })
        .map(|disabled| crate::key_hook::active_app_is_protected(&disabled))
        .unwrap_or(true)
}

#[cfg(target_os = "macos")]
fn platform_modifier() -> Key {
    Key::Meta
}

#[cfg(not(target_os = "macos"))]
fn platform_modifier() -> Key {
    Key::Control
}

/// Execute a built-in rewrite with a fresh capture, then a guarded insertion.
#[allow(clippy::too_many_arguments)]
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
    execute_transform(
        app,
        api_key,
        provider,
        model,
        custom_base_url,
        trigger,
        show_done_toast,
        |text| {
            ai_client::get_prompt(command, param, text)
                .with_context(|| format!("Unknown command: {command}"))
        },
    )
    .await
}

/// Execute a rewrite with a user-defined instruction.
#[allow(clippy::too_many_arguments)]
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
    execute_transform(
        app,
        api_key,
        provider,
        model,
        custom_base_url,
        trigger,
        show_done_toast,
        |text| Ok(ai_client::get_custom_prompt(system_prompt, text)),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn execute_transform(
    app: &AppHandle,
    api_key: &str,
    provider: &str,
    model: &str,
    custom_base_url: &str,
    trigger: &str,
    show_done_toast: bool,
    build_prompt: impl FnOnce(&str) -> Result<String>,
) -> Result<()> {
    let started_at = Instant::now();
    if active_target_is_protected(app) {
        bail!("Flick will not transform text in a protected app or password field")
    }
    let original_target = active_target::get()
        .context("Flick could not verify the active app before transforming text")?;
    let selected_text = capture_text(&original_target, true).await?;
    // capture_text restores the clipboard before the request starts, so a
    // slow provider cannot overwrite something copied meanwhile.
    let clean_text = transformation_source(&selected_text, trigger)?;
    let prompt = build_prompt(clean_text)?;
    let _ = app.emit("flick://transforming", ());
    let transformed =
        transform_with_provider(api_key, provider, model, custom_base_url, &prompt).await?;
    crate::history::remember_result(app, &transformed);
    if active_target_is_protected(app) {
        bail!("Flick will not paste into a protected app or password field. Use Copy last result to recover the completed text.")
    }
    verify_original_target(&original_target)?;
    let current_selection = capture_text(&original_target, false).await
        .context("The text selection changed while the result was being prepared. Use Copy last result to recover the completed text.")?;
    if current_selection != selected_text {
        bail!("The selected text changed while Flick was working. Nothing was replaced; use Copy last result to recover the completed text.");
    }
    paste_text_in_target(&transformed, &original_target).await?;
    let _ = crate::history::record(app, "transform", &transformed);
    let _ = app.emit(
        if show_done_toast {
            "flick://done"
        } else {
            "flick://transform-finished"
        },
        (),
    );
    log::info!(
        "Replacement completed in {}ms",
        started_at.elapsed().as_millis()
    );
    Ok(())
}

fn transformation_source<'a>(selected: &'a str, trigger: &str) -> Result<&'a str> {
    // A failed copy or a changed field must never send unrelated clipboard
    // contents to the provider just because the trigger fired earlier.
    let text = selected.trim_end().strip_suffix(trigger)
        .context("The trigger was not found in the focused text. Flick did not send or replace anything.")?
        .trim();
    if text.is_empty() {
        bail!("No text found after stripping trigger");
    }
    Ok(text)
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

/// Capture only the user's explicit selection, without selecting the field.
pub async fn capture_selected_text() -> Result<String> {
    let target = active_target::get().context("Flick could not verify the source app")?;
    capture_text(&target, false).await
}

async fn capture_text(target: &active_target::ActiveTarget, select_all: bool) -> Result<String> {
    let mut transaction = ClipboardTransaction::begin()?;
    let sentinel = format!(
        "__flick_selection_probe_{}_{}__",
        std::process::id(),
        COPY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    transaction.set_text(sentinel.clone())?;
    sleep(KEY_SETTLE_DELAY).await;
    verify_original_target(target)?;
    if select_all {
        simulate_key_chord('a')?;
        sleep(KEY_SETTLE_DELAY).await;
        verify_original_target(target)?;
    }
    simulate_key_chord('c')?;
    let started = Instant::now();
    loop {
        sleep(CLIPBOARD_COPY_DELAY).await;
        match transaction.get_text() {
            Ok(text) if text != sentinel => {
                verify_original_target(target)?;
                transaction.claim_text(text.clone());
                transaction.finish()?;
                if text.trim().is_empty() {
                    bail!("No text selection was copied");
                }
                return Ok(text);
            }
            Ok(_)
            | Err(arboard::Error::ClipboardOccupied)
            | Err(arboard::Error::ContentNotAvailable) => {}
            Err(error) => return Err(error).context("Could not read the selected text"),
        }
        if started.elapsed() >= CLIPBOARD_COPY_TIMEOUT {
            bail!("No text selection was copied. Select editable text and try again.");
        }
    }
}

/// Paste into the current target, detecting a focus change during preparation.
pub async fn paste_text_transaction(text: &str) -> Result<()> {
    let target = active_target::get().context("Flick could not verify the target app")?;
    paste_text_in_target(text, &target).await
}

async fn paste_text_in_target(text: &str, target: &active_target::ActiveTarget) -> Result<()> {
    let mut transaction = ClipboardTransaction::begin()?;
    transaction.set_text(text.to_owned())?;
    verify_original_target(target)?;
    simulate_key_chord('v')?;
    sleep(CLIPBOARD_RESTORE_DELAY).await;
    transaction.finish().context(
        "Text was pasted, but Flick could not restore the clipboard. Do not insert the text again.",
    )
}

/// Paste the plain text representation with the same transaction guarantees.
pub async fn paste_plain_text_from_clipboard() -> Result<()> {
    let target = active_target::get().context("Flick could not verify the target app")?;
    let mut transaction = ClipboardTransaction::begin()?;
    let text = transaction
        .get_text()
        .context("Clipboard does not contain text to paste")?;
    if text.is_empty() {
        bail!("Clipboard does not contain text to paste");
    }
    transaction.set_text(text)?;
    verify_original_target(&target)?;
    simulate_key_chord('v')?;
    sleep(CLIPBOARD_RESTORE_DELAY).await;
    transaction.finish().context(
        "Text was pasted, but Flick could not restore the clipboard. Do not paste the text again.",
    )
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

fn simulate_key_chord(key: char) -> Result<()> {
    #[cfg(target_os = "linux")]
    if try_linux_key_chord(&key.to_string()) {
        return Ok(());
    }
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| anyhow::anyhow!("Failed to create keyboard input: {error:?}"))?;
    enigo
        .key(platform_modifier(), Direction::Press)
        .map_err(|error| anyhow::anyhow!("Failed to hold shortcut modifier: {error:?}"))?;
    // Release the modifier even if injecting the character fails.
    let pressed = enigo.key(Key::Unicode(key), Direction::Click);
    let released = enigo.key(platform_modifier(), Direction::Release);
    pressed.map_err(|error| anyhow::anyhow!("Failed to send shortcut: {error:?}"))?;
    released.map_err(|error| anyhow::anyhow!("Failed to release shortcut modifier: {error:?}"))?;
    Ok(())
}

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
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && run("wtype", &["-M", "ctrl", "-P", key, "-p", key, "-m", "ctrl"])
    {
        return true;
    }
    std::env::var_os("DISPLAY").is_some() && run("xdotool", &["key", "--clearmodifiers", &chord])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_clipboard_is_never_transformed() {
        assert!(transformation_source("private clipboard text", "!fix").is_err());
        assert_eq!(
            transformation_source("rewrite this !fix ", "!fix").unwrap(),
            "rewrite this"
        );
        assert!(transformation_source("!fix", "!fix").is_err());
    }

    #[test]
    fn restoration_preserves_new_user_copies_including_identical_text() {
        assert!(still_owns_clipboard(
            "temporary",
            Some("temporary"),
            Some(10),
            Some(10)
        ));
        assert!(!still_owns_clipboard(
            "temporary",
            Some("new copy"),
            Some(10),
            Some(11)
        ));
        assert!(!still_owns_clipboard(
            "temporary",
            Some("temporary"),
            Some(10),
            Some(11)
        ));
        assert!(!still_owns_clipboard("temporary", None, None, None));
    }

    #[test]
    fn concurrent_clipboard_transactions_refuse_to_interleave() {
        // Hold the production lock without initializing a native handle.
        // Both transaction setup and explicit Copy must refuse before touching
        // the system clipboard.
        let _first = CLIPBOARD.try_lock().unwrap();
        assert!(clipboard_access().is_err());
        assert!(copy_text_to_clipboard("must not be copied").is_err());
    }
}
