//! Shared validation for settings and exact global shortcut matching.
use std::sync::Mutex;
use std::time::{Duration, Instant};

static CAPTURE_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

// A lease prevents a closed/crashed renderer from disabling shortcuts forever.
#[tauri::command]
pub fn set_shortcut_capture(active: bool) {
    *CAPTURE_UNTIL
        .lock()
        .unwrap_or_else(|error| error.into_inner()) =
        active.then(|| Instant::now() + Duration::from_secs(30));
    CAPTURE_UNTIL.clear_poison();
}
pub fn capturing() -> bool {
    CAPTURE_UNTIL
        .lock()
        .map(|until| until.is_some_and(|until| Instant::now() < until))
        .unwrap_or(false)
}

pub fn normalize(value: &str) -> Result<String, String> {
    let mut modifiers = [false; 4];
    let mut key = None;
    for token in value
        .split('+')
        .map(|part| part.trim().to_ascii_uppercase())
    {
        let modifier = match token.as_str() {
            "CTRL" | "CONTROL" => Some(0),
            "ALT" | "OPTION" => Some(1),
            "SHIFT" => Some(2),
            "CMD" | "COMMAND" | "META" => Some(3),
            _ => None,
        };
        if let Some(index) = modifier {
            if modifiers[index] {
                return Err("Do not repeat a modifier.".into());
            }
            modifiers[index] = true;
        } else {
            let normalized = match token.as_str() {
                "SPACE" => "Space".to_string(),
                token if token.len() == 1 && token.bytes().all(|b| b.is_ascii_alphanumeric()) => {
                    token.to_string()
                }
                token
                    if token
                        .strip_prefix('F')
                        .and_then(|n| n.parse::<u8>().ok())
                        .is_some_and(|n| (1..=12).contains(&n)) =>
                {
                    token.to_string()
                }
                _ => return Err("Use a letter, number, Space, or F1–F12.".into()),
            };
            if key.replace(normalized).is_some() {
                return Err("Use one key with your modifiers.".into());
            }
        }
    }
    if !modifiers[0] && !modifiers[1] && !modifiers[3] {
        return Err("Include Control, Alt/Option, or Command.".into());
    }
    let key = key.ok_or("Choose a key as well as modifiers.")?;
    let mut parts = Vec::new();
    for (index, name) in ["Ctrl", "Alt", "Shift", "Cmd"].iter().enumerate() {
        if modifiers[index] {
            parts.push((*name).to_string());
        }
    }
    parts.push(key);
    let shortcut = parts.join("+");
    if matches!(shortcut.as_str(), "Alt+F4" | "Cmd+Q" | "Cmd+L") {
        return Err("That shortcut is reserved by the operating system. Choose another.".into());
    }
    Ok(shortcut)
}

pub fn validate_config(config: &crate::config::FlickConfig) -> Result<(), String> {
    let mut used = std::collections::HashSet::new();
    for value in [
        &config.composer_shortcut,
        &config.dictation_shortcut,
        &config.copy_last_result_shortcut,
        &config.paste_plain_text_shortcut,
    ] {
        let normalized = normalize(value)?;
        if !used.insert(normalized) {
            return Err("Each Flick action needs a different shortcut.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_custom_shortcuts_and_rejects_ambiguous_bindings() {
        assert_eq!(normalize("option + control + r").unwrap(), "Ctrl+Alt+R");
        assert_eq!(normalize("Cmd+Shift+2").unwrap(), "Shift+Cmd+2");
        for bad in [
            "",
            "R",
            "Shift+A",
            "Ctrl++R",
            "Ctrl+R+T",
            "Alt+F4",
            "Ctrl+Ctrl+R",
        ] {
            assert!(normalize(bad).is_err(), "{bad}");
        }
        let mut config = crate::config::FlickConfig::default();
        assert!(validate_config(&config).is_ok());
        config.composer_shortcut = config.dictation_shortcut.clone();
        assert!(validate_config(&config).is_err());
    }
}
