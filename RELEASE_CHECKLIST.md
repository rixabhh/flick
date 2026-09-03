# Flick 2.0 release checklist

## Required signing material

- Windows: an Authenticode code-signing certificate and timestamp service credentials.
- macOS: Developer ID Application certificate, App Store Connect issuer/key/ID, and notarization credentials.
- Updates: the Tauri updater signing key and public key configured for the release feed.
- Linux: distribution-specific package-signing key where the selected channel requires one.

Do not place any signing material in this repository, settings file, template export, diagnostics bundle, or application package. Configure it as protected CI secrets only.

## Required beta acceptance

- Verify the signed installer on clean Windows x64 and ARM64 machines.
- Verify signed/notarized builds on macOS Intel and Apple Silicon.
- Verify Linux x64 on X11 and the supported Wayland desktop environments.
- Exercise a physical microphone, a Bluetooth microphone, and device switching during recording.
- Check global shortcuts, selected-text composer capture, protected fields, target-change fallback, and paste restoration in browsers, Slack, Teams, Discord, and native editors.
- Test model interruption/resume, cancellation, no-network offline transcription, model deletion, history retention, and local-data deletion.
- Confirm multi-monitor/DPI overlay behavior and keyboard-layout handling.
- Publish beta builds with crash/log collection disabled unless the user expressly opts in.

## Promotion rule

Promote a platform only after its signed beta passes the above functional and paste-reliability checks. Keep the beta release draft until every target has its own acceptance evidence.
