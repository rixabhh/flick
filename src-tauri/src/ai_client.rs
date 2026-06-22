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

/// Built-in prompt templates - per PRD §9.
/// Returns the full prompt with {{text}} substituted.
pub fn get_prompt(command: &str, param: Option<&str>, text: &str) -> Option<String> {
    let template = match command {
        "fix" => "Fix all grammar, spelling, and punctuation errors in the following text. Return only the corrected text, no explanation: {{text}}".to_string(),
        "formal" => "Rewrite the following text in a formal, professional tone. Return only the rewritten text: {{text}}".to_string(),
        "casual" => "Rewrite the following text in a casual, friendly, conversational tone. Return only the rewritten text: {{text}}".to_string(),
        "shorter" => "Make the following text shorter and more concise while keeping the core meaning. Return only the shortened text: {{text}}".to_string(),
        "longer" => "Expand the following text with more detail and context. Return only the expanded text: {{text}}".to_string(),
        "improve" => "Improve the following text for clarity, flow, grammar, and readability while preserving the original meaning and tone. Return only the improved text: {{text}}".to_string(),
        "rephrase" => "Rephrase the following text in a different way while keeping the same meaning. Return only the rephrased text: {{text}}".to_string(),
        "bullet" => "Convert the following text into a clear, well-structured bullet point list. Return only the bullet points: {{text}}".to_string(),
        "explain" => "Rewrite the following text in simple, easy-to-understand language. Return only the simplified text: {{text}}".to_string(),
        "translate" => {
            let lang = param.unwrap_or("English");
            format!("Translate the following text to {}. Return only the translated text, nothing else: {{{{text}}}}", lang)
        },
        _ => return None,
    };

    Some(template.replace("{{text}}", text))
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
        assert!(prompt.contains("Fix all grammar"));
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
