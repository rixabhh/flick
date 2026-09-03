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
use tauri::{AppHandle, Emitter, Manager};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

struct Active {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: usize,
    rate: u32,
}
struct Capture {
    samples: Vec<f32>,
    channels: usize,
    rate: u32,
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
        device_id: Option<String>,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DictationTarget {
    app_name: String,
    process_path: String,
}

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
        std::thread::Builder::new()
            .name("flick-audio".into())
            .spawn(move || audio_loop(receiver, flag, level))
            .expect("audio thread");
        Self {
            sender,
            recording,
            transcribing,
            input_level,
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
    let mut active: Option<Active> = None;
    for command in receiver {
        match command {
            Command::Start {
                device_id,
                response,
            } => {
                let result = if active.is_some() {
                    Ok(())
                } else {
                    create(device_id.as_deref(), Arc::clone(&input_level)).map(|value| {
                        active = Some(value);
                        recording.store(true, Ordering::SeqCst);
                    })
                };
                let _ = response.send(result);
            }
            Command::Stop(response) => {
                let result = active
                    .take()
                    .map(|value| {
                        recording.store(false, Ordering::SeqCst);
                        let Active {
                            stream,
                            samples,
                            channels,
                            rate,
                        } = value;
                        drop(stream);
                        let captured_samples = samples
                            .lock()
                            .map_err(|_| anyhow::anyhow!("Audio buffer unavailable"))?
                            .clone();
                        Ok(Capture {
                            samples: captured_samples,
                            channels,
                            rate,
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
                        drop(value.stream);
                        Ok(())
                    })
                    .unwrap_or_else(|| Err(anyhow::anyhow!("Dictation is not recording")));
                let _ = response.send(result);
            }
        }
    }
}

pub fn is_recording(app: &AppHandle) -> bool {
    app.try_state::<DictationState>()
        .is_some_and(|state| state.recording.load(Ordering::SeqCst))
}

fn show_overlay(app: &AppHandle, state: &str) {
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
    let details = whisper_rs::print_system_info().to_string();
    DictationRuntimeInfo {
        acceleration: classify_acceleration(&details).to_string(),
        details,
    }
}

fn classify_acceleration(details: &str) -> &'static str {
    let details = details.to_ascii_lowercase();
    if details.contains("cuda") {
        "CUDA GPU"
    } else if details.contains("vulkan") {
        "Vulkan GPU"
    } else if details.contains("metal") {
        "Metal GPU"
    } else if details.contains("opencl") {
        "OpenCL GPU"
    } else if details.contains("avx2") || details.contains("neon") {
        "CPU optimized"
    } else {
        "CPU"
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
    start(&app).map_err(|error| error.to_string())?;
    tokio::time::sleep(std::time::Duration::from_millis(750)).await;
    let level = dictation_input_level(app.clone());
    cancel(&app).map_err(|error| error.to_string())?;
    Ok(level)
}
#[tauri::command]
pub fn start_dictation(app: AppHandle) -> Result<(), String> {
    start(&app).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cancel_dictation(app: AppHandle) -> Result<(), String> {
    cancel(&app).map_err(|error| error.to_string())
}
pub fn start(app: &AppHandle) -> Result<()> {
    let state = app
        .try_state::<DictationState>()
        .context("Dictation is not initialized")?;
    if state.transcribing.load(Ordering::SeqCst) {
        bail!("Dictation is still transcribing the previous recording");
    }
    remember_target(app);
    let device_id = app
        .try_state::<crate::AppState>()
        .and_then(|state| {
            state
                .config
                .lock()
                .ok()
                .map(|config| config.dictation_device_id.clone())
        })
        .filter(|id| !id.is_empty());
    let (sender, receiver) = mpsc::channel();
    state
        .sender
        .send(Command::Start {
            device_id,
            response: sender,
        })
        .context("Audio thread unavailable")?;
    receiver.recv().context("Audio thread did not respond")??;
    show_overlay(app, "recording");
    Ok(())
}

fn foreground_target() -> Option<DictationTarget> {
    let window = active_win_pos_rs::get_active_window().ok()?;
    Some(DictationTarget {
        app_name: window.app_name.to_ascii_lowercase(),
        process_path: window.process_path.to_string_lossy().to_ascii_lowercase(),
    })
}

fn same_target(expected: &DictationTarget, current: &DictationTarget) -> bool {
    !expected.app_name.is_empty()
        && expected.app_name == current.app_name
        && (expected.process_path.is_empty()
            || current.process_path.is_empty()
            || expected.process_path == current.process_path)
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
    expected
        .zip(foreground_target())
        .is_some_and(|(expected, current)| same_target(&expected, &current))
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
    receiver.recv().context("Audio thread did not respond")??;
    hide_overlay(app);
    Ok(())
}

fn create(device_id: Option<&str>, input_level: Arc<AtomicU32>) -> Result<Active> {
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
    let samples = Arc::new(Mutex::new(Vec::with_capacity(
        rate as usize * channels * 30,
    )));
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
    )?;
    stream
        .play()
        .context("Could not start microphone capture")?;
    Ok(Active {
        stream,
        samples,
        channels,
        rate,
    })
}
fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    input_level: Arc<AtomicU32>,
    max_samples: usize,
) -> Result<Stream> {
    let stream_config: cpal::StreamConfig = config.clone().into();
    let error = |error| log::error!("Microphone stream error: {error}");
    match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| {
                append(&samples, &input_level, max_samples, data.iter().copied())
            },
            error,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
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
    let transcribing = {
        let state = app
            .try_state::<DictationState>()
            .context("Dictation is not initialized")?;
        if state.transcribing.swap(true, Ordering::SeqCst) {
            bail!("Dictation is already transcribing");
        }
        Arc::clone(&state.transcribing)
    };
    let result = stop_and_transcribe_inner(app).await;
    transcribing.store(false, Ordering::SeqCst);
    hide_overlay(app);
    result
}

async fn stop_and_transcribe_inner(app: &AppHandle) -> Result<String> {
    let capture = {
        let state = app
            .try_state::<DictationState>()
            .context("Dictation is not initialized")?;
        let (sender, receiver) = mpsc::channel();
        state
            .sender
            .send(Command::Stop(sender))
            .context("Audio thread unavailable")?;
        receiver.recv().context("Audio thread did not respond")??
    };
    let audio = trim_silence(resample(&capture.samples, capture.channels, capture.rate));
    if audio.len() < 1_600 {
        bail!("No speech was captured. Check your microphone and try again.");
    }
    let settings = crate::config::load_config(app)?;
    if settings.retain_recordings {
        retain_recording(app, &audio, settings.recording_retention_count).await?;
    }
    if crate::models::model_is_english_only(&settings.dictation_model_id)?
        && (settings.dictation_language != "en" || settings.dictation_translate_to_english)
    {
        bail!("Choose the multilingual speech model to dictate in another language or translate.");
    }
    let path = crate::models::verified_installed_model_path(app)
        .await?
        .context("Download a local speech model before dictating")?;
    show_overlay(app, "transcribing");
    let language = settings.dictation_language.clone();
    let translate = settings.dictation_translate_to_english;
    let text = tokio::task::spawn_blocking(move || transcribe(&path, &audio, &language, translate))
        .await
        .context("Transcription task failed")??;
    if text.trim().is_empty() {
        bail!("No speech was detected.");
    }
    Ok(post_process(
        text,
        settings.dictation_filler_cleanup,
        &settings.dictation_corrections,
    ))
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
    if channels == 0 || input.is_empty() {
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
fn transcribe(
    path: &std::path::Path,
    audio: &[f32],
    language: &str,
    translate: bool,
) -> Result<String> {
    let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
        .context("Could not load local speech model")?;
    let mut state = context
        .create_state()
        .context("Could not initialize transcription engine")?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_language((language != "auto").then_some(language));
    params.set_translate(translate);
    state
        .full(params, audio)
        .context("Local transcription failed")?;
    Ok(state
        .as_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string())
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
        let text =
            transcribe(std::path::Path::new(&model), &audio, "en", false).expect("transcribe");
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
    fn classifies_reported_acceleration_without_hardware_guessing() {
        assert_eq!(classify_acceleration("CPU: AVX2 = 1"), "CPU optimized");
        assert_eq!(classify_acceleration("ggml CUDA enabled"), "CUDA GPU");
        assert_eq!(classify_acceleration("generic backend"), "CPU");
    }

    #[test]
    fn paste_target_requires_the_same_foreground_application() {
        let expected = DictationTarget {
            app_name: "slack".into(),
            process_path: "c:/apps/slack.exe".into(),
        };
        assert!(same_target(&expected, &expected));
        assert!(!same_target(
            &expected,
            &DictationTarget {
                app_name: "discord".into(),
                process_path: "c:/apps/discord.exe".into(),
            }
        ));
    }
}
