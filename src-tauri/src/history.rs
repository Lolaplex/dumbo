use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    pub id: String,
    pub created_at: i64,
    pub title: String,
    pub provider_id: String,
    pub model: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatDetail {
    pub chat: ChatSummary,
    pub messages: Vec<ChatMessage>,
}

/// One prompt/answer pair, flattened for the overlay's scroll-through history.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exchange {
    pub id: String,
    pub created_at: i64,
    pub provider_id: String,
    pub model: String,
    pub prompt: String,
    pub answer: String,
}

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Datenordner fehlt: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
    Ok(dir.join("dumbo.db"))
}

fn open(app: &AppHandle) -> Result<Connection, String> {
    Connection::open(db_path(app)?).map_err(|e| format!("Historie nicht öffnen: {e}"))
}

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS chats (
        id TEXT PRIMARY KEY,
        created_at INTEGER NOT NULL,
        title TEXT NOT NULL,
        provider_id TEXT NOT NULL,
        model TEXT NOT NULL,
        kind TEXT DEFAULT 'chat'
    );
    CREATE TABLE IF NOT EXISTS messages (
        id TEXT PRIMARY KEY,
        chat_id TEXT NOT NULL,
        role TEXT NOT NULL,
        content TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        FOREIGN KEY(chat_id) REFERENCES chats(id) ON DELETE CASCADE
    );
";

pub fn init_db(app: &AppHandle) -> Result<(), String> {
    let path = db_path(app)?;
    {
        let conn = Connection::open(&path).map_err(|e| format!("Historie nicht öffnen: {e}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("Historie-Schema fehlgeschlagen: {e}"))?;
        let _ = conn.execute("ALTER TABLE chats ADD COLUMN kind TEXT DEFAULT 'chat'", []);
    }
    for dir in crate::settings::legacy_data_dirs() {
        let src = dir.join("dumbo.db");
        if src.exists() && src != path {
            if let Err(err) = merge_sqlite_history(&path, &src) {
                eprintln!("Historie-Merge Warnung: {err}");
            }
        }
    }
    Ok(())
}

fn attached_has_column(conn: &Connection, schema: &str, table: &str, column: &str) -> bool {
    let sql = format!("PRAGMA {schema}.table_info({table})");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return false;
    };
    let Ok(iter) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return false;
    };
    let names: Vec<String> = iter.flatten().collect();
    names.iter().any(|name| name.eq_ignore_ascii_case(column))
}

