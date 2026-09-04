export type SupportedLanguage = "en" | "de";
export type LanguageSetting = "auto" | "en" | "de";

export const translations = {
  en: {
    save: "Save",
    saved: "Saved",
    delete: "Delete",
    close: "Close",
    cancel: "Cancel",
    loading: "Loading...",
    error: "Error",
    removed: "Removed",

    trayAsk: "Ask",
    traySettings: "Settings",
    trayQuit: "Quit",

    askAnything: "Ask anything (↵ Enter)...",
    askSelection: "Ask about selected text (↵ Enter)",
    askClipboard: "Ask about clipboard (↵ Enter)",
    askAttachments: "Ask about attachments (↵ Enter)...",
    historyMeta: "History",
    copyAnswer: "Copy answer",
    copied: "Copied!",
    stopTts: "Stop speech output (Click)",
    loadingTts: "Speech loading (Click to cancel)",
    today: "Today",
    yesterday: "Yesterday",

    settingsTitle: "Settings",
    tabGeneral: "General",
    tabSpeech: "Speech (TTS)",
    tabHistory: "History",

    chatAndModel: "Chat & Model",
    overlayHotkey: "Overlay Shortcut",
    overlayHotkeyNote: "Alt+Space opens the overlay. Esc closes, Ctrl+, opens Settings.",
    overlayHotkeyPlaceholder: "e.g. Alt+Space or F9",
    clipboardContext: "Include clipboard as context",
    historySave: "Save chat history",
    autostart: "Start with Windows",

    activeProvider: "Active Chat Provider",
    providerName: "Name",
    providerModel: "Model",
    providerBaseUrl: "Base URL",
    providerApiKey: "API Key",
    keySaved: "Key saved (type to overwrite...)",
    keyEmpty: "Enter API key...",
    saveProvider: "Save Settings",
    removeProvider: "Remove Provider",
    openaiKeyHint: "enter sk-...",

    languageLabel: "Language",
    languageAuto: "System (Auto)",
    languageEn: "English",
    languageDe: "Deutsch",
    languageNote: "Display language for overlay and settings.",

    ttsTitle: "Global Text-to-Speech (TTS)",
    ttsBusyGenerating: "Generating speech…",
    ttsHotkey: "TTS Shortcut",
    ttsHotkeyPlaceholder: "e.g. Alt+Shift+S or F8",
    ttsHotkeyNote: "Reads highlighted text or clipboard aloud globally. Press again to stop.",
    ttsProvider: "Speech Provider",
    ttsVoice: "Voice",
    ttsModel: "Model",
    ttsAzureRegion: "Azure Region",
    ttsCustomUrl: "Server URL",
    ttsCustomUrlPlaceholder: "http://127.0.0.1:8880 or .../v1",
    ttsCustomNote: "Expects POST /v1/audio/speech. Works with Kokoro, Piper, LocalAI etc.",
    ttsTestVoice: "Test Voice",
    ttsTesting: "Playing...",
    ttsSampleText: "Dumbo text-to-speech is ready.",
    ttsErrorDefault: "Speech synthesis failed",
    ttsStop: "Stop",
    ttsSaveKey: "Save Key",
    ttsDeleteKey: "Delete Key",
    ttsCheckStatus: "Check Status",
    ttsServerReachable: "Server reachable",
    ttsServerNotReady: "Server responding, not ready",
    ttsServerUnreachable: "Server unreachable",

    historyTitle: "History",
    ttsHistoryTitle: "Spoken text (TTS)",
    copyText: "Copy text",
    copiedText: "Copied!",
    historyEmpty: "No entries yet. Asked questions will appear here.",
    historyClear: "Clear History",
    historyConfirmClear: "Are you sure you want to clear all history entries?",
    historyDisabledNote:
      'History is disabled. Enable "Save chat history" above to store past conversations locally.',

    hotkeyPress: "Press a key...",
    hotkeyClear: "Clear",
    hotkeyClearAria: "Clear hotkey",
    hotkeyManual: "Type manually",
    hotkeyManualAria: "Enter manually",
    hotkeyRecording: "Listening",
    hotkeyCapture: "Record",
    hotkeyPresets: "Presets:",
    hotkeyListening: "Press key or mouse button... (Esc: Cancel)",

    providerAdded: "added",
    providerModelPlaceholder: "e.g. gpt-4o, gemini-2.0-flash",
    ttsProviderCustomLabel: "Custom Local (Kokoro / OpenAI-compatible)",

    voiceLangDe: "German",
    voiceLangEn: "English",
    voiceMale: "Male",
    voiceFemale: "Female",
    voiceMultilingual: "Multilingual",
  },
  de: {
    save: "Speichern",
    saved: "Gespeichert",
    delete: "Löschen",
    close: "Schließen",
    cancel: "Abbrechen",
    loading: "Lädt...",
    error: "Fehler",
    removed: "Entfernt",

    trayAsk: "Fragen",
    traySettings: "Einstellungen",
    trayQuit: "Beenden",

    askAnything: "Frage eingeben (↵ Enter)...",
    askSelection: "Markierten Text fragen (↵ Enter)",
    askClipboard: "Zwischenablage fragen (↵ Enter)",
    askAttachments: "Frage zu den Anhängen eingeben (↵ Enter)...",
    historyMeta: "Verlauf",
    copyAnswer: "Antwort kopieren",
    copied: "Kopiert!",
    stopTts: "Sprachausgabe stoppen (Klick)",
    loadingTts: "Sprachausgabe lädt (Klick zum Abbrechen)",
    today: "Heute",
    yesterday: "Gestern",

    settingsTitle: "Einstellungen",
    tabGeneral: "Allgemein",
    tabSpeech: "Sprachausgabe",
    tabHistory: "Historie",

    chatAndModel: "Chat & Modell",
    overlayHotkey: "Overlay Hotkey",
    overlayHotkeyNote: "Alt+Space öffnet das Overlay. Esc schließt, Ctrl+, öffnet Settings.",
    overlayHotkeyPlaceholder: "z. B. Alt+Space oder F9",
    clipboardContext: "Clipboard als Kontext einbeziehen",
    historySave: "Historie speichern",
    autostart: "Autostart mit Windows",

    activeProvider: "Aktiver Chat Provider",
    providerName: "Name",
    providerModel: "Model",
    providerBaseUrl: "Base URL",
    providerApiKey: "API Key",
    keySaved: "Key gespeichert (überschreiben...)",
    keyEmpty: "API Key eingeben...",
    saveProvider: "Einstellungen speichern",
    removeProvider: "Provider entfernen",
    openaiKeyHint: "sk-... eingeben",

    languageLabel: "Sprache",
    languageAuto: "System (Auto)",
    languageEn: "English",
    languageDe: "Deutsch",
    languageNote: "Oberflächensprache für Overlay und Einstellungen.",

    ttsTitle: "Globale Sprachausgabe (TTS)",
    ttsBusyGenerating: "Sprache wird erzeugt…",
    ttsHotkey: "TTS Hotkey",
    ttsHotkeyPlaceholder: "z. B. Alt+Shift+S oder F8",
    ttsHotkeyNote:
      "Liest programmübergreifend markierten Text oder Zwischenablage direkt vor. Nochmal drücken stoppt sofort.",
    ttsProvider: "Sprachanbieter",
    ttsVoice: "Stimme",
    ttsModel: "Modell",
    ttsAzureRegion: "Azure Region",
    ttsCustomUrl: "Server URL",
    ttsCustomUrlPlaceholder: "http://127.0.0.1:8880 oder .../v1",
    ttsCustomNote: "Erwartet POST /v1/audio/speech. Funktioniert mit Kokoro, Piper, LocalAI etc.",
    ttsTestVoice: "Stimme testen",
    ttsTesting: "Spielt ab...",
    ttsSampleText: "Dumbo Sprachausgabe ist bereit.",
    ttsErrorDefault: "Fehler bei Sprachausgabe",
    ttsStop: "Stopp",
    ttsSaveKey: "Key speichern",
    ttsDeleteKey: "Key löschen",
    ttsCheckStatus: "Status prüfen",
    ttsServerReachable: "Server erreichbar",
    ttsServerNotReady: "Server antwortet, nicht bereit",
    ttsServerUnreachable: "Server nicht erreichbar",

    historyTitle: "Historie",
    ttsHistoryTitle: "Vorgelesener Text (TTS)",
    copyText: "Text kopieren",
    copiedText: "Kopiert!",
    historyEmpty: "Noch keine Einträge. Gestellte Fragen erscheinen hier automatisch.",
    historyClear: "Historie leeren",
    historyConfirmClear: "Möchtest du wirklich alle Historien-Einträge unwiderruflich löschen?",
    historyDisabledNote:
      "Historie ist deaktiviert. Schalte oben „Historie speichern“ ein, um vergangene Konversationen lokal zu sichern.",

    hotkeyPress: "Taste drücken...",
    hotkeyClear: "Löschen",
    hotkeyClearAria: "Hotkey löschen",
    hotkeyManual: "Manuell tippen",
    hotkeyManualAria: "Manuell eingeben",
    hotkeyRecording: "Aktiv",
    hotkeyCapture: "Aufnehmen",
    hotkeyPresets: "Presets:",
    hotkeyListening: "Taste oder Maustaste drücken... (Esc: Abbrechen)",

    providerAdded: "hinzugefügt",
    providerModelPlaceholder: "z. B. gpt-4o, gemini-2.0-flash",
    ttsProviderCustomLabel: "Custom Local (Kokoro / OpenAI-kompatibel)",

    voiceLangDe: "Deutsch",
    voiceLangEn: "Englisch",
    voiceMale: "Männlich",
    voiceFemale: "Weiblich",
    voiceMultilingual: "Multilingual",
  },
} as const;

export type TranslationKey = keyof typeof translations.en;

/** Mutate `.language` — `$state` so `t()` in templates re-runs without restart. */
export const i18n = $state({
  language: "auto" as LanguageSetting,
});

export function getSystemLanguage(): SupportedLanguage {
  if (typeof navigator !== "undefined" && navigator.language?.toLowerCase().startsWith("de")) {
    return "de";
  }
  return "en";
}

export function activeLang(): SupportedLanguage {
  const cfg = i18n.language;
  if (cfg === "de" || cfg === "en") return cfg;
  return getSystemLanguage();
}

export function localeTag(): string {
  return activeLang() === "de" ? "de-DE" : "en-US";
}

export function applyLanguage(setting?: string | null) {
  if (setting === "en" || setting === "de" || setting === "auto") {
    i18n.language = setting;
  }
}

export function t(key: TranslationKey): string {
  const l = activeLang();
  return translations[l][key] ?? translations.en[key] ?? key;
}
