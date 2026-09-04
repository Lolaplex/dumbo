use crate::history;
use crate::providers::{self, Provider, GEMINI_MODEL};
use crate::settings;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct ChatState {
    abort: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachment {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub mime_type: String,
    pub data_url: Option<String>,
    pub text_content: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorMessage {
    pub role: String,
    pub content: String,
}

struct ChatRequest {
    request_id: String,
    provider_id: String,
    model: String,
    prompt: String,
    selection: Option<String>,
    clipboard: Option<String>,
    detailed: bool,
    prior: Vec<PriorMessage>,
    chat_id: Option<String>,
    attachments: Vec<ChatAttachment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatChunk {
    request_id: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatDone {
    request_id: String,
    chat_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatError {
    request_id: String,
    message: String,
}

#[derive(Serialize)]
struct CompletionBody {
    model: String,
    stream: bool,
    messages: Vec<OpenAiMessage>,
}

#[derive(Clone, Serialize)]
struct OpenAiMessage {
    role: String,
    content: Value,
}

#[derive(Clone, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct GeminiNativeBody {
    #[serde(rename = "systemInstruction")]
    system_instruction: GeminiContent,
    contents: Vec<GeminiContent>,
}

#[derive(Serialize)]
struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
}

#[derive(Serialize)]
struct GeminiInlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

const CONCISE_PROMPT: &str =
    "Answer concisely. Prefer short, direct replies. Use markdown when it helps.";
const DETAILED_PROMPT: &str =
    "Answer thoroughly. Explain reasoning, edge cases, and alternatives. Use markdown.";

fn extract_base64(data_url: &str) -> (String, String) {
    if let Some(rest) = data_url.strip_prefix("data:") {
        if let Some((mime, b64)) = rest.split_once(";base64,") {
            return (mime.to_string(), b64.to_string());
        }
    }
    ("image/png".to_string(), data_url.to_string())
}

fn assemble_user(
    prompt: &str,
    selection: Option<&str>,
    clipboard: Option<&str>,
    attachments: &[ChatAttachment],
) -> String {
    let prompt = prompt.trim();
    let selection = selection.map(str::trim).filter(|value| !value.is_empty());
    let clipboard = clipboard.map(str::trim).filter(|value| !value.is_empty());
    let clipboard = clipboard.filter(|value| selection != Some(*value));

    let mut parts = Vec::new();

    for att in attachments {
        if let Some(ref text) = att.text_content {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                parts.push(format!("# File: {}\n```\n{}\n```", att.name, trimmed));
            }
        }
    }

    if let Some(selection) = selection {
        parts.push(format!("# Selection\n{selection}"));
    }
    if let Some(clipboard) = clipboard {
        parts.push(format!("# Clipboard\n{clipboard}"));
    }
    if !prompt.is_empty() {
        if parts.is_empty() {
            return prompt.to_string();
        }
        parts.push(format!("# Question\n{prompt}"));
    } else if parts.is_empty() {
        return String::new();
    }

    parts.join("\n\n")
}

const MAX_PRIOR: usize = 16;

fn sanitize_prior(prior: &[PriorMessage]) -> Vec<ChatMessage> {
    prior
        .iter()
        .filter(|item| item.role == "user" || item.role == "assistant")
        .filter(|item| !item.content.trim().is_empty())
        .rev()
        .take(MAX_PRIOR)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|item| ChatMessage {
            role: item.role.clone(),
            content: item.content.clone(),
        })
        .collect()
}

fn system_prompt(detailed: bool) -> &'static str {
    if detailed {
        DETAILED_PROMPT
    } else {
        CONCISE_PROMPT
    }
}

fn emit_error(app: &AppHandle, request_id: &str, message: impl Into<String>) {
    let _ = app.emit(
        "chat-error",
        ChatError {
            request_id: request_id.to_string(),
            message: message.into(),
        },
    );
}

fn extract_error(raw: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        if let Some(message) = value
            .pointer("/error/message")
            .and_then(|item| item.as_str())
        {
            return message.to_string();
        }
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "Unbekannter API-Fehler.".to_string()
    } else {
        trimmed.chars().take(240).collect()
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .tcp_nodelay(true)
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP-Client fehlt: {e}"))
}

fn use_gemini_native(provider: &Provider) -> bool {
    if provider.kind != "gemini" {
        return false;
    }
    let base = provider.base_url.to_ascii_lowercase();
    base.contains("generativelanguage.googleapis.com")
        || base.contains("aiplatform.googleapis.com")
}

