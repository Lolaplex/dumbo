use base64::prelude::*;
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

use crate::settings::{
    get_config_dir, DEFAULT_AZURE_TTS_REGION, DEFAULT_AZURE_TTS_VOICE, DEFAULT_CUSTOM_TTS_MODEL,
    DEFAULT_CUSTOM_TTS_URL, DEFAULT_CUSTOM_TTS_VOICE, DEFAULT_ELEVEN_TTS_MODEL,
    DEFAULT_ELEVEN_TTS_VOICE, DEFAULT_GEMINI_TTS_MODEL, DEFAULT_GEMINI_TTS_VOICE,
    DEFAULT_OPENAI_TTS_MODEL, DEFAULT_OPENAI_TTS_VOICE,
};

const KEYRING_SERVICE: &str = "dumbo";
const TTS_KEYS_FILE: &str = "tts_keys.json";

static KEY_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn key_cache() -> &'static Mutex<HashMap<String, String>> {
    KEY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn keyring_account(id: &str) -> String {
    format!("tts.{id}")
}

fn read_fallback_keys() -> HashMap<String, String> {
    let path = get_config_dir().join(TTS_KEYS_FILE);
    if !path.exists() {
        return HashMap::new();
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_fallback_key(id: &str, key: &str) {
    let mut map = read_fallback_keys();
    if key.trim().is_empty() {
        map.remove(id);
    } else {
        map.insert(id.to_string(), key.trim().to_string());
    }
    let path = get_config_dir().join(TTS_KEYS_FILE);
    if let Ok(raw) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(path, raw);
    }
}

pub fn read_tts_key(id: &str) -> Result<String, String> {
    let clean_id = id.trim().to_lowercase();
    if clean_id == "custom" {
        return Ok(String::new());
    }
    if let Ok(map) = key_cache().lock() {
        if let Some(cached) = map.get(&clean_id) {
            return Ok(cached.clone());
        }
    }

    let mut found_key = String::new();

    // 1. Try OS Keyring
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&clean_id)) {
        if let Ok(value) = entry.get_password() {
            if !value.trim().is_empty() {
                found_key = value.trim().to_string();
            }
        }
    }

    // 2. Try file fallback if keyring didn't yield a key
    if found_key.is_empty() {
        let fallback_map = read_fallback_keys();
        if let Some(val) = fallback_map.get(&clean_id) {
            if !val.trim().is_empty() {
                found_key = val.trim().to_string();
            }
        }
    }

    // 3. If empty, fallback to general provider key (Gemini, OpenAI)
    if found_key.is_empty() {
        if clean_id == "gemini" {
            if let Ok(gemini_provider_key) = crate::providers::read_key("gemini") {
                if !gemini_provider_key.trim().is_empty() {
                    found_key = gemini_provider_key;
                }
            }
        } else if clean_id == "openai" {
            if let Ok(openai_provider_key) = crate::providers::read_key("openai") {
                if !openai_provider_key.trim().is_empty() {
                    found_key = openai_provider_key;
                }
            }
        }
    }

    if let Ok(mut map) = key_cache().lock() {
        map.insert(clean_id, found_key.clone());
    }
    Ok(found_key)
}

pub fn write_tts_key(id: &str, key: &str) -> Result<(), String> {
    let clean_id = id.trim().to_lowercase();
    let clean_key = key.trim().to_string();

    // 1. In-memory cache
    if let Ok(mut map) = key_cache().lock() {
        if clean_key.is_empty() {
            map.remove(&clean_id);
        } else {
            map.insert(clean_id.clone(), clean_key.clone());
        }
    }

    // 2. Disk fallback file
    write_fallback_key(&clean_id, &clean_key);

    // 3. OS Keyring (best effort, ignore failure if unavailable)
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&clean_id)) {
        if clean_key.is_empty() {
            let _ = entry.delete_credential();
        } else {
            let _ = entry.set_password(&clean_key);
        }
    }

    Ok(())
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::thread;

