<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="Flick Logo" width="128" height="128" />
  <h1>Flick</h1>
  <p><strong>System-wide AI text transformation at your fingertips.</strong></p>

  <p>
    <a href="https://github.com/rixabhh/flick/actions"><img src="https://img.shields.io/github/actions/workflow/status/rixabhh/flick/release.yml?style=flat-square" alt="Build Status"></a>
    <a href="https://v2.tauri.app/"><img src="https://img.shields.io/badge/Tauri-v2-24C8DB?style=flat-square&logo=tauri" alt="Tauri v2"></a>
    <a href="https://svelte.dev/"><img src="https://img.shields.io/badge/Svelte-UI-FF3E00?style=flat-square&logo=svelte" alt="Svelte UI"></a>
    <a href="https://choosealicense.com/licenses/mit/"><img src="https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square" alt="License"></a>
  </p>
</div>

<br />

Flick is a cross-platform desktop utility that brings the power of LLMs directly to your cursor. Simply type a command prefix (e.g., `!translate` or `!casual`) followed by your text anywhere on your computer, and Flick instantly rewrites it in-place. 

No context switching, no copy-pasting—just native text replacement powered by Google's Gemini Flash.

## 📋 Table of Contents

- [Features](#-features)
- [How It Works](#-how-it-works)
- [Available Commands](#-available-commands)
- [Architecture](#-architecture)
- [Getting Started](#-getting-started)
- [Security & Privacy](#-security--privacy)
- [License](#-license)

## ✨ Features

- **Global Integration:** Works seamlessly across any application, text editor, or browser.
- **In-place Execution:** Replaces text natively at the cursor position without opening external windows.
- **Customizable Pipelines:** Define custom triggers (e.g., `!summarize`, `!professional`) tailored to your workflow.
- **BYOK (Bring Your Own Key):** Connect directly to the Gemini Flash API using your personal API key.
- **Lightweight Footprint:** Built on Tauri v2 and Rust for minimal memory usage and lightning-fast execution.

## 🚀 How It Works

Flick runs silently in your system tray, monitoring keyboard input via an efficient, low-level OS hook. 

1. **Type a trigger:** Start typing anywhere, prefixing your text with a command (e.g., `!casual`).
2. **Detection:** Flick captures the buffer and detects the trigger once you stop typing.
3. **Processing:** The text is routed through your OS clipboard, sent to the Gemini API, and instantly pasted back to your active cursor position.

## ⚡ Available Commands

### Built-in 
| Command | Description |
|---|---|
| `!translate [lang]` | Translates the preceding text into the specified language. |
| `!grammar` | Corrects spelling and grammatical errors. |

### Custom Triggers (Configurable)
You can easily add new commands in the Settings panel:
- `!casual` - Rewrites text to sound more casual.
- `!professional` - Elevates the tone for business communication.
- `!summarize` - Condenses long paragraphs.

## 🏗️ Architecture

Flick follows a strict separation of concerns, utilizing Tauri's split-process architecture:

- **Core Engine (Rust):** Handles the global event hook (`rdev`), input buffering, trigger detection, OS keychain integration, and the clipboard manipulation pipeline.
- **Frontend (Svelte + Vite):** A responsive, fluid settings dashboard and floating toast indicators built with modern web technologies.

## 🛠️ Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v20 or higher)
- [Rust toolchain](https://rustup.rs/) (v1.75 or higher)
- [Tauri v2 OS dependencies](https://v2.tauri.app/start/prerequisites/)

### Build & Run

1. **Clone the repository**
   ```bash
   git clone https://github.com/rixabhh/flick.git
   cd flick
   ```

2. **Install frontend dependencies**
   ```bash
   npm install
   ```

3. **Run in development mode**
   ```bash
   npm run tauri dev
   ```

4. **Build release binaries**
   ```bash
   npm run tauri build
   ```

## 🔒 Security & Privacy

We take privacy seriously. Flick is designed to be as secure as possible:

- **Local Memory Buffer:** Keystrokes are temporarily held in an ephemeral memory buffer that is strictly bounded in size. The buffer is immediately cleared upon mouse clicks, Enter, or navigation keys.
- **Secure Key Storage:** API keys are encrypted and stored using your operating system's native secure credential manager (Windows Credential Manager, macOS Keychain, or Linux Secret Service).
- **Direct API Communication:** Flick communicates exclusively and directly with the Google Gemini API. There are no telemetry, analytics, or middleman servers.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
