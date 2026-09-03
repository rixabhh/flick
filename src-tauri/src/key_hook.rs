// Flick - key_hook.rs
// Per PRD §8.1: Global keyboard listener via rdev in a dedicated background thread.
// Also listens for mouse clicks to reset buffer (Open Question #1: Yes).

use rdev::{listen, Event, EventType, Key};
use std::collections::HashSet;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const HOLD_OR_TOGGLE_THRESHOLD: Duration = Duration::from_millis(300);

/// Events sent from the key hook to the main processing loop.
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// A printable character was typed.
    Char(char),
    /// Backspace was pressed.
    Backspace,
    /// Buffer should be cleared (Enter, Tab, Escape, Arrow keys, mouse click).
    Clear,
    /// Opens the reply composer. Context is captured only after this event.
    OpenComposer,
    /// Starts or stops a local dictation session.
    ToggleDictation,
    /// Begins a push-to-talk dictation session.
    StartDictation,
    /// Stops a push-to-talk dictation session and transcribes it.
    StopDictation,
    /// Discards a currently recording dictation session without transcribing.
    CancelDictation,
    /// Starts dictation when idle, or stops an existing toggle session. A
    /// matching release after the hold threshold stops the newly started run.
    HoldOrTogglePress,
    /// Copies the newest optional local history entry.
    CopyLastResult,
    /// Pastes the current clipboard text without source formatting.
    PastePlainText,
}

/// Resolve only foreground application metadata at an action boundary. The
/// exclusion list is never used to inspect screen or conversation content.
pub fn active_app_is_disabled(disabled_apps: &[String]) -> bool {
    active_app_is_protected(disabled_apps)
}

/// Refuse actions in dedicated credential-manager applications by default.
/// This is intentionally based only on foreground app metadata, never on a
/// field value, accessibility tree, or screen contents. Users can extend the
/// protection with their own per-app exclusion list.
pub fn active_app_is_protected(disabled_apps: &[String]) -> bool {
    let secure_input = focused_input_is_secure();
    let Some(window) = crate::active_target::get() else {
        return secure_input;
    };
    let app_name = window.app_name;
    let title = window.title;
    let path = window.process_path;
    matches_disabled_app(disabled_apps, &app_name, &title, &path)
        || matches_sensitive_app(&app_name, &path)
        || secure_input
}

/// Check the foreground application's metadata against a user-managed list.
/// This intentionally never reads text displayed in the active window.
pub fn active_app_matches(apps: &[String]) -> bool {
    if apps.is_empty() {
        return false;
    }
    let Some(window) = crate::active_target::get() else {
        return false;
    };
    let app_name = window.app_name;
    let title = window.title;
    let path = window.process_path;
    matches_disabled_app(apps, &app_name, &title, &path)
}

fn matches_disabled_app(disabled_apps: &[String], app_name: &str, title: &str, path: &str) -> bool {
    let app_name = app_name.to_ascii_lowercase();
    let title = title.to_ascii_lowercase();
    let path = path.to_ascii_lowercase();
    disabled_apps.iter().any(|entry| {
        let entry = entry.trim().to_ascii_lowercase();
        !entry.is_empty()
            && (app_name.contains(&entry) || title.contains(&entry) || path.contains(&entry))
    })
}

fn matches_sensitive_app(app_name: &str, path: &str) -> bool {
    const CREDENTIAL_APPS: &[&str] = &[
        "1password",
        "bitwarden",
        "keepass",
        "keeper password",
        "dashlane",
        "lastpass",
        "enpass",
        "proton pass",
    ];
    let app_name = app_name.to_ascii_lowercase();
    let path = path.to_ascii_lowercase();
    CREDENTIAL_APPS
        .iter()
        .any(|name| app_name.contains(name) || path.contains(name))
}

/// Query the focused native control only on Windows. UI Automation exposes an
/// explicit password flag, so Flick can refuse the action without fetching the
/// field value, its text, or an accessibility subtree. Other platforms retain
/// the app-level protection until their native secure-input APIs are available.
#[cfg(target_os = "windows")]
fn focused_input_is_secure() -> bool {
    use uiautomation::UIAutomation;

    UIAutomation::new()
        .and_then(|automation| automation.get_focused_element())
        .and_then(|element| element.is_password())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn focused_input_is_secure() -> bool {
    false
}

/// Start the global keyboard/mouse hook on a dedicated OS thread.
/// Returns a receiver channel for HookEvents.
/// The rdev listener must run on a raw OS thread (not Tokio) because it
/// blocks the thread with a platform-specific event loop.
pub fn start_hook() -> mpsc::Receiver<HookEvent> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        log::info!("Global key hook thread started");

        let callback = move |event: Event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    if let Some(hook_event) = map_key_event(key) {
                        let _ = tx.send(hook_event);
                    }
                }
                // Mouse click resets buffer - Open Question #1 resolution
                EventType::ButtonPress(_) => {
                    let _ = tx.send(HookEvent::Clear);
                }
                _ => {}
            }
        };

        if let Err(e) = listen(callback) {
            log::error!("Key hook listener error: {:?}", e);
        }
    });

    rx
}

