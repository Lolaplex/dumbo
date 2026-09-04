use std::sync::OnceLock;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    De,
    En,
}

impl Locale {
    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::De => "de",
            Locale::En => "en",
        }
    }

    pub fn bcp47(&self) -> &'static str {
        match self {
            Locale::De => "de-DE",
            Locale::En => "en-US",
        }
    }
}

#[cfg(windows)]
fn detect_os_locale() -> Locale {
    extern "system" {
        fn GetUserDefaultLocaleName(lpLocaleName: *mut u16, cchLocaleName: i32) -> i32;
    }
    let mut buffer = [0u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if len > 0 {
        if let Ok(name) = String::from_utf16(&buffer[..len as usize - 1]) {
            if name.to_ascii_lowercase().starts_with("de") {
                return Locale::De;
            }
        }
    }
    Locale::En
}

#[cfg(not(windows))]
fn detect_os_locale() -> Locale {
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            if val.to_ascii_lowercase().starts_with("de") {
                return Locale::De;
            }
        }
    }
    Locale::En
}

static SYSTEM_LOCALE: OnceLock<Locale> = OnceLock::new();

pub fn system_locale() -> Locale {
    *SYSTEM_LOCALE.get_or_init(detect_os_locale)
}

pub fn resolve_locale(language_setting: &str) -> Locale {
    match language_setting.trim().to_ascii_lowercase().as_str() {
        "de" => Locale::De,
        "en" => Locale::En,
        _ => system_locale(),
    }
}

pub fn app_locale(app: &AppHandle) -> Locale {
    if let Ok(settings) = crate::settings::load(app) {
        resolve_locale(&settings.language)
    } else {
        system_locale()
    }
}

pub fn t<'a>(locale: Locale, key: &'a str) -> &'a str {
    match (locale, key) {
        // Window Titles
        (Locale::De, "window_settings") => "Einstellungen",
        (Locale::En, "window_settings") => "Settings",
        (Locale::De, "window_tray_menu") => "Dumbo Menü",
        (Locale::En, "window_tray_menu") => "Dumbo Menu",

        // TTS Errors & Status
        (Locale::De, "tts_no_text") => "Kein Text markiert (und Zwischenablage ist leer).",
        (Locale::En, "tts_no_text") => "No text selected (and clipboard is empty).",
        (Locale::De, "tts_key_missing") => "TTS API-Key fehlt. Bitte in den Einstellungen eintragen.",
        (Locale::En, "tts_key_missing") => "TTS API key is missing. Please enter it in Settings.",
        (Locale::De, "tts_azure_key_missing") => "Azure Speech Key fehlt.",
        (Locale::En, "tts_azure_key_missing") => "Azure Speech key is missing.",
        (Locale::De, "tts_gemini_no_audio") => "Gemini hat keine Audiodaten zurückgegeben.",
        (Locale::En, "tts_gemini_no_audio") => "Gemini returned no audio data.",
        (Locale::De, "tts_audio_empty") => "Audiodaten leer.",
        (Locale::En, "tts_audio_empty") => "Audio data is empty.",
        (Locale::De, "tts_custom_ready") => "TTS Server bereit",
        (Locale::En, "tts_custom_ready") => "TTS server ready",
        (Locale::De, "tts_custom_loading") => "Server lädt Modell im Hintergrund...",
        (Locale::En, "tts_custom_loading") => "Server is loading model in background...",
        (Locale::De, "tts_custom_not_init") => "TTS Server läuft, Modell noch nicht initialisiert",
        (Locale::En, "tts_custom_not_init") => "TTS server running, model not yet initialized",
        (Locale::De, "tts_custom_unreachable") => "TTS Server nicht erreichbar (weder /health noch /v1/models)",
        (Locale::En, "tts_custom_unreachable") => "TTS server unreachable (neither /health nor /v1/models)",
        (Locale::De, "tts_custom_kokoro_reachable") => "Kokoro/OpenAI-TTS erreichbar",
        (Locale::En, "tts_custom_kokoro_reachable") => "Kokoro/OpenAI-TTS reachable",
        (Locale::De, "tts_custom_openai_reachable") => "OpenAI-kompatibler TTS Server erreichbar",
        (Locale::En, "tts_custom_openai_reachable") => "OpenAI-compatible TTS server reachable",

        // Chat Errors
        (Locale::De, "chat_base_url_missing") => "Base-URL fehlt. Bitte in den Einstellungen eintragen.",
        (Locale::En, "chat_base_url_missing") => "Base URL is missing. Please enter it in Settings.",
        (Locale::De, "chat_key_missing") => "Kein API-Key. In Settings eintragen und speichern.",
        (Locale::En, "chat_key_missing") => "No API key. Please enter it in Settings and save.",
        (Locale::De, "chat_unknown_error") => "Unbekannter API-Fehler.",
        (Locale::En, "chat_unknown_error") => "Unknown API error.",

        // Overlay & Hotkeys
        (Locale::De, "tray_menu_missing") => "Tray-Menü fehlt.",
        (Locale::En, "tray_menu_missing") => "Tray menu is missing.",

        // Providers
        (Locale::De, "provider_not_found") => "Provider nicht gefunden.",
        (Locale::En, "provider_not_found") => "Provider not found.",
        (Locale::De, "provider_name_missing") => "Name fehlt.",
        (Locale::En, "provider_name_missing") => "Name is missing.",

        // History
        (Locale::De, "chat_not_found") => "Chat nicht gefunden.",
        (Locale::En, "chat_not_found") => "Chat not found.",

        // Fallback
        (_, _) => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_explicit_locales() {
        assert_eq!(resolve_locale("de"), Locale::De);
        assert_eq!(resolve_locale("DE"), Locale::De);
        assert_eq!(resolve_locale("en"), Locale::En);
        assert_eq!(resolve_locale("EN"), Locale::En);
    }

    #[test]
    fn translates_known_keys() {
        assert_eq!(t(Locale::De, "window_settings"), "Einstellungen");
        assert_eq!(t(Locale::En, "window_settings"), "Settings");
        assert_eq!(t(Locale::De, "tts_no_text"), "Kein Text markiert (und Zwischenablage ist leer).");
        assert_eq!(t(Locale::En, "tts_no_text"), "No text selected (and clipboard is empty).");
    }

    #[test]
    fn fallbacks_to_key_when_unknown() {
        assert_eq!(t(Locale::De, "unknown_key"), "unknown_key");
        assert_eq!(t(Locale::En, "unknown_key"), "unknown_key");
    }
}
