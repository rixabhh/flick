//! Local speech-model catalog and verified downloader.
//!
//! Binaries are downloaded only from the explicit catalog below, written to a
//! temporary file, hashed while streaming, then atomically renamed. A failed
//! or cancelled download can therefore never masquerade as an installed model.

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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
    pub size_bytes: u64,
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
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin?download=true",
    sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
    size_bytes: 77_704_715,
    english_only: true,
}, CatalogModel {
    id: "whisper-base-multilingual",
    name: "Whisper Base Multilingual",
    description: "Balanced local dictation and English translation across supported languages.",
    language: "Multilingual",
    file_name: "ggml-base.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin?download=true",
    sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    size_bytes: 147_951_465,
    english_only: false,
}, CatalogModel {
    id: "whisper-base-en",
    name: "Whisper Base English",
    description: "Balanced local English dictation model for most laptops.",
    language: "English",
    file_name: "ggml-base.en.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin?download=true",
    sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    size_bytes: 147_964_211,
    english_only: true,
}, CatalogModel {
    id: "whisper-small-multilingual",
    name: "Whisper Small Multilingual",
    description: "Higher-accuracy local dictation and English translation across supported languages.",
    language: "Multilingual",
    file_name: "ggml-small.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin?download=true",
    sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    size_bytes: 487_601_967,
    english_only: false,
}, CatalogModel {
    id: "whisper-small-en",
    name: "Whisper Small English",
    description: "Higher-accuracy local English dictation; needs more memory and CPU.",
    language: "English",
    file_name: "ggml-small.en.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin?download=true",
    sha256: "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
    size_bytes: 487_614_201,
    english_only: true,
}, CatalogModel {
    id: "whisper-tiny-multilingual",
    name: "Whisper Tiny Multilingual",
    description: "Fast local dictation and English translation across supported languages.",
    language: "Multilingual",
    file_name: "ggml-tiny.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin?download=true",
    sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    size_bytes: 77_691_713,
    english_only: false,
}, CatalogModel {
    id: "whisper-medium-multilingual",
    name: "Whisper Medium Multilingual",
    description: "High-accuracy multilingual dictation; requires substantial memory and CPU/GPU capacity.",
    language: "Multilingual",
    file_name: "ggml-medium.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin?download=true",
    sha256: "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
    size_bytes: 1_533_763_059,
    english_only: false,
}, CatalogModel {
    id: "whisper-large-v3-turbo",
    name: "Whisper Large v3 Turbo",
    description: "Fast, high-accuracy multilingual dictation for powerful computers.",
    language: "Multilingual",
    file_name: "ggml-large-v3-turbo.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin?download=true",
    sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
    size_bytes: 1_624_555_275,
    english_only: false,
}, CatalogModel {
    id: "whisper-large-v3",
    name: "Whisper Large v3",
    description: "Highest-accuracy multilingual local dictation; requires substantial memory and compute.",
    language: "Multilingual",
    file_name: "ggml-large-v3.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin?download=true",
    sha256: "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
    size_bytes: 3_095_033_483,
    english_only: false,
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
            .is_some_and(|extension| extension.eq_ignore_ascii_case("bin")))
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
    let id = crate::config::load_config(app)?.dictation_model_id;
    verified_model_path(app, &id).await
}

async fn verified_model_path(app: &AppHandle, id: &str) -> Result<Option<PathBuf>> {
    if custom_file_name(id).is_some() {
        return custom_model_path(app, id);
    }
    let model = catalog_model(id)?;
    let path = models_dir(app)?.join(model.file_name);
    Ok((path.is_file() && verify_file(&path, model.sha256).await?).then_some(path))
}

pub fn model_is_english_only(id: &str) -> Result<bool> {
    if custom_file_name(id).is_some() {
        return Ok(false);
    }
    Ok(catalog_model(id)?.english_only)
}

