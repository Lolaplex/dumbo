use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};

const FILE_NAME: &str = "settings.json";
/// Frozen. A new identifier = empty AppData = looks like settings were wiped.
const CONFIG_ID: &str = "com.lolaplex.dumbo";
const LEGACY_IDS: &[&str] = &["com.lolax.dumbo"];
const MIGRATE_STAMP: &str = ".migrated-com.lolax.dumbo";
const USER_FILES: &[&str] = &[
    "settings.json",
    "providers.json",
    "tts_keys.json",
    "provider_keys.json",
    "dumbo.db",
];
pub const DEFAULT_HOTKEY: &str = "alt+space";
pub const DEFAULT_TTS_HOTKEY: &str = "alt+shift+s";

pub const DEFAULT_GEMINI_TTS_VOICE: &str = "Puck";
pub const DEFAULT_GEMINI_TTS_MODEL: &str = "gemini-2.0-flash";
pub const DEFAULT_OPENAI_TTS_VOICE: &str = "alloy";
pub const DEFAULT_OPENAI_TTS_MODEL: &str = "tts-1";
pub const DEFAULT_ELEVEN_TTS_VOICE: &str = "PhufIH7nYh2Up1uej6aY";
pub const DEFAULT_ELEVEN_TTS_MODEL: &str = "eleven_multilingual_v2";
pub const DEFAULT_AZURE_TTS_VOICE: &str = "de-DE-ConradNeural";
pub const DEFAULT_AZURE_TTS_REGION: &str = "westeurope";
pub const DEFAULT_CUSTOM_TTS_URL: &str = "http://127.0.0.1:8880";
pub const DEFAULT_CUSTOM_TTS_VOICE: &str = "af_bella";
pub const DEFAULT_CUSTOM_TTS_MODEL: &str = "kokoro";

static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_config_dir(path: PathBuf) {
    let _ = CONFIG_DIR.set(path);
}

pub fn get_config_dir() -> PathBuf {
    CONFIG_DIR.get().cloned().unwrap_or_else(|| {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata).join(CONFIG_ID)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config").join(CONFIG_ID)
        } else {
            PathBuf::from(".")
        }
    })
}

fn roaming_parent() -> Option<PathBuf> {
    std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
}

pub fn legacy_data_dirs() -> Vec<PathBuf> {
    let Some(root) = roaming_parent() else {
        return Vec::new();
    };
    LEGACY_IDS.iter().map(|id| root.join(id)).collect()
}

/// Copy missing user files from legacy identifier dirs. Never overwrites existing files.
/// Providers: additive merge (restore custom providers lost when identifier changed).
pub fn prepare_config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let config = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Konfigurationsordner fehlt: {e}"))?;
    fs::create_dir_all(&config).map_err(|e| format!("Konfigurationsordner nicht anlegbar: {e}"))?;
    migrate_legacy_once(&config);
    if let Ok(data) = app.path().app_data_dir() {
        if data != config {
            fs::create_dir_all(&data).map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
            migrate_legacy_once(&data);
        }
    }
    init_config_dir(config.clone());
    Ok(config)
}

fn migrate_legacy_once(dest: &Path) {
    let stamp = dest.join(MIGRATE_STAMP);
    if stamp.exists() {
        return;
    }
    let Some(root) = roaming_parent() else {
        let _ = fs::write(&stamp, b"1");
        return;
    };
    for id in LEGACY_IDS {
        let src_dir = root.join(id);
        if !src_dir.exists() || src_dir == dest {
            continue;
        }
        for name in USER_FILES {
            let src = src_dir.join(name);
            let dst = dest.join(name);
            if !src.exists() {
                continue;
            }
            if *name == "providers.json" {
                merge_providers_additive(&dst, &src);
                continue;
            }
            if dst.exists() {
                continue;
            }
            let _ = fs::copy(&src, &dst);
        }
    }
    let _ = fs::write(&stamp, b"1");
}

fn merge_providers_additive(dest: &Path, src: &Path) {
    let Ok(src_raw) = fs::read_to_string(src) else {
        return;
    };
    if !dest.exists() {
        let _ = fs::copy(src, dest);
        return;
    }
    let Ok(dst_raw) = fs::read_to_string(dest) else {
        return;
    };
    let Ok(mut dst_val) = serde_json::from_str::<Value>(&dst_raw) else {
        return;
    };
    let Ok(src_val) = serde_json::from_str::<Value>(&src_raw) else {
        return;
    };
    let Some(dst_list) = dst_val.get_mut("providers").and_then(|v| v.as_array_mut()) else {
        return;
    };
    let Some(src_list) = src_val.get("providers").and_then(|v| v.as_array()) else {
        return;
    };
    let existing: std::collections::HashSet<String> = dst_list
        .iter()
        .filter_map(|p| p.get("id").and_then(|i| i.as_str()).map(str::to_string))
        .collect();
    let mut added = false;
    for provider in src_list {
        let Some(id) = provider.get("id").and_then(|i| i.as_str()) else {
            continue;
        };
        if existing.contains(id) {
            continue;
        }
        dst_list.push(provider.clone());
        added = true;
    }
    if added {
        if let Ok(raw) = serde_json::to_string_pretty(&dst_val) {
            let _ = fs::write(dest, raw);
        }
    }
}