static IS_PLAYING: AtomicBool = AtomicBool::new(false);
static IS_SYNTHESIZING: AtomicBool = AtomicBool::new(false);
static LAST_TTS_BUSY: AtomicBool = AtomicBool::new(false);
static TTS_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static TTS_APP: OnceLock<AppHandle> = OnceLock::new();
static AUDIO_TX: OnceLock<Mutex<Sender<AudioCommand>>> = OnceLock::new();

pub fn init(app: &AppHandle) {
    let _ = TTS_APP.set(app.clone());
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsLiveState {
    pub synthesizing: bool,
    pub playing: bool,
    pub busy: bool,
}

#[tauri::command]
pub fn get_tts_state() -> TtsLiveState {
    let synthesizing = IS_SYNTHESIZING.load(Ordering::SeqCst);
    let playing = IS_PLAYING.load(Ordering::SeqCst);
    TtsLiveState {
        synthesizing,
        playing,
        busy: synthesizing || playing,
    }
}

fn sync_tts_busy_event() {
    let synthesizing = IS_SYNTHESIZING.load(Ordering::SeqCst);
    let playing = IS_PLAYING.load(Ordering::SeqCst);
    let busy = synthesizing || playing;

    if let Some(app) = TTS_APP.get() {
        let _ = app.emit("tts-state", TtsLiveState {
            synthesizing,
            playing,
            busy,
        });
        if LAST_TTS_BUSY.load(Ordering::SeqCst) != busy {
            LAST_TTS_BUSY.store(busy, Ordering::SeqCst);
            let _ = app.emit("tts-busy", busy);
        }
    }
}

fn set_playing(playing: bool) {
    IS_PLAYING.store(playing, Ordering::SeqCst);
    sync_tts_busy_event();
}

fn set_synthesizing(synthesizing: bool) {
    IS_SYNTHESIZING.store(synthesizing, Ordering::SeqCst);
    sync_tts_busy_event();
}

enum AudioCommand {
    Enqueue(Vec<u8>),
    Stop,
}

fn begin_tts_session() -> u64 {
    TTS_SESSION.fetch_add(1, Ordering::SeqCst) + 1
}

fn is_tts_session_active(session: u64) -> bool {
    TTS_SESSION.load(Ordering::SeqCst) == session
}

/// Split text into sentence-sized TTS phrases (not word-level).
fn split_tts_phrases(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let mut phrases = Vec::new();
    let mut chunk = String::new();

    for ch in trimmed.chars() {
        chunk.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') {
            let phrase = chunk.trim().to_string();
            if !phrase.is_empty() {
                phrases.push(phrase);
            }
            chunk.clear();
        }
    }

    let tail = chunk.trim();
    if !tail.is_empty() {
        phrases.push(tail.to_string());
    }

    if phrases.is_empty() {
        return vec![trimmed.to_string()];
    }

    let mut merged = Vec::new();
    for phrase in phrases {
        if phrase.len() > 240 {
            merged.extend(split_long_phrase(&phrase));
        } else {
            merged.push(phrase);
        }
    }
    merged
}

fn split_long_phrase(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut chunk = String::new();

    for ch in text.chars() {
        chunk.push(ch);
        if ch == ';' || ch == ',' {
            let phrase = chunk.trim().to_string();
            if !phrase.is_empty() {
                parts.push(phrase);
            }
            chunk.clear();
        }
    }

    let tail = chunk.trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }

    if parts.is_empty() {
        vec![text.trim().to_string()]
    } else {
        parts
    }
}