#[tauri::command]
pub async fn list_local_models(app: AppHandle) -> Result<Vec<ModelInfo>, String> {
    let active_id = crate::config::load_config(&app)
        .map_err(|error| error.to_string())?
        .dictation_model_id;
    let directory = models_dir(&app).map_err(|error| error.to_string())?;
    let mut models = Vec::with_capacity(CATALOG.len());
    for model in CATALOG {
        let path = directory.join(model.file_name);
        // A file with the right name is not an installed model until its
        // published digest verifies. This keeps an interrupted or manually
        // replaced binary out of both the UI's installed count and selection.
        let installed = path.is_file()
            && verify_file(&path, model.sha256)
                .await
                .map_err(|error| error.to_string())?;
        models.push(ModelInfo {
            id: model.id.to_string(),
            name: model.name.to_string(),
            description: model.description.to_string(),
            language: model.language.to_string(),
            size_bytes: model.size_bytes,
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
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("bin"))
                && !catalog_files.contains(name)
            {
                models.push(ModelInfo {
                    id: format!("custom:{name}"),
                    name: format!("Custom local model: {name}"),
                    description: "User-provided local model. Flick never uploads it; compatibility is checked when it is loaded.".into(),
                    language: "User supplied".into(),
                    size_bytes: entry.metadata().map(|metadata| metadata.len()).unwrap_or(0),
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
    let mut config = crate::config::load_config(&app).map_err(|error| error.to_string())?;
    config.dictation_model_id = id;
    crate::config::save_config(&app, &config).map_err(|error| error.to_string())?;
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut current) = state.config.lock() {
            *current = config;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn download_local_model(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<ModelDownloadState>();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let mut active = state
            .active
            .lock()
            .map_err(|_| "Model download state is unavailable".to_string())?;
        if active.contains_key(&id) {
            return Err("This model is already downloading".to_string());
        }
        active.insert(id.clone(), Arc::clone(&cancel));
    }
    let result = download_model_with_cancel(&app, &id, &cancel)
        .await
        .map_err(|error| error.to_string());
    if let Ok(mut active) = state.active.lock() {
        active.remove(&id);
    }
    result
}

#[tauri::command]
pub fn cancel_local_model_download(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<ModelDownloadState>();
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
    if destination.is_file() && verify_file(&destination, model.sha256).await? {
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
    if existing == model.size_bytes && verify_file(&temporary, model.sha256).await? {
        return finalize_verified_download(&temporary, &destination).await;
    }
    if existing >= model.size_bytes {
        tokio::fs::remove_file(&temporary)
            .await
            .context("Could not discard invalid partial model")?;
        existing = 0;
    }
    let response = reqwest::Client::new()
        .get(model.url)
        .header(reqwest::header::RANGE, format!("bytes={existing}-"))
        .send()
        .await
        .context("Could not start model download")?
        .error_for_status()
        .context("Model server rejected the download")?;
    let resuming = should_resume(existing, response.status());
    let resumed_bytes = if resuming { existing } else { 0 };
    let total = response
        .content_length()
        .unwrap_or(model.size_bytes.saturating_sub(resumed_bytes))
        + resumed_bytes;
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
        hash_file(&temporary).await?
    } else {
        Sha256::new()
    };
    let mut received = resumed_bytes;

    while let Some(chunk) = stream.next().await {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            bail!("Model download cancelled; partial download can be resumed");
        }
        let chunk = chunk.context("Model download was interrupted")?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .context("Could not write model data")?;
        received += chunk.len() as u64;
        let _ = app.emit(
            "flick://model-download",
            serde_json::json!({ "id": id, "received": received, "total": total }),
        );
    }
    file.flush()
        .await
        .context("Could not finish model download")?;
    let actual = format!("{:x}", hasher.finalize());
    if actual != model.sha256 {
        let _ = tokio::fs::remove_file(&temporary).await;
        bail!("Model integrity check failed. The download was discarded.");
    }
    finalize_verified_download(&temporary, &destination).await
}

/// Promote a verified temporary model file atomically. Windows does not
/// replace an existing destination on rename, so only an already-invalid
/// destination is removed after the replacement bytes have been validated.
async fn finalize_verified_download(temporary: &Path, destination: &Path) -> Result<()> {
    if destination.is_file() {
        tokio::fs::remove_file(&destination)
            .await
            .context("Could not replace invalid installed model")?;
    }
    tokio::fs::rename(&temporary, &destination)
        .await
        .context("Could not finalize model download")?;
    Ok(())
}

fn should_resume(existing: u64, status: reqwest::StatusCode) -> bool {
    existing > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT
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
    let mut config = crate::config::load_config(&app).map_err(|error| error.to_string())?;
    if config.dictation_model_id == id {
        config.dictation_model_id = crate::config::FlickConfig::default().dictation_model_id;
        crate::config::save_config(&app, &config).map_err(|error| error.to_string())?;
        if let Some(state) = app.try_state::<crate::AppState>() {
            if let Ok(mut current) = state.config.lock() {
                *current = config;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn resumes_only_when_the_server_confirms_a_byte_range() {
        assert!(should_resume(42, reqwest::StatusCode::PARTIAL_CONTENT));
        assert!(!should_resume(0, reqwest::StatusCode::PARTIAL_CONTENT));
        assert!(!should_resume(42, reqwest::StatusCode::OK));
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