fn gemini_stream_url(base_url: &str, model: &str) -> String {
    let model = model.trim().trim_start_matches("models/");
    let origin = base_url
        .trim_end_matches('/')
        .trim_end_matches("/openai")
        .trim_end_matches("/chat/completions");
    format!("{origin}/models/{model}:streamGenerateContent?alt=sse")
}

fn json_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            if text.is_empty() {
                None
            } else {
                Some(text.clone())
            }
        }
        Value::Array(parts) => {
            let mut text = String::new();
            for part in parts {
                if let Some(piece) = part.get("text").and_then(Value::as_str) {
                    text.push_str(piece);
                }
            }
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        _ => None,
    }
}

fn openai_chunk_text(data: &str) -> Result<Option<String>, String> {
    let value: Value =
        serde_json::from_str(data).map_err(|e| format!("Stream-JSON ungültig: {e}"))?;
    if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
        return Err(message.to_string());
    }
    let mut out = String::new();
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        for choice in choices {
            if let Some(text) = choice.pointer("/delta/content").and_then(json_text) {
                out.push_str(&text);
            } else if let Some(text) = choice.pointer("/message/content").and_then(json_text) {
                out.push_str(&text);
            } else if let Some(text) = choice.get("text").and_then(json_text) {
                out.push_str(&text);
            }
        }
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

fn gemini_chunk_text(data: &str) -> Result<Option<String>, String> {
    let value: Value =
        serde_json::from_str(data).map_err(|e| format!("Stream-JSON ungültig: {e}"))?;
    if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
        return Err(message.to_string());
    }
    let mut out = String::new();
    if let Some(candidates) = value.get("candidates").and_then(Value::as_array) {
        for candidate in candidates {
            let Some(parts) = candidate.pointer("/content/parts").and_then(Value::as_array) else {
                continue;
            };
            for part in parts {
                if part.get("thought").and_then(Value::as_bool).unwrap_or(false) {
                    continue;
                }
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    out.push_str(text);
                }
            }
        }
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

fn take_sse_events(buffer: &mut String, flush: bool) -> Vec<String> {
    if buffer.contains('\r') {
        *buffer = buffer.replace("\r\n", "\n").replace('\r', "\n");
    }
    let mut events = Vec::new();
    while let Some(index) = buffer.find('\n') {
        let line = buffer[..index].to_string();
        buffer.replace_range(..index + 1, "");
        push_sse_line(&mut events, &line);
    }
    if flush {
        push_sse_line(&mut events, buffer);
        buffer.clear();
    }
    events
}

fn push_sse_line(events: &mut Vec<String>, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Some(data) = trimmed.strip_prefix("data:") {
        let data = data.trim();
        if !data.is_empty() && data != "[DONE]" {
            events.push(data.to_string());
        }
        return;
    }
    if trimmed.starts_with('{') {
        events.push(trimmed.to_string());
    }
}

#[tauri::command]
pub fn abort_chat(state: tauri::State<ChatState>, request_id: String) -> Result<(), String> {
    if let Ok(map) = state.abort.lock() {
        if let Some(flag) = map.get(&request_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn start_chat(
    app: AppHandle,
    state: tauri::State<'_, ChatState>,
    request_id: String,
    provider_id: String,
    model: String,
    prompt: String,
    selection: Option<String>,
    clipboard: Option<String>,
    detailed: bool,
    prior: Option<Vec<PriorMessage>>,
    chat_id: Option<String>,
    attachments: Option<Vec<ChatAttachment>>,
) -> Result<(), String> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut map) = state.abort.lock() {
        map.insert(request_id.clone(), flag.clone());
    }

    let request = ChatRequest {
        request_id: request_id.clone(),
        provider_id,
        model,
        prompt,
        selection,
        clipboard,
        detailed,
        prior: prior.unwrap_or_default(),
        chat_id,
        attachments: attachments.unwrap_or_default(),
    };
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_chat(app_clone.clone(), request, flag).await;
        if let Some(state) = app_clone.try_state::<ChatState>() {
            if let Ok(mut map) = state.abort.lock() {
                map.remove(&request_id);
            }
        }
        if let Err(err) = result {
            emit_error(&app_clone, &request_id, err);
        }
    });
    Ok(())
}

