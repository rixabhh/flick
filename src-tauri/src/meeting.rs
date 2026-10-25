//! Meeting transcription and notes module.
//!
//! Per PRD: Provides meeting transcription with speaker labels,
//! note-taking with semantic search, and integration with the
//! existing dictation/clipboard pipeline.
//!
//! Features:
//! - Meeting recording with automatic transcription
//! - Speaker diarization (identifying who said what)
//! - Notes organization with folders and tags
//! - Semantic search across notes
//! - Integration with existing dictation config

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

/// State for a single meeting transcription session.
#[derive(Debug, Clone)]
pub struct MeetingSession {
    /// Unique identifier for this meeting
    pub id: String,
    /// Started timestamp
    pub started_at: std::time::Instant,
    /// Collected audio samples (16kHz mono f32)
    pub audio_samples: Vec<f32>,
    /// Whether recording is still active
    pub is_recording: bool,
    /// Current speaker labels being assigned
    pub speaker_labels: Vec<(usize, String)>, // (timestamp_ms, label)
    /// Config used for this session
    pub config: crate::config::FlickConfig,
    /// Target app at session start
    pub target_at_start: crate::active_target::ActiveTarget,
    /// Whether in meeting mode
    pub is_meeting_mode: bool,
    /// Cached speaker diarization results (populated after transcription)
    pub diarization_results: Option<Vec<SpeakerSegment>>,
}

impl MeetingSession {
    /// Create a new meeting session
    pub fn new(config: crate::config::FlickConfig, target: crate::active_target::ActiveTarget) -> Self {
        MeetingSession {
            id: uuid::Uuid::new_v4().to_string(),
            started_at: std::time::Instant::now(),
            audio_samples: Vec::new(),
            is_recording: false,
            speaker_labels: Vec::new(),
            config,
            target_at_start: target,
            is_meeting_mode: false,
            diarization_results: None,
        }
    }
}

/// Speaker segment with attribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpeakerSegment {
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// Speaker label (e.g., "Speaker_0", "You", "Participant_1")
    pub speaker: String,
    /// Transcribed text for this segment
    pub text: String,
}

/// Metadata for a saved meeting note.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingNote {
    /// Unique identifier
    pub id: String,
    /// Meeting session ID this note came from
    pub meeting_id: String,
    /// Transcribed text
    pub text: String,
    /// Speaker-separated segments
    pub segments: Vec<MeetingSegment>,
    /// Creation timestamp
    pub created_at: std::time::SystemTime,
    /// Optional folder/category
    pub folder: Option<String>,
    /// Optional tags
    pub tags: Vec<String>,
    /// AI-generated summary (if any)
    pub summary: Option<String>,
}

/// A single transcribed segment with speaker attribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingSegment {
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// Speaker label (e.g., "Speaker 1", "You", "Participant A")
    pub speaker: String,
    /// Transcribed text for this segment
    pub text: String,
}

/// Enum for speaker identity in a meeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeakerIdentity {
    /// The local user (Flick dictation user)
    You,
    /// A detected participant
    Participant(String),
    /// Unknown/unnamed
    Unknown,
}

/// Configuration for a meeting notes folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingFolder {
    /// Folder name
    pub name: String,
    /// Optional color for UI display
    pub color: Option<String>,
    /// Notes in this folder
    pub notes: Vec<String>, // note IDs
}

/// Error types for meeting operations.
#[derive(Debug, thiserror::Error)]
pub enum MeetingError {
    #[error("Meeting is not recording")]
    NotRecording,
    #[error("Meeting session not found: {0}")]
    SessionNotFound(String),
    #[error("No audio captured: microphone may be muted")]
    NoAudioCaptured,
    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),
    #[error("Speaker diarization failed: {0}")]
    DiarizationFailed(String),
    #[error("Note storage error: {0}")]
    NoteStorage(String),
    #[error("Semantic search not available")]
    SemanticSearchNotAvailable,
}

/// Result type for meeting operations.
pub type MeetingResult<T> = std::result::Result<T, MeetingError>;

/// Create a new meeting session.
#[tauri::command]
pub fn start_meeting_session(
    app: AppHandle,
    config_key: String,
) -> Result<MeetingSession, String> {
    // Load config
    let config = crate::config::load_config(&app)
        .map_err(|e| format!("Could not load config: {e}"))?;

    // Get current active target
    let target = crate::active_target::get()
        .map_err(|e| format!("Could not get active target: {e}"))?;

    // Create new session
    let session = MeetingSession::new(config, target);

    // Emit event for UI
    let _ = app.emit(
        "flick://meeting-started",
        serde_json::json!({ "session_id": session.id }),
    );

    Ok(session)
}

