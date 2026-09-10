//! Local microphone capture and Whisper transcription.
//!
//! Native streams live on one dedicated audio thread; Tauri state owns only a
//! channel to that thread, which is safe on platforms where CPAL streams are
//! deliberately not Send/Sync.

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

struct Active {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: usize,
    rate: u32,
    settings: crate::config::FlickConfig,
    abandoned: Arc<AtomicBool>,
}
struct Capture {
    samples: Vec<f32>,
    channels: usize,
    rate: u32,
    settings: crate::config::FlickConfig,
}
#[derive(Debug, Clone, Serialize)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct DictationRuntimeInfo {
    pub acceleration: String,
    pub details: String,
}
enum Command {
    Start {
        settings: crate::config::FlickConfig,
        abandoned: Arc<AtomicBool>,
        response: mpsc::Sender<Result<()>>,
    },
    Stop(mpsc::Sender<Result<Capture>>),
    Cancel(mpsc::Sender<Result<()>>),
}
pub struct DictationState {
    sender: mpsc::Sender<Command>,
    recording: Arc<AtomicBool>,
    transcribing: Arc<AtomicBool>,
    input_level: Arc<AtomicU32>,
    starting: AtomicBool,
}

/// Post-processing and insertion must use the same choices as capture, even if
/// settings are edited while the microphone or transcription engine is busy.
pub struct DictationResult {
    pub text: String,
    pub settings: crate::config::FlickConfig,
    pub target: Option<crate::active_target::ActiveTarget>,
}

/// Ensure failed, cancelled, or unwound operations release their busy state.
struct ResetFlag<'a>(&'a AtomicBool);

impl Drop for ResetFlag<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

type DictationTarget = crate::active_target::ActiveTarget;

/// Only foreground application identity is retained while transcription runs;
/// no title, selected text, clipboard, or UI content is stored here.
pub struct DictationTargetState {
    target: Mutex<Option<DictationTarget>>,
}

impl Default for DictationTargetState {
    fn default() -> Self {
        Self {
            target: Mutex::new(None),
        }
    }
}

impl DictationState {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let recording = Arc::new(AtomicBool::new(false));
        let transcribing = Arc::new(AtomicBool::new(false));
        let input_level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
        let flag = Arc::clone(&recording);
        let level = Arc::clone(&input_level);
        if let Err(error) = std::thread::Builder::new()
            .name("flick-audio".into())
            .spawn(move || audio_loop(receiver, flag, level))
        {
            // Failing to allocate a helper thread should leave dictation
            // unavailable, never prevent Flick itself from launching. The
            // disconnected channel below gives each command a clear recovery
            // error if this exceptionally rare OS failure occurs.
            log::error!("Could not start Flick's audio worker: {error}");
        }
        Self {
            sender,
            recording,
            transcribing,
            input_level,
            starting: AtomicBool::new(false),
        }
    }
}

impl Default for DictationState {
    fn default() -> Self {
        Self::new()
    }
}

fn audio_loop(
    receiver: mpsc::Receiver<Command>,
    recording: Arc<AtomicBool>,
    input_level: Arc<AtomicU32>,
) {
    let _recording_guard = ResetFlag(&recording);
    let mut active: Option<Active> = None;
    for command in receiver {
        match command {
            Command::Start {
                settings,
                abandoned,
                response,
            } => {
                if active.is_some() {
                    let _ = response.send(Err(anyhow::anyhow!("Dictation is already recording")));
                    continue;
                }
                let result = if abandoned.load(Ordering::SeqCst) {
                    Err(anyhow::anyhow!("Microphone start was cancelled"))
                } else {
                    create(settings, Arc::clone(&input_level), Arc::clone(&abandoned))
                };
                active = complete_audio_start(result, response, &abandoned, &recording);
                if active.is_none() {
                    input_level.store(0.0f32.to_bits(), Ordering::Relaxed);
                }
            }
            Command::Stop(response) => {
                let result = active
                    .take()
                    .map(|value| {
                        recording.store(false, Ordering::SeqCst);
                        input_level.store(0.0f32.to_bits(), Ordering::Relaxed);
                        let Active {
                            stream,
                            samples,
                            channels,
                            rate,
                            settings,
                            abandoned,
                        } = value;
                        abandoned.store(true, Ordering::SeqCst);
                        drop(stream);
                        let captured_samples = std::mem::take(
                            &mut *samples
                                .lock()
                                .map_err(|_| anyhow::anyhow!("Audio buffer unavailable"))?,
                        );
                        Ok(Capture {
                            samples: captured_samples,
                            channels,
                            rate,
                            settings,
                        })
                    })
                    .unwrap_or_else(|| Err(anyhow::anyhow!("Dictation is not recording")));
                let _ = response.send(result);
            }
            Command::Cancel(response) => {
                let result = active
                    .take()
                    .map(|value| {
                        recording.store(false, Ordering::SeqCst);
                        input_level.store(0.0f32.to_bits(), Ordering::Relaxed);
                        value.abandoned.store(true, Ordering::SeqCst);
                        drop(value.stream);
                        Ok(())
                    })
                    .unwrap_or(Ok(()));
                let _ = response.send(result);
            }
        }
    }
}

