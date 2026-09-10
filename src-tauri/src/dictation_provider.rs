//! Provider-neutral dictation transcription contract.
//!
//! Audio capture, target protection, paste-back, history, and post-processing
//! belong to Flick's shared dictation flow. Only the final audio-to-text step
//! varies by provider. There is deliberately no fallback: choosing cloud is an
//! explicit privacy choice and choosing local must keep audio on-device.

use anyhow::{bail, Context, Result};
use reqwest::{multipart, Url};
use serde::{Deserialize, Serialize};
use std::{io::Cursor, path::Path, sync::OnceLock};
use transcribe_cpp::{Model, RunOptions, Task};

pub const LOCAL_WHISPER_PROVIDER_ID: &str = "local-whisper";
pub const CLOUD_OPENAI_COMPATIBLE_PROVIDER_ID: &str = "cloud-openai-compatible";
const CLOUD_API_KEY_SCOPE: &str = "dictation-openai-compatible";
const CLOUD_REQUEST_TIMEOUT_SECS: u64 = 90;

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
            label: "Local models (Whisper & verified GGUF)".to_string(),
            kind: DictationProviderKind::Local,
            sends_audio_off_device: false,
            supports_translation: true,
            requires_local_model: true,
        }
    }

    fn cloud_openai_compatible() -> Self {
        Self {
            id: CLOUD_OPENAI_COMPATIBLE_PROVIDER_ID.to_string(),
            label: "Cloud (OpenAI-compatible)".to_string(),
            kind: DictationProviderKind::Cloud,
            sends_audio_off_device: true,
            // The /audio/translations endpoint is model-specific. Do not
            // promise translation for an arbitrary compatible endpoint.
            supports_translation: false,
            requires_local_model: false,
        }
    }
}

/// A normalized mono 16 kHz utterance. The capture pipeline owns the samples;
/// providers only receive this request and never control recording or paste-back.
pub struct TranscriptionRequest {
    pub audio: Vec<f32>,
    pub language: String,
    pub translate_to_english: bool,
    pub local_model_path: Option<std::path::PathBuf>,
}

/// `transcribe-cpp` auto-detects legacy Whisper GGML `.bin` files and modern
/// GGUF architectures such as Parakeet. Existing downloads remain usable while
/// Flick gains more local model families without changing privacy behavior.
pub(crate) fn transcribe_local_whisper(
    audio: &[f32],
    language: &str,
    translate_to_english: bool,
    path: &Path,
) -> Result<String> {
    initialize_local_engine()?;
    let model = Model::load(path).context("Could not load local speech model")?;
    let mut session = model
        .session()
        .context("Could not initialize local transcription engine")?;
    let source_language = (language != "auto").then(|| language.to_string());
    let translating = translate_to_english && source_language.as_deref() != Some("en");
    let options = RunOptions {
        task: if translating {
            Task::Translate
        } else {
            Task::Transcribe
        },
        language: source_language,
        target_language: translating.then(|| "en".to_string()),
        ..Default::default()
    };
    session
        .run(audio, &options)
        .map(|result| result.text.trim().to_string())
        .context("Local transcription failed")
}

fn initialize_local_engine() -> Result<()> {
    static INITIALIZATION: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    INITIALIZATION
        .get_or_init(|| {
            transcribe_cpp::init_logging();
            transcribe_cpp::init_backends_default().map_err(|error| error.to_string())
        })
        .as_ref()
        // `OnceLock::get_or_init` yields a reference to the cached Result.
        // Do not let that reference escape this provider boundary: callers
        // need the unit success value, while a failed initialization remains
        // cached and reported consistently on every later dictation attempt.
        .map(|_| ())
        .map_err(|error| {
            anyhow::anyhow!("Could not initialize local transcription engine: {error}")
        })
}

#[derive(Debug, Deserialize)]
struct CloudTranscriptionResponse {
    text: String,
}

fn cloud_transcription_url(base_url: &str) -> Result<Url> {
    let base_url = base_url.trim();
    if base_url.is_empty() {
        bail!("Enter a cloud transcription endpoint before dictating");
    }
    let mut url =
        Url::parse(base_url).context("Cloud transcription endpoint must be a valid URL")?;
    let host = url.host_str().unwrap_or_default();
    let local_http = url.scheme() == "http" && matches!(host, "localhost" | "127.0.0.1" | "::1");
    if url.scheme() != "https" && !local_http {
        bail!("Cloud transcription endpoint must use HTTPS (except localhost)");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("Cloud transcription endpoint must not contain credentials");
    }
    let path = url.path().trim_end_matches('/');
    url.set_path(&format!("{path}/audio/transcriptions"));
    Ok(url)
}

fn wav_bytes(audio: &[f32]) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut output, spec)
            .context("Could not prepare audio for cloud transcription")?;
        for sample in audio {
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .context("Could not prepare audio for cloud transcription")?;
        }
        writer
            .finalize()
            .context("Could not finalize cloud transcription audio")?;
    }
    Ok(output.into_inner())
}

