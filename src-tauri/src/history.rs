//! Private, local-only activity history.

use anyhow::{Context, Result};
use arboard::Clipboard;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub created_at: i64,
    pub kind: String,
    pub text: String,
    pub saved: bool,
}

fn connection(app: &AppHandle) -> Result<Connection> {
    let directory = app
        .path()
        .app_data_dir()
        .context("Failed to resolve application data directory")?;
    std::fs::create_dir_all(&directory).context("Failed to create application data directory")?;
    let connection = Connection::open(directory.join("history.sqlite3"))
        .context("Failed to open local history")?;
    connection.execute_batch("CREATE TABLE IF NOT EXISTS history (id INTEGER PRIMARY KEY, created_at INTEGER NOT NULL, kind TEXT NOT NULL, text TEXT NOT NULL, saved INTEGER NOT NULL DEFAULT 0);")?;
    Ok(connection)
}

pub fn record(app: &AppHandle, kind: &str, text: &str) -> Result<()> {
    let enabled = app
        .try_state::<crate::AppState>()
        .and_then(|state| {
            state
                .config
                .lock()
                .ok()
                .map(|config| config.history_enabled)
        })
        .unwrap_or(false);
    if !enabled {
        return Ok(());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let connection = connection(app)?;
    connection.execute(
        "INSERT INTO history (created_at, kind, text) VALUES (?1, ?2, ?3)",
        params![timestamp, kind, text],
    )?;
    let limit = app
        .try_state::<crate::AppState>()
        .and_then(|state| state.config.lock().ok().map(|config| config.history_limit))
        .unwrap_or(100) as i64;
    retain_unsaved(&connection, limit)?;
    Ok(())
}

fn retain_unsaved(connection: &Connection, limit: i64) -> Result<()> {
    connection.execute(
        "DELETE FROM history WHERE id IN (SELECT id FROM history WHERE saved = 0 ORDER BY id DESC LIMIT -1 OFFSET ?1)",
        params![limit.max(1)],
    )?;
    Ok(())
}

pub fn latest_text(app: &AppHandle) -> Result<Option<String>> {
    let connection = connection(app)?;
    connection
        .query_row(
            "SELECT text FROM history ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to read latest history entry")
}

/// Copy the latest optional local-history result without reading or changing
/// the active application. This is shared by the tray, CLI, and global key.
pub fn copy_last_result(app: &AppHandle) -> Result<bool> {
    let Some(text) = latest_text(app)? else {
        return Ok(false);
    };
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .context("Could not write the clipboard")?;
    Ok(true)
}

#[tauri::command]
pub fn copy_history_entry(app: AppHandle, id: i64) -> Result<(), String> {
    let connection = connection(&app).map_err(|error| error.to_string())?;
    let text: String = connection
        .query_row(
            "SELECT text FROM history WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "History entry no longer exists.".to_string())?;
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .map_err(|error| format!("Could not write the clipboard: {error}"))
}

#[tauri::command]
pub fn get_history(
    app: AppHandle,
    limit: Option<usize>,
    query: Option<String>,
) -> Result<Vec<HistoryEntry>, String> {
    let connection = connection(&app).map_err(|error| error.to_string())?;
    let query = query.unwrap_or_default();
    let pattern = format!("%{}%", query.trim());
    let mut statement = connection
        .prepare("SELECT id, created_at, kind, text, saved FROM history WHERE ?1 = '' OR text LIKE ?2 COLLATE NOCASE ORDER BY id DESC LIMIT ?3")
        .map_err(|error| error.to_string())?;
    let entries = statement
        .query_map(
            params![query.trim(), pattern, limit.unwrap_or(100) as i64],
            |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    kind: row.get(2)?,
                    text: row.get(3)?,
                    saved: row.get::<_, i64>(4)? != 0,
                })
            },
        )
        .map_err(|error| error.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    Ok(entries)
}

#[tauri::command]
pub fn set_history_saved(app: AppHandle, id: i64, saved: bool) -> Result<(), String> {
    let connection = connection(&app).map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE history SET saved = ?1 WHERE id = ?2",
            params![saved as i32, id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), String> {
    let connection = connection(&app).map_err(|error| error.to_string())?;
    connection
        .execute("DELETE FROM history", [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_removes_only_the_oldest_unsaved_entries() {
        let connection = Connection::open_in_memory().expect("open in-memory history");
        connection
            .execute_batch("CREATE TABLE history (id INTEGER PRIMARY KEY, created_at INTEGER NOT NULL, kind TEXT NOT NULL, text TEXT NOT NULL, saved INTEGER NOT NULL DEFAULT 0);")
            .expect("create history table");
        for text in ["one", "two", "three"] {
            connection
                .execute(
                    "INSERT INTO history (created_at, kind, text) VALUES (1, 'dictation', ?1)",
                    params![text],
                )
                .expect("insert history");
        }
        connection
            .execute("UPDATE history SET saved = 1 WHERE text = 'one'", [])
            .expect("save history entry");

        retain_unsaved(&connection, 1).expect("enforce retention");
        let unsaved: i64 = connection
            .query_row("SELECT COUNT(*) FROM history WHERE saved = 0", [], |row| {
                row.get(0)
            })
            .expect("count unsaved entries");
        let saved: i64 = connection
            .query_row("SELECT COUNT(*) FROM history WHERE saved = 1", [], |row| {
                row.get(0)
            })
            .expect("count saved entries");
        assert_eq!(unsaved, 1);
        assert_eq!(saved, 1);
    }
}
