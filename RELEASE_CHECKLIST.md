# Flick 2.0 release checklist

## Required signing material

- Windows: an Authenticode code-signing certificate and timestamp service credentials.
- macOS: Developer ID Application certificate, App Store Connect issuer/key/ID, and notarization credentials.
- Updates: currently manual downloads. An automatic updater must not be advertised until its plugin, signed feed, and upgrade/rollback testing exist.
- Linux: distribution-specific package-signing key where the selected channel requires one.

Do not place any signing material in this repository, settings file, template export, diagnostics bundle, or application package. Configure it as protected CI secrets only.

## Reproducible build and draft process

1. Run `Verify` on `main`. It must finish successfully for all five native targets and the browser regression suite.
2. Run `Release` manually on `main` with **Create draft unchecked** and **tag empty**. This runs native tests and the actual release-mode installer builds without creating tags or releases. Installers and SHA-256 manifests are retained as workflow artifacts for 14 days.
3. For an intentional candidate release, keep `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` versions identical; create the corresponding `v<version>` tag on the verified main commit. Pushing that tag runs Release. Alternatively choose an existing matching tag and check Create draft.
4. All platforms must finish before the final job assembles a draft. A failed platform never publishes a partial release. Published releases cannot be overwritten by reruns; draft assets can be replaced deliberately by a rerun.
5. Validate artifact checksums, signing, clean-machine installation and feature acceptance before manually promoting the draft. A build-only success is not signing or native acceptance evidence.

The macOS bundle and both compiler jobs use macOS **11.0** as the minimum.
Tauri previously defaulted the packaging compiler to 10.13, overriding the CI
environment and breaking `std::filesystem` in the speech engine. Do not remove
`bundle.macOS.minimumSystemVersion`. `Info.plist` declares microphone use, and
the hardened-runtime entitlements allow microphone input.

The transparent macOS pills enable Tauri's `macos-private-api` feature. This
configuration targets direct downloads and Developer ID notarization, **not
the Mac App Store**. A store edition requires a separate public-API overlay
implementation before submission.

Optional macOS CI signing uses `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD` (app-specific), and `APPLE_TEAM_ID`. Missing secrets produce
unsigned test candidates, not certified releases. Configure and validate the
Windows certificate/signing integration before public promotion; it is not
currently configured. No signing secrets were listed in the repository audit.

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