/// Insert missing chats/messages from a legacy dumbo.db. Existing ids stay.
fn merge_sqlite_history(dest: &Path, src: &Path) -> Result<(usize, usize), String> {
    if dest == src {
        return Ok((0, 0));
    }
    let conn = Connection::open(dest).map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    let _ = conn.execute("ALTER TABLE chats ADD COLUMN kind TEXT DEFAULT 'chat'", []);
    conn.execute("PRAGMA foreign_keys = OFF", [])
        .map_err(|e| e.to_string())?;
    conn.execute(
        "ATTACH DATABASE ?1 AS legacy",
        rusqlite::params![src.to_string_lossy().as_ref()],
    )
    .map_err(|e| format!("Historie ATTACH: {e}"))?;
    let src_has_kind = attached_has_column(&conn, "legacy", "chats", "kind");
    let chats = if src_has_kind {
        conn.execute(
            "INSERT OR IGNORE INTO chats (id, created_at, title, provider_id, model, kind)
             SELECT id, created_at, title, provider_id, model, COALESCE(NULLIF(kind, ''), 'chat')
             FROM legacy.chats",
            [],
        )
        .map_err(|e| e.to_string())?
    } else {
        conn.execute(
            "INSERT OR IGNORE INTO chats (id, created_at, title, provider_id, model, kind)
             SELECT id, created_at, title, provider_id, model, 'chat'
             FROM legacy.chats",
            [],
        )
        .map_err(|e| e.to_string())?
    };
    let messages = conn
        .execute(
            "INSERT OR IGNORE INTO messages (id, chat_id, role, content, created_at)
             SELECT id, chat_id, role, content, created_at FROM legacy.messages",
            [],
        )
        .map_err(|e| e.to_string())?;
    let _ = conn.execute("DETACH DATABASE legacy", []);
    Ok((chats, messages))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn title_from(prompt: &str) -> String {
    let trimmed = prompt.trim().replace('\n', " ");
    let count = trimmed.chars().count();
    if count <= 72 {
        trimmed
    } else {
        trimmed.chars().take(72).collect::<String>() + "…"
    }
}

/// Append a user+assistant pair to an existing chat, or create one.
pub fn save_turn(
    app: &AppHandle,
    chat_id: Option<&str>,
    provider_id: &str,
    model: &str,
    prompt: &str,
    answer: &str,
) -> Result<String, String> {
    write_turn(&open(app)?, chat_id, provider_id, model, prompt, answer)
}

fn write_turn(
    conn: &Connection,
    chat_id: Option<&str>,
    provider_id: &str,
    model: &str,
    prompt: &str,
    answer: &str,
) -> Result<String, String> {
    let created_at = now();
    let chat_id = match chat_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(existing) => {
            let found = conn
                .query_row(
                    "SELECT 1 FROM chats WHERE id = ?1",
                    rusqlite::params![existing],
                    |_| Ok(()),
                )
                .is_ok();
            if found {
                conn.execute(
                    "UPDATE chats SET created_at = ?1 WHERE id = ?2",
                    rusqlite::params![created_at, existing],
                )
                .map_err(|e| e.to_string())?;
                existing.to_string()
            } else {
                insert_chat(conn, existing, created_at, prompt, provider_id, model)?;
                existing.to_string()
            }
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            insert_chat(conn, &id, created_at, prompt, provider_id, model)?;
            id
        }
    };
    conn.execute(
        "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, 'user', ?3, ?4)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), chat_id, prompt, created_at],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, 'assistant', ?3, ?4)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            chat_id,
            answer,
            created_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(chat_id)
}

fn insert_chat(
    conn: &Connection,
    id: &str,
    created_at: i64,
    prompt: &str,
    provider_id: &str,
    model: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO chats (id, created_at, title, provider_id, model, kind) VALUES (?1, ?2, ?3, ?4, ?5, 'chat')",
        rusqlite::params![id, created_at, title_from(prompt), provider_id, model],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Save a spoken TTS event as a history item (kind = 'tts')
pub fn save_tts_turn(
    app: &AppHandle,
    provider_id: &str,
    voice_or_model: &str,
    text: &str,
) -> Result<String, String> {
    let conn = open(app)?;
    let created_at = now();
    let id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO chats (id, created_at, title, provider_id, model, kind) VALUES (?1, ?2, ?3, ?4, ?5, 'tts')",
        rusqlite::params![id, created_at, title_from(text), provider_id, voice_or_model],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, 'tts', ?3, ?4)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), id, text, created_at],
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn list_chats(app: AppHandle) -> Result<Vec<ChatSummary>, String> {
    let conn = open(&app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, created_at, title, provider_id, model, COALESCE(kind, 'chat') FROM chats ORDER BY created_at DESC LIMIT 80",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ChatSummary {
                id: row.get(0)?,
                created_at: row.get(1)?,
                title: row.get(2)?,
                provider_id: row.get(3)?,
                model: row.get(4)?,
                kind: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Newest first. Chats without both roles are skipped so the overlay never
/// lands on a half exchange. TTS entries are excluded from the overlay.
#[tauri::command]
pub fn list_exchanges(app: AppHandle, limit: Option<u32>) -> Result<Vec<Exchange>, String> {
    read_exchanges(&open(&app)?, limit.unwrap_or(40).clamp(1, 200))
}

fn read_exchanges(conn: &Connection, limit: u32) -> Result<Vec<Exchange>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.created_at, c.provider_id, c.model,
                (SELECT content FROM messages m WHERE m.chat_id = c.id AND m.role = 'user'
                 ORDER BY m.created_at ASC, m.rowid ASC LIMIT 1) AS prompt,
                (SELECT content FROM messages m WHERE m.chat_id = c.id AND m.role = 'assistant'
                 ORDER BY m.created_at ASC, m.rowid ASC LIMIT 1) AS answer
             FROM chats c
             WHERE c.kind = 'chat' OR c.kind IS NULL
             ORDER BY c.created_at DESC, c.rowid DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![limit], |row| {
            Ok(Exchange {
                id: row.get(0)?,
                created_at: row.get(1)?,
                provider_id: row.get(2)?,
                model: row.get(3)?,
                prompt: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                answer: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|item| !item.prompt.trim().is_empty() && !item.answer.trim().is_empty())
        .collect())
}

#[tauri::command]
pub fn get_chat(app: AppHandle, id: String) -> Result<ChatDetail, String> {
    let conn = open(&app)?;
    let chat = conn
        .query_row(
            "SELECT id, created_at, title, provider_id, model, COALESCE(kind, 'chat') FROM chats WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(ChatSummary {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    title: row.get(2)?,
                    provider_id: row.get(3)?,
                    model: row.get(4)?,
                    kind: row.get(5)?,
                })
            },
        )
        .map_err(|_| crate::i18n::t(crate::i18n::app_locale(&app), "chat_not_found").to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, role, content, created_at FROM messages WHERE chat_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let messages = stmt
        .query_map(rusqlite::params![id], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ChatDetail { chat, messages })
}

