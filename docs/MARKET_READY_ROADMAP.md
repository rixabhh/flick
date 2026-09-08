# Flick market-ready roadmap

## Product objective

Ship Flick as a reliable, approachable desktop assistant whose three workflows
feel equally deliberate and dependable:

1. **Dictate** — say text, see clear state, and safely receive the result in
   the original target.
2. **Reply** — explicitly select context, create an editable draft, and copy
   or insert it without surprising data collection or loss.
3. **Transform** — invoke a `!` command in place and get an understandable,
   cancellable result from the configured writing provider.

The release is market-ready only when a new user can complete each workflow
without documentation, recover from expected failures without losing work, and
trust Flick with sensitive daily communication.

## Audit baseline — 8 September 2026

### What is already built

- A Tauri 2 / Svelte 5 desktop app for Windows x64/ARM64, macOS Intel/Apple
  Silicon, and Linux x64.
- A local-first writing transformer with Gemini, OpenRouter, and custom
  OpenAI-compatible text endpoints, keychain-backed credentials, custom
  commands, clipboard restoration, protected-target exclusions, history, and
  diagnostics export.
- A selected-text reply composer with editable context and draft, tone,
  explicit Copy/Insert actions, a target-change guard, and localization.
- Dictation with microphone selection, hold/push/toggle triggers, adaptive VAD,
  local Whisper/Parakeet inference, an explicit OpenAI-compatible cloud path,
  filler cleanup, personal corrections, recording retention, optional
  text-only LLM cleanup, and safe paste-back.
- Verified/resumable/atomic Hugging Face model downloads, Whisper `.bin` and
  GGUF discovery, and test coverage for model/download and text-processing
  behavior.
- Browser smoke tests and native verification on all five current CI targets.

### Current constraints and risks

- The initial local engine now accepts Whisper GGML and compatible GGUF
  artifacts, including Parakeet V3. It does not yet support ONNX families,
  Hugging Face cache discovery, user-visible hardware/resource guidance, or
  a tested cancellation path for cloud requests.
- The model page presents a flat list, while the dictation settings describe
  engine availability that does not exist. It does not help a user choose for
  speed, language, privacy, RAM, quality, or connection status.
- The three workflows are implemented as separate surfaces with uneven states,
  error language, loading feedback, and recovery affordances. The settings
  page is a large tabbed control center rather than an opinionated first-run
  path.
- CI proves compilation and a browser UI smoke flow, but does not exercise a
  real microphone, real speech fixtures by default, paste reliability in real
  target applications, interrupted model downloads, network failure recovery,
  or accessibility keyboard journeys across the native windows.
- The work now continues directly on `main`; no separate development branch is
  used. No GitHub issues or releases exist.
  This means there is no external defect triage, crash signal, beta cohort,
  or published installer evidence to use as a release decision.

### GitHub Actions findings

The repository has six historical failed runs, all in the **Release** workflow.
The June failures were cross-platform Tauri packaging failures; the September
3 failures were macOS packaging failures after Windows and Linux had passed.
The corresponding fixes installed platform LLVM/native dependencies, switched
Windows ARM Whisper to clang with exceptions enabled, and set supported macOS
deployment targets.

The recovery is verified: the latest Release run (33787866787) is successful
for browser smoke plus Linux x64, Windows x64/ARM64, macOS Intel, and macOS
Apple Silicon. The latest Verify run (33788517323) is also successful across
the same browser/native matrix. Earlier Verify cancellations are expected from
its `cancel-in-progress` concurrency policy, not test failures. Release
success has not yet produced a published GitHub release; the workflow creates
draft prereleases only when invoked for a tag.

Workflow topology is intentionally limited to three active workflows: **Verify**
for pushes and pull requests on `main`, **Release** for tags/manual packaging,
and GitHub's managed **pages-build-deployment** workflow. The former duplicate
checked-in Pages workflow was removed. Historical runs remain visible in GitHub
as audit history and are not active workflows.