pub fn is_recording(app: &AppHandle) -> bool {
    app.try_state::<DictationState>()
        .is_some_and(|state| state.recording.load(Ordering::SeqCst))
}

fn show_overlay(app: &AppHandle, state: &str, settings: &crate::config::FlickConfig) {
    let _ = app.emit(
        "flick://dictation-session",
        serde_json::json!({
            "state": state,
            "provider_id": settings.dictation_provider,
            "app_language": settings.app_language,
        }),
    );
    let _ = app.emit("flick://dictation-state", state);
    // Compositors may promote even non-focusable WebKit windows to the active
    // surface. That makes paste-back unsafe, particularly under Wayland, so
    // Linux defaults to tray/state events rather than an on-screen overlay.
    #[cfg(not(target_os = "linux"))]
    {
        if let Some(window) = app.get_webview_window("dictation") {
            // The window configuration explicitly disables focus. This is only a
            // status surface and must never become the user's text target.
            let _ = window.show();
        }
    }
}

fn hide_overlay(app: &AppHandle) {
    let _ = app.emit("flick://dictation-state", "idle");
    #[cfg(not(target_os = "linux"))]
    {
        if let Some(window) = app.get_webview_window("dictation") {
            let _ = window.hide();
        }
    }
}
#[tauri::command]
pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    host.input_devices()
        .map_err(|error| format!("Could not enumerate microphones: {error}"))?
        .enumerate()
        .map(|(index, device)| {
            let name = device
                .name()
                .unwrap_or_else(|_| format!("Microphone {}", index + 1));
            Ok(InputDevice {
                id: format!("{index}:{name}"),
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
            })
        })
        .collect()
}
#[tauri::command]
pub fn dictation_input_level(app: AppHandle) -> f32 {
    app.try_state::<DictationState>()
        .map(|state| f32::from_bits(state.input_level.load(Ordering::Relaxed)))
        .unwrap_or(0.0)
}
#[tauri::command]
pub fn dictation_runtime_info() -> DictationRuntimeInfo {
    DictationRuntimeInfo {
        acceleration: "Native local engine".to_string(),
        details: "Whisper GGML and verified GGUF models (including Parakeet, Canary, Qwen3 ASR, SenseVoice, and Moonshine) run on this device. Flick never uploads audio when Local models is selected.".to_string(),
    }
}

/// Native device creation may outlive its caller's timeout. Keep a stream only
/// when the caller still accepts it; otherwise dropping it releases the mic.
fn complete_audio_start<T>(
    result: Result<T>,
    response: mpsc::Sender<Result<()>>,
    abandoned: &AtomicBool,
    recording: &AtomicBool,
) -> Option<T> {
    match result {
        Ok(value) if !abandoned.load(Ordering::SeqCst) => {
            recording.store(true, Ordering::SeqCst);
            if response.send(Ok(())).is_ok() {
                Some(value)
            } else {
                abandoned.store(true, Ordering::SeqCst);
                recording.store(false, Ordering::SeqCst);
                None
            }
        }
        Ok(_) => {
            let _ = response.send(Err(anyhow::anyhow!("Microphone start was cancelled")));
            None
        }
        Err(error) => {
            let _ = response.send(Err(error));
            None
        }
    }
}

