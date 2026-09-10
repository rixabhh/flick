# Flick 2.0.1 release candidate

Choose the installer matching your operating system and processor:

- Windows: `x64` for Intel/AMD; `arm64` for Snapdragon/Windows on ARM.
- macOS 11 or later: `aarch64` for Apple Silicon; `x64` for Intel Macs.
- Linux: x64 desktop packages; compositor compatibility must be checked on the target desktop.

This is a release candidate, not a claim of completed platform certification.
Unsigned builds can trigger operating-system warnings. Do not disable system
security checks to install them. Public promotion requires signing/notarization
and the device acceptance evidence in RELEASE_CHECKLIST.md.

Included: local speech model choice, optional cloud dictation with a separate
key, safe text transformations, reply drafting, configurable floating pills,
and recoverable model transfers. Local audio is never silently sent to cloud.

Verify downloaded files against the included target-specific SHA256SUMS file.
Report issues with OS/processor, reproduction steps and a redacted diagnostic
bundle. Never include API keys, private messages, or recordings in an issue.
# 2.0.1 candidate additions

- Compact 390 × 470 reply companion opens near the pointer and stays within the monitor work area. Context and request collapse when a draft is ready; Copy and Insert remain visible.
- Custom shortcut recording with duplicate/reserved-key validation; fixed Alt tracking, repeated-key toggles, extra-modifier collisions and push-to-talk release ordering.
- Removed native accessibility work from the OS keyboard callback, disk settings reads from microphone startup, and AppleScript process launches from macOS target checks. The pill distinguishes microphone startup from active recording.
- Fixed heap/stack ownership in async model hashing after a Windows `0xc00000fd` crash report. Added future-size regression checks.
- Expanded the local library to 370 pinned artifacts with family/search/quantization filters and per-model licenses. Existing model IDs remain valid.
- Native transcription is isolated in a bounded-lifetime worker using anonymous audio pipes. Engine failures do not intentionally exit the UI process; there is no cloud fallback.
- Release packaging now checks actual Whisper, Moonshine and Parakeet transcription, plus invalid-model recovery, before uploading installers.

This is still a release candidate: signing credentials, real-device permissions,
clipboard compatibility and acceptance across the full model catalog remain
separate release gates. Upgrade from 2.0.0 to identify this candidate reliably.