fn get_audio_tx() -> Sender<AudioCommand> {
    AUDIO_TX
        .get_or_init(|| {
            let (tx, rx) = channel::<AudioCommand>();
            thread::spawn(move || {
                let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
                    eprintln!("Audio-Ausgabegerät konnte nicht initialisiert werden.");
                    return;
                };
                let mut current_sink: Option<Sink> = None;

                while let Ok(cmd_result) = rx
                    .recv_timeout(std::time::Duration::from_millis(60))
                    .map(Some)
                    .or_else(|e| match e {
                        std::sync::mpsc::RecvTimeoutError::Timeout => Ok(None),
                        std::sync::mpsc::RecvTimeoutError::Disconnected => Err(()),
                    })
                {
                    if let Some(cmd) = cmd_result {
                        match cmd {
                            AudioCommand::Enqueue(bytes) => {
                                let cursor = Cursor::new(bytes);
                                match Decoder::new(cursor) {
                                    Ok(source) => {
                                        if let Some(ref sink) = current_sink {
                                            sink.append(source);
                                            set_playing(true);
                                        } else {
                                            match Sink::try_new(&stream_handle) {
                                                Ok(sink) => {
                                                    sink.append(source);
                                                    sink.play();
                                                    set_playing(true);
                                                    current_sink = Some(sink);
                                                }
                                                Err(err) => {
                                                    eprintln!("Audio-Sink Fehler: {err}");
                                                    set_playing(false);
                                                }
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        eprintln!("Audio-Dekodierung fehlgeschlagen: {err}");
                                    }
                                }
                            }
                            AudioCommand::Stop => {
                                if let Some(sink) = current_sink.take() {
                                    sink.stop();
                                }
                                set_playing(false);
                            }
                        }
                    } else if let Some(ref sink) = current_sink {
                        if sink.empty() {
                            current_sink = None;
                            set_playing(false);
                        }
                    }
                }
            });
            Mutex::new(tx)
        })
        .lock()
        .unwrap()
        .clone()
}

pub fn is_playing() -> bool {
    IS_PLAYING.load(Ordering::SeqCst)
}

pub fn stop_tts() {
    TTS_SESSION.fetch_add(1, Ordering::SeqCst);
    let tx = get_audio_tx();
    let _ = tx.send(AudioCommand::Stop);
    set_playing(false);
    set_synthesizing(false);
}

fn enqueue_audio_bytes(bytes: Vec<u8>) -> Result<(), String> {
    let tx = get_audio_tx();
    tx.send(AudioCommand::Enqueue(bytes))
        .map_err(|e| format!("Audio-Kanal Fehler: {e}"))
}

async fn synthesize_and_play_phrases(
    session: u64,
    provider: &str,
    key: &str,
    model: &str,
    voice: &str,
    region_or_url: &str,
    text: &str,
) -> Result<(), String> {
    let phrases = split_tts_phrases(text);
    if phrases.is_empty() {
        return Ok(());
    }

    let single_phrase = phrases.len() == 1;

    set_synthesizing(true);
    let run_result = async {
        for phrase in phrases {
            if !is_tts_session_active(session) {
                return Ok(());
            }
            let audio = synthesize(provider, key, model, voice, region_or_url, &phrase).await?;
            if !is_tts_session_active(session) {
                return Ok(());
            }
            if single_phrase {
                set_cached_audio(
                    text.to_string(),
                    provider.to_string(),
                    model.to_string(),
                    voice.to_string(),
                    region_or_url.to_string(),
                    audio.clone(),
                );
            }
            enqueue_audio_bytes(audio)?;
        }
        Ok(())
    }
    .await;
    set_synthesizing(false);
    run_result
}

fn pcm_to_wav(pcm_data: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) -> Vec<u8> {
    let mut wav = Vec::with_capacity(44 + pcm_data.len());
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_len = pcm_data.len() as u32;
    let riff_len = 36 + data_len;

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm_data);
    wav
}

#[derive(Deserialize)]
struct GeminiPart {
    #[serde(rename = "inlineData")]
    inline_data: Option<GeminiInlineData>,
}

#[derive(Deserialize)]
struct GeminiInlineData {
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    data: Option<String>,
}

#[derive(Deserialize)]
struct GeminiContent {
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    error: Option<GeminiError>,
}

#[derive(Deserialize)]
struct GeminiError {
    message: Option<String>,
}