const AUDIO_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// Native device calls run off the UI thread. A broken driver or worker must
/// not leave the shortcut handler waiting forever and make Flick appear hung.
fn await_audio_response<T>(receiver: mpsc::Receiver<Result<T>>) -> Result<T> {
    match receiver.recv_timeout(AUDIO_COMMAND_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            bail!("Audio service did not respond within 5 seconds. Check your microphone and try again.")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            bail!("Audio service is unavailable. Restart Flick and try again.")
        }
    }
}
/// Briefly capture from the selected microphone and discard the samples. This
/// is a permission/device check only: it never loads a model, transcribes,
/// saves, or pastes text.
#[tauri::command]
pub async fn preview_input_level(app: AppHandle) -> Result<f32, String> {
    if is_recording(&app) {
        return Err("Stop dictation before testing the microphone.".into());
    }
    let start_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || start(&start_app))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    tokio::time::sleep(std::time::Duration::from_millis(750)).await;
    let level = dictation_input_level(app.clone());
    tauri::async_runtime::spawn_blocking(move || cancel(&app))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(level)
}
#[tauri::command]
pub async fn start_dictation(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || start(&app))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub async fn cancel_dictation(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || cancel(&app))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}
pub fn start(app: &AppHandle) -> Result<()> {
    let state = app
        .try_state::<DictationState>()
        .context("Dictation is not initialized")?;
    if state.starting.swap(true, Ordering::SeqCst) {
        bail!("Microphone is still starting");
    }
    let _starting_guard = ResetFlag(&state.starting);
    if state.transcribing.load(Ordering::SeqCst) {
        bail!("Dictation is still transcribing the previous recording");
    }
    if state.recording.load(Ordering::SeqCst) {
        bail!("Dictation is already recording");
    }
    // Capture one immutable settings snapshot for this recording. In
    // particular, changing Local to Cloud while recording must never upload
    // audio that was captured under the previous privacy choice.
    // The persisted config is mirrored transactionally in AppState. Do not
    // read/migrate settings from disk on every microphone shortcut.
    let settings = app
        .try_state::<crate::AppState>()
        .context("Settings unavailable")?
        .config
        .lock()
        .map_err(|_| anyhow::anyhow!("Settings unavailable"))?
        .clone();
    if !settings.enabled {
        bail!("Enable Flick before dictating");
    }
    crate::dictation_provider::provider_info(&settings.dictation_provider)?;
    let started = std::time::Instant::now();
    show_overlay(app, "starting", &settings);
    // Privacy queries run on the action worker, never inside the OS hook.
    if crate::key_hook::active_app_is_protected(&settings.disabled_apps) {
        hide_overlay(app);
        bail!("Flick will not record in a protected app or password field.");
    }
    remember_target(app);
    let abandoned = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    if state
        .sender
        .send(Command::Start {
            settings: settings.clone(),
            abandoned: Arc::clone(&abandoned),
            response: sender,
        })
        .is_err()
    {
        hide_overlay(app);
        bail!("Audio thread unavailable");
    }
    if let Err(error) = await_audio_response(receiver) {
        abandoned.store(true, Ordering::SeqCst);
        hide_overlay(app);
        return Err(error);
    }
    show_overlay(app, "recording", &settings);
    log::info!("Microphone ready in {} ms", started.elapsed().as_millis());
    Ok(())
}

fn foreground_target() -> Option<DictationTarget> {
    let mut window = crate::active_target::get()?;
    window.title.clear();
    Some(window)
}

fn same_target(expected: &DictationTarget, current: &DictationTarget) -> bool {
    crate::active_target::matches_target(expected, current)
}

fn remember_target(app: &AppHandle) {
    if let Some(state) = app.try_state::<DictationTargetState>() {
        if let Ok(mut target) = state.target.lock() {
            *target = foreground_target();
        }
    }
}

/// Verify that the original dictation target remains the foreground app before
/// automatically pasting a completed transcript. An unavailable identity is
/// treated as unsafe rather than risking insertion into the wrong application.
pub fn target_is_still_appropriate(app: &AppHandle) -> bool {
    let expected = app
        .try_state::<DictationTargetState>()
        .and_then(|state| state.target.lock().ok().and_then(|target| target.clone()));
    captured_target_is_still_appropriate(expected.as_ref())
}

pub fn captured_target_is_still_appropriate(
    expected: Option<&crate::active_target::ActiveTarget>,
) -> bool {
    expected
        .zip(foreground_target())
        .is_some_and(|(expected, current)| same_target(expected, &current))
}

/// Discard an active recording without transcription, history, or paste-back.
pub fn cancel(app: &AppHandle) -> Result<()> {
    let state = app
        .try_state::<DictationState>()
        .context("Dictation is not initialized")?;
    let (sender, receiver) = mpsc::channel();
    state
        .sender
        .send(Command::Cancel(sender))
        .context("Audio thread unavailable")?;
    await_audio_response(receiver)?;
    hide_overlay(app);
    Ok(())
}