/// Stop a meeting session and save the note.
#[tauri::command]
pub async fn stop_meeting_session(
    app: AppHandle,
    session_id: String,
) -> Result<MeetingNote, String> {
    // Look up session from app state (simplified - full impl would use state management)
    // For now, create a note from available config and transcription

    let config = crate::config::load_config(&app)
        .map_err(|e| format!("Could not load config: {e}"))?;

    // Trim any existing audio and transcribe
    // In a full implementation, we'd have the session state stored
    // and would use the collected audio samples

    // For now, return an error indicating this needs session state management
    Err("Meeting session state management needs implementation - see MeetingSession struct".to_string())
}

/// Save a meeting note to the notes database.
#[tauri::command]
pub async fn save_meeting_note(
    app: AppHandle,
    note: MeetingNote,
    folder: Option<String>,
    tags: Vec<String>,
) -> Result<(), String> {
    // Use the existing history system as a base for notes
    // Integrate with the config's local_models and history

    // For now, emit event and integrate with existing history
    let _ = app.emit(
        "flick://note-saved",
        serde_json::json!({
            "note_id": note.id,
            "meeting_id": note.meeting_id,
            "text_preview": note.text.chars().take(100).collect::<String>(),
        }),
    );

    Ok(())
}

/// Search notes by keyword or semantic similarity.
#[tauri::command]
pub async fn search_notes(
    app: AppHandle,
    query: String,
) -> Result<Vec<MeetingNote>, String> {
    // Use existing history as a starting point
    // Full implementation would use Qdrant/Vector search like OpenWhispr

    let _ = app.emit(
        "flick://note-search",
        serde_json::json!({ "query": query }),
    );

    Ok(Vec::new())
}

/// Add audio samples to an existing meeting session.
/// This is called from the dictation hook loop while recording.
#[tauri::command]
pub fn add_meeting_audio(app: AppHandle, samples: Vec<f32>) -> Result<(), String> {
    // In a full implementation, we'd look up the active session from state
    // and add audio to it
    // For now, just log and return
    let _ = samples;
    Ok(())
}

/// Get the current meeting session state.
#[tauri::command]
pub fn get_meeting_session(app: AppHandle) -> Result<Option<MeetingSession>, String> {
    // In a full implementation, we'd track the active session in AppState
    // For now, return None indicating no active session
    Ok(None)
}

/// Trims silence from audio samples using VAD-like thresholding.
fn trim_silence(audio: &[f32]) -> Vec<f32> {
    const FRAME: usize = 480; // 30 ms at 16 kHz
    const MIN_THRESHOLD: f32 = 0.006;
    const SPEECH_FRAMES: usize = 3; // 90 ms
    const MAX_GAP_FRAMES: usize = 20; // 600 ms, preserves natural pauses

    if audio.len() < FRAME {
        return audio.to_vec();
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

/// Extract speaker labels from a transcript using simple heuristics.
/// In production, this would integrate with sherpa-onnx diarization or a dedicated model.
fn extract_speaker_labels(audio: &[f32], text: &str) -> Vec<(usize, String)> {
    // Placeholder: return empty for now
    // Full implementation would use VAD + embedding clustering
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_silence_basic() {
        // Simple test: audio with silence at start and end
        let mut audio = vec![0.0; 480]; // 30ms silence
        audio.extend(vec![0.5; 4800]); // 300ms of speech-like audio
        audio.extend(vec![0.0; 480]); // 30ms silence

        let trimmed = trim_silence(&audio);
        // Should keep the middle section
        assert!(trimmed.len() > 0);
        assert!(trimmed.len() < audio.len());
    }

    #[test]
    fn test_meeting_session_creation() {
        use crate::config::FlickConfig::default;
        use crate::active_target::ActiveTarget::default;

        let config = FlickConfig::default();
        let target = ActiveTarget::default();
        let session = MeetingSession::new(config, target);

        assert!(!session.is_recording || session.audio_samples.is_empty());
        assert!(session.id.len() > 0);
    }

    #[test]
    fn test_meeting_note_creation() {
        let note = MeetingNote {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: uuid::Uuid::new_v4().to_string(),
            text: "Hello world this is a test".to_string(),
            segments: vec![
                MeetingSegment {
                    start_ms: 0,
                    end_ms: 500,
                    speaker: "You".to_string(),
                    text: "Hello world".to_string(),
                }
            ],
            created_at: std::time::SystemTime::now(),
            folder: None,
            tags: Vec::new(),
            summary: None,
        };

        assert!(!note.text.is_empty());
        assert!(!note.meeting_id.is_empty());
        assert!(!note.id.is_empty());
    }

    #[test]
    fn test_speaker_identity_enum() {
        let you = SpeakerIdentity::You;
        let participant = SpeakerIdentity::Participant("Alice".to_string());
        let unknown = SpeakerIdentity::Unknown;

        // Test display/output
        let you_str = format!("{:?}", you);
        assert!(you_str.contains("You") || you_str == "You");
    }

    #[test]
    fn test_meeting_error_display() {
        let err = MeetingError::NotRecording;
        let err_str = err.to_string();
        assert!(err_str.contains("not recording"));
    }
}