/// Overlay `patch` onto `base`. Null / missing patch keys leave base values.
fn overlay_value(base: Value, patch: Value) -> Value {
    match (base, patch) {
        (Value::Object(mut base_map), Value::Object(patch_map)) => {
            for (key, val) in patch_map {
                if val.is_null() {
                    continue;
                }
                let next = match base_map.remove(&key) {
                    Some(existing) => overlay_value(existing, val),
                    None => val,
                };
                base_map.insert(key, next);
            }
            Value::Object(base_map)
        }
        (_, patch) => patch,
    }
}

fn settings_from_json(raw: &str) -> Result<AppSettings, String> {
    let user: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let defaults = serde_json::to_value(AppSettings::default()).map_err(|e| e.to_string())?;
    let merged = overlay_value(defaults, user);
    let mut settings: AppSettings =
        serde_json::from_value(merged).map_err(|e| e.to_string())?;
    settings.normalize();
    Ok(settings)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub history_enabled: bool,
    #[serde(default = "default_provider_id")]
    pub active_provider_id: String,
    #[serde(default)]
    pub clipboard_context: bool,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_tts_hotkey")]
    pub tts_hotkey: String,
    #[serde(default = "default_tts_provider")]
    pub tts_provider: String,
    #[serde(default = "default_tts_voice")]
    pub tts_voice: String,
    #[serde(default = "default_tts_model")]
    pub tts_model: String,
    #[serde(default = "default_tts_azure_region")]
    pub tts_azure_region: String,

    #[serde(default = "default_local_tts_voice")]
    pub tts_local_voice: String,
    #[serde(default = "default_local_tts_url")]
    pub tts_local_url: String,
    #[serde(default = "default_custom_tts_url")]
    pub tts_custom_url: String,
    #[serde(default = "default_custom_tts_voice")]
    pub tts_custom_voice: String,
    #[serde(default = "default_custom_tts_model")]
    pub tts_custom_model: String,
    #[serde(default = "default_gemini_tts_voice")]
    pub tts_gemini_voice: String,
    #[serde(default = "default_gemini_tts_model")]
    pub tts_gemini_model: String,
    #[serde(default = "default_openai_tts_voice")]
    pub tts_openai_voice: String,
    #[serde(default = "default_openai_tts_model")]
    pub tts_openai_model: String,
    #[serde(default = "default_eleven_tts_voice")]
    pub tts_eleven_voice: String,
    #[serde(default = "default_eleven_tts_model")]
    pub tts_eleven_model: String,
    #[serde(default = "default_azure_tts_voice")]
    pub tts_azure_voice: String,
    #[serde(default = "default_azure_tts_region")]
    pub tts_azure_region_setting: String,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_hotkey() -> String {
    DEFAULT_HOTKEY.to_string()
}
fn default_provider_id() -> String {
    "gemini".to_string()
}
fn default_tts_hotkey() -> String {
    DEFAULT_TTS_HOTKEY.to_string()
}
fn default_tts_provider() -> String {
    "azure".to_string()
}
fn default_tts_voice() -> String {
    DEFAULT_AZURE_TTS_VOICE.to_string()
}
fn default_tts_model() -> String {
    String::new()
}
fn default_tts_azure_region() -> String {
    DEFAULT_AZURE_TTS_REGION.to_string()
}
fn default_local_tts_voice() -> String {
    String::new()
}
fn default_local_tts_url() -> String {
    String::new()
}
fn default_custom_tts_url() -> String {
    DEFAULT_CUSTOM_TTS_URL.to_string()
}
fn default_custom_tts_voice() -> String {
    DEFAULT_CUSTOM_TTS_VOICE.to_string()
}
fn default_custom_tts_model() -> String {
    DEFAULT_CUSTOM_TTS_MODEL.to_string()
}
fn default_gemini_tts_voice() -> String {
    DEFAULT_GEMINI_TTS_VOICE.to_string()
}
fn default_gemini_tts_model() -> String {
    DEFAULT_GEMINI_TTS_MODEL.to_string()
}
fn default_openai_tts_voice() -> String {
    DEFAULT_OPENAI_TTS_VOICE.to_string()
}
fn default_openai_tts_model() -> String {
    DEFAULT_OPENAI_TTS_MODEL.to_string()
}
fn default_eleven_tts_voice() -> String {
    DEFAULT_ELEVEN_TTS_VOICE.to_string()
}
fn default_eleven_tts_model() -> String {
    DEFAULT_ELEVEN_TTS_MODEL.to_string()
}
fn default_azure_tts_voice() -> String {
    DEFAULT_AZURE_TTS_VOICE.to_string()
}
fn default_azure_tts_region() -> String {
    DEFAULT_AZURE_TTS_REGION.to_string()
}
fn default_language() -> String {
    "auto".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            history_enabled: false,
            active_provider_id: "gemini".to_string(),
            clipboard_context: false,
            autostart: false,
            tts_hotkey: DEFAULT_TTS_HOTKEY.to_string(),
            tts_provider: "azure".to_string(),
            tts_voice: DEFAULT_AZURE_TTS_VOICE.to_string(),
            tts_model: String::new(),
            tts_azure_region: DEFAULT_AZURE_TTS_REGION.to_string(),
            tts_local_voice: String::new(),
            tts_local_url: String::new(),
            tts_custom_url: DEFAULT_CUSTOM_TTS_URL.to_string(),
            tts_custom_voice: DEFAULT_CUSTOM_TTS_VOICE.to_string(),
            tts_custom_model: DEFAULT_CUSTOM_TTS_MODEL.to_string(),
            tts_gemini_voice: DEFAULT_GEMINI_TTS_VOICE.to_string(),
            tts_gemini_model: DEFAULT_GEMINI_TTS_MODEL.to_string(),
            tts_openai_voice: DEFAULT_OPENAI_TTS_VOICE.to_string(),
            tts_openai_model: DEFAULT_OPENAI_TTS_MODEL.to_string(),
            tts_eleven_voice: DEFAULT_ELEVEN_TTS_VOICE.to_string(),
            tts_eleven_model: DEFAULT_ELEVEN_TTS_MODEL.to_string(),
            tts_azure_voice: DEFAULT_AZURE_TTS_VOICE.to_string(),
            tts_azure_region_setting: DEFAULT_AZURE_TTS_REGION.to_string(),
            language: default_language(),
        }
    }
}

