# Flick 2.0 beta candidate

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
