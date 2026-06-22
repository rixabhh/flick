# Flick ⚡️

> AI-powered text transformation anywhere you type.

Flick is a cross-platform desktop utility that brings the power of AI directly to your cursor. Simply type a trigger command (like `!translate` or `!casual`) followed by your text, and Flick instantly rewrites it right where you're typing — in any app, text editor, or browser.

![Flick App Icon](src-tauri/icons/128x128.png)

## ✨ Features

- **Global Text Transformation:** Works everywhere. Any app, any text field.
- **In-place Magic:** Replaces your text as you type, without opening new windows or context menus.
- **Custom Commands:** Create your own triggers like `!code`, `!summarize`, or `!professional`.
- **Bring Your Own Key:** Powered by Google's Gemini Flash model for lightning-fast responses. Securely uses your own API key.
- **Privacy First:** Only reads the buffer of what you're actively typing.

## 🚀 How It Works

1. Start typing anywhere.
2. Type a command starting with `!` (e.g., `!translate to spanish`).
3. Flick detects the trigger, captures your text, and sends it to the AI.
4. Your text is magically replaced with the AI's response.

### Built-in Commands

- `!translate [language]` - Translates your text to the specified language.
- `!grammar` - Fixes spelling and grammatical errors.
- `!casual` - Rewrites your text to sound more casual and friendly.
- `!professional` - Elevates your tone for business communication.
- `!summarize` - Condenses long paragraphs into concise summaries.
- `!expand` - Expands brief notes into full sentences.

## 🛠️ Tech Stack

Flick is built for performance and native integration across Windows, macOS, and Linux:

- **Core Engine:** [Rust](https://www.rust-lang.org/)
- **Desktop Framework:** [Tauri v2](https://v2.tauri.app/)
- **Frontend UI:** [Svelte](https://svelte.dev/) + [Vite](https://vitejs.dev/)
- **AI Integration:** [Google Gemini API](https://ai.google.dev/) (Flash model)

## 📦 Installation

*(Pre-built binaries for Windows, macOS, and Linux will be available in the Releases section soon!)*

### Building from Source

**Prerequisites:**
- [Node.js](https://nodejs.org/) (v20+)
- [Rust](https://rustup.rs/) (v1.75+)
- [Tauri dependencies](https://v2.tauri.app/start/prerequisites/) for your OS (e.g., MSVC Build Tools on Windows).

**Steps:**
1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/flick.git
   cd flick
   ```
2. Install frontend dependencies:
   ```bash
   npm install
   ```
3. Run the application in development mode:
   ```bash
   npm run tauri dev
   ```
4. Build the release binary:
   ```bash
   npm run tauri build
   ```

## 🔒 Security & Privacy

Flick is designed with privacy in mind.
- **Local Keystroke Processing:** Keystrokes are buffered locally in memory and never written to disk. The buffer is automatically cleared on mouse clicks, Enter, or navigation keys.
- **API Key Security:** Your Gemini API key is encrypted and stored securely using your operating system's native keychain (Credential Manager on Windows, Keychain on macOS, Secret Service on Linux).
- **Direct API Calls:** Network requests are made directly from your machine to the Google Gemini API. There is no middleman server.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
