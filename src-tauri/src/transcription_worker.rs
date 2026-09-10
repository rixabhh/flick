//! Keep native model aborts, stack overflows and driver faults out of the app.
//! PCM travels through an anonymous pipe, never a recording on disk.
use anyhow::{bail, Context, Result};
use std::{
    io::{Read, Write},
    path::Path,
};
use tokio::io::AsyncWriteExt;

const FLAG: &str = "--flick-transcribe-worker";
const MAX_SAMPLES: usize = 16_000 * 600;

/// Called before Tauri/single-instance initialization in a helper process.
pub fn run_if_requested() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|arg| arg != FLAG) {
        return false;
    }
    let result = (|| -> Result<String> {
        if args.len() != 5 {
            bail!("Invalid worker request");
        }
        let mut bytes = Vec::new();
        std::io::stdin()
            .take((MAX_SAMPLES * 4 + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.is_empty() || bytes.len() > MAX_SAMPLES * 4 || bytes.len() % 4 != 0 {
            bail!("Invalid audio length");
        }
        let audio: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        if audio.iter().any(|sample| !sample.is_finite()) {
            bail!("Invalid audio sample");
        }
        crate::dictation_provider::transcribe_local_whisper(
            &audio,
            &args[3].to_string_lossy(),
            args[4] == "true",
            Path::new(&args[2]),
        )
    })();
    let message = match result {
        Ok(text) => serde_json::json!({"flick_worker": 1, "text": text}),
        Err(error) => serde_json::json!({"flick_worker": 1, "error": error.to_string()}),
    };
    let _ = writeln!(std::io::stdout(), "{message}");
    true
}

pub async fn transcribe(
    audio: &[f32],
    path: &Path,
    language: &str,
    translate: bool,
) -> Result<String> {
    if audio.is_empty() || audio.len() > MAX_SAMPLES {
        bail!("Record between a moment and ten minutes of audio.");
    }
    let mut command = tokio::process::Command::new(
        std::env::current_exe().context("Could not locate Flick's transcription worker")?,
    );
    command
        .args([
            std::ffi::OsStr::new(FLAG),
            path.as_os_str(),
            std::ffi::OsStr::new(language),
            std::ffi::OsStr::new(if translate { "true" } else { "false" }),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let mut child = command
        .spawn()
        .context("Could not start the local speech engine")?;
    let mut input = child
        .stdin
        .take()
        .context("Speech worker input unavailable")?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(300), async {
        for samples in audio.chunks(4096) {
            let bytes: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
            input.write_all(&bytes).await.context("Local speech engine stopped while receiving audio")?;
        }
        drop(input);
        let output = child.wait_with_output().await.context("Could not read the local speech engine result")?;
        if !output.status.success() {
            bail!("The local speech engine stopped unexpectedly ({}). Flick is still running. Try a smaller model or retry; no audio was uploaded.", output.status);
        }
        decode_response(&output.stdout)
    }).await;
    result.context("Local transcription exceeded five minutes and was stopped. Try a smaller model; no audio was uploaded.")?
}

fn decode_response(bytes: &[u8]) -> Result<String> {
    // Native libraries may print diagnostics. Only accept our tagged final
    // result envelope, never paste library output into the user's document.
    let value: serde_json::Value = String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|value| value["flick_worker"] == 1)
        .context("Local speech engine returned no valid result")?;
    if let Some(error) = value["error"].as_str() {
        bail!("{error}");
    }
    let text = value["text"]
        .as_str()
        .context("Local speech engine returned an invalid transcript")?
        .trim();
    if text.is_empty() {
        bail!("No speech was recognized. Try again closer to the microphone.");
    }
    Ok(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_never_treats_diagnostics_or_empty_output_as_a_transcript() {
        assert_eq!(
            decode_response(b"native diagnostic\n{\"flick_worker\":1,\"text\":\"hello\"}\n")
                .unwrap(),
            "hello"
        );
        for invalid in [
            b"engine failed".as_slice(),
            b"{\"flick_worker\":1,\"text\":\" \"}",
            b"{\"flick_worker\":1,\"error\":\"bad model\"}",
        ] {
            assert!(decode_response(invalid).is_err());
        }
    }
}
