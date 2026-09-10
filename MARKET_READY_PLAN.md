# Flick market-ready plan

This is the delivery plan for turning Flick from a feature-complete beta into
a product that can be installed, trusted, and sold. It records what is already
implemented, the evidence required to call it stable, and the remaining work
in the order that reduces user-facing risk first.

## Current baseline — September 2026

| Area | State | Evidence / boundary |
| --- | --- | --- |
| Desktop targets | Automated verification | Windows x64/ARM64, macOS Intel/Apple Silicon, and Linux x64 build and test in `Verify`. |
| Writing and reply workflows | Implemented | Explicit selection, editable draft, protected-target refusal, and copy recovery are in place. |
| Local dictation | Implemented | Whisper tiers plus verified Parakeet, Canary, Qwen3 ASR, SenseVoice, and Moonshine artifacts. |
| Cloud dictation | Implemented | Explicit OpenAI-compatible endpoint, separate keychain key, HTTPS requirement outside localhost, and no silent local-to-cloud fallback. |
| Model transfer safety | Implemented | One active transfer, cancellation, confirmed range resume, streaming SHA-256, atomic promotion, and an error event for every terminal state. |
| macOS-style workflow polish | Implemented and needs physical review | Floating transcription/transform state, focused reply composer, reduced-motion handling, and user-selectable pill placement. |
| Product site and README | Implemented | GitHub Pages landing page and onboarding-oriented README are live. |
| Signed public release | Blocked on release material | Certificates, notarization credentials, updater keys, and physical acceptance are intentionally not in source control. |

“Implemented” is not a release claim. A platform becomes releasable only when
the acceptance evidence below exists for its signed build.

## Product-quality bar

Flick is ready to market only when every primary flow meets these outcomes:

1. **Predictable start and recovery.** Opening Settings, Models, History, or
   an overlay never blocks the app or terminates a webview. A failed operation
   has a human-readable retry/copy/recovery path.
2. **Intentional data handling.** Text, audio, context, and keys remain local
   unless the user explicitly selects a cloud provider and triggers a request.
3. **Safe insertion.** A result is pasted only back into the intended,
   non-protected target. If that cannot be proven, it remains available to copy.
4. **Native-feeling feedback.** Recording, processing, cancellation, reply
   draft editing, and text transformation make their state obvious without
   interrupting the current app.
5. **Truthful compatibility.** A model is shown with its actual source,
   download size, engine, languages, auto-detect support, and translation
   support. Unsupported settings are prevented before transcription starts.

## Implementation sequence

### 1. Stabilize the model lifecycle

**Completed in code**

- Keep the Models view metadata-only; do not hash or load multi-gigabyte files
  while rendering it.
- Use pinned Hugging Face revisions and expected SHA-256/size for every
  catalog artifact.
- Stream to a `.partial` file, resume only after a confirmed `206` response,
  hash the complete file, then atomically promote it.
- Restrict transfers to one active job; cancellation, HTTP failure, integrity
  failure, and a recovered worker panic all clear the job state and report an
  actionable event.
- Keep unverified custom GGML/GGUF models conservative: explicit language only,
  no automatic detection or translation claim.

**Before beta promotion**

- Add small licensed audio fixtures for each supported engine family, or record
  a reproducible fixture-generation procedure, then run an opt-in real-engine
  smoke suite on each native platform.
- Exercise low-disk-space, offline, captive-portal, server-no-range, checksum
  mismatch, cancel/restart, app restart during download, and model deletion on
  clean machines.
- Add a support article showing the model disk/RAM trade-offs and troubleshooting
  steps rather than asking users to infer them from model names.

**Exit evidence:** a screen recording and diagnostics bundle from each platform
for the interruption cases, plus successful real-audio smoke results for every
advertised model family.

### 2. Make dictation dependable and understandable

**Completed in code**

- Support local Whisper and verified GGUF engines through the local catalog.
- Offer a separately configured OpenAI-compatible cloud path without a fallback
  that could unexpectedly upload audio.
- Gate language, language detection, and English translation by selected-model
  capabilities.
- Offer bottom-center, bottom-left, bottom-right, and top-center floating-pill
  placement for dictation and transformation feedback.

**Next implementation items**

- Add a short in-product model recommendation step: fastest/offline,
  multilingual, accuracy-focused, and cloud. It must be descriptive—not an
  automatic provider or model switch.