impl AppSettings {
    pub fn normalize(&mut self) {
        if self.hotkey.trim().is_empty() {
            self.hotkey = DEFAULT_HOTKEY.to_string();
        }
        if self.tts_hotkey.trim().is_empty() {
            self.tts_hotkey = DEFAULT_TTS_HOTKEY.to_string();
        }
        if self.tts_custom_url.trim().is_empty() {
            self.tts_custom_url = DEFAULT_CUSTOM_TTS_URL.to_string();
        }
        if self.tts_custom_voice.trim().is_empty() {
            self.tts_custom_voice = DEFAULT_CUSTOM_TTS_VOICE.to_string();
        }
        if self.tts_custom_model.trim().is_empty() {
            self.tts_custom_model = DEFAULT_CUSTOM_TTS_MODEL.to_string();
        }
        if self.tts_gemini_voice.trim().is_empty() {
            self.tts_gemini_voice = DEFAULT_GEMINI_TTS_VOICE.to_string();
        }
        if self.tts_gemini_model.trim().is_empty() {
            self.tts_gemini_model = DEFAULT_GEMINI_TTS_MODEL.to_string();
        }
        if self.tts_openai_voice.trim().is_empty() {
            self.tts_openai_voice = DEFAULT_OPENAI_TTS_VOICE.to_string();
        }
        if self.tts_openai_model.trim().is_empty() {
            self.tts_openai_model = DEFAULT_OPENAI_TTS_MODEL.to_string();
        }
        if self.tts_eleven_voice.trim().is_empty() {
            self.tts_eleven_voice = DEFAULT_ELEVEN_TTS_VOICE.to_string();
        }
        if self.tts_eleven_model.trim().is_empty() {
            self.tts_eleven_model = DEFAULT_ELEVEN_TTS_MODEL.to_string();
        }
        if self.tts_azure_voice.trim().is_empty() {
            self.tts_azure_voice = DEFAULT_AZURE_TTS_VOICE.to_string();
        }
        if self.tts_azure_region_setting.trim().is_empty() {
            if !self.tts_azure_region.trim().is_empty() {
                self.tts_azure_region_setting = self.tts_azure_region.clone();
            } else {
                self.tts_azure_region_setting = DEFAULT_AZURE_TTS_REGION.to_string();
            }
        }
        self.tts_azure_region = self.tts_azure_region_setting.clone();

        // Sync active tts_voice and tts_model depending on selected provider
        match self.tts_provider.to_lowercase().as_str() {
            "local" => {
                self.tts_provider = "custom".to_string();
                self.tts_voice = self.tts_custom_voice.clone();
                self.tts_model = self.tts_custom_model.clone();
            }
            "custom" => {
                self.tts_voice = self.tts_custom_voice.clone();
                self.tts_model = self.tts_custom_model.clone();
            }
            "openai" => {
                self.tts_voice = self.tts_openai_voice.clone();
                self.tts_model = self.tts_openai_model.clone();
            }
            "elevenlabs" => {
                self.tts_voice = self.tts_eleven_voice.clone();
                self.tts_model = self.tts_eleven_model.clone();
            }
            "azure" => {
                self.tts_voice = self.tts_azure_voice.clone();
                self.tts_azure_region = self.tts_azure_region_setting.clone();
            }
            _ => {
                self.tts_voice = self.tts_gemini_voice.clone();
                self.tts_model = self.tts_gemini_model.clone();
            }
        }
    }
}