### Progress since the baseline

- `ee7648a` introduced the provider contract while preserving Local Whisper as
  the private default.
- `f3f5280` added the first opt-in cloud provider: an OpenAI-compatible
  `/audio/transcriptions` transport with a separate keychain credential,
  HTTPS-only endpoints (except localhost), a 90-second timeout, WAV encoding,
  a visible audio-upload disclosure, and no silent provider fallback. Its
  cross-platform verification is the release gate for this implementation.
- `b0d7a22` added Parakeet TDT 0.6B v3 as a pinned, checksum-verified GGUF
  catalog item from Hugging Face and switched the local runtime to a model
  engine that supports both existing Whisper artifacts and compatible GGUF.
  Translation remains intentionally unavailable for Parakeet rather than
  making an unsupported capability claim.
- `5a7cf23` fixed reply-context capture so the composer hides before the
  operating-system selection is read; it no longer captures its own textarea.
- `07d7196` completed transform lifecycle signalling so a disabled completion
  toast does not leave a permanent “Transforming” status and provider failures
  surface to the user.
- `f183e7e` added a shared, user-controlled position for dictation and
  transformation pills and a native-window UI pass: animated recording and
  processing states, accessible status/error states, keyboard-first reply
  controls, and reduced-motion behavior.
- `7598f40`, `b22ca99`, and `24bb72e` made local dictation capability-aware:
  model selection now clears invalid translation state, rejects unsupported
  language hints before native inference, exposes the active model’s facts in
  Settings, and presents those facts in the model chooser.
- `93dc22c` and `46f5fa7` close the transform paste race: the original target
  and protected-field state are rechecked after provider latency. A refused
  paste never overwrites the newly focused app and leaves a session-only
  recovery result for **Copy last result** without forcing persistent history.

## Architecture decision: provider-aware dictation

Introduce a stable `DictationProvider` contract so that recording, target
protection, post-processing, history, and paste-back remain shared. Only the
transcription transport changes by provider.

```text
microphone -> normalize/VAD -> DictationProvider -> transcript
                                      |              |
                           local engine or cloud     v
                            request with timeout  shared cleanup/history
                                                    -> guarded paste/copy
```

Provider categories:

| Category | Initial engines/providers | User value |
| --- | --- | --- |
| Local Whisper | Existing Whisper models, preserved | Offline, established behavior |
| Local ONNX | Parakeet first; then Moonshine and SenseVoice | Fast and accurate alternatives |
| Cloud OpenAI-compatible STT | OpenAI Audio Transcriptions API shape; custom base URL | BYOK interoperability |
| Cloud marketplace | OpenRouter only after API capability validation | Optional choice, never a silent fallback |

Cloud audio must be opt-in per provider. The UI must display the destination,
model, data type (audio), estimated upload size, and failure fallback before a
recording begins. API keys remain keychain-only. Local dictation must remain
the default and must never silently route audio to a cloud service.

## Implementation sequence and commits

Each commit must be independently buildable, have focused tests, use
`rixabhh <rishabh0singh0@gmail.com>` as its author, and be pushed directly to
`main`.

1. `docs: add market-ready audit and delivery roadmap`
   - This document, CI evidence, scope, release criteria, and commit plan.

2. `refactor(dictation): introduce provider-neutral transcription contract`
   - Split audio capture/VAD, transcription orchestration, model metadata, and
     post-processing into explicit interfaces.
   - Migrate existing Whisper behavior without changing its settings values or
     on-disk model paths.
   - Add state-machine tests for idle, recording, processing, cancelling,
     error, and target-change paste fallback.

3. `feat(dictation): add opt-in cloud transcription providers`
   - The initial OpenAI-compatible transport is implemented in `f3f5280`.
     Complete it with a non-billable connection check where supported,
     request-size limits, cancellation, redacted diagnostics, and mock-server
     integration coverage for success, auth failure, malformed response,
     timeout, and no-unintended-retry.
   - Support OpenRouter only when its STT route and response contract are
     confirmed.