- Add per-model readiness states: not installed, downloading, verifying,
  ready, incompatible configuration, and recoverable error. Preserve the
  existing transfer event as the source of truth.
- Add device-switch and permission-denied test coverage around active recording.
- Measure end-to-end latency from shortcut to paste on representative hardware;
  surface only meaningful wait states, never spinners without a cancellation
  path.
- Perform a macOS native review for focus, accessibility permission language,
  VoiceOver labels, keyboard navigation, safe-area placement, and reduced motion.

**Exit evidence:** every dictation state is reachable by keyboard, announced to
assistive technology, and recoverable without restarting Flick.

### 3. Finish polished writing workflows

**Completed in code**

- The reply composer starts from an explicit selection, keeps the draft editable,
  and offers explicit Copy/Insert actions.
- Text transformation and dictation re-check the original paste target and keep
  completed output recoverable if focus changes.
- The floating UI has a compact status hierarchy and a shared user-selected
  placement preference.

**Next implementation items**

- Run qualitative acceptance sessions in Mail, Slack, Teams, Discord, browsers,
  Notes, Word, and a native code editor on each supported desktop platform.
- Tune animation durations, easing, contrast, type scale, and shadows with
  reduced-motion and high-contrast settings enabled; avoid animation that
  delays recording, copy, cancel, or insertion.
- Add screenshot-based visual regression coverage for idle, recording,
  transcribing, success, error, protected-target, and recovery states.
- Audit every actionable element for a visible focus ring, semantic label,
  escape/cancel behavior, and 44px-equivalent pointer target where the platform
  permits.

**Exit evidence:** annotated screenshot review and physical keyboard-only
walkthroughs across all major states.

### 4. Release engineering and support readiness

**Implementation items requiring external authority**

- Put Windows Authenticode, macOS Developer ID/notarization, Tauri updater, and
  any distribution signing material into protected CI secrets. Never commit,
  export, or paste them into diagnostics.
- Verify a signed installer on clean Windows x64/ARM64, macOS Intel/Apple
  Silicon, and Linux x64 machines.
- Configure a staged beta channel and a rollback procedure before enabling an
  updater feed.
- Define a concise support intake template: Flick version, platform/CPU,
  installed model ID, provider type (not key), reproduction steps, and optional
  redacted diagnostics.
- Publish privacy, retention, supported-platform, and model-compatibility
  policies on the product site before any public sales launch.

**Exit evidence:** every checkbox in [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md)
has a dated owner and a link to its signed-build acceptance record.

### 5. Keep the public front truthful

- Keep README quick start, GitHub Pages, in-app labels, and release notes driven
  from the same supported-model and platform facts.
- Show beta status until signed builds and physical acceptance complete; do not
  market automated CI as proof of microphone, paste, permission, or notarization
  behavior on customer devices.
- For each release, publish a focused changelog: user-visible changes, known
  limits, upgrade notes, and recovery steps.

## CI operating model

There are intentionally three visible workflows:

| Workflow | Responsibility | Trigger |
| --- | --- | --- |
| `Verify` | Frontend/unit/browser checks and the full native matrix | Push and pull request |
| `Release` | Draft signed-release packaging only | Version tag / manual release path |
| `pages-build-deployment` | GitHub-managed deployment of the Pages site | Pages source update |

Cancelled historical `Verify` entries are normally superseded commits, not
product failures. Investigate an entry only when GitHub marks it **failed**;
record its exact job/log cause, add a regression test where feasible, then
require a later full green matrix before promoting a build.

## Android assessment (not scheduled)

Android is feasible, but not a direct desktop build. The reusable pieces are
the provider contracts, cloud transcription client, model catalog metadata,
and settings model. Android would need a dedicated interaction model for
runtime microphone permission, foreground-service recording, notification and
quick-settings controls, audio-focus/phone-call interruption, battery limits,
keyboard/IME insertion, secure-field rules, and an on-device inference runtime
appropriate to Android hardware. Treat it as a separate product track after
desktop signed-beta acceptance—not as a release blocker for Flick desktop.

## Definition of done

Flick is a market-ready desktop product when the current full CI matrix is
green, signed installers are accepted on real target hardware, all release
checklist scenarios have evidence, the website/README accurately describe that
release, and every failure path preserves the user’s work or offers a clear
recovery action.
