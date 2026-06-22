// Flick — lib.rs
// Application entry point. Wires all modules together:
// - Registers IPC commands
// - Initializes system tray
// - Starts keyboard hook on background thread
// - Runs trigger detection + replacement pipeline

pub mod ai_client;
pub mod buffer;
pub mod commands;
pub mod config;
pub mod key_hook;
pub mod keychain;
pub mod replacer;
pub mod trigger;
pub mod tray;

use buffer::TextBuffer;
use config::FlickConfig;
use key_hook::HookEvent;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Shared application state accessible from commands and the event loop.
pub struct AppState {
    pub enabled: Mutex<bool>,
    pub custom_triggers: Mutex<Vec<String>>,
    pub config: Mutex<FlickConfig>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // On second instance launch, show the settings window
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            commands::save_api_key,
            commands::load_api_key,
            commands::test_api_connection,
            commands::get_config,
            commands::save_config,
            commands::toggle_enabled,
            commands::add_custom_command,
            commands::update_custom_command,
            commands::delete_custom_command,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Load config
            let cfg = config::load_config(&app_handle)
                .unwrap_or_else(|_| FlickConfig::default());

            let custom_triggers = config::get_custom_trigger_names(&cfg);

            // Initialize shared state
            app.manage(AppState {
                enabled: Mutex::new(cfg.enabled),
                custom_triggers: Mutex::new(custom_triggers),
                config: Mutex::new(cfg),
            });

            // Set up system tray — per §8.6
            if let Err(e) = tray::setup_tray(&app_handle) {
                log::error!("Failed to set up system tray: {}", e);
            }

            // Linux: Wayland detection warning — per §12.3
            #[cfg(target_os = "linux")]
            {
                if std::env::var("WAYLAND_DISPLAY").is_ok() {
                    log::warn!("Wayland detected. Flick works best on X11. Wayland support is limited.");
                    let _ = app_handle.emit("flick://warning", serde_json::json!({
                        "message": "Flick works best on X11. Wayland support is limited."
                    }));
                }
            }

            // Start the keyboard hook and trigger detection loop
            let handle_for_hook = app_handle.clone();
            std::thread::spawn(move || {
                run_hook_loop(handle_for_hook);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Failed to run Flick");
}

/// Main hook event loop — runs on a dedicated thread.
/// Receives key events, updates the buffer, checks for triggers,
/// and dispatches the replacement pipeline.
fn run_hook_loop(app: AppHandle) {
    let text_buffer = TextBuffer::new();
    let rx = key_hook::start_hook_with_name_detection();

    log::info!("Hook event loop started");

    // Create a Tokio runtime for async operations (AI calls, clipboard)
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime for hook loop");

    for event in rx {
        // Check if Flick is enabled
        let is_enabled = app
            .try_state::<AppState>()
            .map(|s| *s.enabled.lock().unwrap())
            .unwrap_or(false);

        if !is_enabled {
            // Still process events to keep the buffer sane, but skip triggers
            match event {
                HookEvent::Char(_) => {}
                HookEvent::Backspace => { text_buffer.pop_char(); }
                HookEvent::Clear => { text_buffer.clear(); }
            }
            continue;
        }

        match event {
            HookEvent::Char(c) => {
                text_buffer.push_char(c);

                // Check for trigger match — per §8.2
                let tail = text_buffer.get_tail(40);
                let custom_triggers = app
                    .try_state::<AppState>()
                    .map(|s| s.custom_triggers.lock().unwrap().clone())
                    .unwrap_or_default();

                if let Some(trigger_match) = trigger::detect(&tail, &custom_triggers) {
                    log::info!(
                        "Trigger detected: {} (param: {:?})",
                        trigger_match.command,
                        trigger_match.param
                    );

                    // Clear the buffer immediately
                    text_buffer.clear();

                    // Load API key
                    let api_key = match keychain::load_api_key() {
                        Ok(key) => key,
                        Err(e) => {
                            log::error!("No API key configured: {}", e);
                            let _ = app.emit(
                                "flick://error",
                                serde_json::json!({"message": "No API key configured. Open Settings to add one."}),
                            );
                            continue;
                        }
                    };

                    // Get config for show_done_toast
                    let show_done_toast = app
                        .try_state::<AppState>()
                        .map(|s| s.config.lock().unwrap().show_done_toast)
                        .unwrap_or(true);

                    let app_clone = app.clone();
                    let trigger_clone = trigger_match.clone();

                    // Check if this is a custom command
                    let is_custom = ai_client::get_prompt(
                        &trigger_match.command,
                        trigger_match.param.as_deref(),
                        "",
                    )
                    .is_none();

                    if is_custom {
                        // Find the custom command prompt
                        let prompt_template = {
                            if let Some(state) = app.try_state::<AppState>() {
                                let cfg = state.config.lock().unwrap();
                                cfg.custom_commands
                                    .iter()
                                    .find(|c| c.trigger == trigger_match.command)
                                    .map(|c| c.prompt.clone())
                            } else {
                                None
                            }
                        };

                        if let Some(template) = prompt_template {
                            rt.spawn(async move {
                                if let Err(e) = replacer::execute_custom_replacement(
                                    &app_clone,
                                    &api_key,
                                    &template,
                                    &trigger_clone.full_trigger,
                                    show_done_toast,
                                )
                                .await
                                {
                                    log::error!("Custom replacement failed: {}", e);
                                }
                            });
                        } else {
                            log::error!("Custom command not found: {}", trigger_match.command);
                        }
                    } else {
                        // Built-in command
                        rt.spawn(async move {
                            if let Err(e) = replacer::execute_replacement(
                                &app_clone,
                                &api_key,
                                &trigger_clone.command,
                                trigger_clone.param.as_deref(),
                                &trigger_clone.full_trigger,
                                show_done_toast,
                            )
                            .await
                            {
                                log::error!("Replacement failed: {}", e);
                            }
                        });
                    }
                }
            }
            HookEvent::Backspace => {
                text_buffer.pop_char();
            }
            HookEvent::Clear => {
                text_buffer.clear();
            }
        }
    }

    log::warn!("Hook event loop ended (receiver closed)");
}
