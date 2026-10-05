# Flick 2.0.1

Flick is a private desktop writing companion for transforming text, drafting replies, and dictating in the app you already use.

Choose the installer matching your operating system and processor:

- Windows: `x64` for Intel/AMD; `arm64` for Snapdragon/Windows on ARM.
- macOS 11 or later: `aarch64` for Apple Silicon; `x64` for Intel Macs.
- Linux: x64 desktop packages; compositor compatibility depends on the target desktop.

## Highlights

- Compact, transparent status pills for text transformation and local dictation.
- A focused reply companion with captured selection context, a prominent tone choice, editable draft, and Copy/Insert controls.
- Custom shortcuts with duplicate and reserved-key protection, including stable push-to-talk behavior.
- Safer insertion: Flick verifies the original target and leaves the result available to copy if focus has changed.
- Local speech model choice, optional OpenAI-compatible cloud transcription, and no silent provider fallback.
- Model downloads that resume safely and verify their expected hashes before becoming available.
- Native transcription isolated from the interface process so recoverable engine errors keep the app responsive.

Verify downloaded files against the target-specific `SHA256SUMS` file. When reporting an issue, include the OS and processor, clear reproduction steps, and a redacted diagnostic bundle. Never include API keys, private messages, recordings, or transcripts.
