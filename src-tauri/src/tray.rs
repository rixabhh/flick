// Flick - tray.rs
// Per PRD §8.6: System tray icon and menu.
// Menu: Enabled toggle, Open Settings, Check for Updates, Quit.

use anyhow::Result;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, CheckMenuItemBuilder},
    AppHandle, Manager,
};

/// Set up the system tray icon and menu - per §8.6.
pub fn setup_tray(app: &AppHandle) -> Result<()> {
    let enabled_item = CheckMenuItemBuilder::with_id("enabled", "Enabled")
        .checked(true)
        .build(app)
        .map_err(|e| anyhow::anyhow!("Failed to create enabled menu item: {}", e))?;

    let settings_item = MenuItemBuilder::with_id("settings", "Open Settings")
        .build(app)
        .map_err(|e| anyhow::anyhow!("Failed to create settings menu item: {}", e))?;

    let quit_item = MenuItemBuilder::with_id("quit", "Quit")
        .build(app)
        .map_err(|e| anyhow::anyhow!("Failed to create quit menu item: {}", e))?;

    let menu = MenuBuilder::new(app)
        .item(&enabled_item)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build tray menu: {}", e))?;

    let tray = app.tray_by_id("flick-tray");
    if let Some(tray) = tray {
        tray.set_menu(Some(menu))
            .map_err(|e| anyhow::anyhow!("Failed to set tray menu: {}", e))?;

        tray.on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "enabled" => {
                    handle_toggle_enabled(app);
                }
                "settings" => {
                    handle_open_settings(app);
                }
                "quit" => {
                    log::info!("Quit requested from tray");
                    app.exit(0);
                }
                _ => {}
            }
        });
    }

    Ok(())
}

/// Handle the "Enabled" toggle from the tray menu.
fn handle_toggle_enabled(app: &AppHandle) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        let mut enabled = state.enabled.lock().unwrap();
        *enabled = !*enabled;
        let new_val = *enabled;
        drop(enabled);

        // Persist the change
        if let Ok(mut cfg) = crate::config::load_config(app) {
            cfg.enabled = new_val;
            let _ = crate::config::save_config(app, &cfg);
        }

        log::info!("Flick {} via tray", if new_val { "enabled" } else { "disabled" });
    }
}

/// Handle "Open Settings" from the tray menu.
fn handle_open_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        log::warn!("Settings window not found");
    }
}