4. `feat(dictation): add verified local Parakeet support`
   - Port the minimal proven Handy model descriptor approach: engine type,
     Hugging Face source with pinned revision, artifact format, capabilities,
     checksums, download status, and compatibility checks.
   - Ship Parakeet V3 as the first additional local provider; add Moonshine/SenseVoice only
     after their fixtures, licensing, memory budget, and platform packages are
     verified.
   - Use Hugging Face Hub cache discovery so already-downloaded eligible models
     are recognized without duplication. Preserve resumable, cancellable,
     checksum-verified, atomic downloads; use a verified mirror only after an
     explicit Hub failure; add model load/unload and incompatible-language
     tests.

5. `feat(dictation): make model choice and recording recovery product-grade`
   - Replace the flat model list with Local/Cloud choices, recommended presets,
     capability badges, resource estimates, download/error/retry status, and
     an explicit privacy mode.
   - Add a compact recording lifecycle: ready, listening, processing, ready to
     paste, copied/fallback, and actionable errors.

6. `refactor(composer): make reply drafting resilient and keyboard-first`
   - Establish a clear state model for capture, draft, edit, generate,
     cancel/retry, copy, and insertion fallback.
   - Preserve context and draft across provider failures; add keyboard focus,
     Escape semantics, character guidance, accessible live announcements, and
     copy-first recovery.
   - Add unit tests for target changes and end-to-end tests for selection,
     provider failure, regenerate, and explicit insertion.

7. `refactor(transform): standardize command execution and recovery`
   - Build the same pending/success/failure/cancel vocabulary used by dictation
     and reply.
   - Validate malformed command syntax before network calls, provide a safe
     retry/copy outcome, and surface provider/model configuration errors inline.
   - Add provider-contract tests and cross-app replacement regression tests.

8. `feat(ui): unify the product shell and first-run experience`
   - Replace dense configuration-first presentation with a short setup path:
     choose writing provider, choose local or cloud dictation, test microphone,
     and try each workflow.
   - Apply one tokenized visual system across settings, composer, model chooser,
     toast, and recording overlay; eliminate duplicated styles and hard-coded
     states.
   - Complete keyboard navigation, contrast, reduced-motion, DPI, window-size,
     and English/Spanish acceptance checks.

9. `test: add release-critical native workflow coverage`
   - Add deterministic audio fixtures and transcription assertions for every
     supported engine/provider, mock cloud tests, download interruption tests,
     and state-machine tests.
   - Expand Playwright coverage for setup, model management, dictation
     recovery, reply compose/retry/copy, and text transformations.
   - Add a manually executed hardware matrix for real microphones, Bluetooth
     switching, browser/Slack/Teams/Discord/native editors, multi-monitor/DPI,
     X11, and supported Wayland environments.

10. `ci: gate beta promotion on packaged acceptance evidence`
    - Separate verification from signed release packaging; retain artifacts and
      publish a machine-readable test summary.
    - Run security/dependency checks, license provenance review, artifact
      smoke-install tests, and updater-signature checks.
    - Require signed/notarized installer evidence before promoting a platform.

## Release exit criteria

- All automated checks are green on the five build targets, with no allowed
  failures or unreviewed cancellations.
- Every selected dictation provider has an explicit privacy disclosure,
  cancellation behavior, connection test, and deterministic integration test.
- A new user can finish first-run setup in under three minutes and recover from
  microphone, model, network, and target-change failures without losing text.
- Dictation, Reply, and Transform pass the native application acceptance
  matrix and accessibility review on each supported platform.
- Signed/notarized beta installers are clean-installed and smoke-tested on
  physical Windows x64/ARM64 and macOS Intel/Apple Silicon machines, plus
  Linux X11 and supported Wayland desktop environments.
- A small opt-in beta has a support/diagnostics path, a public issue template,
  and an owner for triage before general availability.
