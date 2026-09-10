//! Local speech-model catalog and verified downloader.
//!
//! Binaries are downloaded only from the explicit catalog below, written to a
//! temporary file, hashed while streaming, then atomically renamed. A failed
//! or cancelled download can therefore never masquerade as an installed model.

use anyhow::{bail, Context, Result};
use futures_util::{FutureExt, StreamExt};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Cancellation state is deliberately process-local. Downloaded model bytes
/// never leave the app data folder and a cancelled partial file is retained
/// only for a future verified resume.
pub struct ModelDownloadState {
    active: Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
}

impl Default for ModelDownloadState {
    fn default() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub engine: String,
    pub supports_translation: bool,
    pub supports_language_detection: bool,
    pub supported_languages: Vec<String>,
    pub size_bytes: u64,
    /// The binary is present but has not yet been verified by Flick. This is
    /// deliberately separate from `installed`: opening Models must never read
    /// gigabytes of data just to render a settings page.
    pub available_locally: bool,
    pub installed: bool,
    pub active: bool,
}

struct CatalogModel {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    language: &'static str,
    file_name: &'static str,
    url: &'static str,
    sha256: &'static str,
    size_bytes: u64,
    english_only: bool,
}

// The checksums are Git LFS object IDs published by the model repository.
// Keep this catalog deliberately small for the first-run experience; users may
// also put compatible ggml models in Flick's models directory.
const CATALOG: &[CatalogModel] = &[CatalogModel {
    id: "whisper-tiny-en",
    name: "Whisper Tiny English",
    description: "Fastest local English dictation model; ideal for testing and low-end devices.",
    language: "English",
    file_name: "ggml-tiny.en.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-tiny.en.bin?download=true",
    sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
    size_bytes: 77_704_715,
    english_only: true,
}, CatalogModel {
    id: "whisper-base-multilingual",
    name: "Whisper Base Multilingual",
    description: "Balanced local dictation and English translation across supported languages.",
    language: "Multilingual",
    file_name: "ggml-base.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.bin?download=true",
    sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    size_bytes: 147_951_465,
    english_only: false,
}, CatalogModel {
    id: "whisper-base-en",
    name: "Whisper Base English",
    description: "Balanced local English dictation model for most laptops.",
    language: "English",
    file_name: "ggml-base.en.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en.bin?download=true",
    sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    size_bytes: 147_964_211,
    english_only: true,
}, CatalogModel {
    id: "whisper-small-multilingual",
    name: "Whisper Small Multilingual",
    description: "Higher-accuracy local dictation and English translation across supported languages.",
    language: "Multilingual",
    file_name: "ggml-small.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small.bin?download=true",
    sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    size_bytes: 487_601_967,
    english_only: false,
}, CatalogModel {
    id: "whisper-small-en",
    name: "Whisper Small English",
    description: "Higher-accuracy local English dictation; needs more memory and CPU.",
    language: "English",
    file_name: "ggml-small.en.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small.en.bin?download=true",
    sha256: "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
    size_bytes: 487_614_201,
    english_only: true,
}, CatalogModel {
    id: "whisper-tiny-multilingual",
    name: "Whisper Tiny Multilingual",
    description: "Fast local dictation and English translation across supported languages.",
    language: "Multilingual",
    file_name: "ggml-tiny.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-tiny.bin?download=true",
    sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    size_bytes: 77_691_713,
    english_only: false,
}, CatalogModel {
    id: "whisper-medium-multilingual",
    name: "Whisper Medium Multilingual",
    description: "High-accuracy multilingual dictation; requires substantial memory and CPU/GPU capacity.",
    language: "Multilingual",
    file_name: "ggml-medium.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-medium.bin?download=true",
    sha256: "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
    size_bytes: 1_533_763_059,
    english_only: false,
}, CatalogModel {
    id: "whisper-large-v3-turbo",
    name: "Whisper Large v3 Turbo",
    description: "Fast, high-accuracy multilingual dictation for powerful computers.",
    language: "Multilingual",
    file_name: "ggml-large-v3-turbo.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-large-v3-turbo.bin?download=true",
    sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
    size_bytes: 1_624_555_275,
    english_only: false,
}, CatalogModel {
    id: "whisper-large-v3",
    name: "Whisper Large v3",
    description: "Highest-accuracy multilingual local dictation; requires substantial memory and compute.",
    language: "Multilingual",
    file_name: "ggml-large-v3.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-large-v3.bin?download=true",
    sha256: "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
    size_bytes: 3_095_033_483,
    english_only: false,
}, CatalogModel {
    // Pinned to the catalog revision Handy uses. This is a Hugging Face model,
    // not an unverified redirect: Flick still streams, hashes, and atomically
    // promotes the exact GGUF file before it may be selected.
    id: "parakeet-tdt-0.6b-v3-q8",
    name: "Parakeet TDT 0.6B v3",
    description: "Fast, accurate local dictation across 25 European languages. Translation to English is not supported by this model.",
    language: "25 European languages",
    file_name: "parakeet-tdt-0.6b-v3-Q8_0.gguf",
    url: "https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/85ac09ea12fc4b1112fa76810059364bc6adc9de/parakeet-tdt-0.6b-v3-Q8_0.gguf?download=true",
    sha256: "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7",
    size_bytes: 739_508_576,
    english_only: false,
}, CatalogModel {
    // Pinned catalog entries are imported from Handy's verified GGUF registry.
    // Each remains independently streamed, hashed, and atomically promoted by
    // Flick; the source catalog is metadata, never a trust bypass.
    id: "canary-180m-flash-q8",
    name: "Canary 180M Flash",
    description: "Small, fast English, German, Spanish, and French dictation with verified English translation.",
    language: "English, German, Spanish, French",
    file_name: "canary-180m-flash-Q8_0.gguf",
    url: "https://huggingface.co/handy-computer/canary-180m-flash-gguf/resolve/b147f9dc52b59f0998e410540a84727bd86457fd/canary-180m-flash-Q8_0.gguf?download=true",
    sha256: "e13c7f5d0952b056a027cfffec13e3a3a134d1608babed24f983568f141e297c",
    size_bytes: 218_447_552,
    english_only: false,
}, CatalogModel {
    id: "qwen3-asr-0.6b-q8",
    name: "Qwen3-ASR 0.6B",
    description: "Accurate 30-language dictation with automatic language detection; transcription only.",
    language: "30 languages",
    file_name: "Qwen3-ASR-0.6B-Q8_0.gguf",
    url: "https://huggingface.co/handy-computer/Qwen3-ASR-0.6B-gguf/resolve/e4e16599b900eb0cb36e524514756bb92eb092b7/Qwen3-ASR-0.6B-Q8_0.gguf?download=true",
    sha256: "f081b2d5e23bd669d92cc331d722a8a0681943b8e6f34b48996fd5c319b5acd8",
    size_bytes: 850_423_456,
    english_only: false,
}, CatalogModel {
    id: "sensevoice-small-q8",
    name: "SenseVoice Small",
    description: "Fast Chinese, Cantonese, English, Japanese, and Korean dictation with automatic language detection.",
    language: "Chinese, Cantonese, English, Japanese, Korean",
    file_name: "SenseVoiceSmall-Q8_0.gguf",
    url: "https://huggingface.co/handy-computer/SenseVoiceSmall-gguf/resolve/4a08b8e900b38a977e32eb08d5d0697d6e72ba04/SenseVoiceSmall-Q8_0.gguf?download=true",
    sha256: "6c759ee4c9748c9b3f7a5a60ca74f0f7e685fb9d45d1378fce7cfd62f59adf29",
    size_bytes: 252_684_608,
    english_only: false,
}, CatalogModel {
    id: "moonshine-tiny-q8",
    name: "Moonshine Tiny",
    description: "Ultra-lightweight English dictation for quick, offline capture; transcription only.",
    language: "English",
    file_name: "moonshine-tiny-Q8_0.gguf",
    url: "https://huggingface.co/handy-computer/moonshine-tiny-gguf/resolve/f5c11906eba3f44cf305eed30feb9cbfb0b4b9d0/moonshine-tiny-Q8_0.gguf?download=true",
    sha256: "2fd348d7b38f97d309cc3ec6848f3f57f537b80244950f07d2637e463f95a3a1",
    size_bytes: 35_466_912,
    english_only: true,
}];

