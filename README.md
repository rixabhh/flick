<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="Flick Logo" width="128" height="128" />
  <h1>Flick</h1>
  <p><strong>System-wide AI text transformation at your fingertips.</strong></p>

  <p>
    <a href="https://v2.tauri.app/"><img src="https://img.shields.io/badge/Tauri-v2-24C8DB?style=flat-square&logo=tauri" alt="Tauri v2"></a>
    <a href="https://svelte.dev/"><img src="https://img.shields.io/badge/Svelte-UI-FF3E00?style=flat-square&logo=svelte" alt="Svelte UI"></a>
    <a href="https://choosealicense.com/licenses/mit/"><img src="https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square" alt="License"></a>
  </p>
</div>

<br />

Flick is a cross-platform desktop utility that brings AI text transformation directly to your cursor. Simply type a trigger command (for example `!translate` or `!casual`) followed by your text anywhere on your computer, and Flick instantly rewrites it in-place.

No context switching, no copy-pasting — just native text replacement powered by your selected provider and model.

## 📋 Table of Contents

- [Features](#-features)
- [How It Works](#-how-it-works)
- [Available Commands](#-available-commands)
- [Architecture](#-architecture)
- [Getting Started](#-getting-started)
- [Installation Guide](#-installation-guide)
- [Security & Privacy](#-security--privacy)
- [License](#-license)

## ✨ Features

- **Global Integration:** Works seamlessly across any application, text editor, or browser.
- **In-place Execution:** Replaces text natively at the cursor position without opening external windows.
- **Provider Selection:** Choose between Gemini and OpenRouter from the settings panel.
- **Model Control:** Select a Gemini model or enter an OpenRouter model manually.
- **Customizable Pipelines:** Define custom triggers (e.g., `!summarize`, `!professional`) tailored to your workflow.
- **Hindi + Hinglish Aware:** Built-in and custom prompts preserve Hindi, Hinglish, and code-mixed writing unless you explicitly translate.
- **BYOK (Bring Your Own Key):** Connect directly to your chosen provider using your personal API key.
- **Secure Key Storage:** API keys are stored in your OS keychain/credential manager.
- **Lightweight Footprint:** Built on Tauri v2 and Rust for minimal memory usage and lightning-fast execution.

## 🚀 How It Works

Flick runs silently in your system tray, monitoring keyboard input via an efficient, low-level OS hook. 

1. **Type a trigger:** Start typing anywhere, prefixing your text with a command (e.g., `!casual`).
2. **Detection:** Flick captures the buffer and detects the trigger once you stop typing.
3. **Processing:** The text is routed through your OS clipboard, sent to the selected AI provider, and instantly pasted back to your active cursor position.

## ⚡ Available Commands

Flick comes with 9 powerful built-in commands designed to handle the most common writing tasks. 

### Built-in Triggers
| Command | What it does for you | Example Input |
|---|---|---|
| `!fix` | Cleans up grammar, spelling, and punctuation errors. | "this rly needs fixing!fix" |
| `!formal` | Elevates your text into a professional, business-ready tone. | "hey im gonna be late to the meeting!formal" |
| `!casual` | Softens your text to sound more friendly and relaxed. | "Please advise on the status of the project.!casual" |
| `!shorter` | Condenses long-winded paragraphs into concise summaries. | "(A very long paragraph)!shorter" |
| `!longer` | Expands brief notes or bullet points with rich detail and context. | "Product launch next week!longer" |
| `!rephrase` | Rewrites your sentence completely while keeping the exact same meaning. | "It's hard to understand this.!rephrase" |
| `!bullet` | Structures messy notes into a clean, readable bullet point list. | "milk eggs bread and some juice!bullet" |
| `!explain` | Breaks down complex text into simple, easy-to-understand language. | "(Dense academic text)!explain" |
| `!translate:<lang>` | Instantly translates your text to the specified language. | "Hello, how are you today?!translate:spanish" |

### Build Your Own (Custom Triggers)
Need something specific to your workflow? You can easily create custom triggers in the Settings panel:
- Define a trigger (e.g., `!code`, `!tweet`, `!docs`)
- Write a system prompt describing what the command should do (e.g., `"Turn this into a concise product update"`). Existing `{{text}}` templates are still supported.

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

## � Installation Guide

### Windows

1. Download the latest `.msi` or `.exe` installer from the Releases page.
2. Run the installer and follow the prompts.
3. If Windows shows a SmartScreen warning, click **More info** and then **Run anyway**.
4. After install, launch Flick from the Start menu or system tray.
5. Open Settings and add your API key.

### macOS

1. Download the latest `.dmg` file from the Releases page.
2. Open the `.dmg` and drag Flick into the Applications folder.
3. If macOS blocks the app because it is from an unidentified developer, open **System Settings → Privacy & Security** and allow it.
4. You may also need to right-click the app once and select **Open** the first time.
5. Launch Flick and grant any required accessibility or keyboard permissions if prompted.
6. Open Settings and add your API key.

> macOS can be a bit stricter about permissions, so the first launch is often the only tricky step.

### Linux

1. Download the appropriate package for your distro:
   - `.deb` for Debian/Ubuntu-based systems
   - `.AppImage` for most other distributions
2. For `.deb` packages, install it with:
   ```bash
   sudo apt install ./flick_*.deb
   ```
3. For `.AppImage`, make it executable and run it:
   ```bash
   chmod +x Flick.AppImage
   ./Flick.AppImage
   ```
4. If your desktop environment blocks the app, allow it to run from the file manager or terminal.
5. Launch Flick and add your API key in Settings.

## �🔒 Security & Privacy

We take privacy seriously. Flick is designed to be as secure as possible:

- **Local Memory Buffer:** Keystrokes are temporarily held in an ephemeral memory buffer that is strictly bounded in size. The buffer is immediately cleared upon mouse clicks, Enter, or navigation keys.
- **Secure Key Storage:** API keys are stored using your operating system's native secure credential manager (Windows Credential Manager, macOS Keychain, or Linux Secret Service).
- **Direct API Communication:** Flick communicates directly with your chosen provider (Gemini or OpenRouter) and does not route requests through any extra server.
- **No Telemetry:** The app does not collect analytics or usage telemetry by default.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
