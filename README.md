<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="Flick logo" width="112" height="112" />
  <h1>Flick</h1>
  <p><strong>Speak, rewrite, and reply anywhere.</strong></p>
  <p>A small desktop assistant that works in the apps you already use.</p>
  <p>
    <a href="https://rixabhh.github.io/flick/">Website</a> ·
    <a href="https://github.com/rixabhh/flick/releases">Downloads</a> ·
    <a href="#quick-start">Setup guide</a> ·
    <a href="#privacy">Privacy</a>
  </p>
</div>

Flick helps you dictate, improve text, and draft replies without moving your work into another editor. Choose private local speech models, or connect your own cloud transcription provider when you want one.

> Flick 2.0.1 is a release candidate. The code and unsigned build candidates are available now; public release still requires signed and notarized installers plus final testing on physical devices. See the [release checklist](RELEASE_CHECKLIST.md).

## What can I do with Flick?

| I want to… | What to do |
| --- | --- |
| Speak instead of type | Press your dictation shortcut, wait for **Recording**, speak, then press it again. |
| Improve a sentence | Add a command such as `!fix` or `!shorter` after the text. |
| Draft a reply | Select a message, press your reply shortcut, choose a tone, and review the draft. |

Flick stays out of the way until you use a shortcut or command. The dictation and transformation pill can sit at the top or bottom of your display, and the reply assistant opens as a compact companion near your cursor.

## Quick start

### 1. Install the right build

