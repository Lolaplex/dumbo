# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- CI runs only on pull requests to `main`.

### Added
- Overlay can be dragged via a hover-only handle; the parked spot is stored as work-area fractions and restored on that monitor.

## [0.44.1] - 2026-09-13

### Changed
- Dropped unused mobile and store branding assets.
- CI lint/build runs only on pull requests (fast path, ~18s).

### Fixed
- Windows Autostart boot launch uses native `winreg` with a quoted auto-launch path.
- Dependency override pins `cookie` to `^0.7.2`.

## [0.44.0] - 2026-09-09

### Added
- Native Anthropic Claude provider support (`claude-3-7-sonnet-latest`, Messages API streaming).
- Native Tauri 2 Auto-Updater with Minisign cryptographic verification against GitHub Releases (`latest.json`).
- Non-blocking in-app update checker and one-click restart installation in Settings.
- Windows autostart boot synchronization ensuring system registry matches user configuration.

### Changed
- Settings UI restructured into dedicated General, Chat, Text-to-Speech, and History sections.
- Package versioning set to 0.44.0 across frontend and Rust crates.
- NSIS installer defaults: in-place upgrade without uninstalling the previous version; Desktop shortcut unchecked by default; "Run Dumbo" stays checked.

### Fixed
- Stale background process collisions on dev launch with predev task cleanup.

## [0.43.0] - 2026-09-04

### Added
- Runtime i18n (English / Deutsch) across settings, hotkeys, status alerts, and native window titles.
- Language-neutral TTS: Azure SSML voice-locale follows the selected voice.
- Chat replies in the user's input language instead of unsolicited translation.

### Changed
- MCP Bridge is opt-in via `DUMBO_MCP_BRIDGE`.

### Fixed
- Windows subsystem startup no longer flashes a console window.
- Tray menu recovers after errors; middle-click on the tray icon exits cleanly.

## [0.42.0] - 2026-09-04

### Added
- First public Windows overlay: Alt+Space ask, selection capture, streaming answers, local SQLite history.
- Global TTS (Azure, ElevenLabs, Gemini, OpenAI, or a local OpenAI-compatible speech endpoint).
- Chat via Gemini, OpenAI, OpenRouter, Ollama, LM Studio, or custom endpoints; keys stay in Windows Credential Manager.
- English and Deutsch without restart; settings and history survive upgrades (older AppData is merged, not overwritten).

[Unreleased]: https://github.com/Lolaplex/dumbo/compare/v0.44.1...HEAD
[0.44.1]: https://github.com/Lolaplex/dumbo/compare/v0.44.0...v0.44.1
[0.44.0]: https://github.com/Lolaplex/dumbo/compare/v0.43.0...v0.44.0
[0.43.0]: https://github.com/Lolaplex/dumbo/compare/v0.42.0...v0.43.0
[0.42.0]: https://github.com/Lolaplex/dumbo/releases/tag/v0.42.0