/// Map an rdev Key to a HookEvent.
fn map_key_event(key: Key) -> Option<HookEvent> {
    match key {
        // Buffer-clearing keys - per §8.1
        Key::Return => Some(HookEvent::Clear),
        Key::Tab => Some(HookEvent::Clear),
        Key::Escape => Some(HookEvent::CancelDictation),
        Key::UpArrow | Key::DownArrow | Key::LeftArrow | Key::RightArrow => Some(HookEvent::Clear),
        Key::Home | Key::End | Key::PageUp | Key::PageDown => Some(HookEvent::Clear),

        // Backspace
        Key::Backspace => Some(HookEvent::Backspace),

        // Printable characters
        Key::Space => Some(HookEvent::Char(' ')),
        Key::Num0 => Some(HookEvent::Char('0')),
        Key::Num1 => Some(HookEvent::Char('1')),
        Key::Num2 => Some(HookEvent::Char('2')),
        Key::Num3 => Some(HookEvent::Char('3')),
        Key::Num4 => Some(HookEvent::Char('4')),
        Key::Num5 => Some(HookEvent::Char('5')),
        Key::Num6 => Some(HookEvent::Char('6')),
        Key::Num7 => Some(HookEvent::Char('7')),
        Key::Num8 => Some(HookEvent::Char('8')),
        Key::Num9 => Some(HookEvent::Char('9')),
        Key::KeyA => Some(HookEvent::Char('a')),
        Key::KeyB => Some(HookEvent::Char('b')),
        Key::KeyC => Some(HookEvent::Char('c')),
        Key::KeyD => Some(HookEvent::Char('d')),
        Key::KeyE => Some(HookEvent::Char('e')),
        Key::KeyF => Some(HookEvent::Char('f')),
        Key::KeyG => Some(HookEvent::Char('g')),
        Key::KeyH => Some(HookEvent::Char('h')),
        Key::KeyI => Some(HookEvent::Char('i')),
        Key::KeyJ => Some(HookEvent::Char('j')),
        Key::KeyK => Some(HookEvent::Char('k')),
        Key::KeyL => Some(HookEvent::Char('l')),
        Key::KeyM => Some(HookEvent::Char('m')),
        Key::KeyN => Some(HookEvent::Char('n')),
        Key::KeyO => Some(HookEvent::Char('o')),
        Key::KeyP => Some(HookEvent::Char('p')),
        Key::KeyQ => Some(HookEvent::Char('q')),
        Key::KeyR => Some(HookEvent::Char('r')),
        Key::KeyS => Some(HookEvent::Char('s')),
        Key::KeyT => Some(HookEvent::Char('t')),
        Key::KeyU => Some(HookEvent::Char('u')),
        Key::KeyV => Some(HookEvent::Char('v')),
        Key::KeyW => Some(HookEvent::Char('w')),
        Key::KeyX => Some(HookEvent::Char('x')),
        Key::KeyY => Some(HookEvent::Char('y')),
        Key::KeyZ => Some(HookEvent::Char('z')),
        Key::Minus => Some(HookEvent::Char('-')),
        Key::Equal => Some(HookEvent::Char('=')),
        Key::LeftBracket => Some(HookEvent::Char('[')),
        Key::RightBracket => Some(HookEvent::Char(']')),
        Key::BackSlash => Some(HookEvent::Char('\\')),
        Key::SemiColon => Some(HookEvent::Char(';')),
        Key::Quote => Some(HookEvent::Char('\'')),
        Key::Comma => Some(HookEvent::Char(',')),
        Key::Dot => Some(HookEvent::Char('.')),
        Key::Slash => Some(HookEvent::Char('/')),
        Key::BackQuote => Some(HookEvent::Char('`')),

        // Exclamation mark is Shift+1, but rdev reports it as Num1 with shift state.
        // We handle the `!` prefix detection by also checking for the IntlBackslash
        // and other special keys. The actual `!` character must be detected from
        // the name field in rdev events, but since rdev's Key enum doesn't distinguish
        // shifted characters, we rely on the buffer processing to receive '!' via
        // a special path. In practice, rdev will report Shift+1 as Key::Num1.
        // The exclamation mark character needs special handling - see lib.rs event loop.
        _ => None,
    }
}

