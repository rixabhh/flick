// Flick — key_hook.rs
// Per PRD §8.1: Global keyboard listener via rdev in a dedicated background thread.
// Also listens for mouse clicks to reset buffer (Open Question #1: Yes).

use rdev::{listen, Event, EventType, Key};
use std::sync::mpsc;
use std::thread;

/// Events sent from the key hook to the main processing loop.
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// A printable character was typed.
    Char(char),
    /// Backspace was pressed.
    Backspace,
    /// Buffer should be cleared (Enter, Tab, Escape, Arrow keys, mouse click).
    Clear,
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
                // Mouse click resets buffer — Open Question #1 resolution
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
        // Buffer-clearing keys — per §8.1
        Key::Return => Some(HookEvent::Clear),
        Key::Tab => Some(HookEvent::Clear),
        Key::Escape => Some(HookEvent::Clear),
        Key::UpArrow | Key::DownArrow | Key::LeftArrow | Key::RightArrow => {
            Some(HookEvent::Clear)
        }
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
        // The exclamation mark character needs special handling — see lib.rs event loop.
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
pub fn start_hook_with_name_detection() -> mpsc::Receiver<HookEvent> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        log::info!("Global key hook thread started (with name detection)");

        let callback = move |event: Event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    // First, check for clear/backspace keys
                    match key {
                        Key::Return | Key::Tab | Key::Escape |
                        Key::UpArrow | Key::DownArrow | Key::LeftArrow | Key::RightArrow |
                        Key::Home | Key::End | Key::PageUp | Key::PageDown => {
                            let _ = tx.send(HookEvent::Clear);
                            return;
                        }
                        Key::Backspace => {
                            let _ = tx.send(HookEvent::Backspace);
                            return;
                        }
                        // Skip modifier-only keys
                        Key::ShiftLeft | Key::ShiftRight |
                        Key::ControlLeft | Key::ControlRight |
                        Key::Alt | Key::AltGr |
                        Key::MetaLeft | Key::MetaRight |
                        Key::CapsLock | Key::NumLock => {
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
