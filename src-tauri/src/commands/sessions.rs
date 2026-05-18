use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::inference::ChatMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub model_id: String,
    pub message_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionFile {
    meta: SessionMeta,
    messages: Vec<ChatMessage>,
}

fn sessions_dir(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_local_data_dir().unwrap().join("sessions");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn session_path(app: &AppHandle, session_id: &str) -> PathBuf {
    sessions_dir(app).join(format!("{session_id}.json"))
}

#[tauri::command]
pub fn list_sessions(app: AppHandle) -> Vec<SessionMeta> {
    let dir = sessions_dir(&app);
    let mut sessions: Vec<SessionMeta> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
                .filter_map(|e| {
                    std::fs::read_to_string(e.path())
                        .ok()
                        .and_then(|s| serde_json::from_str::<SessionFile>(&s).ok())
                        .map(|f| f.meta)
                })
                .collect()
        })
        .unwrap_or_default();

    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    sessions
}

#[tauri::command]
pub fn get_session(session_id: String, app: AppHandle) -> Vec<ChatMessage> {
    let path = session_path(&app, &session_id);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<SessionFile>(&s).ok())
        .map(|f| f.messages)
        .unwrap_or_default()
}

#[tauri::command]
pub fn save_session(
    session_id: String,
    messages: Vec<ChatMessage>,
    model_id: String,
    app: AppHandle,
) -> Result<(), String> {
    let path = session_path(&app, &session_id);

    // Derive title from first user message
    let title = messages
        .iter()
        .find(|m| m.role == "user")
        .map(|m| {
            let truncated = m.content.chars().take(40).collect::<String>();
            if m.content.len() > 40 { format!("{truncated}…") } else { truncated }
        })
        .unwrap_or_else(|| "New Chat".to_string());

    let created_at = messages
        .first()
        .map(|m| m.timestamp.clone())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let file = SessionFile {
        meta: SessionMeta {
            id: session_id,
            title,
            created_at,
            model_id,
            message_count: messages.len(),
        },
        messages,
    };

    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_session(session_id: String, app: AppHandle) -> Result<(), String> {
    let path = session_path(&app, &session_id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