async fn synthesize_gemini(
    key: &str,
    model: &str,
    voice: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    if key.trim().is_empty() {
        return Err("Gemini API Key fehlt.".into());
    }
    let model_name = if model.trim().is_empty() {
        DEFAULT_GEMINI_TTS_MODEL
    } else {
        model.trim()
    };
    let voice_name = if voice.trim().is_empty() {
        DEFAULT_GEMINI_TTS_VOICE
    } else {
        voice.trim()
    };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model_name, key.trim()
    );

    let prompt = format!(
        "Lies den folgenden Text klar, flüssig und mit natürlicher Betonung auf Deutsch vor. Füge keine Einleitung oder Kommentare hinzu:\n\n{}",
        text
    );

    let payload = serde_json::json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }],
        "generationConfig": {
            "responseModalities": ["AUDIO"],
            "speechConfig": {
                "voiceConfig": {
                    "prebuiltVoiceConfig": {
                        "voiceName": voice_name
                    }
                }
            }
        }
    });

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Gemini Request fehlgeschlagen: {e}"))?;

    let parsed: GeminiResponse = res
        .json()
        .await
        .map_err(|e| format!("Gemini Antwort ungültig: {e}"))?;

    if let Some(err) = parsed.error {
        return Err(format!(
            "Gemini Fehler: {}",
            err.message.unwrap_or_else(|| "Unbekannt".into())
        ));
    }

    let inline = parsed
        .candidates
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.content)
        .and_then(|c| c.parts)
        .and_then(|parts| {
            parts.into_iter().find_map(|p| p.inline_data)
        })
        .ok_or_else(|| "Gemini hat keine Audiodaten zurückgegeben.".to_string())?;

    let b64 = inline.data.ok_or_else(|| "Audiodaten leer.".to_string())?;
    let raw_bytes = BASE64_STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("Base64 Dekodierungsfehler: {e}"))?;

    let mime = inline.mime_type.unwrap_or_default();
    if mime.contains("audio/pcm") || mime.contains("rate=24000") {
        Ok(pcm_to_wav(&raw_bytes, 24000, 1, 16))
    } else {
        Ok(raw_bytes)
    }
}

async fn synthesize_elevenlabs(
    key: &str,
    model: &str,
    voice_id: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    if key.trim().is_empty() {
        return Err("ElevenLabs API Key fehlt.".into());
    }
    let vid = if voice_id.trim().is_empty() {
        DEFAULT_ELEVEN_TTS_VOICE
    } else {
        voice_id.trim()
    };
    let mid = if model.trim().is_empty() {
        DEFAULT_ELEVEN_TTS_MODEL
    } else {
        model.trim()
    };

    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{vid}/stream");

    let payload = serde_json::json!({
        "text": text,
        "model_id": mid,
        "voice_settings": {
            "stability": 0.5,
            "similarity_boost": 0.75
        }
    });

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("xi-api-key", key.trim())
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("ElevenLabs Request fehlgeschlagen: {e}"))?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(format!("ElevenLabs Fehler: {err_text}"));
    }

    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("Audio-Stream fehlerhaft: {e}"))?;
    Ok(bytes.to_vec())
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

async fn synthesize_azure(
    key: &str,
    region: &str,
    voice: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    if key.trim().is_empty() {
        return Err("Azure Speech Key fehlt.".into());
    }
    let reg = if region.trim().is_empty() {
        DEFAULT_AZURE_TTS_REGION
    } else {
        region.trim()
    };
    let vname = if voice.trim().is_empty() {
        DEFAULT_AZURE_TTS_VOICE
    } else {
        voice.trim()
    };

    let lang = if vname.starts_with("en-") {
        "en-US"
    } else if vname.starts_with("fr-") {
        "fr-FR"
    } else if vname.starts_with("es-") {
        "es-ES"
    } else {
        "de-DE"
    };

    let url = format!("https://{reg}.tts.speech.microsoft.com/cognitiveservices/v1");

    let ssml = format!(
        "<speak version='1.0' xml:lang='{lang}'><voice xml:lang='{lang}' name='{vname}'>{}</voice></speak>",
        escape_xml(text)
    );

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", key.trim())
        .header("Content-Type", "application/ssml+xml")
        .header(
            "X-Microsoft-OutputFormat",
            "audio-24khz-48kbitrate-mono-mp3",
        )
        .header("User-Agent", "Dumbo")
        .body(ssml)
        .send()
        .await
        .map_err(|e| format!("Azure Request fehlgeschlagen: {e}"))?;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        return Err(format!("Azure Speech Fehler ({status}): {err_text}"));
    }

    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("Audio-Stream fehlerhaft: {e}"))?;
    Ok(bytes.to_vec())
}

