<div align="center">
  <img src="static/splash-icon.png" alt="Dumbo Logo" width="100" />
  <h1>Dumbo</h1>
  <p><strong>Ultra-fast, lightweight AI desktop overlay & speech assistant for Windows.</strong></p>
  <p>
    <img alt="Windows 10/11" src="https://img.shields.io/badge/Windows-10%20%2F%2011-0078D4?logo=windows&logoColor=white" />
    <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=black" />
    <img alt="Svelte 5" src="https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white" />
    <img alt="License MIT" src="https://img.shields.io/badge/License-MIT-blue.svg" />
  </p>
</div>

---

Dumbo is a minimal Windows desktop companion designed for instant answers without breaking your flow. Press **Alt+Space** anywhere: Dumbo glides in, captures your selected text automatically, and streams an answer. Press **Alt+Shift+S** to read aloud highlighted text using natural neural text-to-speech.

No heavy Electron memory bloat. Zero subscription lock-in. Powered by Tauri 2, Svelte 5, and Rust.

## Features

- **Instant Hotkey Overlay (`Alt+Space`)**: Frameless, centered floating HUD that appears in milliseconds without stealing window focus unexpectedly.
- **Context-Aware Text Capture**: Highlight code or text in any app (browser, IDE, PDF, Word) — Dumbo grabs it via Windows UI Automation as context for your prompt.
- **Global Text-to-Speech (`Alt+Shift+S`)**: Program-wide screen reader for highlighted text and clipboard using high-quality neural voices (Azure, ElevenLabs, Gemini, OpenAI, or local Kokoro). Press again to stop immediately.
- **Bring Your Own Key (BYOK)**: Keys are stored securely in the native **Windows Credential Manager** via OS keyring — never in plaintext files or project configs.
- **Multi-Turn Chat & History**: Reopening within 60 seconds keeps your current thread alive. Older conversations are saved in an embedded, local SQLite database for instant mouse-wheel scrollback.
- **Multi-Provider Flexibility**: Out of the box presets for Google Gemini, OpenAI, OpenRouter, local Ollama, LM Studio, or custom endpoints.
- **Zero Background Bloat**: Native Rust runtime with minimal RAM usage.

## Download & Installation

Grab the latest installer from the **[Releases](https://github.com/Lolaplex/dumbo/releases)** page:

| Platform | Installer | Format |
|----------|-----------|--------|
| **Windows 10 / 11** | `Dumbo_0.43.0_x64-setup.exe` | NSIS Setup (x64) |
| **Windows 10 / 11** | `Dumbo_0.43.0_x64_en-US.msi` | MSI Package (x64) |

1. Download and run the setup file (or MSI package).
2. Open **Settings** via the system tray icon or shortcut `Ctrl+,`.
3. Select your preferred provider (e.g. **Gemini**), enter your API key, and hit **Save**.
4. Press `Alt+Space` to start asking.

> <sub>**Note on Windows SmartScreen:** Indie open-source binaries are unsigned. If Windows SmartScreen shows a prompt on first install, click *More info* &rarr; *Run anyway*.</sub>

## Shortcuts

| Action | Shortcut | Description |
|--------|----------|-------------|
| **Toggle Overlay** | `Alt+Space` | Open or close the quick-ask overlay |
| **Ask (Short)** | `Enter` | Submit prompt for a concise, direct answer |
| **Ask (Detailed)** | `Shift+Enter` | Submit prompt for a comprehensive answer |
| **Read Aloud (TTS)** | `Alt+Shift+S` | Speak highlighted text or clipboard; press again to cancel |
| **Settings** | `Ctrl+,` | Open the configuration window |
| **Copy Answer** | `Ctrl+C` | Copy answer text when visible |
| **Browse History** | `Mouse Wheel` or `Alt+↑` / `Alt+↓` | Scroll through past questions and answers |
| **Dismiss** | `Esc` | Hide the overlay immediately |

*All hotkeys can be customized in **Settings &rarr; General**.*

## Supported Providers

### LLM Chat
| Provider | Default URL | Notes |
|----------|-------------|-------|
| **Gemini** | `https://generativelanguage.googleapis.com/...` | Recommended default. Fast streaming with thinking support |
| **OpenAI / Compatible** | `https://api.openai.com/v1` | Supports GPT-4o, Claude via proxies, OpenRouter, Groq, custom endpoints |
| **Ollama** | `http://127.0.0.1:11434/v1` | 100% offline local inference |
| **LM Studio** | `http://127.0.0.1:1234/v1` | Local model server with OpenAI-compatible API |

### Text-to-Speech (TTS)
- **ElevenLabs**: Multilingual v2 high-expressiveness models.
- **Microsoft Azure Speech**: Neural voices with configurable regional endpoints.
- **Google Gemini & OpenAI Audio**: Cloud speech synthesis.
- **Custom / Local Server**: Any OpenAI-compatible speech endpoint (`POST /v1/audio/speech`), e.g. local Kokoro, Piper, or LocalAI servers (default: `http://127.0.0.1:8880`).

## Development

If you want to build Dumbo from source:

### Prerequisites
- **Windows 10 / 11**
- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/)
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) & Visual Studio C++ Build Tools

### Setup
```powershell
git clone https://github.com/Lolaplex/dumbo.git
cd dumbo
npm install
npm run tauri dev
```

### Build Binary
```powershell
npm run tauri build
```
The NSIS installer will be generated in `src-tauri/target/release/bundle/nsis/`.

## Architecture

```
dumbo/
├── src/               # Svelte 5 frontend (overlay HUD, settings, tray)
│   ├── lib/           # Components (HotkeyInput, MarkdownView, OverlayAsk, SettingsPage)
│   └── routes/        # App entry & settings window routing
├── src-tauri/         # Rust backend (Tauri 2)
│   ├── src/chat.rs    # Streaming SSE client, provider engines, thinking extraction
│   ├── src/history.rs # SQLite thread persistence & exchange scrollback
│   ├── src/overlay.rs # Windows HWND lifecycle, tray, global shortcuts
│   ├── src/selection.rs# UIAutomation text capture & clipboard fallback
│   ├── src/tts.rs     # Multi-engine audio synthesis & Rodio playback pipeline
│   └── tauri.conf.json# Window flags (transparent, click-through, unclosable)
```

## Roadmap

- [x] **v0.2.0**: Overlay ask, auto-selection, session continuity, Windows keyring.
- [x] **v0.42.0**: Multi-engine TTS screen reader (`Alt+Shift+S`), Kokoro support, settings overhaul.
- [x] **v0.43.0**: Complete i18n localization, neutral TTS synthesis, tray crash recovery, zero-console startup.
- [ ] **Dictation (STT)**: Dedicated push-to-talk button for voice input.
- [ ] **Live Speech Pipeline**: Continuous voice interaction with floating pill indicator.
- [ ] **Screen Context**: One-click screen region capture attached directly to multimodal prompts.
- [ ] **MCP Client**: Tool calling via Model Context Protocol servers.

## License

MIT License &copy; 2026 Lolaplex — see [LICENSE](LICENSE).

---

<div align="center">
  A project by <strong>Lolaplex</strong> · <a href="https://github.com/Lolaplex">github.com/Lolaplex</a>
</div>

