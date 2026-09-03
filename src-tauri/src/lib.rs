// Flick - lib.rs
// Application entry point. Wires all modules together:
// - Registers IPC commands
// - Initializes system tray
// - Starts keyboard hook on background thread
// - Runs trigger detection + replacement pipeline

pub mod ai_client;
pub mod buffer;
pub mod commands;
pub mod composer;
pub mod config;
pub mod diagnostics;
pub mod dictation;
pub mod history;
pub mod key_hook;
pub mod keychain;
pub mod models;
pub mod replacer;
pub mod tray;
pub mod trigger;

use buffer::TextBuffer;
use config::FlickConfig;
use key_hook::HookEvent;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// Shared application state accessible from commands and the event loop.
pub struct AppState {
    pub enabled: Mutex<bool>,
    pub custom_triggers: Mutex<Vec<String>>,
    pub config: Mutex<FlickConfig>,
}

/// The app identity that explicitly opened the reply composer. It never
/// contains selected text, clipboard data, window contents, or chat history.
pub struct ComposerTargetState {
    pub target: Mutex<Option<composer::TargetIdentity>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_cli_args(app, args);
        }))
        .invoke_handler(tauri::generate_handler![
            commands::save_api_key,
            commands::load_api_key,
            commands::delete_api_key,
            commands::test_api_connection,
            commands::get_config,
            commands::save_config,
            commands::toggle_enabled,
            commands::add_custom_command,
            commands::update_custom_command,
            commands::delete_custom_command,
            commands::export_command_templates,
            commands::import_command_templates,
            composer::capture_reply_context,
            composer::generate_reply,
            composer::insert_reply,
            composer::copy_reply,
            dictation::start_dictation,
            dictation::stop_dictation,
            dictation::cancel_dictation,
            dictation::list_input_devices,
            dictation::dictation_input_level,
            dictation::preview_input_level,
            dictation::dictation_runtime_info,
            dictation::clear_retained_recordings,
            diagnostics::export_diagnostics,
            models::list_local_models,
            models::download_local_model,
            models::cancel_local_model_download,
            models::set_active_local_model,
            models::delete_local_model,
            history::get_history,
            history::copy_history_entry,
            history::set_history_saved,
            history::clear_history,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Load config
            let cfg = config::load_config(&app_handle).unwrap_or_else(|_| FlickConfig::default());

            let custom_triggers = config::get_custom_trigger_names(&cfg);

            // Initialize shared state
            app.manage(AppState {
                enabled: Mutex::new(cfg.enabled),
                custom_triggers: Mutex::new(custom_triggers),
                config: Mutex::new(cfg),
            });
            app.manage(ComposerTargetState {
                target: Mutex::new(None),
            });
            app.manage(dictation::DictationState::new());
            app.manage(dictation::DictationTargetState::default());
            app.manage(models::ModelDownloadState::default());
            handle_cli_args(&app_handle, std::env::args().collect());

            // Set up system tray - per §8.6
            if let Err(e) = tray::setup_tray(&app_handle) {
                log::error!("Failed to set up system tray: {}", e);
            }

            // Show the settings window on first launch so the packaged app
            // doesn't appear to do nothing when started from the installer.
            if let Some(window) = app_handle.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Linux: Wayland detection warning - per §12.3
            #[cfg(target_os = "linux")]
            {
                if std::env::var("WAYLAND_DISPLAY").is_ok() {
                    log::warn!(
                        "Wayland detected. Flick works best on X11. Wayland support is limited."
                    );
                    let _ = app_handle.emit(
                        "flick://warning",
                        serde_json::json!({
                            "message": "Flick works best on X11. Wayland support is limited."
                        }),
                    );
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliAction {
    OpenSettings,
    OpenComposer,
    ToggleDictation,
    CancelDictation,
    CopyLastResult,
}

fn parse_cli_action(args: &[String]) -> Option<CliAction> {
    args.iter().find_map(|argument| match argument.as_str() {
        "--open-settings" => Some(CliAction::OpenSettings),
        "--open-composer" => Some(CliAction::OpenComposer),
        "--toggle-dictation" => Some(CliAction::ToggleDictation),
        "--cancel-dictation" => Some(CliAction::CancelDictation),
        "--copy-last-result" => Some(CliAction::CopyLastResult),
        _ => None,
    })
}

fn handle_cli_args(app: &AppHandle, args: Vec<String>) {
    let Some(action) = parse_cli_action(&args) else {
        return;
    };
    match action {
        CliAction::OpenSettings => tray::open_settings(app),
        CliAction::OpenComposer => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { composer::open_from_shortcut(&app).await });
        }
        CliAction::ToggleDictation => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if dictation::is_recording(&app) {
                    finish_dictation(&app).await;
                } else if let Err(error) = dictation::start(&app) {
                    let _ = app.emit(
                        "flick://error",
                        serde_json::json!({"message": error.to_string()}),
                    );
                }
            });
        }
        CliAction::CancelDictation => {
            if dictation::is_recording(app) {
                if let Err(error) = dictation::cancel(app) {
                    log::warn!("Could not cancel dictation: {error}");
                }
            }
        }
        CliAction::CopyLastResult => {
            if let Err(error) = history::copy_last_result(app) {
                log::warn!("Could not copy last result: {error}");
            }
        }
    }
}

