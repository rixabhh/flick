// Flick - ai_client.rs
// Per PRD §10.1: AI API client.
// Default provider is Gemini with the free lite model.
// 10-second timeout per §8.3.

use anyhow::{bail, Context, Result};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OUTPUT_TOKENS: u32 = 1024;
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const LANGUAGE_POLICY: &str = "Language handling: detect whether the input is English, Hindi, Hinglish, or another language. Unless the task explicitly asks for translation, preserve the same language, script, and natural code-mixed style. For Hinglish, keep the Hindi-English mix natural instead of forcing pure English. Preserve names, URLs, code, numbers, emojis, and intentional formatting where possible. Return only the transformed text.";

static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("failed to build shared HTTP client")
});

fn provider_error_message(body_text: &str) -> String {
    serde_json::from_str::<Value>(body_text)
        .ok()
        .and_then(|body| {
            body.get("error")
                .and_then(|error| {
                    error
                        .get("message")
                        .and_then(|v| v.as_str())
                        .or_else(|| error.get("reason").and_then(|v| v.as_str()))
                })
                .or_else(|| body.get("message").and_then(|v| v.as_str()))
                .or_else(|| body.get("reason").and_then(|v| v.as_str()))
                .map(str::to_string)
        })
        .unwrap_or_else(|| body_text.trim().to_string())
}

fn first_text_from_openrouter_content(content: &Value) -> Option<&str> {
    content.as_str().or_else(|| {
        content.as_array().and_then(|parts| {
            parts.iter().find_map(|part| {
                part.as_str().or_else(|| {
                    part.get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| part.get("content").and_then(|v| v.as_str()))
                })
            })
        })
    })
}

/// Built-in prompt instructions - per PRD §9.
/// Returns a complete, language-aware prompt for the selected command.
pub fn get_prompt(command: &str, param: Option<&str>, text: &str) -> Option<String> {
    let task = match command {
        "fix" => "Fix grammar, spelling, punctuation, and awkward phrasing without changing the user's meaning or language style.".to_string(),
        "formal" => "Rewrite in a formal, professional tone while preserving the user's original language or Hinglish/Hindi style.".to_string(),
        "casual" => "Rewrite in a casual, friendly, conversational tone while preserving the user's original language or Hinglish/Hindi style.".to_string(),
        "shorter" => "Make the text shorter and more concise while keeping the core meaning and original language style.".to_string(),
        "longer" => "Expand the text with useful detail and context while keeping the same meaning and original language style.".to_string(),
        "improve" => "Improve clarity, flow, grammar, and readability while preserving the original meaning, tone, and language style.".to_string(),
        "rephrase" => "Rephrase the text in a different way while keeping the same meaning and original language style.".to_string(),
        "bullet" => "Convert the text into a clear, well-structured bullet point list while preserving the original language style.".to_string(),
        "explain" => "Rewrite the text in simple, easy-to-understand language while preserving the original language or Hinglish/Hindi style.".to_string(),
        "translate" => {
            let lang = param.unwrap_or("English");
            format!("Translate the text to {}. Keep names, URLs, code, numbers, and formatting intact where possible.", lang)
        },
        _ => return None,
    };

    Some(build_instruction_prompt(&task, text))
}

fn build_instruction_prompt(task: &str, text: &str) -> String {
    format!("{LANGUAGE_POLICY}\n\nTask: {task}\n\nText:\n{text}")
}

/// Build a custom command prompt. Existing {{text}} templates still work, while
/// new commands can be plain instructions describing what the command should do.
pub fn get_custom_prompt(system_prompt: &str, text: &str) -> String {
    let instruction = system_prompt.trim();

    if instruction.contains("{{text}}") {
        return format!(
            "{LANGUAGE_POLICY}\n\nTask:\n{}",
            instruction.replace("{{text}}", text)
        );
    }

    build_instruction_prompt(instruction, text)
}