async fn run_chat(
    app: AppHandle,
    request: ChatRequest,
    abort: Arc<AtomicBool>,
) -> Result<(), String> {
    let provider = providers::get_provider(&app, &request.provider_id)?;
    if provider.base_url.trim().is_empty() {
        return Err("Base-URL fehlt. Bitte in den Einstellungen eintragen.".into());
    }
    let key = providers::read_key(&provider.id)?;
    if key.trim().is_empty() && provider.kind != "ollama" && provider.kind != "lmstudio" {
        return Err("Kein API-Key. In Settings unter Gemini einfügen und Save drücken.".into());
    }
    let model = if request.model.trim().is_empty() {
        if provider.model.is_empty() {
            GEMINI_MODEL.to_string()
        } else {
            provider.model.clone()
        }
    } else {
        request.model.clone()
    };

    let user = assemble_user(
        &request.prompt,
        request.selection.as_deref(),
        request.clipboard.as_deref(),
        &request.attachments,
    );
    let system = system_prompt(request.detailed);
    let prior = sanitize_prior(&request.prior);

    let answer = if use_gemini_native(&provider) {
        stream_gemini(
            &app,
            &provider,
            &key,
            &model,
            system,
            &prior,
            &user,
            &request.attachments,
            &request.request_id,
            &abort,
        )
        .await?
    } else {
        stream_openai(
            &app,
            &provider,
            &key,
            &model,
            system,
            &prior,
            &user,
            &request.attachments,
            &request.request_id,
            &abort,
        )
        .await?
    };

    let chat_id = persist_if_enabled(&app, &provider.id, &model, &request, &answer);

    let _ = app.emit(
        "chat-done",
        ChatDone {
            request_id: request.request_id.clone(),
            chat_id,
        },
    );
    Ok(())
}

async fn stream_openai(
    app: &AppHandle,
    provider: &Provider,
    key: &str,
    model: &str,
    system: &str,
    prior: &[ChatMessage],
    user: &str,
    attachments: &[ChatAttachment],
    request_id: &str,
    abort: &AtomicBool,
) -> Result<String, String> {
    let mut messages = Vec::with_capacity(prior.len() + 2);
    messages.push(OpenAiMessage {
        role: "system".into(),
        content: Value::String(system.into()),
    });
    for item in prior {
        messages.push(OpenAiMessage {
            role: item.role.clone(),
            content: Value::String(item.content.clone()),
        });
    }

    let image_attachments: Vec<&ChatAttachment> = attachments
        .iter()
        .filter(|a| a.kind == "image" && a.data_url.is_some())
        .collect();

    let user_content = if image_attachments.is_empty() {
        Value::String(user.to_string())
    } else {
        let mut parts = vec![serde_json::json!({
            "type": "text",
            "text": user,
        })];
        for att in image_attachments {
            if let Some(ref data_url) = att.data_url {
                parts.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": data_url,
                    }
                }));
            }
        }
        Value::Array(parts)
    };

    messages.push(OpenAiMessage {
        role: "user".into(),
        content: user_content,
    });

    let body = CompletionBody {
        model: model.to_string(),
        stream: true,
        messages,
    };
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let mut builder = http_client()?
        .post(&url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .json(&body);
    if !key.is_empty() {
        builder = builder.bearer_auth(key);
    }
    read_sse_stream(app, builder, request_id, abort, openai_chunk_text).await
}