async fn synthesize_openai(
    key: &str,
    model: &str,
    voice: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    if key.trim().is_empty() {
        return Err("OpenAI API Key fehlt.".into());
    }
    let m = if model.trim().is_empty() {
        DEFAULT_OPENAI_TTS_MODEL
    } else {
        model.trim()
    };
    let v = if voice.trim().is_empty() {
        DEFAULT_OPENAI_TTS_VOICE
    } else {
        voice.trim()
    };

    let url = "https://api.openai.com/v1/audio/speech";
    let payload = serde_json::json!({
        "model": m,
        "voice": v,
        "input": text,
        "response_format": "mp3"
    });

    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .header("Authorization", format!("Bearer {}", key.trim()))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("OpenAI TTS Request fehlgeschlagen: {e}"))?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(format!("OpenAI TTS Fehler: {err_text}"));
    }

    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("Audio-Stream fehlerhaft: {e}"))?;
    Ok(bytes.to_vec())
}

fn normalize_tts_base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// Accept both `http://host:8880` and `http://host:8880/v1`.
fn openai_api_root(base: &str) -> String {
    let clean = normalize_tts_base(base);
    if clean.ends_with("/v1") {
        clean
    } else {
        format!("{clean}/v1")
    }
}

fn openai_speech_url(base: &str) -> String {
    format!("{}/audio/speech", openai_api_root(base))
}

async fn synthesize_openai_compatible(
    url: &str,
    voice: &str,
    model: &str,
    text: &str,
    default_url: &str,
    default_voice: &str,
    default_model: &str,
    response_format: &str,
    api_key: Option<&str>,
    timeout_secs: u64,
) -> Result<Vec<u8>, String> {
    let base_url = if url.trim().is_empty() {
        default_url
    } else {
        url.trim()
    };
    let voice_name = if voice.trim().is_empty() {
        default_voice
    } else {
        voice.trim()
    };
    let model_name = if model.trim().is_empty() {
        default_model
    } else {
        model.trim()
    };

    let speech_url = openai_speech_url(base_url);
    // Kokoro streams by default; force a complete body so rodio can decode it.
    let payload = serde_json::json!({
        "model": model_name,
        "voice": voice_name,
        "input": text,
        "response_format": response_format,
        "stream": false
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("HTTP Client Fehler: {e}"))?;

    let mut req = client
        .post(&speech_url)
        .header("Content-Type", "application/json")
        .json(&payload);
    if let Some(key) = api_key.map(str::trim).filter(|k| !k.is_empty()) {
        req = req.header("Authorization", format!("Bearer {key}"));
    }

    let res = req
        .send()
        .await
        .map_err(|e| format!("TTS Server nicht erreichbar ({speech_url}): {e}"))?;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        let detail = parse_tts_error_body(&err_text);
        return Err(format!("TTS Fehler (Status {status}): {detail}"));
    }

    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("Audio-Stream fehlerhaft: {e}"))?;
    if bytes.is_empty() {
        return Err("TTS Server lieferte leeren Audio-Body".into());
    }
    Ok(bytes.to_vec())
}



async fn synthesize_custom(
    url: &str,
    voice: &str,
    model: &str,
    text: &str,
    api_key: &str,
) -> Result<Vec<u8>, String> {
    // Prefer mp3 (Kokoro default). If the server rejects it, retry wav.
    match synthesize_openai_compatible(
        url,
        voice,
        model,
        text,
        DEFAULT_CUSTOM_TTS_URL,
        DEFAULT_CUSTOM_TTS_VOICE,
        DEFAULT_CUSTOM_TTS_MODEL,
        "mp3",
        Some(api_key),
        300,
    )
    .await
    {
        Ok(bytes) => Ok(bytes),
        Err(first) => {
            let lower = first.to_lowercase();
            let format_issue = lower.contains("response_format")
                || lower.contains("unsupported")
                || lower.contains("mp3")
                || lower.contains("format");
            if !format_issue {
                return Err(first);
            }
            synthesize_openai_compatible(
                url,
                voice,
                model,
                text,
                DEFAULT_CUSTOM_TTS_URL,
                DEFAULT_CUSTOM_TTS_VOICE,
                DEFAULT_CUSTOM_TTS_MODEL,
                "wav",
                Some(api_key),
                300,
            )
            .await
            .map_err(|second| format!("{first} | wav-Retry: {second}"))
        }
    }
}

