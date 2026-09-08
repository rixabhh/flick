//! Provider-neutral dictation transcription contract.
//!
//! Audio capture, target protection, paste-back, history, and post-processing
//! belong to Flick's shared dictation flow. Only the final audio-to-text step
//! varies by provider. Keeping that boundary explicit lets local ONNX engines
//! and opt-in cloud transports join without weakening the proven local Whisper
//! path or silently sending microphone audio anywhere.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const LOCAL_WHISPER_PROVIDER_ID: &str = "local-whisper";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DictationProviderKind {
    Local,
    Cloud,
}

/// Stable, UI-safe facts about a transcription provider. Credentials and
/// endpoint URLs deliberately do not appear here.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DictationProviderInfo {
    pub id: String,
    pub label: String,
    pub kind: DictationProviderKind,
    pub sends_audio_off_device: bool,
    pub supports_translation: bool,
    pub requires_local_model: bool,
}

impl DictationProviderInfo {
    fn local_whisper() -> Self {
        Self {
            id: LOCAL_WHISPER_PROVIDER_ID.to_string(),
            label: "Local Whisper".to_string(),
            kind: DictationProviderKind::Local,
            sends_audio_off_device: false,
            supports_translation: true,
            requires_local_model: true,
        }
    }
}

/// A normalized mono 16 kHz utterance. The capture pipeline owns the samples;
/// providers only receive this immutable request and never control recording or
/// paste-back.
pub struct TranscriptionRequest<'a> {
    pub audio: &'a [f32],
    pub language: &'a str,
    pub translate_to_english: bool,
    pub local_model_path: Option<&'a Path>,
}

trait DictationTranscriber {
    fn transcribe(&self, request: TranscriptionRequest<'_>) -> Result<String>;
}

struct LocalWhisperTranscriber;

impl DictationTranscriber for LocalWhisperTranscriber {
    fn transcribe(&self, request: TranscriptionRequest<'_>) -> Result<String> {
        let path = request
            .local_model_path
            .context("Download a local speech model before dictating")?;
        let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .context("Could not load local speech model")?;
        let mut state = context
            .create_state()
            .context("Could not initialize transcription engine")?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language((request.language != "auto").then_some(request.language));
        params.set_translate(request.translate_to_english);
        state
            .full(params, request.audio)
            .context("Local transcription failed")?;
        Ok(state
            .as_iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string())
    }
}

/// Returns the providers that are safe to choose in this build. Entries are
/// added only with a concrete engine/transport implementation; future settings
/// values are rejected instead of falling back and unexpectedly changing the
/// user's privacy choice.
pub fn available_providers() -> Vec<DictationProviderInfo> {
    vec![DictationProviderInfo::local_whisper()]
}

/// Exposes only implementation-backed provider choices to the settings UI.
/// The UI must not manufacture a cloud choice before its consent, credential,
/// transport, and failure-recovery path exist.
#[tauri::command]
pub fn list_dictation_providers() -> Vec<DictationProviderInfo> {
    available_providers()
}

pub fn provider_info(id: &str) -> Result<DictationProviderInfo> {
    available_providers()
        .into_iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| anyhow::anyhow!("Dictation provider '{id}' is not available in this version of Flick"))
}

/// Route transcription through the configured provider. No fallback is used:
/// falling back from a selected cloud provider to a local engine (or vice
/// versa) would make both quality and privacy behavior unpredictable.
pub fn transcribe(provider_id: &str, request: TranscriptionRequest<'_>) -> Result<String> {
    match provider_id {
        LOCAL_WHISPER_PROVIDER_ID => LocalWhisperTranscriber.transcribe(request),
        unsupported => bail!("Dictation provider '{unsupported}' is not available in this version of Flick"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_whisper_is_the_private_default_provider() {
        let providers = available_providers();
        assert_eq!(providers.len(), 1);
        let whisper = &providers[0];
        assert_eq!(whisper.id, LOCAL_WHISPER_PROVIDER_ID);
        assert_eq!(whisper.kind, DictationProviderKind::Local);
        assert!(!whisper.sends_audio_off_device);
        assert!(whisper.requires_local_model);
    }

    #[test]
    fn unavailable_provider_never_falls_back() {
        assert!(provider_info("cloud-openai").is_err());
    }
}