pub fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    prepare_config_dir(app)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join(FILE_NAME))
}

fn read_disk_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(Value::Object(serde_json::Map::new()))
}

pub fn load(app: &AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        let mut settings = AppSettings::default();
        settings.normalize();
        save_to_disk(&path, &settings)?;
        return Ok(settings);
    }
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Settings Datei konnte nicht gelesen werden: {e}");
            let mut settings = AppSettings::default();
            settings.normalize();
            return Ok(settings);
        }
    };
    match settings_from_json(&raw) {
        Ok(settings) => Ok(settings),
        Err(e) => {
            eprintln!("Settings JSON ungültig ({e}), Original bleibt unangetastet.");
            let bak = path.with_extension("json.bak");
            let _ = fs::copy(&path, &bak);
            let mut settings = AppSettings::default();
            settings.normalize();
            Ok(settings)
        }
    }
}

fn save_to_disk(path: &PathBuf, settings: &AppSettings) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &raw).map_err(|e| format!("Settings nicht speicherbar: {e}"))?;
    fs::copy(&tmp, path).map_err(|e| format!("Settings nicht speicherbar: {e}"))?;
    let _ = fs::remove_file(&tmp);
    Ok(())
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    load(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Value) -> Result<AppSettings, String> {
    let path = settings_path(&app)?;
    let disk = if path.exists() {
        read_disk_json(&path)
    } else {
        Value::Object(serde_json::Map::new())
    };
    let defaults = serde_json::to_value(AppSettings::default()).map_err(|e| e.to_string())?;
    let merged = overlay_value(defaults, overlay_value(disk, settings));
    let mut next: AppSettings = serde_json::from_value(merged).map_err(|e| e.to_string())?;
    next.normalize();
    save_to_disk(&path, &next)?;

    if let Err(err) = crate::overlay::register_hotkeys(&app, &next.hotkey, &next.tts_hotkey) {
        eprintln!("Hotkeys Registrierung Warnung: {err}");
    }
    if let Err(err) = apply_autostart(&app, next.autostart) {
        eprintln!("Autostart Warnung: {err}");
    }

    crate::overlay::update_window_titles(&app, &next.language);

    let _ = app.emit("settings-changed", &next);
    Ok(next)
}

fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        let launcher = app.autolaunch();
        if enabled {
            launcher.enable().map_err(|e| format!("Autostart fehlgeschlagen: {e}"))?;
        } else {
            launcher
                .disable()
                .map_err(|e| format!("Autostart aus fehlgeschlagen: {e}"))?;
        }
    }
    let _ = (app, enabled);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overlay_keeps_disk_when_patch_omits_key() {
        let disk = json!({"hotkey": "ctrl+x", "language": "en"});
        let incoming = json!({"language": "de"});
        let merged = overlay_value(disk, incoming);
        assert_eq!(merged["hotkey"], "ctrl+x");
        assert_eq!(merged["language"], "de");
    }

    #[test]
    fn overlay_skips_null() {
        let disk = json!({"hotkey": "alt+space"});
        let incoming = json!({"hotkey": null});
        let merged = overlay_value(disk, incoming);
        assert_eq!(merged["hotkey"], "alt+space");
    }

    #[test]
    fn settings_from_json_fills_only_missing_fields() {
        let raw = r#"{"hotkey":"ctrl+q","historyEnabled":true}"#;
        let s = settings_from_json(raw).expect("parse");
        assert_eq!(s.hotkey, "ctrl+q");
        assert!(s.history_enabled);
        assert_eq!(s.language, "auto");
    }
}
