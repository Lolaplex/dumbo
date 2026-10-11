use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{SampleFormat, WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::settings::get_config_dir;

const KEYRING_SERVICE: &str = "dumbo";
const STT_KEYS_FILE: &str = "stt_keys.json";
const DEV_PROMPT_KEYWORDS: &str = "TypeScript, Python, Rust, Tauri, Svelte, FastMCP, Lolaplex, Lolax, refactor, function, bug, commit, test, Coolify, Veadio, endpoint, database, API, interface, component, async, await";

static KEY_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn key_cache() -> &'static Mutex<HashMap<String, String>> {
    KEY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn keyring_account(id: &str) -> String {
    format!("stt.{id}")
}

fn read_fallback_keys() -> HashMap<String, String> {
    let path = get_config_dir().join(STT_KEYS_FILE);
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
    let path = get_config_dir().join(STT_KEYS_FILE);
    if let Ok(raw) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(path, raw);
    }
}

pub fn read_stt_key(id: &str) -> Result<String, String> {
    let clean_id = id.trim().to_lowercase();
    if clean_id.is_empty() {
        return Ok(String::new());
    }

    // 1. Env variable takes top precedence
    if clean_id == "groq" {
        if let Ok(env_key) = std::env::var("GROQ_API_KEY") {
            if !env_key.trim().is_empty() {
                return Ok(env_key.trim().to_string());
            }
        }
    }

    // 2. Process memory cache
    if let Ok(map) = key_cache().lock() {
        if let Some(cached) = map.get(&clean_id) {
            return Ok(cached.clone());
        }
    }

    let mut found_key = String::new();

    // 3. OS Keyring
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&clean_id)) {
        if let Ok(value) = entry.get_password() {
            if !value.trim().is_empty() {
                found_key = value.trim().to_string();
            }
        }
    }

    // 4. Fallback file
    if found_key.is_empty() {
        let fallback_map = read_fallback_keys();
        if let Some(val) = fallback_map.get(&clean_id) {
            if !val.trim().is_empty() {
                found_key = val.trim().to_string();
            }
        }
    }

    if !found_key.is_empty() {
        if let Ok(mut map) = key_cache().lock() {
            map.insert(clean_id, found_key.clone());
        }
    }

    Ok(found_key)
}

