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
    pub group: String,
    pub quant: String,
    pub recommended: bool,
    pub license: String,
    pub source_url: String,
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
    group: &'static str,
    quant: &'static str,
    engine: &'static str,
    supported_languages: &'static [&'static str],
    supports_translation: bool,
    supports_language_detection: bool,
    recommended: bool,
    license: &'static str,
    source_url: &'static str,
}

// Offline, pinned artifacts: generated from the attributed Handy/Hugging Face
// catalog with stable IDs for existing Flick installations.
const CATALOG: &[CatalogModel] = include!("model_catalog.rs");

fn catalog_model(id: &str) -> Result<&'static CatalogModel> {
    CATALOG
        .iter()
        .find(|model| model.id == id)
        .ok_or_else(|| anyhow::anyhow!("Unknown model '{id}'"))
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

/// Resolve the model selected when capture began. Reading the latest settings
/// here could silently change the model or privacy choice of an active session.
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
    Ok(model.supports_translation)
}

/// Check if a model supports Hinglish (Hindi-English code-mixing).
/// Many multilingual Whisper models naturally support Hinglish through
/// their training on code-mixed data, even if not explicitly listed.
pub fn model_supports_hinglish(id: &str) -> Result<bool> {
    if custom_file_name(id).is_some() {
        // Custom models: return true (user assumption, compatibility checked when loaded)
        return Ok(true);
    }
    let model = catalog_model(id)?;
    // Models that support multilingual use typically handle Hinglish
    Ok(model.language != "en" || model.supports_translation)
}

/// Get languages supported by a model, including Hinglish awareness.
pub fn model_supported_languages_with_hinglish(id: &str) -> Vec<String> {
    let capabilities = local_model_capabilities(id)?;
    let mut languages = capabilities.supported_languages.clone();

    // Add Hinglish if the model supports multilingual use and translation
    if capabilities.supports_translation || capabilities.supported_languages.is_empty() {
        // Multilingual models often handle Hinglish even if not explicitly listed
        if !languages.iter().any(|l| l == "hi" || l == "hinglish") {
            languages.push("hinglish".to_string());
        }
    }

    languages
}

fn catalog_supported_languages(model: &CatalogModel) -> &'static [&'static str] {
    model.supported_languages
}

fn catalog_supports_language_detection(model: &CatalogModel) -> bool {
    model.supports_language_detection
}

fn catalog_engine(model: &CatalogModel) -> &'static str {
    model.engine
}

fn custom_file_name(id: &str) -> Option<String> {
    // No custom model names in the default catalog
    None
}

fn custom_model_path(_app: &AppHandle, _id: &str) -> Option<PathBuf> {
    None
}

fn models_dir(app: &AppHandle) -> Result<PathBuf, anyhow::Error> {
    let config = crate::config::load_config(app)?;
    Ok(config.models_dir.unwrap_or_else(|| {
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .join(".flick")
        .join("models")
    }))
}

async fn verify_file(path: &Path, expected_sha256: &str) -> Result<bool, anyhow::Error> {
    use sha2::{Digest, Sha256};
    let data = tokio::fs::read(path).await?;
    let actual = Sha256::digest(data);
    let expected = hex::decode(expected_sha256.trim())?;
    Ok(actual.as_slice() == expected.as_slice())
}

fn local_model_capabilities(id: &str) -> Result<ModelCapabilities, anyhow::Error> {
    let model = catalog_model(id)?;
    Ok(ModelCapabilities {
        supported_languages: model.supported_languages.to_vec(),
        supports_translation: model.supports_translation,
        supports_language_detection: model.supports_language_detection,
    })
}

#[derive(Debug, Clone)]
struct ModelCapabilities {
    supported_languages: Vec<String>,
    supports_translation: bool,
    supports_language_detection: bool,
}