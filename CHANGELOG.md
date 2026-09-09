# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.44.0] - 2026-09-09

### Added
- Native Anthropic Claude provider support (`claude-3-7-sonnet-latest`, Messages API streaming).
- Native Tauri 2 Auto-Updater with Minisign cryptographic verification against GitHub Releases (`latest.json`).
- Non-blocking in-app update checker and one-click restart installation in Settings.
- Windows autostart boot synchronization ensuring system registry matches user configuration.

### Changed
- Settings UI restructured into dedicated General, Chat, Text-to-Speech, and History sections.
- Upgraded package versioning to 0.44.0 across frontend and Rust crates.
- NSIS installer defaults updated: smooth in-place upgrade without uninstalling previous versions by default, and Desktop shortcut unchecked by default while keeping "Run Dumbo" checked.

### Fixed
- Stale background process collisions on dev launch with predev task cleanup.

## [0.43.0] - 2026-09-05

### Added
- Initial setup and alignment with Autonomous GitHub Standard.
- Windows overlay chat and MCP interface.