#[tauri::command]
pub fn delete_chat(app: AppHandle, id: String) -> Result<(), String> {
    let conn = open(&app)?;
    conn.execute("DELETE FROM messages WHERE chat_id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM chats WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), String> {
    let conn = open(&app)?;
    conn.execute("DELETE FROM messages", [])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM chats", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_exchanges, write_turn, SCHEMA};
    use rusqlite::Connection;

    fn seed(conn: &Connection, id: &str, created_at: i64, roles: &[(&str, &str)]) {
        conn.execute(
            "INSERT INTO chats (id, created_at, title, provider_id, model) VALUES (?1, ?2, ?3, 'gemini', 'm')",
            rusqlite::params![id, created_at, format!("title {id}")],
        )
        .unwrap();
        for (index, (role, content)) in roles.iter().enumerate() {
            conn.execute(
                "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![format!("{id}-{index}"), id, role, content, created_at],
            )
            .unwrap();
        }
    }

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn
    }

    #[test]
    fn exchanges_are_newest_first() {
        let conn = db();
        seed(&conn, "old", 100, &[("user", "q1"), ("assistant", "a1")]);
        seed(&conn, "new", 200, &[("user", "q2"), ("assistant", "a2")]);
        let items = read_exchanges(&conn, 40).unwrap();
        let prompts: Vec<&str> = items.iter().map(|item| item.prompt.as_str()).collect();
        assert_eq!(prompts, vec!["q2", "q1"]);
        assert_eq!(items[0].answer, "a2");
    }

    #[test]
    fn half_exchanges_are_skipped() {
        let conn = db();
        seed(&conn, "partial", 100, &[("user", "q1")]);
        seed(&conn, "full", 90, &[("user", "q2"), ("assistant", "a2")]);
        let items = read_exchanges(&conn, 40).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].prompt, "q2");
    }

    #[test]
    fn append_keeps_same_chat() {
        let conn = db();
        let id = write_turn(&conn, None, "gemini", "m", "q1", "a1").unwrap();
        let again = write_turn(&conn, Some(&id), "gemini", "m", "q2", "a2").unwrap();
        assert_eq!(id, again);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE chat_id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 4);
        let chats: i64 = conn
            .query_row("SELECT COUNT(*) FROM chats", [], |row| row.get(0))
            .unwrap();
        assert_eq!(chats, 1);
    }

    #[test]
    fn limit_caps_rows() {
        let conn = db();
        for index in 0..5 {
            seed(
                &conn,
                &format!("c{index}"),
                100 + index,
                &[("user", "q"), ("assistant", "a")],
            );
        }
        assert_eq!(read_exchanges(&conn, 2).unwrap().len(), 2);
    }

    #[test]
    fn merge_adds_legacy_rows_without_clobber() {
        let dir = std::env::temp_dir().join(format!("dumbo-hist-merge-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let dest_path = dir.join("dest.db");
        let src_path = dir.join("src.db");
        {
            let dest = Connection::open(&dest_path).unwrap();
            dest.execute_batch(SCHEMA).unwrap();
            dest.execute(
                "INSERT INTO chats (id, created_at, title, provider_id, model, kind) VALUES ('keep', 1, 'modern', 'gemini', 'm', 'chat')",
                [],
            )
            .unwrap();
            dest.execute(
                "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES ('keep-0', 'keep', 'user', 'new', 1)",
                [],
            )
            .unwrap();
        }
        {
            let src = Connection::open(&src_path).unwrap();
            src.execute_batch(
                "CREATE TABLE chats (
                    id TEXT PRIMARY KEY,
                    created_at INTEGER NOT NULL,
                    title TEXT NOT NULL,
                    provider_id TEXT NOT NULL,
                    model TEXT NOT NULL
                );
                CREATE TABLE messages (
                    id TEXT PRIMARY KEY,
                    chat_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            )
            .unwrap();
            src.execute(
                "INSERT INTO chats (id, created_at, title, provider_id, model) VALUES ('keep', 1, 'legacy-title', 'gemini', 'm')",
                [],
            )
            .unwrap();
            src.execute(
                "INSERT INTO chats (id, created_at, title, provider_id, model) VALUES ('old', 2, 'legacy-only', 'gemini', 'm')",
                [],
            )
            .unwrap();
            src.execute(
                "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES ('keep-0', 'keep', 'user', 'old-text', 1)",
                [],
            )
            .unwrap();
            src.execute(
                "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES ('old-0', 'old', 'user', 'q', 2)",
                [],
            )
            .unwrap();
        }
        let (chats, messages) = super::merge_sqlite_history(&dest_path, &src_path).unwrap();
        assert_eq!(chats, 1);
        assert_eq!(messages, 1);
        let dest = Connection::open(&dest_path).unwrap();
        let title: String = dest
            .query_row("SELECT title FROM chats WHERE id = 'keep'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(title, "modern");
        let n: i64 = dest
            .query_row("SELECT COUNT(*) FROM chats", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