fn parse_tts_error_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "Keine Fehlerdetails vom Server".to_string();
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(detail) = json.get("detail") {
            if let Some(msg) = detail.as_str() {
                return msg.to_string();
            }
            return detail.to_string();
        }
        if let Some(err) = json.get("error") {
            if let Some(msg) = err.get("message").and_then(|v| v.as_str()) {
                return msg.to_string();
            }
            if let Some(msg) = err.as_str() {
                return msg.to_string();
            }
            return err.to_string();
        }
        if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
            return msg.to_string();
        }
    }
    trimmed.to_string()
}

struct CachedTts {
    text: String,
    provider: String,
    model: String,
    voice: String,
    azure_region: String,
    audio: Vec<u8>,
}

static LAST_TTS: OnceLock<Mutex<Option<CachedTts>>> = OnceLock::new();

fn last_tts_cache() -> &'static Mutex<Option<CachedTts>> {
    LAST_TTS.get_or_init(|| Mutex::new(None))
}

fn get_cached_audio(
    text: &str,
    provider: &str,
    model: &str,
    voice: &str,
    azure_region: &str,
) -> Option<Vec<u8>> {
    let cache = last_tts_cache().lock().ok()?;
    let cached = cache.as_ref()?;
    let text_matches = (text.is_empty() && !cached.text.is_empty()) || cached.text == text;
    if text_matches
        && cached.provider.eq_ignore_ascii_case(provider)
        && cached.model == model
        && cached.voice == voice
        && cached.azure_region == azure_region
    {
        Some(cached.audio.clone())
    } else {
        None
    }
}

fn set_cached_audio(
    text: String,
    provider: String,
    model: String,
    voice: String,
    azure_region: String,
    audio: Vec<u8>,
) {
    if let Ok(mut cache) = last_tts_cache().lock() {
        *cache = Some(CachedTts {
            text,
            provider,
            model,
            voice,
            azure_region,
            audio,
        });
    }
}

pub async fn synthesize(
    provider: &str,
    key: &str,
    model: &str,
    voice: &str,
    region_or_url: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    match provider.to_lowercase().as_str() {
        "local" | "custom" => synthesize_custom(region_or_url, voice, model, text, key).await,
        "openai" => synthesize_openai(key, model, voice, text).await,
        "elevenlabs" => synthesize_elevenlabs(key, model, voice, text).await,
        "azure" => synthesize_azure(key, region_or_url, voice, text).await,
        _ => synthesize_gemini(key, model, voice, text).await,
    }
}