/// Main hook event loop - runs on a dedicated thread.
/// Receives key events, updates the buffer, checks for triggers,
/// and dispatches the replacement pipeline.
fn run_hook_loop(app: AppHandle) {
    let text_buffer = TextBuffer::new();
    let rx = key_hook::start_hook_with_name_detection(app.clone());

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
                HookEvent::Backspace => {
                    text_buffer.pop_char();
                }
                HookEvent::Clear => {
                    text_buffer.clear();
                }
                HookEvent::CancelDictation => {
                    text_buffer.clear();
                }
                HookEvent::OpenComposer => {}
                HookEvent::ToggleDictation
                | HookEvent::StartDictation
                | HookEvent::StopDictation
                | HookEvent::HoldOrTogglePress
                | HookEvent::CopyLastResult
                | HookEvent::PastePlainText => {}
            }
            continue;
        }

        match event {
            HookEvent::Char(c) => {
                text_buffer.push_char(c);

                // Check for trigger match - per §8.2
                let tail = text_buffer.get_tail(40);
                let custom_triggers = app
                    .try_state::<AppState>()
                    .map(|s| s.custom_triggers.lock().unwrap().clone())
                    .unwrap_or_default();

                if let Some(trigger_match) = trigger::detect(&tail, &custom_triggers) {
                    let disabled_apps = app
                        .try_state::<AppState>()
                        .and_then(|state| {
                            state
                                .config
                                .lock()
                                .ok()
                                .map(|config| config.disabled_apps.clone())
                        })
                        .unwrap_or_default();
                    if key_hook::active_app_is_disabled(&disabled_apps) {
                        text_buffer.clear();
                        continue;
                    }
                    log::info!(
                        "Trigger detected: {} (param: {:?})",
                        trigger_match.command,
                        trigger_match.param
                    );

                    // Clear the buffer immediately
                    text_buffer.clear();

                    // Get config values needed for runtime behavior
                    let (show_done_toast, provider, model, custom_base_url) = app
                        .try_state::<AppState>()
                        .map(|s| {
                            let cfg = s.config.lock().unwrap();
                            (
                                cfg.show_done_toast,
                                cfg.provider.clone(),
                                cfg.model.clone(),
                                cfg.custom_base_url.clone(),
                            )
                        })
                        .unwrap_or((
                            true,
                            "gemini".to_string(),
                            "gemini-2.5-flash-lite".to_string(),
                            String::new(),
                        ));

                    // Self-hosted OpenAI-compatible servers can deliberately run
                    // without authentication. Cloud providers still require a key.
                    let api_key = if provider == "custom" {
                        keychain::load_api_key(&provider).unwrap_or_default()
                    } else {
                        match keychain::load_api_key(&provider) {
                            Ok(key) => key,
                            Err(e) => {
                                log::error!("No API key configured: {}", e);
                                let _ = app.emit(
                                    "flick://error",
                                    serde_json::json!({"message": "No API key configured. Open Settings to add one."}),
                                );
                                continue;
                            }
                        }
                    };

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
                                    &provider,
                                    &model,
                                    &custom_base_url,
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
                                &provider,
                                &model,
                                &custom_base_url,
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
            HookEvent::OpenComposer => {
                let app_clone = app.clone();
                rt.spawn(async move {
                    composer::open_from_shortcut(&app_clone).await;
                });
            }
            HookEvent::ToggleDictation => {
                let app_clone = app.clone();
                if dictation::is_recording(&app_clone) {
                    rt.spawn(async move {
                        finish_dictation(&app_clone).await;
                    });
                } else {
                    begin_dictation(&app_clone);
                }
            }
            HookEvent::StartDictation => {
                begin_dictation(&app);
            }
            HookEvent::HoldOrTogglePress => {
                let app_clone = app.clone();
                if dictation::is_recording(&app_clone) {
                    rt.spawn(async move {
                        finish_dictation(&app_clone).await;
                    });
                } else {
                    begin_dictation(&app_clone);
                }
            }
            HookEvent::StopDictation => {
                let app_clone = app.clone();
                rt.spawn(async move {
                    if dictation::is_recording(&app_clone) {
                        finish_dictation(&app_clone).await;
                    }
                });
            }
            HookEvent::CancelDictation => {
                if dictation::is_recording(&app) {
                    if let Err(error) = dictation::cancel(&app) {
                        let _ = app.emit(
                            "flick://error",
                            serde_json::json!({"message": error.to_string()}),
                        );
                    }
                }
                text_buffer.clear();
            }
            HookEvent::CopyLastResult => match history::copy_last_result(&app) {
                Ok(true) => {
                    let _ = app.emit("flick://toast", "Last result copied");
                }
                Ok(false) => {
                    let _ = app.emit(
                        "flick://error",
                        serde_json::json!({"message": "No local history entry is available to copy."}),
                    );
                }
                Err(error) => {
                    let _ = app.emit(
                        "flick://error",
                        serde_json::json!({"message": error.to_string()}),
                    );
                }
            },
            HookEvent::PastePlainText => {
                let app_clone = app.clone();
                rt.spawn(async move {
                    if let Err(error) = replacer::paste_plain_text_from_clipboard().await {
                        let _ = app_clone.emit(
                            "flick://error",
                            serde_json::json!({"message": error.to_string()}),
                        );
                    }
                });
            }
        }
    }

    log::warn!("Hook event loop ended (receiver closed)");
}