fn catalog_model(id: &str) -> Result<&'static CatalogModel> {
    CATALOG
        .iter()
        .find(|model| model.id == id)
        .ok_or_else(|| anyhow::anyhow!("Unknown model '{id}'"))
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .context("Failed to resolve application data directory")?
        .join("models");
    std::fs::create_dir_all(&dir).context("Failed to create models directory")?;
    Ok(dir)
}

pub fn model_path(app: &AppHandle, id: &str) -> Result<PathBuf> {
    let model = catalog_model(id)?;
    Ok(models_dir(app)?.join(model.file_name))
}

fn custom_file_name(id: &str) -> Option<&str> {
    let name = id.strip_prefix("custom:")?;
    (!name.is_empty()
        && !name.contains(['/', '\\'])
        && std::path::Path::new(name)
            .extension()
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("bin") || extension.eq_ignore_ascii_case("gguf")
            }))
    .then_some(name)
}

fn custom_model_path(app: &AppHandle, id: &str) -> Result<Option<PathBuf>> {
    Ok(custom_file_name(id)
        .map(|name| models_dir(app).map(|directory| directory.join(name)))
        .transpose()?
        .filter(|path| path.is_file()))
}

pub fn installed_model_path(app: &AppHandle) -> Option<PathBuf> {
    CATALOG.iter().find_map(|model| {
        let path = models_dir(app).ok()?.join(model.file_name);
        path.is_file().then_some(path)
    })
}

/// Return only a catalog model whose on-disk bytes still match the published
/// digest. Dictation calls this immediately before native model loading.
pub async fn verified_installed_model_path(app: &AppHandle) -> Result<Option<PathBuf>> {
    let config = crate::config::load_config(app)?;
    verified_configured_model_path(app, &config).await
}