/// Try to convert an rdev Event to a character, using the event name for
/// shifted characters (like '!' from Shift+1).
pub fn event_to_char(event: &Event) -> Option<char> {
    if let EventType::KeyPress(_) = event.event_type {
        // rdev provides the actual character typed via the `name` field
        // which accounts for shift state and keyboard layout.
        if let Some(ref name) = event.name {
            if name.len() == 1 {
                return name.chars().next();
            }
        }
    }
    None
}

/// Start the hook with a raw event callback that uses `event.name` for accurate
/// character detection (handles shift state, keyboard layout, etc.).
pub fn start_hook_with_name_detection(app: AppHandle) -> mpsc::Receiver<HookEvent> {
    let (tx, rx) = mpsc::channel();
    let modifiers = Arc::new(Mutex::new(HashSet::new()));
    let dictation_press = Arc::new(Mutex::new(None::<(Key, Instant)>));

    thread::spawn(move || {
        log::info!("Global key hook thread started (with name detection)");

        let callback_modifiers = Arc::clone(&modifiers);
        let callback_dictation_press = Arc::clone(&dictation_press);
        let callback = move |event: Event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    if matches!(
                        key,
                        Key::ShiftLeft
                            | Key::ShiftRight
                            | Key::ControlLeft
                            | Key::ControlRight
                            | Key::MetaLeft
                            | Key::MetaRight
                    ) {
                        if let Ok(mut active) = callback_modifiers.lock() {
                            active.insert(key);
                        }
                        return;
                    }
                    let config = app
                        .try_state::<crate::AppState>()
                        .and_then(|state| state.config.lock().ok().map(|config| config.clone()));
                    if let (Some(config), Ok(active)) = (config, callback_modifiers.lock()) {
                        if shortcut_matches(&config.composer_shortcut, key, &active) {
                            if !active_app_is_disabled(&config.disabled_apps) {
                                let _ = tx.send(HookEvent::OpenComposer);
                            }
                            return;
                        }
                        if shortcut_matches(&config.copy_last_result_shortcut, key, &active) {
                            if !active_app_is_disabled(&config.disabled_apps) {
                                let _ = tx.send(HookEvent::CopyLastResult);
                            }
                            return;
                        }
                        if shortcut_matches(&config.paste_plain_text_shortcut, key, &active) {
                            if !active_app_is_disabled(&config.disabled_apps) {
                                let _ = tx.send(HookEvent::PastePlainText);
                            }
                            return;
                        }
                        if shortcut_matches(&config.dictation_shortcut, key, &active) {
                            if !active_app_is_disabled(&config.disabled_apps) {
                                let event = match config.dictation_mode.as_str() {
                                    "push-to-talk" => HookEvent::StartDictation,
                                    "hold-or-toggle" => {
                                        if let Ok(mut pressed) = callback_dictation_press.lock() {
                                            *pressed = Some((key, Instant::now()));
                                        }
                                        HookEvent::HoldOrTogglePress
                                    }
                                    _ => HookEvent::ToggleDictation,
                                };
                                let _ = tx.send(event);
                            }
                            return;
                        }
                    }
                    // First, check for clear/backspace keys
                    match key {
                        Key::Return
                        | Key::Tab
                        | Key::UpArrow
                        | Key::DownArrow
                        | Key::LeftArrow
                        | Key::RightArrow
                        | Key::Home
                        | Key::End
                        | Key::PageUp
                        | Key::PageDown => {
                            let _ = tx.send(HookEvent::Clear);
                            return;
                        }
                        Key::Backspace => {
                            let _ = tx.send(HookEvent::Backspace);
                            return;
                        }
                        Key::Escape => {
                            let _ = tx.send(HookEvent::CancelDictation);
                            return;
                        }
                        // Skip modifier-only keys
                        Key::Alt | Key::AltGr | Key::CapsLock | Key::NumLock => {
                            return;
                        }
                        _ => {}
                    }

                    // Use event.name for accurate character detection
                    if let Some(ref name) = event.name {
                        if name.len() == 1 {
                            if let Some(c) = name.chars().next() {
                                let _ = tx.send(HookEvent::Char(c));
                            }
                        }
                    }
                }
                EventType::KeyRelease(key) => {
                    let config = app
                        .try_state::<crate::AppState>()
                        .and_then(|state| state.config.lock().ok().map(|config| config.clone()));
                    if let (Some(config), Ok(active)) = (config, callback_modifiers.lock()) {
                        if config.dictation_mode == "push-to-talk"
                            && shortcut_matches(&config.dictation_shortcut, key, &active)
                        {
                            let _ = tx.send(HookEvent::StopDictation);
                        }
                        if config.dictation_mode == "hold-or-toggle" {
                            let was_held = callback_dictation_press
                                .lock()
                                .ok()
                                .and_then(|mut pressed| {
                                    if pressed
                                        .as_ref()
                                        .is_some_and(|(pressed_key, _)| *pressed_key == key)
                                    {
                                        pressed.take()
                                    } else {
                                        None
                                    }
                                })
                                .is_some_and(|(_, started)| {
                                    started.elapsed() >= HOLD_OR_TOGGLE_THRESHOLD
                                });
                            if was_held {
                                let _ = tx.send(HookEvent::StopDictation);
                            }
                        }
                    }
                    if let Ok(mut active) = callback_modifiers.lock() {
                        active.remove(&key);
                    }
                }
                EventType::ButtonPress(_) => {
                    let _ = tx.send(HookEvent::Clear);
                }
                _ => {}
            }
        };

        if let Err(e) = listen(callback) {
            log::error!("Key hook listener error: {:?}", e);
        }
    });

    rx
}

