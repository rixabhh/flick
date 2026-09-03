//! Privacy-safe diagnostics for support and beta feedback.
//!
//! The bundle deliberately excludes credentials, clipboard data, drafts,
//! transcript history, and custom command contents.

use anyhow::{Context, Result};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
struct Diagnostics {
    generated_at: u64,
    app_version: &'static str,
    os: &'static str,
    architecture: &'static str,
    provider: String,
    model: String,
    dictation_model_id: String,
    history_enabled: bool,
    model_bytes: u64,
}

fn directory_size(path: &std::path::Path) -> u64 {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            entry
                .metadata()
                .map(|metadata| {
                    if metadata.is_dir() {
                        directory_size(&entry.path())
                    } else {
                        metadata.len()
                    }
                })
                .unwrap_or(0)
        })
        .sum()
}

#[tauri::command]
pub fn export_diagnostics(app: AppHandle) -> Result<String, String> {
    let config = crate::config::load_config(&app).map_err(|error| error.to_string())?;
    let generated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let model_dir = crate::models::models_dir(&app).map_err(|error| error.to_string())?;
    let bundle = Diagnostics {
        generated_at,
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        provider: config.provider,
        model: config.model,
        dictation_model_id: config.dictation_model_id,
        history_enabled: config.history_enabled,
        model_bytes: directory_size(&model_dir),
    };
    let directory = app
        .path()
        .app_data_dir()
        .context("Could not resolve app data directory")
        .map_err(|error| error.to_string())?
        .join("diagnostics");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = directory.join(format!("flick-diagnostics-{generated_at}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&bundle).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_nested_model_files() {
        let root = std::env::temp_dir().join(format!("flick-diagnostics-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).expect("create test directory");
        std::fs::write(root.join("one.bin"), [0u8; 3]).expect("write file");
        std::fs::write(root.join("nested/two.bin"), [0u8; 5]).expect("write nested file");
        assert_eq!(directory_size(&root), 8);
        std::fs::remove_dir_all(root).expect("remove test directory");
    }
}
