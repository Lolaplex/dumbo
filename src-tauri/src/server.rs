use serde::Deserialize;
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Deserialize)]
struct SpeakRequest {
    text: String,
}

pub fn start_agent_bridge(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let addr = "127.0.0.1:12480";
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => {
                println!("Dumbo Agent Voice Bridge lauscht auf http://{addr}");
                l
            }
            Err(e) => {
                eprintln!("Dumbo Agent Voice Bridge Port {addr} nicht bindbar: {e}");
                return;
            }
        };

        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                continue;
            };

            let app_handle = app.clone();
            tokio::spawn(async move {
                let mut buffer = [0u8; 16384];
                let Ok(n) = socket.read(&mut buffer).await else {
                    return;
                };
                if n == 0 {
                    return;
                }

                let request = String::from_utf8_lossy(&buffer[..n]);
                let mut lines = request.lines();
                let Some(first_line) = lines.next() else {
                    return;
                };

                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() < 2 {
                    return;
                }

                let method = parts[0];
                let path = parts[1];

                if method == "OPTIONS" {
                    let response = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n";
                    let _ = socket.write_all(response.as_bytes()).await;
                    return;
                }

                if method == "GET" && path == "/v1/status" {
                    let body = r#"{"status":"ok","agentVoiceBridge":true}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    return;
                }

                if method == "POST" && path == "/v1/speak" {
                    // Extract body after empty line
                    if let Some(pos) = request.find("\r\n\r\n") {
                        let body_str = &request[pos + 4..];
                        if let Ok(req) = serde_json::from_str::<SpeakRequest>(body_str) {
                            crate::tts::speak_text(&app_handle, &req.text);
                            let body = r#"{"status":"speaking"}"#;
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            let _ = socket.write_all(response.as_bytes()).await;
                            return;
                        }
                    }
                    let body = r#"{"error":"invalid_json"}"#;
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    return;
                }

                if method == "POST" && path == "/v1/stop" {
                    crate::tts::stop_tts();
                    let body = r#"{"status":"stopped"}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    return;
                }

                let body = r#"{"error":"not_found"}"#;
                let response = format!(
                    "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
}