fn shortcut_matches(shortcut: &str, key: Key, active: &HashSet<Key>) -> bool {
    let tokens: Vec<String> = shortcut
        .split('+')
        .map(|token| token.trim().to_ascii_uppercase())
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return false;
    }
    let has_control = active.contains(&Key::ControlLeft) || active.contains(&Key::ControlRight);
    let has_meta = active.contains(&Key::MetaLeft) || active.contains(&Key::MetaRight);
    let has_shift = active.contains(&Key::ShiftLeft) || active.contains(&Key::ShiftRight);
    let has_alt = active.contains(&Key::Alt) || active.contains(&Key::AltGr);
    let wants_control = tokens
        .iter()
        .any(|token| token == "CTRL" || token == "CONTROL");
    let wants_meta = tokens
        .iter()
        .any(|token| token == "CMD" || token == "COMMAND" || token == "META");
    let wants_shift = tokens.iter().any(|token| token == "SHIFT");
    let wants_alt = tokens
        .iter()
        .any(|token| token == "ALT" || token == "OPTION");
    if (wants_control && !has_control)
        || (wants_meta && !has_meta)
        || (wants_shift && !has_shift)
        || (wants_alt && !has_alt)
    {
        return false;
    }
    let key_token = format!("{key:?}").to_ascii_uppercase().replace("KEY", "");
    tokens.iter().any(|token| {
        !matches!(
            token.as_str(),
            "CTRL" | "CONTROL" | "CMD" | "COMMAND" | "META" | "SHIFT" | "ALT" | "OPTION"
        ) && (token == &key_token || (token == "SPACE" && key == Key::Space))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurable_shortcuts_require_the_requested_modifiers() {
        let mut modifiers = HashSet::new();
        modifiers.insert(Key::ControlLeft);
        modifiers.insert(Key::ShiftLeft);
        assert!(shortcut_matches("Ctrl+Shift+Space", Key::Space, &modifiers));
        assert!(!shortcut_matches("Ctrl+Alt+Space", Key::Space, &modifiers));
        assert!(shortcut_matches("Ctrl+Shift+R", Key::KeyR, &modifiers));
        modifiers.remove(&Key::ShiftLeft);
        modifiers.insert(Key::Alt);
        assert!(shortcut_matches("Ctrl+Alt+C", Key::KeyC, &modifiers));
        assert!(shortcut_matches("Ctrl+Alt+V", Key::KeyV, &modifiers));
    }

    #[test]
    fn exclusion_matching_is_case_insensitive() {
        let disabled = vec!["slack".to_string(), "DISCORD".to_string()];
        assert!(matches_disabled_app(
            &disabled,
            "Slack",
            "Work chat",
            "C:/Apps/Slack.exe"
        ));
        assert!(matches_disabled_app(
            &disabled,
            "",
            "",
            "C:/Apps/Discord.exe"
        ));
        assert!(!matches_disabled_app(
            &disabled,
            "Flick",
            "Settings",
            "C:/Apps/Flick.exe"
        ));
    }

    #[test]
    fn dedicated_credential_apps_are_protected_without_screen_inspection() {
        assert!(matches_sensitive_app("Bitwarden", "C:/Apps/Bitwarden.exe"));
        assert!(matches_sensitive_app("", "C:/Apps/KeePassXC.exe"));
        assert!(!matches_sensitive_app("Slack", "C:/Apps/Slack.exe"));
    }

    #[test]
    fn hold_or_toggle_uses_a_deliberate_hold_threshold() {
        assert!(Duration::from_millis(299) < HOLD_OR_TOGGLE_THRESHOLD);
        assert_eq!(Duration::from_millis(300), HOLD_OR_TOGGLE_THRESHOLD);
    }
}