fn create(
    settings: crate::config::FlickConfig,
    input_level: Arc<AtomicU32>,
    abandoned: Arc<AtomicBool>,
) -> Result<Active> {
    let device_id =
        (!settings.dictation_device_id.is_empty()).then_some(settings.dictation_device_id.as_str());
    let host = cpal::default_host();
    let device = match device_id {
        Some(id) => host
            .input_devices()
            .context("Could not enumerate microphones")?
            .enumerate()
            .find_map(|(index, device)| {
                let name = device.name().ok()?;
                (format!("{index}:{name}") == id).then_some(device)
            })
            .context("Selected microphone is no longer available")?,
        None => host.default_input_device().context("No microphone found")?,
    };
    let config = device
        .default_input_config()
        .context("Could not read microphone configuration")?;
    let channels = config.channels() as usize;
    let rate = config.sample_rate().0;
    if channels == 0 || rate == 0 {
        bail!("The selected microphone returned an invalid audio format");
    }
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(rate as usize * channels * 2)
        .context("Not enough memory to start this microphone")?;
    let samples = Arc::new(Mutex::new(buffer));
    // Cap by the actual device format.  Some USB interfaces expose more than
    // two channels; using a fixed stereo cap would silently shorten their
    // maximum recording duration.
    let max_samples = rate as usize * channels * 600;
    let stream = build_stream(
        &device,
        &config,
        Arc::clone(&samples),
        input_level,
        max_samples,
        Arc::clone(&abandoned),
    )?;
    if abandoned.load(Ordering::SeqCst) {
        bail!("Microphone start was cancelled");
    }
    stream
        .play()
        .context("Could not start microphone capture")?;
    Ok(Active {
        stream,
        samples,
        channels,
        rate,
        settings,
        abandoned,
    })
}
fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    input_level: Arc<AtomicU32>,
    max_samples: usize,
    abandoned: Arc<AtomicBool>,
) -> Result<Stream> {
    let stream_config: cpal::StreamConfig = config.clone().into();
    let error = |error| log::error!("Microphone stream error: {error}");
    match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| {
                if !abandoned.load(Ordering::SeqCst) {
                    append(&samples, &input_level, max_samples, data.iter().copied());
                }
            },
            error,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
                if abandoned.load(Ordering::SeqCst) {
                    return;
                }
                append(
                    &samples,
                    &input_level,
                    max_samples,
                    data.iter().map(|s| *s as f32 / i16::MAX as f32),
                )
            },
            error,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _| {
                if abandoned.load(Ordering::SeqCst) {
                    return;
                }
                append(
                    &samples,
                    &input_level,
                    max_samples,
                    data.iter().map(|s| (*s as f32 - 32768.0) / 32768.0),
                )
            },
            error,
            None,
        ),
        _ => bail!("Unsupported microphone sample format"),
    }
    .context("Could not create microphone stream")
}
fn append(
    samples: &Arc<Mutex<Vec<f32>>>,
    input_level: &AtomicU32,
    max_samples: usize,
    input: impl Iterator<Item = f32>,
) {
    if let Ok(mut buffer) = samples.lock() {
        let received: Vec<f32> = input.collect();
        let level = if received.is_empty() {
            0.0
        } else {
            (received.iter().map(|sample| sample * sample).sum::<f32>() / received.len() as f32)
                .sqrt()
                .min(1.0)
        };
        input_level.store(level.to_bits(), Ordering::Relaxed);
        buffer.extend(received);
        let overflow = buffer.len().saturating_sub(max_samples);
        if overflow > 0 {
            buffer.drain(..overflow);
        }
    }
}

#[tauri::command]
pub async fn stop_dictation(app: AppHandle) -> Result<String, String> {
    stop_and_transcribe(&app).await.map_err(|e| e.to_string())
}
pub async fn stop_and_transcribe(app: &AppHandle) -> Result<String> {
    stop_and_transcribe_session(app)
        .await
        .map(|result| result.text)
}

pub async fn stop_and_transcribe_session(app: &AppHandle) -> Result<DictationResult> {
    let transcribing = {
        let state = app
            .try_state::<DictationState>()
            .context("Dictation is not initialized")?;
        if state.transcribing.swap(true, Ordering::SeqCst) {
            bail!("Dictation is already transcribing");
        }
        Arc::clone(&state.transcribing)
    };
    let _transcription_guard = ResetFlag(&transcribing);
    let result = stop_and_transcribe_inner(app).await;
    hide_overlay(app);
    result
}

