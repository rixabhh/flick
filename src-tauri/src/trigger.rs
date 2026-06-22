// Flick - trigger.rs
// Per PRD §8.2: Trigger detection with regex for built-in + custom commands.
// Checked against the buffer tail (last 40 chars) on every keypress.

use regex::Regex;
use once_cell::sync::Lazy;

/// Result of a successful trigger match.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerMatch {
    /// The command name (e.g., "fix", "formal", "translate")
    pub command: String,
    /// Optional parameter (e.g., "spanish" for !translate:spanish)
    pub param: Option<String>,
    /// The full trigger string to strip from text (e.g., "!fix", "!translate:spanish")
    pub full_trigger: String,
}

// Built-in simple commands regex - per §8.2
static SIMPLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"!(?P<cmd>fix|formal|casual|shorter|longer|improve|rephrase|bullet|explain)$").unwrap()
});

// Built-in parameterized commands regex - per §8.2
static PARAM_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"!(?P<cmd>translate):(?P<param>[a-zA-Z]+)$").unwrap()
});

/// Check the buffer tail for a built-in trigger match.
pub fn detect_builtin(text: &str) -> Option<TriggerMatch> {
    // Check simple commands first
    if let Some(caps) = SIMPLE_RE.captures(text) {
        let cmd = caps.name("cmd").unwrap().as_str().to_string();
        let full = format!("!{}", cmd);
        return Some(TriggerMatch {
            command: cmd,
            param: None,
            full_trigger: full,
        });
    }

    // Check parameterized commands
    if let Some(caps) = PARAM_RE.captures(text) {
        let cmd = caps.name("cmd").unwrap().as_str().to_string();
        let param = caps.name("param").unwrap().as_str().to_string();
        let full = format!("!{}:{}", cmd, param);
        return Some(TriggerMatch {
            command: cmd,
            param: Some(param),
            full_trigger: full,
        });
    }

    None
}

/// Check the buffer tail for a custom trigger match.
/// Custom triggers are provided as a list of trigger words (without the `!` prefix).
pub fn detect_custom(text: &str, custom_triggers: &[String]) -> Option<TriggerMatch> {
    for trigger in custom_triggers {
        if text
            .strip_suffix(trigger.as_str())
            .is_some_and(|prefix| prefix.ends_with('!'))
        {
            return Some(TriggerMatch {
                command: trigger.clone(),
                param: None,
                full_trigger: format!("!{}", trigger),
            });
        }
    }
    None
}

/// Detect any trigger (built-in or custom) in the given text.
pub fn detect(text: &str, custom_triggers: &[String]) -> Option<TriggerMatch> {
    // Built-in triggers take priority
    if let Some(m) = detect_builtin(text) {
        return Some(m);
    }
    detect_custom(text, custom_triggers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_fix() {
        let result = detect_builtin("hello world!fix");
        assert_eq!(
            result,
            Some(TriggerMatch {
                command: "fix".to_string(),
                param: None,
                full_trigger: "!fix".to_string(),
            })
        );
    }

    #[test]
    fn test_simple_formal() {
        let result = detect_builtin("some text!formal");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "formal");
    }

    #[test]
    fn test_all_simple_commands() {
        for cmd in &["fix", "formal", "casual", "shorter", "longer", "improve", "rephrase", "bullet", "explain"] {
            let input = format!("test text!{}", cmd);
            let result = detect_builtin(&input);
            assert!(result.is_some(), "Failed for command: {}", cmd);
            assert_eq!(result.unwrap().command, *cmd);
        }
    }

    #[test]
    fn test_translate_spanish() {
        let result = detect_builtin("hello!translate:spanish");
        assert_eq!(
            result,
            Some(TriggerMatch {
                command: "translate".to_string(),
                param: Some("spanish".to_string()),
                full_trigger: "!translate:spanish".to_string(),
            })
        );
    }

    #[test]
    fn test_translate_hindi() {
        let result = detect_builtin("some text!translate:hindi");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.command, "translate");
        assert_eq!(m.param, Some("hindi".to_string()));
    }

    #[test]
    fn test_no_match() {
        assert!(detect_builtin("hello world").is_none());
        assert!(detect_builtin("!unknown").is_none());
        assert!(detect_builtin("text !fix more").is_none()); // not at end
    }

    #[test]
    fn test_custom_trigger() {
        let customs = vec!["summarize".to_string(), "tldr".to_string()];
        let result = detect_custom("my long text!summarize", &customs);
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "summarize");
    }

    #[test]
    fn test_custom_no_match() {
        let customs = vec!["summarize".to_string()];
        let result = detect_custom("my text!unknown", &customs);
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_builtin_priority() {
        let customs = vec!["fix".to_string()]; // shadows built-in
        let result = detect("text!fix", &customs);
        assert!(result.is_some());
        // Built-in should win
        assert_eq!(result.unwrap().command, "fix");
    }

    #[test]
    fn test_partial_trigger_no_match() {
        // Trigger must be at the very end
        assert!(detect_builtin("!fix is great").is_none());
    }
}
