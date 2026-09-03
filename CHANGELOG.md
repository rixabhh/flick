# Flick release notes

## 2.0.0 — Beta

### Write anywhere

- Added a selected-text reply composer with tone controls, editable drafts, and explicit Copy or Insert actions.
- Added Gemini, OpenRouter, and OpenAI-compatible provider support. Local compatible endpoints may omit an API key.
- Preserved existing `!` writing commands and migrated custom commands to stable IDs.
- Added an original-target guard before reply insertion; when the target cannot be confirmed, Flick keeps the draft available to copy.

### Dictate privately

- Added local microphone capture and offline Whisper transcription.
- Added verified, resumable local model downloads, custom local model discovery, multilingual dictation, translation to English, filler cleanup, and personal corrections.
- Added multilingual Base, Small, Medium, Large v3 Turbo, and Large v3 model choices, plus adaptive local voice-activity detection before transcription.
- Added toggle, push-to-talk, and hold-or-toggle activation modes, a non-focus-stealing recording status overlay, and Escape-to-discard.
- Added optional local WAV retention, append-space, and opt-in per-app auto-submit.

### Privacy and reliability

- Added optional local SQLite history, saved entries, retention limits, deletion controls, and a tray action to copy the last result.
- Added a privacy dashboard, app exclusion list, local diagnostics export, theme selection, and first-run setup guidance.
- Added English and Spanish command-center and reply-composer localization with a local fallback-safe translation layer.
- Added command-line actions for external hotkey tools: settings, composer, dictation toggle, and copy-last-result.
- Added verification workflows for Windows x64/ARM64, macOS Intel/Apple Silicon, and Linux x64.

### Upgrade notes

- Existing provider, model, enabled, launch, toast, trigger, and custom-command settings are retained during migration.
- Offline dictation requires a downloaded local model. Audio is not sent to an AI provider for local transcription.