/// Resolve the model selected when capture began. Reading the latest settings
/// here could silently change the model or privacy choice of an active session.
pub async fn verified_configured_model_path(
    app: &AppHandle,
    config: &crate::config::FlickConfig,
) -> Result<Option<PathBuf>> {
    let id = &config.dictation_model_id;
    if let Some(model) = CATALOG.iter().find(|model| model.id == id.as_str()) {
        let path = models_dir(app)?.join(model.file_name);
        let is_recorded = config.local_models.iter().any(|saved| {
            saved.id == model.id
                && saved.file_name == model.file_name
                && saved.size_bytes == model.size_bytes
                && saved.sha256 == model.sha256
                && saved.installed
        });
        // We checksum before recording a model and before an explicit model
        // change. Subsequent dictations only make a fast metadata check.
        if is_recorded
            && path
                .metadata()
                .is_ok_and(|metadata| metadata.len() == model.size_bytes)
        {
            return Ok(Some(path));
        }
    }
    verified_model_path(app, id).await
}

async fn verified_model_path(app: &AppHandle, id: &str) -> Result<Option<PathBuf>> {
    if custom_file_name(id).is_some() {
        return custom_model_path(app, id);
    }
    let model = catalog_model(id)?;
    let path = models_dir(app)?.join(model.file_name);
    Ok((path.is_file() && verify_file(&path, model.sha256).await?).then_some(path))
}

/// Register one model transfer at a time. Local speech models are large enough
/// that competing downloads make the app appear unresponsive and can exhaust
/// disk space on a laptop. A later queue can replace this guard without
/// weakening the single, well-defined active-transfer lifecycle.
fn begin_model_download(
    state: &ModelDownloadState,
    id: &str,
) -> Result<Arc<std::sync::atomic::AtomicBool>, String> {
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut active = state
        .active
        .lock()
        .map_err(|_| "Model download state is unavailable".to_string())?;
    if active.contains_key(id) {
        return Err("This model is already downloading".to_string());
    }
    if !active.is_empty() {
        return Err(
            "Another model is downloading. Finish or cancel it before starting another."
                .to_string(),
        );
    }
    active.insert(id.to_string(), Arc::clone(&cancel));
    Ok(cancel)
}

/// Translation is opt-in per pinned catalog artifact. Custom models are
/// deliberately conservative: Flick will transcribe them locally but never
/// sends a translation task unless a catalog entry verifies the capability.
pub fn model_supports_translation(id: &str) -> Result<bool> {
    if custom_file_name(id).is_some() {
        return Ok(false);
    }
    let model = catalog_model(id)?;
    Ok((model.id.starts_with("whisper-") && !model.english_only)
        || model.id == "canary-180m-flash-q8")
}

// Kept alongside the pinned artifacts rather than inferred from display labels
// or file extensions. The language selector is not a promise that every
// language or automatic detection mode is safe to send to every engine.
const PARAKEET_V3_LANGUAGES: &[&str] = &[
    "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv", "lt", "mt",
    "pl", "pt", "ro", "sk", "sl", "es", "sv", "ru", "uk",
];

const CANARY_180M_LANGUAGES: &[&str] = &["en", "de", "es", "fr"];
const QWEN3_ASR_LANGUAGES: &[&str] = &[
    "zh", "en", "yue", "ar", "de", "fr", "es", "pt", "id", "it", "ko", "ru", "th", "vi", "ja",
    "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cs", "fil", "fa", "el", "ro", "hu", "mk",
];
const SENSEVOICE_LANGUAGES: &[&str] = &["zh", "yue", "en", "ja", "ko"];

fn catalog_supported_languages(model: &CatalogModel) -> &'static [&'static str] {
    match model.id {
        "parakeet-tdt-0.6b-v3-q8" => PARAKEET_V3_LANGUAGES,
        "canary-180m-flash-q8" => CANARY_180M_LANGUAGES,
        "qwen3-asr-0.6b-q8" => QWEN3_ASR_LANGUAGES,
        "sensevoice-small-q8" => SENSEVOICE_LANGUAGES,
        _ if model.english_only => &["en"],
        _ => &[],
    }
}

fn catalog_supports_language_detection(model: &CatalogModel) -> bool {
    !model.english_only && model.id != "canary-180m-flash-q8"
}

