// Flick - ai_client.rs
// Per PRD §10.1: Gemini Flash API client.
// Model: gemini-2.0-flash, temperature 0.3, maxOutputTokens 2048.
// 10-second timeout per §8.3.

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

const GEMINI_ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Built-in prompt templates - per PRD §9.
/// Returns the full prompt with {{text}} substituted.
pub fn get_prompt(command: &str, param: Option<&str>, text: &str) -> Option<String> {
    let template = match command {
        "fix" => "Fix all grammar, spelling, and punctuation errors in the following text. Return only the corrected text, no explanation: {{text}}".to_string(),
        "formal" => "Rewrite the following text in a formal, professional tone. Return only the rewritten text: {{text}}".to_string(),
        "casual" => "Rewrite the following text in a casual, friendly, conversational tone. Return only the rewritten text: {{text}}".to_string(),
        "shorter" => "Make the following text shorter and more concise while keeping the core meaning. Return only the shortened text: {{text}}".to_string(),
        "longer" => "Expand the following text with more detail and context. Return only the expanded text: {{text}}".to_string(),
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

/// Send a text transformation request to the Gemini Flash API.
pub async fn transform_text(api_key: &str, prompt: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("Failed to build HTTP client")?;

    let url = format!("{}?key={}", GEMINI_ENDPOINT, api_key);

    let body = json!({
        "contents": [{
            "parts": [{
                "text": prompt
            }]
        }],
        "generationConfig": {
            "temperature": 0.3,
            "maxOutputTokens": 2048
        }
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("Gemini API request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        bail!("Gemini API returned {}: {}", status, body_text);
    }

    let response_json: Value = response
        .json()
        .await
        .context("Failed to parse Gemini response JSON")?;

    // Extract text from response - per §10.1
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

/// Test the API connection with a minimal request.
pub async fn test_connection(api_key: &str) -> Result<()> {
    let prompt = "Reply with exactly: OK";
    let result = transform_text(api_key, prompt).await?;
    if result.is_empty() {
        bail!("Gemini returned empty response");
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
        for cmd in &["fix", "formal", "casual", "shorter", "longer", "rephrase", "bullet", "explain"] {
            assert!(get_prompt(cmd, None, "test").is_some(), "Missing prompt for: {}", cmd);
        }
        assert!(get_prompt("translate", Some("french"), "test").is_some());
    }
}
