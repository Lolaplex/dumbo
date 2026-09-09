<div align="center">
  <img src="static/splash-icon.png" alt="Dumbo Logo" width="96" />
  <h1>Dumbo</h1>
  <p>
    <a href="https://github.com/Lolaplex/dumbo/releases"><img src="https://img.shields.io/badge/version-0.44.0-blue.svg?style=flat-square" alt="Version 0.44.0" /></a>
    <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-0078D4.svg?style=flat-square&logo=windows&logoColor=white" alt="Windows 10/11" />
    <img src="https://img.shields.io/badge/Tauri-2.0-FFC131.svg?style=flat-square&logo=tauri&logoColor=black" alt="Tauri 2" />
    <img src="https://img.shields.io/badge/Svelte-5-FF3E00.svg?style=flat-square&logo=svelte&logoColor=white" alt="Svelte 5" />
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="License" /></a>
  </p>
  <p>
    <strong>Ultra-fast, lightweight AI desktop overlay &amp; speech assistant for Windows.</strong><br>
    One instant hotkey. Zero Electron memory bloat. Bring your own key.
  </p>
</div>

---

## Quickstart

Grab the latest installer from **[Releases](https://github.com/Lolaplex/dumbo/releases)**:

Windows only at the moment. MacOS coming soon.

1. Run the installer.
2. Open **Settings** via the system tray or shortcut `Ctrl+,`.
3. Pick your provider (Gemini, Claude, OpenAI, OpenRouter, Local/Custom), enter your API key, and hit **Save**.
4. Press `Alt+Space` anywhere to ask.

> [!TIP]
> **🤖 Smart Updates:**
> Dumbo includes automatic cryptographic updates via Minisign. Whenever a new version is released on GitHub, check and install in one click right inside Settings.

---

## What it does

Dumbo sits in your Windows system tray and gives you instant, non-intrusive AI assistance without switching windows or breaking your focus.

- **Instant Floating HUD (`Alt+Space`)**: Frameless, centered floating prompt that appears in sub-milliseconds without stealing window focus unexpectedly.
- **Context-Aware Selection**: Highlight code or text in any app (browser, IDE, PDF, Word) — Dumbo captures it via Windows UI Automation as prompt context.
- **Global Neural TTS (`Alt+Shift+S`)**: High-quality screen reader for highlighted text and clipboard using Azure, ElevenLabs, Gemini, OpenAI, or local Kokoro/Piper. Press again to stop immediately.
- **Native Keyring Security (BYOK)**: API keys are encrypted in the native **Windows Credential Manager** via OS keyring — never stored in plaintext files.
- **Local SQLite History**: Continuous multi-turn threads with embedded SQLite scrollback.
- **Windows Autostart**: Clean boot synchronization with native Windows registry.

---

## Shortcuts

| Action | Shortcut | Description |
|---|---|---|
| **Toggle Overlay** | `Alt+Space` | Open or close the quick-ask overlay |
| **Ask (Short)** | `Enter` | Submit prompt for a direct, concise answer |
| **Ask (Detailed)** | `Shift+Enter` | Submit prompt for an in-depth answer |
| **Read Aloud (TTS)** | `Alt+Shift+S` | Speak highlighted text or clipboard; press again to stop |
| **Settings** | `Ctrl+,` | Open configuration window |
| **Dismiss** | `Esc` | Hide the overlay immediately |

*All shortcuts and language preferences (English / Deutsch) can be adjusted in **Settings**.*

---

## License

MIT License &copy; 2026 Lolaplex — see [LICENSE](LICENSE).