/// Starting capture is intentionally synchronous at the hook event boundary.
/// This makes press/release ordering deterministic: a long release can never
/// try to stop a session before its microphone stream exists.
fn begin_dictation(app: &AppHandle) {
    if !dictation::is_recording(app) {
        if let Err(error) = dictation::start(app) {
            let _ = app.emit(
                "flick://error",
                serde_json::json!({"message": error.to_string()}),
            );
        }
    }
}

async fn finish_dictation(app: &AppHandle) {
    match dictation::stop_and_transcribe(app).await {
        Ok(text) => {
            let text = maybe_cleanup_dictation(app, text).await;
            let _ = history::record(app, "dictation", &text);
            let append_space = app
                .try_state::<AppState>()
                .and_then(|state| {
                    state
                        .config
                        .lock()
                        .ok()
                        .map(|config| config.append_trailing_space)
                })
                .unwrap_or(false);
            let output = if append_space {
                format!("{} ", text)
            } else {
                text
            };
            let disabled_apps = app
                .try_state::<AppState>()
                .and_then(|state| {
                    state
                        .config
                        .lock()
                        .ok()
                        .map(|config| config.disabled_apps.clone())
                })
                .unwrap_or_default();
            let target_changed = !dictation::target_is_still_appropriate(app);
            let protected_target = key_hook::active_app_is_protected(&disabled_apps);
            if target_changed || protected_target {
                let copied = arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(output.clone()))
                    .is_ok();
                let reason = if protected_target {
                    "Dictation target is protected"
                } else {
                    "Dictation target changed"
                };
                let message = if copied {
                    format!("{reason}. The transcript was copied instead of pasted.")
                } else {
                    format!("{reason}. Flick did not paste into that target.")
                };
                let _ = app.emit("flick://error", serde_json::json!({"message": message}));
                return;
            }
            if let Err(error) = replacer::paste_text_transaction(&output).await {
                let _ = app.emit(
                    "flick://error",
                    serde_json::json!({"message": error.to_string()}),
                );
            } else {
                let auto_submit_apps = app
                    .try_state::<AppState>()
                    .and_then(|state| {
                        state
                            .config
                            .lock()
                            .ok()
                            .map(|config| config.auto_submit_apps.clone())
                    })
                    .unwrap_or_default();
                if key_hook::active_app_matches(&auto_submit_apps) {
                    if let Err(error) = replacer::submit_current_target().await {
                        let _ = app.emit(
                            "flick://error",
                            serde_json::json!({"message": format!("Transcript pasted, but automatic submit failed: {error}")}),
                        );
                    }
                }
            }
        }
        Err(error) => {
            let _ = app.emit(
                "flick://error",
                serde_json::json!({"message": error.to_string()}),
            );
        }
    }
}