pub fn trigger_tts(app: &AppHandle) {
    if is_playing() {
        stop_tts();
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let session = begin_tts_session();

        let ctx = crate::selection::capture_context(&app);
        let text = ctx
            .selection
            .or(ctx.clipboard)
            .unwrap_or_default()
            .trim()
            .to_string();

        let Ok(mut settings) = crate::settings::load(&app) else {
            return;
        };
        settings.normalize();

        let provider = settings.tts_provider.to_lowercase();

        let (voice, model, region_or_url) = match provider.as_str() {
            "local" => (
                settings.tts_local_voice.as_str(),
                "chatterbox-nano",
                settings.tts_local_url.as_str(),
            ),
            "custom" => (
                settings.tts_custom_voice.as_str(),
                settings.tts_custom_model.as_str(),
                settings.tts_custom_url.as_str(),
            ),
            "openai" => (
                settings.tts_openai_voice.as_str(),
                settings.tts_openai_model.as_str(),
                "",
            ),
            "elevenlabs" => (
                settings.tts_eleven_voice.as_str(),
                settings.tts_eleven_model.as_str(),
                "",
            ),
            "azure" => (
                settings.tts_azure_voice.as_str(),
                "",
                settings.tts_azure_region_setting.as_str(),
            ),
            _ => (
                settings.tts_gemini_voice.as_str(),
                settings.tts_gemini_model.as_str(),
                "",
            ),
        };

        if text.is_empty() {
            let _ = app.emit("tts-error", "Kein Text markiert (und Zwischenablage ist leer).");
            return;
        }

        let voice_or_model = if !voice.trim().is_empty() {
            voice
        } else {
            model
        };

        if let Some(cached) = get_cached_audio(&text, &provider, model, voice, region_or_url) {
            let _ = enqueue_audio_bytes(cached);
            if settings.history_enabled {
                let _ = crate::history::save_tts_turn(&app, &provider, voice_or_model, &text);
            }
            return;
        }

        let key = read_tts_key(&provider).unwrap_or_default();
        if key.trim().is_empty()
            && (provider == "azure" || provider == "elevenlabs" || provider == "openai" || provider == "gemini")
        {
            let msg = format!("TTS API-Key für '{provider}' fehlt. Bitte in Einstellungen eintragen.");
            eprintln!("{msg}");
            let _ = app.emit("tts-error", msg);
            return;
        }

        if let Err(err) = synthesize_and_play_phrases(
            session,
            &provider,
            &key,
            model,
            voice,
            region_or_url,
            &text,
        )
        .await
        {
            eprintln!("TTS Fehler: {err}");
            let _ = app.emit("tts-error", format!("TTS-Fehler: {err}"));
        } else if settings.history_enabled {
            let _ = crate::history::save_tts_turn(&app, &provider, voice_or_model, &text);
        }
    });
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsTestPayload {
    pub provider: String,
    pub voice: String,
    pub model: String,
    pub azure_region: String,
    pub text: String,
}

#[tauri::command]
pub fn set_tts_key(id: String, key: String) -> Result<(), String> {
    write_tts_key(&id, &key)
}