fn catalog_engine(model: &CatalogModel) -> &'static str {
    if model.file_name.ends_with(".gguf") {
        match model.id {
            "parakeet-tdt-0.6b-v3-q8" => "Parakeet / GGUF",
            "canary-180m-flash-q8" => "Canary / GGUF",
            "qwen3-asr-0.6b-q8" => "Qwen3 ASR / GGUF",
            "sensevoice-small-q8" => "SenseVoice / GGUF",
            "moonshine-tiny-q8" => "Moonshine / GGUF",
            _ => "Compatible GGUF",
        }
    } else {
        "Whisper / GGML"
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalModelCapabilities {
    pub id: String,
    pub supports_translation: bool,
    pub supports_language_detection: bool,
    /// Empty means the catalog has no finite language restriction to present
    /// (multilingual Whisper) or Flick cannot verify a custom model's set.
    pub supported_languages: Vec<String>,
}

pub fn local_model_capabilities(id: &str) -> Result<LocalModelCapabilities> {
    if custom_file_name(id).is_some() {
        return Ok(LocalModelCapabilities {
            id: id.to_string(),
            supports_translation: false,
            // A user-provided GGML/GGUF file may be any supported engine
            // architecture. Never pass an automatic-detection instruction to
            // an artifact whose capabilities Flick cannot verify; callers can
            // still choose an explicit language when they know the model.
            supports_language_detection: false,
            supported_languages: Vec::new(),
        });
    }
    let model = catalog_model(id)?;
    let supported_languages = catalog_supported_languages(model)
        .iter()
        .map(|language| (*language).to_string())
        .collect();
    Ok(LocalModelCapabilities {
        id: id.to_string(),
        supports_translation: model_supports_translation(id)?,
        supports_language_detection: catalog_supports_language_detection(model),
        supported_languages,
    })
}

pub fn model_supports_language(id: &str, language: &str) -> Result<bool> {
    let capabilities = local_model_capabilities(id)?;
    if language == "auto" {
        return Ok(capabilities.supports_language_detection);
    }
    Ok(capabilities.supported_languages.is_empty()
        || capabilities
            .supported_languages
            .iter()
            .any(|supported| supported == language))
}

#[tauri::command]
pub async fn active_local_model_capabilities(
    app: AppHandle,
) -> Result<LocalModelCapabilities, String> {
    let config = crate::config::load_config(&app).map_err(|error| error.to_string())?;
    local_model_capabilities(&config.dictation_model_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_local_models(app: AppHandle) -> Result<Vec<ModelInfo>, String> {
    let config = crate::config::load_config(&app).map_err(|error| error.to_string())?;
    let active_id = config.dictation_model_id;
    let directory = models_dir(&app).map_err(|error| error.to_string())?;
    let mut models = Vec::with_capacity(CATALOG.len());
    for model in CATALOG {
        let path = directory.join(model.file_name);
        let capabilities = local_model_capabilities(model.id).map_err(|error| error.to_string())?;
        // Do not hash every installed model here. Large Whisper models are
        // multiple gigabytes and doing that on every visit made the Models tab
        // look frozen (and could exhaust a small machine's IO budget). A model
        // becomes trusted only after download or an explicit Verify & use
        // action; both paths run the full SHA-256 verification.
        let available_locally = path
            .metadata()
            .map(|metadata| metadata.len() == model.size_bytes)
            .unwrap_or(false);
        let installed = available_locally
            && config.local_models.iter().any(|saved| {
                saved.id == model.id
                    && saved.file_name == model.file_name
                    && saved.size_bytes == model.size_bytes
                    && saved.sha256 == model.sha256
                    && saved.installed
            });
        models.push(ModelInfo {
            id: model.id.to_string(),
            name: model.name.to_string(),
            description: model.description.to_string(),
            language: model.language.to_string(),
            engine: catalog_engine(model).to_string(),
            supports_translation: capabilities.supports_translation,
            supports_language_detection: capabilities.supports_language_detection,
            supported_languages: capabilities.supported_languages,
            size_bytes: model.size_bytes,
            available_locally,
            installed,
            active: installed && active_id == model.id,
        });
    }
    let catalog_files: std::collections::HashSet<&str> =
        CATALOG.iter().map(|model| model.file_name).collect();
    if let Ok(entries) = std::fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if path.is_file()
                && path.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("bin") || extension.eq_ignore_ascii_case("gguf")
                })
                && !catalog_files.contains(name)
            {
                models.push(ModelInfo {
                    id: format!("custom:{name}"),
                    name: format!("Custom local model: {name}"),
                    description: "User-provided local GGML/GGUF model. Flick never uploads it; compatibility is checked when it is loaded.".into(),
                    language: "User supplied".into(),
                    engine: if name.ends_with(".gguf") {
                        "Compatible GGUF".into()
                    } else {
                        "Compatible GGML".into()
                    },
                    supports_translation: false,
                    supports_language_detection: false,
                    supported_languages: Vec::new(),
                    size_bytes: entry.metadata().map(|metadata| metadata.len()).unwrap_or(0),
                    available_locally: true,
                    installed: true,
                    active: active_id == format!("custom:{name}"),
                });
            }
        }
    }
    Ok(models)
}