async fn stream_gemini(
    app: &AppHandle,
    provider: &Provider,
    key: &str,
    model: &str,
    system: &str,
    prior: &[ChatMessage],
    user: &str,
    attachments: &[ChatAttachment],
    request_id: &str,
    abort: &AtomicBool,
) -> Result<String, String> {
    let mut contents: Vec<GeminiContent> = prior
        .iter()
        .map(|item| GeminiContent {
            role: Some(if item.role == "assistant" {
                "model".into()
            } else {
                "user".into()
            }),
            parts: vec![GeminiPart::Text {
                text: item.content.clone(),
            }],
        })
        .collect();

    let mut user_parts: Vec<GeminiPart> = Vec::new();
    for att in attachments {
        if att.kind == "image" {
            if let Some(ref data_url) = att.data_url {
                let (mime_type, data) = extract_base64(data_url);
                user_parts.push(GeminiPart::InlineData {
                    inline_data: GeminiInlineData { mime_type, data },
                });
            }
        }
    }
    if !user.is_empty() || user_parts.is_empty() {
        user_parts.push(GeminiPart::Text {
            text: user.to_string(),
        });
    }

    contents.push(GeminiContent {
        role: Some("user".into()),
        parts: user_parts,
    });

    let body = GeminiNativeBody {
        system_instruction: GeminiContent {
            role: None,
            parts: vec![GeminiPart::Text {
                text: system.to_string(),
            }],
        },
        contents,
    };
    let url = gemini_stream_url(&provider.base_url, model);
    // Native Gemini rejects Authorization: Bearer — it treats that as OAuth
    // and returns 401 "Expected OAuth 2 access token". API keys go in x-goog-api-key.
    let builder = http_client()?
        .post(&url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("x-goog-api-key", key)
        .json(&body);
    read_sse_stream(app, builder, request_id, abort, gemini_chunk_text).await
}

async fn read_sse_stream(
    app: &AppHandle,
    builder: reqwest::RequestBuilder,
    request_id: &str,
    abort: &AtomicBool,
    parse: fn(&str) -> Result<Option<String>, String>,
) -> Result<String, String> {
    let response = builder
        .send()
        .await
        .map_err(|e| format!("Request fehlgeschlagen: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        return Err(format!("{status}: {}", extract_error(&raw)));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut answer = String::new();

    while let Some(chunk) = stream.next().await {
        if abort.load(Ordering::Relaxed) {
            break;
        }
        let bytes = chunk.map_err(|e| format!("Stream abgebrochen: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));
        emit_parsed_events(app, request_id, &mut answer, take_sse_events(&mut buffer, false), parse)?;
    }

    if !abort.load(Ordering::Relaxed) {
        emit_parsed_events(app, request_id, &mut answer, take_sse_events(&mut buffer, true), parse)?;
    }

    Ok(answer)
}

fn emit_parsed_events(
    app: &AppHandle,
    request_id: &str,
    answer: &mut String,
    events: Vec<String>,
    parse: fn(&str) -> Result<Option<String>, String>,
) -> Result<(), String> {
    for data in events {
        if let Some(text) = parse(&data)? {
            answer.push_str(&text);
            let _ = app.emit(
                "chat-chunk",
                ChatChunk {
                    request_id: request_id.to_string(),
                    text,
                },
            );
        }
    }
    Ok(())
}

fn persist_if_enabled(
    app: &AppHandle,
    provider_id: &str,
    model: &str,
    request: &ChatRequest,
    answer: &str,
) -> Option<String> {
    let enabled = settings::load(app)
        .map(|item| item.history_enabled)
        .unwrap_or(false);
    if !enabled || answer.trim().is_empty() {
        return None;
    }
    history::save_turn(
        app,
        request.chat_id.as_deref(),
        provider_id,
        model,
        &request.prompt,
        answer,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::{
        assemble_user, gemini_chunk_text, gemini_stream_url, openai_chunk_text, sanitize_prior,
        take_sse_events, PriorMessage,
    };

    #[test]
    fn assemble_plain_prompt_without_headers() {
        assert_eq!(assemble_user("hey", None, None, &[]), "hey");
    }

    #[test]
    fn assemble_skips_duplicate_clipboard() {
        let out = assemble_user("hey", Some("copied"), Some("copied"), &[]);
        assert!(!out.contains("# Clipboard"));
        assert!(out.contains("# Selection\ncopied"));
        assert!(out.contains("# Question\nhey"));
    }

    #[test]
    fn sse_splits_on_single_lf_and_crlf() {
        let mut buffer = "data: {\"a\":1}\r\ndata: {\"a\":2}\n".to_string();
        let events = take_sse_events(&mut buffer, false);
        assert_eq!(events, vec![r#"{"a":1}"#, r#"{"a":2}"#]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn sse_flushes_leftover_json() {
        let mut buffer = r#"{"choices":[{"message":{"content":"Hi"}}]}"#.to_string();
        let events = take_sse_events(&mut buffer, true);
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("Hi"));
    }

    #[test]
    fn openai_reads_delta_and_message() {
        let delta = r#"{"choices":[{"delta":{"content":"Hey"}}]}"#;
        assert_eq!(openai_chunk_text(delta).unwrap(), Some("Hey".into()));
        let full = r#"{"choices":[{"message":{"content":"there"}}]}"#;
        assert_eq!(openai_chunk_text(full).unwrap(), Some("there".into()));
    }

    #[test]
    fn gemini_skips_thought_parts() {
        let raw = r#"{"candidates":[{"content":{"parts":[{"thought":true,"text":"hmm"},{"text":"Hi"}]}}]}"#;
        assert_eq!(gemini_chunk_text(raw).unwrap(), Some("Hi".into()));
    }

    #[test]
    fn gemini_url_strips_openai_suffix() {
        let url = gemini_stream_url(
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "models/gemini-3.5-flash-lite",
        );
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.5-flash-lite:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn prior_keeps_last_sixteen_and_drops_empty() {
        let prior: Vec<PriorMessage> = (0..20)
            .map(|index| PriorMessage {
                role: if index % 2 == 0 { "user".into() } else { "assistant".into() },
                content: format!("m{index}"),
            })
            .collect();
        let kept = sanitize_prior(&prior);
        assert_eq!(kept.len(), 16);
        assert_eq!(kept[0].content, "m4");
        assert_eq!(kept.last().unwrap().content, "m19");
        let empty = sanitize_prior(&[PriorMessage {
            role: "user".into(),
            content: "  ".into(),
        }]);
        assert!(empty.is_empty());
    }
}