async fn transcribe_cloud_openai_compatible(
    request: TranscriptionRequest,
    base_url: String,
    model: String,
) -> Result<String> {
    if request.translate_to_english {
        bail!("The selected cloud transcription provider does not support translation. Turn off ‘Translate speech to English’ or use Local Whisper.");
    }
    let model = model.trim();
    if model.is_empty() {
        bail!("Enter a cloud transcription model before dictating");
    }
    let api_key = crate::keychain::load_api_key(CLOUD_API_KEY_SCOPE)
        .context("Save a cloud transcription API key before dictating")?;
    if api_key.trim().is_empty() {
        bail!("Save a cloud transcription API key before dictating");
    }
    let file = multipart::Part::bytes(wav_bytes(&request.audio)?)
        .file_name("flick-dictation.wav")
        .mime_str("audio/wav")
        .context("Could not prepare cloud transcription audio")?;
    let mut form = multipart::Form::new()
        .part("file", file)
        .text("model", model.to_string())
        .text("response_format", "json");
    if request.language != "auto" {
        form = form.text("language", request.language);
    }
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(CLOUD_REQUEST_TIMEOUT_SECS))
        .build()
        .context("Could not initialize cloud transcription client")?
        .post(cloud_transcription_url(&base_url)?)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .context("Cloud transcription request failed")?;
    if !response.status().is_success() {
        bail!(
            "Cloud transcription request failed (HTTP {}). Check the endpoint, model, and API key.",
            response.status()
        );
    }
    response
        .json::<CloudTranscriptionResponse>()
        .await
        .context("Cloud transcription returned an invalid response")
        .map(|response| response.text.trim().to_string())
}

/// Returns the providers that are safe to choose in this build. Entries are
/// added only with a concrete engine/transport implementation; unknown values
/// are rejected instead of falling back and changing the user's privacy choice.
pub fn available_providers() -> Vec<DictationProviderInfo> {
    vec![
        DictationProviderInfo::local_whisper(),
        DictationProviderInfo::cloud_openai_compatible(),
    ]
}

/// Exposes only implementation-backed provider choices to the settings UI.
#[tauri::command]
pub fn list_dictation_providers() -> Vec<DictationProviderInfo> {
    available_providers()
}

pub fn provider_info(id: &str) -> Result<DictationProviderInfo> {
    available_providers()
        .into_iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| {
            anyhow::anyhow!("Dictation provider '{id}' is not available in this version of Flick")
        })
}

/// Route transcription through the configured provider. No fallback is used.
pub async fn transcribe(
    provider_id: String,
    request: TranscriptionRequest,
    cloud_base_url: String,
    cloud_model: String,
) -> Result<String> {
    match provider_id.as_str() {
        LOCAL_WHISPER_PROVIDER_ID => {
            let path = request
                .local_model_path
                .as_ref()
                .context("Choose and download a local speech model before dictating")?;
            crate::transcription_worker::transcribe(
                &request.audio,
                path,
                &request.language,
                request.translate_to_english,
            )
            .await
        }
        CLOUD_OPENAI_COMPATIBLE_PROVIDER_ID => {
            transcribe_cloud_openai_compatible(request, cloud_base_url, cloud_model).await
        }
        unsupported => {
            bail!("Dictation provider '{unsupported}' is not available in this version of Flick")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_whisper_is_the_private_default_provider() {
        let whisper = provider_info(LOCAL_WHISPER_PROVIDER_ID).unwrap();
        assert_eq!(whisper.kind, DictationProviderKind::Local);
        assert!(!whisper.sends_audio_off_device);
        assert!(whisper.requires_local_model);
    }

    #[test]
    fn cloud_provider_is_explicit_about_audio_transfer() {
        let cloud = provider_info(CLOUD_OPENAI_COMPATIBLE_PROVIDER_ID).unwrap();
        assert_eq!(cloud.kind, DictationProviderKind::Cloud);
        assert!(cloud.sends_audio_off_device);
        assert!(!cloud.supports_translation);
        assert!(!cloud.requires_local_model);
    }

    #[test]
    fn cloud_endpoint_rejects_insecure_non_local_urls() {
        assert!(cloud_transcription_url("http://example.com/v1").is_err());
        assert!(cloud_transcription_url("http://localhost:8080/v1").is_ok());
        assert_eq!(
            cloud_transcription_url("https://api.example.com/v1/")
                .unwrap()
                .as_str(),
            "https://api.example.com/v1/audio/transcriptions"
        );
    }

    #[test]
    fn wav_payload_is_a_16khz_mono_file() {
        let bytes = wav_bytes(&[0.0, 0.5, -0.5]).unwrap();
        let reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
    }

    #[test]
    fn unavailable_provider_never_falls_back() {
        assert!(provider_info("not-a-provider").is_err());
    }
}