#[tauri::command]
pub async fn set_active_local_model(app: AppHandle, id: String) -> Result<(), String> {
    if verified_model_path(&app, &id)
        .await
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Err("Download and verify this model before selecting it.".to_string());
    }
    crate::config::update_config(&app, |config| {
        if custom_file_name(&id).is_none() {
            remember_verified_model(config, &id)?;
        }
        // Capability changes must not leave a previously valid language or
        // translation selection in a state that fails on the next dictation.
        if !model_supports_translation(&id)? {
            config.dictation_translate_to_english = false;
        }
        if !model_supports_language(&id, &config.dictation_language)? {
            config.dictation_language =
                if local_model_capabilities(&id)?.supports_language_detection {
                    "auto".to_string()
                } else {
                    "en".to_string()
                };
        }
        config.dictation_model_id = id.clone();
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn download_local_model(app: AppHandle, id: String) -> Result<(), String> {
    // Reject malformed or unknown IDs before changing the download state. This
    // keeps an invalid IPC request from leaving the UI in a waiting state.
    catalog_model(&id).map_err(|error| error.to_string())?;
    let state = app
        .try_state::<ModelDownloadState>()
        .ok_or_else(|| "Model downloader is still starting. Please try again.".to_string())?;
    let cancel = begin_model_download(&state, &id)?;
    let _ = app.emit(
        "flick://model-download",
        serde_json::json!({ "id": id.clone(), "state": "started" }),
    );
    // Downloads can last minutes and may fail for reasons outside Flick's
    // control. Run them outside the IPC request so a network or native-library
    // failure cannot close the settings webview or leave its buttons stuck.
    tauri::async_runtime::spawn(async move {
        // The release profile unwinds panics, but a recovered task must still
        // clear its state and notify the webview rather than strand a button
        // in its progress state. Normal network and integrity errors keep
        // their original, actionable messages below.
        let result = AssertUnwindSafe(download_model_with_cancel(&app, &id, &cancel))
            .catch_unwind()
            .await;
        if let Some(active) = app.try_state::<ModelDownloadState>() {
            if let Ok(mut downloads) = active.active.lock() {
                downloads.remove(&id);
            }
        }
        let payload = match result {
            Ok(Ok(())) => serde_json::json!({ "id": id, "state": "complete" }),
            Ok(Err(error)) => {
                serde_json::json!({ "id": id, "state": "failed", "message": error.to_string() })
            }
            Err(_) => serde_json::json!({
                "id": id,
                "state": "failed",
                "message": "Flick recovered from an unexpected download failure. No model was installed; please retry."
            }),
        };
        let _ = app.emit("flick://model-download", payload);
    });
    Ok(())
}

#[tauri::command]
pub fn cancel_local_model_download(app: AppHandle, id: String) -> Result<(), String> {
    let state = app
        .try_state::<ModelDownloadState>()
        .ok_or_else(|| "Model downloader is not available.".to_string())?;
    let active = state
        .active
        .lock()
        .map_err(|_| "Model download state is unavailable".to_string())?;
    let Some(cancel) = active.get(&id) else {
        return Err("No active download for this model".to_string());
    };
    cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// Return the one protected download currently owned by this Flick process.
/// The models view may be closed while a large file is transferring; exposing
/// this small piece of lifecycle state lets a reopened view recover its
/// Cancel affordance instead of inviting the user to start a conflicting
/// request.
#[tauri::command]
pub fn active_local_model_download(app: AppHandle) -> Result<Option<String>, String> {
    let state = app
        .try_state::<ModelDownloadState>()
        .ok_or_else(|| "Model downloader is still starting. Please try again.".to_string())?;
    let downloads = state
        .active
        .lock()
        .map_err(|_| "Model download state is unavailable".to_string())?;
    Ok(downloads.keys().next().cloned())
}

pub async fn download_model(app: &AppHandle, id: &str) -> Result<()> {
    download_model_with_cancel(app, id, &std::sync::atomic::AtomicBool::new(false)).await
}

async fn download_model_with_cancel(
    app: &AppHandle,
    id: &str,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let model = catalog_model(id)?;
    let destination = model_path(app, id)?;
    if destination.is_file()
        && until_download_cancelled(verify_file(&destination, model.sha256), cancelled).await??
    {
        check_download_cancelled(cancelled)?;
        mark_catalog_model_verified(app, id)?;
        return Ok(());
    }

    let temporary = destination.with_extension("partial");
    let mut existing = tokio::fs::metadata(&temporary)
        .await
        .map(|meta| meta.len())
        .unwrap_or(0);
    // A complete verified temporary file can be left behind only by a crash
    // between hashing and rename. Promote it without doing another network
    // request. Impossible-size partial files cannot be resumed safely.
    if existing == model.size_bytes
        && until_download_cancelled(verify_file(&temporary, model.sha256), cancelled).await??
    {
        check_download_cancelled(cancelled)?;
        finalize_verified_download(&temporary, &destination).await?;
        mark_catalog_model_verified(app, id)?;
        return Ok(());
    }
    if existing >= model.size_bytes {
        tokio::fs::remove_file(&temporary)
            .await
            .context("Could not discard invalid partial model")?;
        existing = 0;
    }
    check_download_cancelled(cancelled)?;
    // A large model may legitimately take hours. Limit connection and idle
    // reads, not total download duration, and allow Cancel during both waits.
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()
        .context("Could not initialize model download client")?;
    let response = until_download_cancelled(
        client
            .get(model.url)
            .header(reqwest::header::RANGE, format!("bytes={existing}-"))
            .send(),
        cancelled,
    )
    .await?
    .context("Could not start model download")?
    .error_for_status()
    .context("Model server rejected the download")?;
    let resumed_bytes = download_resume_offset(
        existing,
        model.size_bytes,
        response.status(),
        response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok()),
        response.content_length(),
    )?;
    let resuming = resumed_bytes > 0;
    let total = model.size_bytes;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resuming)
        .truncate(!resuming)
        .open(&temporary)
        .await
        .context("Could not create temporary model file")?;
    let mut hasher = if resuming {
        until_download_cancelled(hash_file(&temporary), cancelled).await??
    } else {
        Sha256::new()
    };
    let mut received = resumed_bytes;

    while let Some(chunk) = until_download_cancelled(stream.next(), cancelled).await? {
        let chunk = chunk.context("Model download was interrupted")?;
        let next_received = received
            .checked_add(chunk.len() as u64)
            .filter(|received| *received <= total)
            .context("Model server sent more bytes than the verified model size. Partial download can be resumed.")?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .context("Could not write model data")?;
        received = next_received;
        let _ = app.emit(
            "flick://model-download",
            serde_json::json!({ "id": id, "received": received, "total": total }),
        );
    }
    file.flush()
        .await
        .context("Could not finish model download")?;
    // Release the handle before removing/renaming the file on Windows.
    drop(file);
    check_download_cancelled(cancelled)?;
    if received != total {
        bail!("Model download ended early. Partial download can be resumed.");
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != model.sha256 {
        let _ = tokio::fs::remove_file(&temporary).await;
        bail!("Model integrity check failed. The download was discarded.");
    }
    check_download_cancelled(cancelled)?;
    finalize_verified_download(&temporary, &destination).await?;
    mark_catalog_model_verified(app, id)?;
    Ok(())
}

/// Persist the fact that a catalog file passed a complete checksum. This lets
/// the UI stay instant on later visits without weakening the verification gate
/// before a model is selected or loaded.
fn mark_catalog_model_verified(app: &AppHandle, id: &str) -> Result<()> {
    crate::config::update_config(app, |config| remember_verified_model(config, id))?;
    Ok(())
}

fn remember_verified_model(config: &mut crate::config::FlickConfig, id: &str) -> Result<()> {
    let model = catalog_model(id)?;
    let record = crate::config::LocalModel {
        id: model.id.to_string(),
        name: model.name.to_string(),
        file_name: model.file_name.to_string(),
        size_bytes: model.size_bytes,
        sha256: model.sha256.to_string(),
        installed: true,
    };
    if let Some(existing) = config
        .local_models
        .iter_mut()
        .find(|saved| saved.id == model.id)
    {
        *existing = record;
    } else {
        config.local_models.push(record);
    }
    Ok(())
}

/// Rename replaces an existing file on Windows and Unix. Do not delete the
/// destination first: a failed promotion must leave the old file intact.
async fn finalize_verified_download(temporary: &Path, destination: &Path) -> Result<()> {
    tokio::fs::rename(&temporary, &destination)
        .await
        .context("Could not finalize model download")?;
    Ok(())
}

fn check_download_cancelled(cancelled: &std::sync::atomic::AtomicBool) -> Result<()> {
    if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        bail!("Model download cancelled; partial download can be resumed");
    }
    Ok(())
}

/// Race waits against cancellation even when the server never sends headers or
/// the next body chunk. Dropping an unfinished request closes that transfer.
async fn until_download_cancelled<T>(
    operation: impl std::future::Future<Output = T>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<T> {
    let cancellation = async {
        while !cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    tokio::select! {
        biased;
        _ = cancellation => bail!("Model download cancelled; partial download can be resumed"),
        value = operation => { check_download_cancelled(cancelled)?; Ok(value) }
    }
}

/// A 206 status alone cannot prove the bytes are the requested suffix. Check
/// the range against both our partial file and the pinned catalog size before
/// opening the file for append. A server ignoring Range (200) restarts safely.
fn download_resume_offset(
    existing: u64,
    expected_size: u64,
    status: reqwest::StatusCode,
    content_range: Option<&str>,
    content_length: Option<u64>,
) -> Result<u64> {
    let offset = match status {
        reqwest::StatusCode::OK => 0,
        reqwest::StatusCode::PARTIAL_CONTENT => {
            let range = content_range
                .and_then(|range| range.strip_prefix("bytes "))
                .and_then(|range| range.split_once('/'))
                .and_then(|(range, size)| {
                    let (start, end) = range.split_once('-')?;
                    Some((
                        start.parse::<u64>().ok()?,
                        end.parse::<u64>().ok()?,
                        size.parse::<u64>().ok()?,
                    ))
                });
            if !matches!(range, Some((start, end, size))
                if start == existing && start <= end && end.checked_add(1) == Some(size) && size == expected_size)
            {
                bail!("Model server returned an invalid byte range. Partial download was preserved; retry the download.");
            }
            existing
        }
        _ => bail!("Model server returned an unexpected download response ({status})."),
    };
    if content_length.is_some_and(|length| Some(length) != expected_size.checked_sub(offset)) {
        bail!("Model server returned an unexpected model size. Partial download was preserved; retry the download.");
    }
    Ok(offset)
}

async fn verify_file(path: &Path, expected: &str) -> Result<bool> {
    Ok(format!("{:x}", hash_file(path).await?.finalize()) == expected)
}

async fn hash_file(path: &Path) -> Result<Sha256> {
    let mut file = tokio::fs::File::open(path)
        .await
        .context("Could not read installed model")?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .await
            .context("Could not read model data")?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher)
}

#[tauri::command]
pub async fn delete_local_model(app: AppHandle, id: String) -> Result<(), String> {
    let path = if custom_file_name(&id).is_some() {
        custom_model_path(&app, &id).map_err(|error| error.to_string())?
    } else {
        Some(model_path(&app, &id).map_err(|error| error.to_string())?)
    };
    if let Some(path) = path.filter(|path| path.is_file()) {
        tokio::fs::remove_file(path)
            .await
            .map_err(|error| format!("Could not remove model: {error}"))?;
    }
    // The UI does not offer removal for the active model, but commands can be
    // invoked independently. Do not persist a dangling model reference after
    // a successful deletion; the default remains intentionally uninstalled
    // until the user chooses or downloads a model again.
    crate::config::update_config(&app, |config| {
        config.local_models.retain(|model| model.id != id);
        if config.dictation_model_id == id {
            config.dictation_model_id = crate::config::FlickConfig::default().dictation_model_id;
        }
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_download_state_allows_only_one_active_transfer() {
        let state = ModelDownloadState::default();
        let first = begin_model_download(&state, "whisper-tiny-en").expect("first transfer starts");
        assert!(!first.load(std::sync::atomic::Ordering::SeqCst));
        let second = begin_model_download(&state, "whisper-base-en");
        assert!(matches!(second, Err(message) if message.contains("Another model is downloading")));
        state
            .active
            .lock()
            .expect("download state lock")
            .remove("whisper-tiny-en");
        assert!(begin_model_download(&state, "whisper-base-en").is_ok());
    }

    #[test]
    fn catalog_has_valid_sha256() {
        let mut ids = std::collections::HashSet::new();
        for model in CATALOG {
            assert!(ids.insert(model.id), "duplicate model id: {}", model.id);
            assert_eq!(model.sha256.len(), 64);
            assert!(model
                .sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit()));
            assert!(model.url.starts_with("https://"));
            assert!(model.size_bytes > 1_000_000);
        }
    }

    #[test]
    fn catalog_offers_verified_multilingual_accuracy_tiers() {
        for id in [
            "whisper-tiny-multilingual",
            "whisper-base-multilingual",
            "whisper-small-multilingual",
            "whisper-medium-multilingual",
            "whisper-large-v3-turbo",
            "whisper-large-v3",
        ] {
            let model = catalog_model(id).expect("catalog model");
            assert!(!model.english_only, "{id} must support multilingual use");
            assert!(model.size_bytes > 1_000_000);
        }
    }

    #[test]
    fn parakeet_v3_is_pinned_and_cannot_claim_translation() {
        let model = catalog_model("parakeet-tdt-0.6b-v3-q8").expect("Parakeet catalog model");
        assert!(model.file_name.ends_with(".gguf"));
        assert!(model
            .url
            .contains("85ac09ea12fc4b1112fa76810059364bc6adc9de"));
        assert_eq!(model.size_bytes, 739_508_576);
        assert!(!model_supports_translation(model.id).unwrap());
    }

    #[test]
    fn custom_gguf_models_are_discoverable_but_never_assumed_to_translate() {
        assert_eq!(
            custom_file_name("custom:dictation.gguf"),
            Some("dictation.gguf")
        );
        assert_eq!(
            custom_file_name("custom:dictation.bin"),
            Some("dictation.bin")
        );
        assert_eq!(custom_file_name("custom:dictation.onnx"), None);
        assert!(!model_supports_translation("custom:dictation.gguf").unwrap());
        assert!(!model_supports_language("custom:dictation.gguf", "auto").unwrap());
    }

    #[test]
    fn model_capabilities_do_not_overstate_language_or_translation_support() {
        let parakeet = local_model_capabilities("parakeet-tdt-0.6b-v3-q8").unwrap();
        assert!(!parakeet.supports_translation);
        assert!(parakeet
            .supported_languages
            .iter()
            .any(|language| language == "es"));
        assert!(!model_supports_language("parakeet-tdt-0.6b-v3-q8", "hi").unwrap());

        assert!(!model_supports_translation("whisper-tiny-en").unwrap());
        assert!(model_supports_language("whisper-tiny-en", "en").unwrap());
        assert!(!model_supports_language("whisper-tiny-en", "fr").unwrap());
        assert!(model_supports_translation("whisper-base-multilingual").unwrap());
    }

    #[test]
    fn verified_gguf_families_keep_their_own_capability_boundaries() {
        let canary = local_model_capabilities("canary-180m-flash-q8").unwrap();
        assert!(canary.supports_translation);
        assert!(!canary.supports_language_detection);
        assert!(model_supports_language("canary-180m-flash-q8", "fr").unwrap());
        assert!(!model_supports_language("canary-180m-flash-q8", "auto").unwrap());

        let qwen = local_model_capabilities("qwen3-asr-0.6b-q8").unwrap();
        assert!(!qwen.supports_translation);
        assert!(qwen.supports_language_detection);
        assert!(model_supports_language("qwen3-asr-0.6b-q8", "hi").unwrap());
        assert!(model_supports_language("qwen3-asr-0.6b-q8", "auto").unwrap());

        assert!(model_supports_language("sensevoice-small-q8", "ja").unwrap());
        assert!(!model_supports_language("sensevoice-small-q8", "fr").unwrap());
        assert!(!model_supports_language("moonshine-tiny-q8", "auto").unwrap());
    }

    #[test]
    fn additional_gguf_artifacts_are_pinned_and_sized() {
        for (id, revision, size_bytes) in [
            (
                "canary-180m-flash-q8",
                "b147f9dc52b59f0998e410540a84727bd86457fd",
                218_447_552,
            ),
            (
                "qwen3-asr-0.6b-q8",
                "e4e16599b900eb0cb36e524514756bb92eb092b7",
                850_423_456,
            ),
            (
                "sensevoice-small-q8",
                "4a08b8e900b38a977e32eb08d5d0697d6e72ba04",
                252_684_608,
            ),
            (
                "moonshine-tiny-q8",
                "f5c11906eba3f44cf305eed30feb9cbfb0b4b9d0",
                35_466_912,
            ),
        ] {
            let model = catalog_model(id).expect("catalog model");
            assert!(model.file_name.ends_with(".gguf"));
            assert!(model.url.contains(revision));
            assert_eq!(model.size_bytes, size_bytes);
        }
    }

    #[tokio::test]
    async fn rejects_a_file_with_the_wrong_digest() {
        let path =
            std::env::temp_dir().join(format!("flick-model-integrity-{}.bin", std::process::id()));
        tokio::fs::write(&path, b"tampered-model")
            .await
            .expect("write test model");
        assert!(!verify_file(&path, CATALOG[0].sha256)
            .await
            .expect("verify test model"));
        tokio::fs::remove_file(path)
            .await
            .expect("remove test model");
    }

    #[test]
    fn resumes_only_when_the_server_confirms_the_exact_requested_suffix() {
        use reqwest::StatusCode;
        assert_eq!(
            download_resume_offset(
                42,
                100,
                StatusCode::PARTIAL_CONTENT,
                Some("bytes 42-99/100"),
                Some(58)
            )
            .unwrap(),
            42
        );
        assert_eq!(
            download_resume_offset(42, 100, StatusCode::OK, None, Some(100)).unwrap(),
            0
        );
        for range in [
            None,
            Some("bytes 0-99/100"),
            Some("bytes 42-89/100"),
            Some("bytes 42-99/101"),
            Some("bytes 42-99/*"),
            Some("bytes 42-18446744073709551615/100"),
        ] {
            assert!(
                download_resume_offset(42, 100, StatusCode::PARTIAL_CONTENT, range, None).is_err(),
                "accepted {range:?}"
            );
        }
        assert!(download_resume_offset(
            42,
            100,
            StatusCode::PARTIAL_CONTENT,
            Some("bytes 42-99/100"),
            Some(100)
        )
        .is_err());
        assert!(download_resume_offset(42, 100, StatusCode::NO_CONTENT, None, None).is_err());
    }

    #[tokio::test]
    async fn cancellation_interrupts_a_server_that_never_sends_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            signal.store(true, std::sync::atomic::Ordering::SeqCst);
            // Keep the socket open without headers. Cancellation must finish
            // the client independently of any network response or timeout.
            std::future::pending::<()>().await;
        });
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            until_download_cancelled(
                reqwest::Client::builder()
                    .no_proxy()
                    .build()
                    .unwrap()
                    .get(format!("http://{address}/model"))
                    .send(),
                &cancelled,
            ),
        )
        .await;
        server.abort();
        assert!(result
            .expect("Cancel must not wait for the server")
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
    }

    #[tokio::test]
    async fn cancellation_wins_over_an_already_ready_completion() {
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        let result = until_download_cancelled(std::future::ready(()), &cancelled).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn finalizes_a_verified_temporary_file_atomically() {
        let directory =
            std::env::temp_dir().join(format!("flick-model-finalize-{}", std::process::id()));
        tokio::fs::create_dir_all(&directory)
            .await
            .expect("create test directory");
        let temporary = directory.join("model.partial");
        let destination = directory.join("model.bin");
        tokio::fs::write(&temporary, b"verified bytes")
            .await
            .expect("write temporary model");
        tokio::fs::write(&destination, b"invalid bytes")
            .await
            .expect("write invalid destination");

        finalize_verified_download(&temporary, &destination)
            .await
            .expect("finalize model");
        assert!(!temporary.exists());
        assert_eq!(
            tokio::fs::read(&destination).await.expect("read model"),
            b"verified bytes"
        );
        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove test directory");
    }
}