#[tauri::command]
pub fn get_tts_key_status(id: String) -> Result<bool, String> {
    let key = read_tts_key(&id)?;
    Ok(!key.trim().is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTtsStatus {
    pub running: bool,
    pub ready: bool,
    pub url: String,
    pub message: String,
    #[serde(default)]
    pub device_backend: Option<String>,
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub cpu_warning: Option<String>,
}

#[tauri::command]
pub async fn get_local_tts_status(url: Option<String>) -> Result<LocalTtsStatus, String> {
    let target_url = url.unwrap_or_else(|| DEFAULT_CUSTOM_TTS_URL.to_string());
    let clean_url = normalize_tts_base(&target_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .map_err(|e| format!("Client Error: {e}"))?;

    // 1) /health check
    let health_url = format!("{clean_url}/health");
    match client.get(&health_url).send().await {
        Ok(res) if res.status().is_success() => {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                let ready = json.get("ready").and_then(|v| v.as_bool()).unwrap_or(false);
                let loading = json.get("loading").and_then(|v| v.as_bool()).unwrap_or(false);
                let error = json.get("error").and_then(|v| v.as_str());
                let device_backend = json
                    .get("deviceBackend")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let device_name = json
                    .get("deviceName")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let cpu_warning = json
                    .get("cpuWarning")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let message = if ready {
                    if let (Some(backend), Some(name)) = (&device_backend, &device_name) {
                        format!("TTS bereit ({backend}: {name})")
                    } else {
                        "TTS Server bereit".to_string()
                    }
                } else if loading {
                    "Server lädt Modell im Hintergrund...".to_string()
                } else if let Some(err) = error {
                    format!("TTS Server Fehler: {err}")
                } else {
                    "TTS Server läuft, Modell noch nicht initialisiert".to_string()
                };

                return Ok(LocalTtsStatus {
                    running: true,
                    ready,
                    url: clean_url,
                    message,
                    device_backend,
                    device_name,
                    cpu_warning,
                });
            }
        }
        _ => {}
    }

    // 2) OpenAI-compatible probe (Kokoro FastAPI, Piper wrappers, …)
    if let Some(status) = probe_openai_compatible_tts(&client, &clean_url).await {
        return Ok(status);
    }

    Ok(LocalTtsStatus {
        running: false,
        ready: false,
        url: clean_url,
        message: "TTS Server nicht erreichbar (weder /health noch /v1/models)".to_string(),
        device_backend: None,
        device_name: None,
        cpu_warning: None,
    })
}

async fn probe_openai_compatible_tts(
    client: &reqwest::Client,
    base: &str,
) -> Option<LocalTtsStatus> {
    let api_root = openai_api_root(base);
    let candidates = [
        format!("{api_root}/models"),
        format!("{api_root}/audio/voices"),
        format!("{base}/docs"),
        format!("{base}/"),
    ];

    for probe in candidates {
        match client.get(&probe).send().await {
            Ok(res) if res.status().is_success() || res.status().as_u16() == 401 => {
                let label = if api_root.contains("8880") || probe.contains("voices") {
                    "Kokoro/OpenAI-TTS erreichbar"
                } else {
                    "OpenAI-kompatibler TTS Server erreichbar"
                };
                return Some(LocalTtsStatus {
                    running: true,
                    ready: true,
                    url: base.to_string(),
                    message: format!("{label} ({probe})"),
                    device_backend: None,
                    device_name: None,
                    cpu_warning: None,
                });
            }
            Ok(res) if res.status().as_u16() == 404 || res.status().as_u16() == 405 => {
                continue;
            }
            _ => continue,
        }
    }

    // Last resort: OPTIONS/POST-shaped reachability via speech URL returning 4xx ≠ connection fail.
    let speech = openai_speech_url(base);
    match client
        .post(&speech)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
    {
        Ok(res) => {
            let code = res.status().as_u16();
            if code == 400 || code == 422 || code == 401 || code == 405 {
                return Some(LocalTtsStatus {
                    running: true,
                    ready: true,
                    url: base.to_string(),
                    message: format!("TTS Endpoint antwortet ({speech})"),
                    device_backend: None,
                    device_name: None,
                    cpu_warning: None,
                });
            }
            None
        }
        Err(_) => None,
    }
}

#[tauri::command]
pub fn stop_tts_cmd() -> Result<(), String> {
    stop_tts();
    Ok(())
}

#[tauri::command]
pub async fn test_tts(payload: TtsTestPayload) -> Result<(), String> {
    let session = begin_tts_session();
    let provider = payload.provider.to_lowercase();

    if let Some(cached) = get_cached_audio(
        &payload.text,
        &provider,
        &payload.model,
        &payload.voice,
        &payload.azure_region,
    ) {
        enqueue_audio_bytes(cached)?;
        return Ok(());
    }

    let key = read_tts_key(&provider)?;
    synthesize_and_play_phrases(
        session,
        &provider,
        &key,
        &payload.model,
        &payload.voice,
        &payload.azure_region,
        &payload.text,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{openai_api_root, openai_speech_url, split_tts_phrases};

    #[test]
    fn splits_on_sentence_boundaries() {
        let parts = split_tts_phrases("Hallo Welt. Das ist ein Test! Noch einer?");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "Hallo Welt.");
        assert_eq!(parts[1], "Das ist ein Test!");
        assert_eq!(parts[2], "Noch einer?");
    }

    #[test]
    fn single_phrase_unchanged() {
        let parts = split_tts_phrases("Nur ein kurzer Satz");
        assert_eq!(parts, vec!["Nur ein kurzer Satz"]);
    }

    #[test]
    fn kokoro_base_with_and_without_v1() {
        assert_eq!(openai_api_root("http://127.0.0.1:8880"), "http://127.0.0.1:8880/v1");
        assert_eq!(openai_api_root("http://127.0.0.1:8880/v1"), "http://127.0.0.1:8880/v1");
        assert_eq!(
            openai_speech_url("http://127.0.0.1:8880/v1"),
            "http://127.0.0.1:8880/v1/audio/speech"
        );
        assert_eq!(
            openai_speech_url("http://127.0.0.1:8880"),
            "http://127.0.0.1:8880/v1/audio/speech"
        );
    }
}