/// Send a text transformation request using the selected provider/model.
pub async fn transform_text(
    api_key: &str,
    provider: &str,
    model: &str,
    prompt: &str,
) -> Result<String> {
    match provider {
        "openrouter" => {
            let response = HTTP_CLIENT
                .post(OPENROUTER_BASE_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("HTTP-Referer", "https://github.com/rixabhh/flick")
                .header("X-Title", "Flick")
                .json(&json!({
                    "model": model,
                    "messages": [{
                        "role": "user",
                        "content": prompt
                    }],
                    "temperature": 0.3,
                    "max_tokens": MAX_OUTPUT_TOKENS
                }))
                .send()
                .await
                .context("OpenRouter API request failed")?;

            if !response.status().is_success() {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                let message = provider_error_message(&body_text);
                bail!("OpenRouter rejected the request ({}): {}", status, message);
            }

            let response_json: Value = response
                .json()
                .await
                .context("Failed to parse OpenRouter response JSON")?;

            let text = response_json
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(first_text_from_openrouter_content)
                .context("Unexpected OpenRouter response structure")?;

            Ok(text.trim().to_string())
        }
        _ => {
            let url = format!(
                "{}/{}:generateContent?key={}",
                GEMINI_BASE_URL, model, api_key
            );

            let body = json!({
                "contents": [{
                    "parts": [{
                        "text": prompt
                    }]
                }],
                "generationConfig": {
                    "temperature": 0.3,
                    "maxOutputTokens": MAX_OUTPUT_TOKENS
                }
            });

            let response = HTTP_CLIENT
                .post(&url)
                .json(&body)
                .send()
                .await
                .context("Gemini API request failed")?;

            if !response.status().is_success() {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                let message = provider_error_message(&body_text);
                bail!("Gemini rejected the request ({}): {}", status, message);
            }

            let response_json: Value = response
                .json()
                .await
                .context("Failed to parse Gemini response JSON")?;

            let text = response_json
                .get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.get(0))
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
                .context("Unexpected Gemini response structure")?;

            Ok(text.trim().to_string())
        }
    }
}

/// Test the API connection with a minimal request.
pub async fn test_connection(api_key: &str, provider: &str, model: &str) -> Result<()> {
    let prompt = "Reply with exactly: OK";
    let result = transform_text(api_key, provider, model, prompt).await?;
    if result.is_empty() {
        bail!("Selected provider returned empty response");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_prompt_fix() {
        let prompt = get_prompt("fix", None, "hello wrold").unwrap();
        assert!(prompt.contains("hello wrold"));
        assert!(prompt.contains("Fix grammar"));
        assert!(prompt.contains("Hinglish"));
    }

    #[test]
    fn test_get_prompt_translate() {
        let prompt = get_prompt("translate", Some("spanish"), "hello").unwrap();
        assert!(prompt.contains("spanish"));
        assert!(prompt.contains("hello"));
    }

    #[test]
    fn test_get_prompt_unknown() {
        assert!(get_prompt("unknown_command", None, "text").is_none());
    }

    #[test]
    fn test_get_custom_prompt_without_template() {
        let prompt = get_custom_prompt("Make this witty.", "plain text");
        assert!(prompt.contains("Make this witty."));
        assert!(prompt.contains("plain text"));
        assert!(prompt.contains("Hinglish"));
    }

    #[test]
    fn test_get_custom_prompt_with_legacy_template() {
        let prompt = get_custom_prompt("Summarize: {{text}}", "long text");
        assert!(prompt.contains("Summarize: long text"));
    }

    #[test]
    fn test_all_builtin_commands_have_prompts() {
        for cmd in &[
            "fix", "formal", "casual", "shorter", "longer", "improve", "rephrase", "bullet",
            "explain",
        ] {
            assert!(
                get_prompt(cmd, None, "test").is_some(),
                "Missing prompt for: {}",
                cmd
            );
        }
        assert!(get_prompt("translate", Some("french"), "test").is_some());
    }
}
