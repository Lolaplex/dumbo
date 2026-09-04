use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

/// Windows Credential Manager often returns `NoEntry` in the same process
/// right after `set_password`. Keep a process-local copy so a just-saved key
/// works without restarting Dumbo.
static KEY_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn key_cache() -> &'static Mutex<HashMap<String, String>> {
    KEY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(id: &str) -> Option<String> {
    key_cache().lock().ok()?.get(id).cloned()
}

fn cache_set(id: &str, key: &str) {
    let Ok(mut map) = key_cache().lock() else {
        return;
    };
    map.insert(id.to_string(), key.to_string());
}

pub const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai";
pub const GEMINI_MODEL: &str = "gemini-3.5-flash-lite";
const KEYRING_SERVICE: &str = "dumbo";
const FILE_NAME: &str = "providers.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProviderFile {
    providers: Vec<Provider>,
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Konfigurationsordner fehlt: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Konfigurationsordner nicht anlegbar: {e}"))?;
    Ok(dir)
}

fn file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join(FILE_NAME))
}

fn keyring_account(id: &str) -> String {
    format!("provider.{id}")
}

const PROVIDER_KEYS_FILE: &str = "provider_keys.json";

fn read_fallback_provider_keys() -> HashMap<String, String> {
    let path = crate::settings::get_config_dir().join(PROVIDER_KEYS_FILE);
    if !path.exists() {
        return HashMap::new();
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_fallback_provider_key(id: &str, key: &str) {
    let mut map = read_fallback_provider_keys();
    if key.trim().is_empty() {
        map.remove(id);
    } else {
        map.insert(id.to_string(), key.trim().to_string());
    }
    let path = crate::settings::get_config_dir().join(PROVIDER_KEYS_FILE);
    if let Ok(raw) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(path, raw);
    }
}

pub fn read_key(id: &str) -> Result<String, String> {
    let clean_id = id.trim().to_string();
    if let Some(cached) = cache_get(&clean_id) {
        return Ok(cached);
    }

    let mut found = String::new();

    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&clean_id)) {
        if let Ok(value) = entry.get_password() {
            if !value.trim().is_empty() {
                found = value.trim().to_string();
            }
        }
    }

    if found.is_empty() {
        let map = read_fallback_provider_keys();
        if let Some(val) = map.get(&clean_id) {
            if !val.trim().is_empty() {
                found = val.trim().to_string();
            }
        }
    }

    cache_set(&clean_id, &found);
    Ok(found)
}

fn write_key(id: &str, key: &str) -> Result<(), String> {
    let clean_id = id.trim().to_string();
    let clean_key = key.trim().to_string();

    cache_set(&clean_id, &clean_key);
    write_fallback_provider_key(&clean_id, &clean_key);

    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&clean_id)) {
        if clean_key.is_empty() {
            let _ = entry.delete_credential();
        } else {
            let _ = entry.set_password(&clean_key);
        }
    }
    Ok(())
}

fn defaults() -> Vec<Provider> {
    vec![
        Provider {
            id: "gemini".into(),
            name: "Gemini".into(),
            kind: "gemini".into(),
            base_url: GEMINI_BASE_URL.into(),
            model: GEMINI_MODEL.into(),
        },
        Provider {
            id: "openai".into(),
            name: "OpenAI / Custom".into(),
            kind: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4.1-mini".into(),
        },
        Provider {
            id: "ollama".into(),
            name: "Ollama".into(),
            kind: "ollama".into(),
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: String::new(),
        },
        Provider {
            id: "lmstudio".into(),
            name: "LM Studio".into(),
            kind: "lmstudio".into(),
            base_url: "http://127.0.0.1:1234/v1".into(),
            model: String::new(),
        },
    ]
}

fn load_file(app: &AppHandle) -> Result<ProviderFile, String> {
    let path = file_path(app)?;
    if !path.exists() {
        return Ok(ProviderFile {
            providers: defaults(),
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Provider nicht lesbar: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Provider ungültig: {e}"))
}

fn save_file(app: &AppHandle, file: &ProviderFile) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    fs::write(file_path(app)?, raw).map_err(|e| format!("Provider nicht speicherbar: {e}"))
}

fn to_view(provider: Provider) -> ProviderView {
    let has_key = read_key(&provider.id)
        .map(|key| !key.is_empty())
        .unwrap_or(false);
    ProviderView {
        id: provider.id,
        name: provider.name,
        kind: provider.kind,
        base_url: provider.base_url,
        model: provider.model,
        has_key,
    }
}

pub fn get_provider(app: &AppHandle, id: &str) -> Result<Provider, String> {
    load_file(app)?
        .providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Provider {id} fehlt."))
}

pub fn ensure_defaults(app: &AppHandle) -> Result<(), String> {
    let path = file_path(app)?;
    if path.exists() {
        return Ok(());
    }
    save_file(
        app,
        &ProviderFile {
            providers: defaults(),
        },
    )
}

#[tauri::command]
pub fn list_providers(app: AppHandle) -> Result<Vec<ProviderView>, String> {
    Ok(load_file(&app)?
        .providers
        .into_iter()
        .map(to_view)
        .collect())
}

#[tauri::command]
pub fn upsert_provider(app: AppHandle, mut provider: Provider) -> Result<ProviderView, String> {
    if provider.id.trim().is_empty() {
        provider.id = uuid::Uuid::new_v4().to_string();
    }
    if provider.name.trim().is_empty() {
        return Err("Name fehlt.".into());
    }
    let mut file = load_file(&app)?;
    if let Some(existing) = file.providers.iter_mut().find(|p| p.id == provider.id) {
        *existing = provider.clone();
    } else {
        file.providers.push(provider.clone());
    }
    save_file(&app, &file)?;
    let _ = app.emit("providers-changed", ());
    Ok(to_view(provider))
}

#[tauri::command]
pub fn delete_provider(app: AppHandle, id: String) -> Result<(), String> {
    let mut file = load_file(&app)?;
    let before = file.providers.len();
    file.providers.retain(|p| p.id != id);
    if file.providers.len() == before {
        return Err("Provider nicht gefunden.".into());
    }
    let _ = write_key(&id, "");
    save_file(&app, &file)?;
    let _ = app.emit("providers-changed", ());
    Ok(())
}

#[tauri::command]
pub fn set_provider_key(app: AppHandle, id: String, key: String) -> Result<(), String> {
    write_key(&id, key.trim())?;
    let _ = app.emit("providers-changed", ());
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Option<Vec<ModelEntry>>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: Option<String>,
}

#[tauri::command]
pub async fn list_models(app: AppHandle, provider_id: String) -> Result<Vec<String>, String> {
    let provider = get_provider(&app, &provider_id)?;
    if provider.base_url.trim().is_empty() {
        return Ok(fallback_models(&provider));
    }
    let key = read_key(&provider.id)?;
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let mut request = reqwest::Client::new().get(&url);
    if !key.is_empty() {
        request = request.bearer_auth(&key);
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Ok(fallback_models(&provider));
    }
    let parsed = response
        .json::<ModelsResponse>()
        .await
        .unwrap_or(ModelsResponse { data: None });
    let mut models: Vec<String> = parsed
        .data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| entry.id)
        .collect();
    if models.is_empty() {
        models = fallback_models(&provider);
    }
    Ok(models)
}

fn fallback_models(provider: &Provider) -> Vec<String> {
    if provider.model.is_empty() {
        Vec::new()
    } else {
        vec![provider.model.clone()]
    }
}