async fn stop_and_transcribe_inner(app: &AppHandle) -> Result<DictationResult> {
    let target = app
        .try_state::<DictationTargetState>()
        .and_then(|state| state.target.lock().ok().and_then(|target| target.clone()));
    let capture = {
        let state = app
            .try_state::<DictationState>()
            .context("Dictation is not initialized")?;
        let (sender, receiver) = mpsc::channel();
        state
            .sender
            .send(Command::Stop(sender))
            .context("Audio thread unavailable")?;
        await_audio_response(receiver)?
    };
    let audio = trim_silence(resample(&capture.samples, capture.channels, capture.rate));
    if audio.len() < 1_600 {
        bail!("No speech was captured. Check your microphone and try again.");
    }
    let settings = capture.settings;
    if settings.retain_recordings {
        retain_recording(app, &audio, settings.recording_retention_count).await?;
    }
    let provider = crate::dictation_provider::provider_info(&settings.dictation_provider)?;
    if provider.requires_local_model
        && !crate::models::model_supports_language(
            &settings.dictation_model_id,
            &settings.dictation_language,
        )?
    {
        bail!(
            "The selected local speech model does not support {}. Choose Auto, English, or a compatible model.",
            settings.dictation_language
        );
    }
    if provider.requires_local_model
        && settings.dictation_translate_to_english
        && !crate::models::model_supports_translation(&settings.dictation_model_id)?
    {
        bail!("The selected local speech model can transcribe but cannot translate to English. Turn off translation or choose a multilingual Whisper model.");
    }
    let path = if provider.requires_local_model {
        crate::models::verified_configured_model_path(app, &settings).await?
    } else {
        None
    };
    show_overlay(app, "transcribing", &settings);
    let text = crate::dictation_provider::transcribe(
        settings.dictation_provider.clone(),
        crate::dictation_provider::TranscriptionRequest {
            audio,
            language: settings.dictation_language.clone(),
            translate_to_english: settings.dictation_translate_to_english,
            local_model_path: path,
        },
        settings.dictation_cloud_base_url.clone(),
        settings.dictation_cloud_model.clone(),
    )
    .await?;
    if text.trim().is_empty() {
        bail!("No speech was detected.");
    }
    let text = post_process(
        text,
        settings.dictation_filler_cleanup,
        &settings.dictation_corrections,
    );
    if text.is_empty() {
        bail!("No speech remained after cleanup. Check your text corrections and try again.");
    }
    Ok(DictationResult {
        text,
        settings,
        target,
    })
}

async fn retain_recording(app: &AppHandle, audio: &[f32], limit: usize) -> Result<()> {
    let directory = app
        .path()
        .app_data_dir()
        .context("Could not resolve application data directory")?
        .join("recordings");
    std::fs::create_dir_all(&directory).context("Could not create recordings directory")?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = directory.join(format!("dictation-{timestamp}.wav"));
    let samples = audio.to_vec();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            hound::WavWriter::create(&path, spec).context("Could not create recording")?;
        for sample in samples {
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .context("Could not write recording")?;
        }
        writer.finalize().context("Could not finalize recording")?;
        Ok(())
    })
    .await
    .context("Recording task failed")??;
    let mut recordings: Vec<_> = std::fs::read_dir(&directory)
        .context("Could not read recordings directory")?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wav")))
            .then_some(path)
        })
        .collect();
    recordings.sort();
    let remove_count = recordings.len().saturating_sub(limit.max(1));
    for path in recordings.into_iter().take(remove_count) {
        std::fs::remove_file(path).context("Could not trim retained recordings")?;
    }
    Ok(())
}