/// Keep the primary dictation path local by default. When the user explicitly
/// opts in, only the final transcript is sent to their selected provider for
/// cleanup; a provider error must never discard or block the local result.
async fn maybe_cleanup_dictation(app: &AppHandle, text: String) -> String {
    let Some((enabled, provider, model, custom_base_url)) =
        app.try_state::<AppState>().and_then(|state| {
            state.config.lock().ok().map(|config| {
                (
                    config.dictation_llm_post_process,
                    config.provider.clone(),
                    config.model.clone(),
                    config.custom_base_url.clone(),
                )
            })
        })
    else {
        return text;
    };
    if !enabled {
        return text;
    }
    let api_key = if provider == "custom" {
        keychain::load_api_key(&provider).unwrap_or_default()
    } else {
        match keychain::load_api_key(&provider) {
            Ok(key) => key,
            Err(error) => {
                log::warn!("Skipping optional dictation cleanup: {error}");
                return text;
            }
        }
    };
    let prompt = ai_client::get_dictation_post_process_prompt(&text);
    let result = if provider == "custom" {
        if custom_base_url.trim().is_empty() {
            log::warn!("Skipping optional dictation cleanup: custom provider has no base URL");
            return text;
        }
        ai_client::transform_openai_compatible(&api_key, &custom_base_url, &model, &prompt).await
    } else {
        ai_client::transform_text(&api_key, &provider, &model, &prompt).await
    };
    match result {
        Ok(cleaned) if !cleaned.trim().is_empty() => cleaned,
        Ok(_) => text,
        Err(error) => {
            log::warn!("Optional dictation cleanup failed; using local transcript: {error}");
            text
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn accepts_only_known_cli_actions() {
        assert_eq!(
            parse_cli_action(&["flick".into(), "--open-composer".into()]),
            Some(CliAction::OpenComposer)
        );
        assert_eq!(
            parse_cli_action(&["flick".into(), "--cancel-dictation".into()]),
            Some(CliAction::CancelDictation)
        );
        assert_eq!(
            parse_cli_action(&["flick".into(), "--unknown".into()]),
            None
        );
    }
}