Open [Downloads](https://github.com/rixabhh/flick/releases) and match the file to your computer:

- **Windows:** choose `x64` for most Intel/AMD PCs or `arm64` for Snapdragon/Windows on ARM devices.
- **macOS:** choose Apple Silicon for M-series Macs or Intel for older Macs.
- **Linux:** choose the x64 package for your distribution.

Approve microphone and accessibility/input permissions when your operating system asks.

### 2. Choose how dictation runs

Open **Settings → Models** and download a small local model. **Whisper Tiny English** is a good first test. Flick shows the language support and download size before you choose it.

If you prefer cloud transcription, open **Settings → Dictate**, choose the cloud option, and enter your own OpenAI-compatible endpoint, model, and API key. Flick never falls back from local to cloud by itself.

### 3. Set your shortcuts

Open **Settings → Write** to record a reply shortcut and **Settings → Dictate** to record a dictation shortcut. You can also customize Copy Last Result and Paste as Plain Text under **Advanced**. Flick rejects shortcuts that conflict with another Flick action.

### 4. Try your first dictation

1. Click a normal text field in any supported app.
2. Press the dictation shortcut.
3. Wait until the pill says **Recording**, then speak.
4. Press the shortcut again, or release it in push-to-talk mode.
5. Flick transcribes and returns the text to the field you started from.

Press `Esc` while recording to discard the audio. Change the pill position under **Settings → Dictate → Floating pill position**.

## Improve text

Write normally, then add a command at the end:

| Command | Result |
| --- | --- |
| `!fix` | Fix spelling, grammar, and punctuation. |
| `!formal` | Use a more professional tone. |
| `!casual` | Make the writing friendlier. |
| `!shorter` | Make the text more concise. |
| `!longer` | Add useful detail. |
| `!rephrase` | Say the same thing more clearly. |
| `!bullet` | Turn the text into a list. |
| `!translate:spanish` | Translate to the language you name. |

Create your own commands under **Settings → Commands**.

## Draft a reply

1. Select only the message or text you want Flick to use as context.
2. Press your reply shortcut.
3. Choose a tone and describe what you want to say.
4. Generate the reply and edit it if needed.
5. Choose **Copy** or **Insert into app**.

Flick does not scan your screen or read whole conversations. It uses only the text you deliberately select, and it checks that you are still in the original app before inserting anything.

## Dictate

### Local models

Flick’s catalog contains **370 pinned model downloads** from Hugging Face, including Whisper, Parakeet TDT/CTC/RNNT, Moonshine, Canary, Qwen3 ASR, SenseVoice, and other families supported by the speech engine. Search by family or show every available quantization when you want more control.

Each listed file has a pinned source revision, expected size, and SHA-256 hash. Downloads use a temporary partial file, resume only when the server confirms the requested range, and become selectable only after verification. Every model page links to its source and license.

The catalog is broader than the release acceptance set: not every model has been tested for speed and accuracy on every device. Release builds run real audio through representative Whisper, Moonshine, and Parakeet models on supported targets; Flick turns engine failures into recoverable errors instead of allowing the desktop app to crash.

### Cloud models

Cloud dictation is optional and OpenAI-compatible. Flick sends audio only when you choose the cloud provider and start a dictation. The transcription key is separate from your writing-provider key and is stored in your operating system keychain.

### Activation modes

- **Toggle:** press once to start and once to stop.
- **Push to talk:** hold the shortcut while speaking.
- **Hold or toggle:** tap for toggle behavior or hold for push to talk.

## Shortcuts

These are the defaults. You can replace them in Settings.

| Action | Windows / Linux | macOS |
| --- | --- | --- |
| Reply assistant | `Ctrl+Shift+Space` | `Cmd+Shift+Space` |
| Dictation | `Ctrl+Space` | `Cmd+Space` |
| Copy last result | `Ctrl+Alt+C` | `Cmd+Alt+C` |
| Paste as plain text | `Ctrl+Alt+V` | `Cmd+Alt+V` |

## If something is not working

- **The microphone does not start:** allow microphone access, then check the selected device under **Dictate**.
- **A shortcut does nothing:** record it again in Settings and make sure another Flick action does not use it.
- **Nothing was pasted:** Flick refuses to paste after you switch apps or into a protected field. Use **Copy last result** instead.
- **A model download stopped:** choose Download again. Flick will safely resume or restart it and verify the whole file.
- **A model cannot be used:** remove the failed download and try the recommended smaller model. The app remains running even if the native engine fails.
- **Cloud generation fails:** check the provider endpoint, model name, and key under **Write** or **Dictate**.
- **Wayland blocks the shortcut:** bind your desktop shortcut to one of the Flick commands listed below.

If the issue continues, export redacted diagnostics from Settings and attach them to a [GitHub issue](https://github.com/rixabhh/flick/issues). Diagnostics exclude API keys, clipboard text, prompts, replies, and transcript contents.

## Privacy

- Local dictation keeps audio on your device.
- Cloud dictation is opt-in and uses only the provider you configure.
- Reply context comes only from your explicit text selection.
- API keys are stored in the operating system keychain.
- Flick refuses protected password fields and apps you exclude.
- Recordings and history are off by default and can be deleted from Settings.
- Flick has no telemetry by default.

Optional AI cleanup sends only the finished transcript—not the audio—to your configured writing provider.

## Desktop commands

Desktop shortcut tools can send a fixed action to a running Flick instance:

```text
flick --open-settings
flick --open-composer
flick --toggle-dictation
flick --cancel-dictation
flick --copy-last-result
```

The command line does not accept text, prompts, or shell commands.

<details>
<summary><strong>Linux notes</strong></summary>

The recording overlay is hidden by default on Linux because some compositors can take focus from the active text field. Flick prefers `xdotool` on X11 and `wtype` or `dotool` on Wayland when available. Restricted Wayland desktops may require a desktop shortcut that calls the Flick command line.

</details>

<details>
<summary><strong>Development and verification</strong></summary>

Install Node.js 20+, Rust 1.77+, and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/rixabhh/flick.git
cd flick
npm ci
npm run tauri dev

npm test
npm run test:e2e
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

CI has three clear responsibilities: **Verify** checks source and tests, **Release** builds and smoke-tests installers without publishing by default, and GitHub’s managed workflow deploys **Pages**. A release draft is created only after every platform succeeds.

The repository structure is straightforward:

- `src-tauri/src/` — Rust desktop services, speech, models, shortcuts, and native integration.
- `src/lib/` — Svelte settings, reply assistant, overlays, and UI helpers.
- `docs/` — the GitHub Pages product site.
- `.github/workflows/` — Verify and Release automation.
- `RELEASE_CHECKLIST.md` — signing, notarization, hardware, and promotion gates.
- `MARKET_READY_PLAN.md` — the product hardening plan and acceptance criteria.

</details>

## License

Flick is available under the [MIT License](LICENSE). Model files keep their own upstream licenses; the Models screen links to each one.