#[tauri::command]
pub fn clear_retained_recordings(app: AppHandle) -> Result<(), String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("recordings");
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
        {
            std::fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
fn resample(input: &[f32], channels: usize, rate: u32) -> Vec<f32> {
    if channels == 0 || rate == 0 || input.is_empty() {
        return Vec::new();
    }
    let mono: Vec<f32> = input
        .chunks(channels)
        .map(|f| f.iter().sum::<f32>() / f.len() as f32)
        .collect();
    if rate == 16_000 {
        return mono;
    }
    let length = mono.len() * 16_000 / rate as usize;
    (0..length)
        .map(|i| {
            let pos = i as f64 * rate as f64 / 16_000.0;
            let a = pos.floor() as usize;
            let b = (a + 1).min(mono.len() - 1);
            mono[a] * (1.0 - (pos - a as f64) as f32) + mono[b] * (pos - a as f64) as f32
        })
        .collect()
}

/// Lightweight local voice-activity detection.
///
/// The detector derives a threshold from the quietest recording frames rather
/// than assuming every microphone has the same noise floor. It requires a
/// short sustained utterance and bridges normal word/sentence pauses, so it
/// only removes non-speech at the outer edges before Whisper receives audio.
/// It intentionally does not split an utterance into separate transcripts.
fn trim_silence(audio: Vec<f32>) -> Vec<f32> {
    const FRAME: usize = 480; // 30 ms at 16 kHz
    const MIN_THRESHOLD: f32 = 0.006;
    const SPEECH_FRAMES: usize = 3; // 90 ms
    const MAX_GAP_FRAMES: usize = 20; // 600 ms, preserves natural pauses
    if audio.len() < FRAME {
        return audio;
    }
    let levels: Vec<f32> = audio
        .chunks(FRAME)
        .map(|frame| {
            (frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len() as f32).sqrt()
        })
        .collect();
    let mut quietest = levels.clone();
    quietest.sort_by(|left, right| left.total_cmp(right));
    let sample_count = (quietest.len() / 5).max(1);
    let noise_floor = quietest[..sample_count].iter().sum::<f32>() / sample_count as f32;
    let threshold = (noise_floor * 2.5).max(MIN_THRESHOLD);
    let mut active: Vec<bool> = levels.iter().map(|level| *level >= threshold).collect();

    // Remove short spikes that are unlikely to be speech.
    let mut run_start = 0;
    while run_start < active.len() {
        if !active[run_start] {
            run_start += 1;
            continue;
        }
        let run_end = active[run_start..]
            .iter()
            .position(|is_active| !is_active)
            .map(|offset| run_start + offset)
            .unwrap_or(active.len());
        if run_end - run_start < SPEECH_FRAMES {
            active[run_start..run_end].fill(false);
        }
        run_start = run_end;
    }

    // A short quiet gap is part of a spoken thought, not an outer boundary.
    let mut index = 0;
    while index < active.len() {
        if active[index] {
            index += 1;
            continue;
        }
        let gap_end = active[index..]
            .iter()
            .position(|is_active| *is_active)
            .map(|offset| index + offset)
            .unwrap_or(active.len());
        if index > 0 && gap_end < active.len() && gap_end - index <= MAX_GAP_FRAMES {
            active[index..gap_end].fill(true);
        }
        index = gap_end;
    }

    let start_frame = active.iter().position(|is_active| *is_active);
    let end_frame = active.iter().rposition(|is_active| *is_active);
    match (start_frame, end_frame) {
        (Some(start), Some(end)) => {
            let start = start.saturating_sub(1) * FRAME;
            let end = ((end + 2) * FRAME).min(audio.len());
            audio[start..end].to_vec()
        }
        _ => Vec::new(),
    }
}
fn post_process(
    text: String,
    remove_fillers: bool,
    corrections: &[crate::config::TextCorrection],
) -> String {
    let mut result = if remove_fillers {
        text.split_whitespace()
            .filter(|word| {
                !matches!(
                    word.trim_matches(|c: char| !c.is_alphanumeric())
                        .to_ascii_lowercase()
                        .as_str(),
                    "um" | "uh" | "erm" | "ah"
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        text
    };
    for correction in corrections {
        if !correction.find.trim().is_empty() {
            result = result.replace(&correction.find, &correction.replace);
        }
    }
    result.trim().to_string()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_audio_worker_returns_a_recoverable_error() {
        let (sender, receiver) = mpsc::channel::<Result<()>>();
        drop(sender);
        let error =
            await_audio_response(receiver).expect_err("closed workers must not hang callers");
        assert!(error.to_string().contains("Audio service is unavailable"));
    }
    #[test]
    fn a_late_microphone_start_is_dropped_when_its_caller_timed_out() {
        struct MockMicrophone(Arc<AtomicBool>);
        impl Drop for MockMicrophone {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        for abandon_before_completion in [false, true] {
            let released = Arc::new(AtomicBool::new(false));
            let abandoned = AtomicBool::new(abandon_before_completion);
            let recording = AtomicBool::new(false);
            let (sender, receiver) = mpsc::channel();
            drop(receiver);
            let capture = complete_audio_start(
                Ok(MockMicrophone(Arc::clone(&released))),
                sender,
                &abandoned,
                &recording,
            );
            assert!(capture.is_none());
            assert!(
                released.load(Ordering::SeqCst),
                "the native microphone must be released"
            );
            assert!(!recording.load(Ordering::SeqCst));
        }
    }
    #[test]
    fn cancelled_operations_release_their_busy_flag() {
        let flag = AtomicBool::new(true);
        {
            let _guard = ResetFlag(&flag);
        }
        assert!(!flag.load(Ordering::SeqCst));
    }
    #[test]
    fn downmixes_and_resamples() {
        let output = resample(&[1.0, -1.0, 1.0, -1.0], 2, 8_000);
        assert_eq!(output.len(), 4);
        assert!(output.iter().all(|s| s.abs() < 0.001));
    }
    #[test]
    fn recording_buffer_obeys_the_device_specific_cap() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let level = AtomicU32::new(0.0f32.to_bits());
        append(&samples, &level, 4, [0.1, 0.2, 0.3, 0.4, 0.5].into_iter());
        assert_eq!(*samples.lock().unwrap(), vec![0.2, 0.3, 0.4, 0.5]);
    }
    #[test]
    fn trims_only_outer_silence() {
        let mut audio = vec![0.0; 960];
        audio.extend(vec![0.1; 1_440]);
        audio.extend(vec![0.0; 960]);
        let trimmed = trim_silence(audio);
        assert!(trimmed.len() >= 1_440);
        assert!(trimmed.len() < 3_360);
        assert!(trimmed.iter().any(|sample| *sample > 0.05));
    }
    #[test]
    fn vad_rejects_brief_noise_and_keeps_a_short_speech_pause() {
        let mut click = vec![0.0; 480 * 3];
        click.extend(vec![0.2; 480 * 2]);
        click.extend(vec![0.0; 480 * 3]);
        assert!(trim_silence(click).is_empty());

        let mut utterance = vec![0.002; 480 * 3];
        utterance.extend(vec![0.08; 480 * 3]);
        utterance.extend(vec![0.002; 480 * 5]);
        utterance.extend(vec![0.08; 480 * 3]);
        utterance.extend(vec![0.002; 480 * 3]);
        let detected = trim_silence(utterance);
        // The bridged pause remains available to Whisper rather than producing
        // two fragments from one sentence.
        assert!(detected.len() >= 480 * 11);
        assert!(detected.iter().any(|sample| *sample > 0.05));
    }
    #[test]
    #[ignore = "requires FLICK_WHISPER_MODEL and FLICK_WHISPER_SAMPLE for a real local-engine smoke test"]
    fn transcribes_real_whisper_audio() {
        let model = std::env::var("FLICK_WHISPER_MODEL").expect("model path");
        let sample = std::env::var("FLICK_WHISPER_SAMPLE").expect("sample wav path");
        let reader = hound::WavReader::open(sample).expect("read wav");
        let spec = reader.spec();
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .map(|sample| sample.expect("wav sample") as f32 / i16::MAX as f32)
            .collect();
        let audio = resample(&samples, spec.channels as usize, spec.sample_rate);
        let text = crate::dictation_provider::transcribe_local_whisper(
            &audio,
            "en",
            false,
            std::path::Path::new(&model),
        )
        .expect("transcribe");
        assert!(
            text.to_lowercase().contains("ask not"),
            "unexpected transcript: {text}"
        );
    }
    #[test]
    fn cleans_fillers_then_applies_user_corrections() {
        let result = post_process(
            "Um, hello uh Flick".into(),
            true,
            &[crate::config::TextCorrection {
                find: "Flick".into(),
                replace: "Flick 2".into(),
            }],
        );
        assert_eq!(result, "hello Flick 2");
    }

    #[test]
    fn paste_target_requires_the_same_foreground_application() {
        let expected = DictationTarget {
            app_name: "slack".into(),
            process_path: "c:/apps/slack.exe".into(),
            ..Default::default()
        };
        assert!(same_target(&expected, &expected));
        assert!(!same_target(
            &expected,
            &DictationTarget {
                app_name: "discord".into(),
                process_path: "c:/apps/discord.exe".into(),
                ..Default::default()
            }
        ));
    }
}