#[tauri::command]
pub fn set_stt_key(id: String, key: String) -> Result<(), String> {
    let clean_id = id.trim().to_lowercase();
    let clean_key = key.trim().to_string();

    if let Ok(mut map) = key_cache().lock() {
        if clean_key.is_empty() {
            map.remove(&clean_id);
        } else {
            map.insert(clean_id.clone(), clean_key.clone());
        }
    }

    write_fallback_key(&clean_id, &clean_key);

    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&clean_id)) {
        if clean_key.is_empty() {
            let _ = entry.delete_credential();
        } else {
            let _ = entry.set_password(&clean_key);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_stt_key_status(id: String) -> Result<bool, String> {
    let key = read_stt_key(&id)?;
    Ok(!key.trim().is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttState {
    pub recording: bool,
    pub transcribing: bool,
    pub last_text: Option<String>,
}

#[allow(dead_code)]
struct SendStream(cpal::Stream);
unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

struct ActiveRecording {
    _stream: SendStream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    is_active: Arc<AtomicBool>,
}

static ACTIVE_RECORDING: OnceLock<Mutex<Option<ActiveRecording>>> = OnceLock::new();
static IS_PROCESSING: AtomicBool = AtomicBool::new(false);

fn active_recording() -> &'static Mutex<Option<ActiveRecording>> {
    ACTIVE_RECORDING.get_or_init(|| Mutex::new(None))
}

pub fn is_recording() -> bool {
    let Ok(guard) = active_recording().lock() else {
        return false;
    };
    guard.is_some()
}

pub fn emit_stt_state(app: &AppHandle, recording: bool, transcribing: bool, text: Option<String>) {
    let state = SttState {
        recording,
        transcribing,
        last_text: text,
    };
    let _ = app.emit("stt-state-changed", state);
}

#[tauri::command]
pub fn get_stt_state() -> SttState {
    SttState {
        recording: is_recording(),
        transcribing: IS_PROCESSING.load(Ordering::SeqCst),
        last_text: None,
    }
}

pub fn toggle_stt(app: &AppHandle) {
    if is_recording() {
        stop_and_process_recording(app);
    } else {
        start_recording(app);
    }
}

#[tauri::command]
pub fn toggle_stt_cmd(app: AppHandle) -> Result<bool, String> {
    toggle_stt(&app);
    Ok(is_recording())
}

pub fn start_recording(app: &AppHandle) {
    if is_recording() || IS_PROCESSING.load(Ordering::SeqCst) {
        return;
    }

    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        eprintln!("Kein Audio-Eingabegerät gefunden.");
        return;
    };

    let Ok(default_config) = device.default_input_config() else {
        eprintln!("Standard-Audio-Konfiguration nicht abrufbar.");
        return;
    };

    let sample_rate = default_config.sample_rate().0;
    let channels = default_config.channels();
    let samples = Arc::new(Mutex::new(Vec::new()));
    let samples_clone = samples.clone();
    let is_active = Arc::new(AtomicBool::new(true));
    let is_active_clone = is_active.clone();

    let err_fn = |err| eprintln!("Audio-Aufnahmefehler: {err}");

    let stream_result = match default_config.sample_format() {
        cpal::SampleFormat::F32 => {
            device.build_input_stream(
                &default_config.into(),
                move |data: &[f32], _: &_| {
                    if !is_active_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(mut buffer) = samples_clone.lock() {
                        if channels == 1 {
                            buffer.extend_from_slice(data);
                        } else {
                            // Convert multi-channel to mono
                            for frame in data.chunks(channels as usize) {
                                let mono = frame.iter().sum::<f32>() / channels as f32;
                                buffer.push(mono);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            device.build_input_stream(
                &default_config.into(),
                move |data: &[i16], _: &_| {
                    if !is_active_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(mut buffer) = samples_clone.lock() {
                        if channels == 1 {
                            for &s in data {
                                buffer.push(s as f32 / 32768.0);
                            }
                        } else {
                            for frame in data.chunks(channels as usize) {
                                let sum = frame.iter().map(|&s| s as f32 / 32768.0).sum::<f32>();
                                buffer.push(sum / channels as f32);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            device.build_input_stream(
                &default_config.into(),
                move |data: &[u16], _: &_| {
                    if !is_active_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(mut buffer) = samples_clone.lock() {
                        if channels == 1 {
                            for &s in data {
                                buffer.push((s as f32 - 32768.0) / 32768.0);
                            }
                        } else {
                            for frame in data.chunks(channels as usize) {
                                let sum = frame
                                    .iter()
                                    .map(|&s| (s as f32 - 32768.0) / 32768.0)
                                    .sum::<f32>();
                                buffer.push(sum / channels as f32);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        format => {
            eprintln!("Nicht unterstütztes Sample-Format: {:?}", format);
            return;
        }
    };

    let Ok(stream) = stream_result else {
        eprintln!("Stream konnte nicht erstellt werden.");
        return;
    };

    if let Err(err) = stream.play() {
        eprintln!("Aufnahmestream konnte nicht gestartet werden: {err}");
        return;
    }

    if let Ok(mut guard) = active_recording().lock() {
        *guard = Some(ActiveRecording {
            _stream: SendStream(stream),
            samples,
            sample_rate,
            is_active,
        });
    }

    emit_stt_state(app, true, false, None);
}

pub fn stop_and_process_recording(app: &AppHandle) {
    let recording_opt = {
        let Ok(mut guard) = active_recording().lock() else {
            return;
        };
        guard.take()
    };

    let Some(recording) = recording_opt else {
        return;
    };

    recording.is_active.store(false, Ordering::SeqCst);
    emit_stt_state(app, false, true, None);
    IS_PROCESSING.store(true, Ordering::SeqCst);

    let raw_samples = {
        let Ok(guard) = recording.samples.lock() else {
            IS_PROCESSING.store(false, Ordering::SeqCst);
            emit_stt_state(app, false, false, None);
            return;
        };
        guard.clone()
    };

    if raw_samples.is_empty() {
        IS_PROCESSING.store(false, Ordering::SeqCst);
        emit_stt_state(app, false, false, None);
        return;
    }

    let src_sample_rate = recording.sample_rate;
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let Ok(settings) = crate::settings::load(&app_handle) else {
            IS_PROCESSING.store(false, Ordering::SeqCst);
            emit_stt_state(&app_handle, false, false, None);
            return;
        };

        let wav_bytes = match encode_wav_16k_mono(&raw_samples, src_sample_rate) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("WAV-Codierungsfehler: {err}");
                IS_PROCESSING.store(false, Ordering::SeqCst);
                emit_stt_state(&app_handle, false, false, None);
                return;
            }
        };

        let groq_key = read_stt_key("groq").unwrap_or_default();
        if groq_key.trim().is_empty() {
            eprintln!("Groq API Key fehlt für STT.");
            IS_PROCESSING.store(false, Ordering::SeqCst);
            emit_stt_state(&app_handle, false, false, None);
            return;
        }

        // 1. Transcribe via Groq Whisper-Large-v3
        let raw_transcript = match transcribe_groq(&groq_key, wav_bytes).await {
            Ok(text) => text,
            Err(err) => {
                eprintln!("Groq Whisper Transkriptionsfehler: {err}");
                IS_PROCESSING.store(false, Ordering::SeqCst);
                emit_stt_state(&app_handle, false, false, None);
                return;
            }
        };

        let trimmed = raw_transcript.trim().to_string();
        if trimmed.is_empty() {
            IS_PROCESSING.store(false, Ordering::SeqCst);
            emit_stt_state(&app_handle, false, false, None);
            return;
        }

        // 2. Optional Prompt Refiner via Groq Llama-3.3-70B
        let final_text = if settings.stt_refiner {
            match refine_prompt_groq(&groq_key, &trimmed).await {
                Ok(refined) if !refined.trim().is_empty() => refined.trim().to_string(),
                _ => trimmed,
            }
        } else {
            trimmed
        };

        // 3. Inject text into active window automatically
        inject_text_into_active_window(&app_handle, &final_text, settings.stt_auto_enter);

        IS_PROCESSING.store(false, Ordering::SeqCst);
        emit_stt_state(&app_handle, false, false, Some(final_text));
    });
}

fn encode_wav_16k_mono(samples: &[f32], src_rate: u32) -> Result<Vec<u8>, String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    let mut writer =
        WavWriter::new(&mut cursor, spec).map_err(|e| format!("WavWriter init: {e}"))?;

    let ratio = src_rate as f64 / 16000.0;
    let target_len = (samples.len() as f64 / ratio) as usize;

    for i in 0..target_len {
        let src_idx = (i as f64 * ratio) as usize;
        if src_idx < samples.len() {
            let clamped = samples[src_idx].clamp(-1.0, 1.0);
            let sample_i16 = (clamped * 32767.0) as i16;
            writer
                .write_sample(sample_i16)
                .map_err(|e| format!("WAV write: {e}"))?;
        }
    }

    writer.finalize().map_err(|e| format!("WAV finalize: {e}"))?;
    Ok(cursor.into_inner())
}

async fn transcribe_groq(key: &str, wav_bytes: Vec<u8>) -> Result<String, String> {
    let client = reqwest::Client::new();
    let part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("speech.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-large-v3")
        .text("prompt", DEV_PROMPT_KEYWORDS)
        .text("response_format", "json");

    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Groq HTTP-Fehler: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Groq API Fehler ({status}): {body}"));
    }

    #[derive(Deserialize)]
    struct TranscribeResp {
        text: String,
    }

    let parsed: TranscribeResp = response
        .json()
        .await
        .map_err(|e| format!("Groq JSON-Fehler: {e}"))?;

    Ok(parsed.text)
}

async fn refine_prompt_groq(key: &str, raw_transcript: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let system_prompt = "You are a prompt refiner for an autonomous coding agent. The user dictated a prompt with speech recognition. Remove verbal filler words ('äh', 'also', 'mhm', stuttering), preserve technical terms and code symbols (camelCase, file extensions, functions, types), and return ONLY the clear, refined instruction in the spoken language (German or English). Never include commentary, preamble, or markdown code fence wrappers around the whole response.";

    let payload = serde_json::json!({
        "model": "llama-3.3-70b-versatile",
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": raw_transcript }
        ],
        "temperature": 0.1,
        "max_tokens": 1000
    });

    let response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Groq Chat HTTP-Fehler: {e}"))?;

    if !response.status().is_success() {
        return Ok(raw_transcript.to_string());
    }

    #[derive(Deserialize)]
    struct ChatChoice {
        message: ChatMessage,
    }
    #[derive(Deserialize)]
    struct ChatMessage {
        content: String,
    }
    #[derive(Deserialize)]
    struct ChatResp {
        choices: Vec<ChatChoice>,
    }

    let parsed: ChatResp = response
        .json()
        .await
        .map_err(|e| format!("Groq Chat JSON: {e}"))?;

    if let Some(first) = parsed.choices.into_iter().next() {
        Ok(first.message.content)
    } else {
        Ok(raw_transcript.to_string())
    }
}

pub fn inject_text_into_active_window(app: &AppHandle, text: &str, auto_enter: bool) {
    crate::selection::release_physical_modifiers();
    thread::sleep(Duration::from_millis(20));

    // Save previous clipboard if possible
    let prev_clip = app.clipboard().read_text().ok();

    // Set new text to clipboard
    let _ = app.clipboard().write_text(text);
    thread::sleep(Duration::from_millis(30));

    // Simulate Ctrl+V via enigo
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.key(Key::Control, Direction::Press);
        let _ = enigo.key(Key::Unicode('v'), Direction::Click);
        let _ = enigo.key(Key::Control, Direction::Release);

        if auto_enter {
            thread::sleep(Duration::from_millis(50));
            let _ = enigo.key(Key::Return, Direction::Click);
        }
    }

    // Restore previous clipboard after a short delay
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if let Some(prev) = prev_clip {
            let _ = app_clone.clipboard().write_text(&prev);
        }
    });
}
