use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::backend::BackendKind;
use crate::backends::load_backend;
use crate::commands::models::models_dir;
use crate::state::{AppState, LoadedModel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStats {
    pub tokens_generated: u32,
    pub duration_ms: u64,
    pub tokens_per_second: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub image_path: Option<String>,
    pub audio_path: Option<String>,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<MessageStats>,
}

#[derive(Clone, Serialize)]
struct TokenPayload {
    session_id: String,
    token: String,
}

#[derive(Clone, Serialize)]
struct InferenceCompletePayload {
    session_id: String,
    stats: MessageStats,
}

#[derive(Clone, Serialize)]
struct InferenceErrorPayload {
    session_id: String,
    error: String,
}

#[derive(Clone, Serialize)]
struct ModelLoadedPayload {
    model_id: String,
    backend: String,
}

#[derive(Clone, Serialize)]
struct ModelLoadErrorPayload {
    model_id: String,
    backend: String,
    error: String,
}

#[derive(Serialize)]
struct ApiMsg {
    role: String,
    content: serde_json::Value,
}

fn build_messages_json(
    messages: &[ChatMessage],
    image_path: &Option<String>,
    tool_context: &Option<String>,
) -> Result<String, String> {
    let last_id = messages.last().map(|m| m.id.as_str()).unwrap_or("");

    let mut api_msgs: Vec<ApiMsg> = Vec::with_capacity(messages.len() + 1);

    let has_system = messages.first().map(|m| m.role.as_str()) == Some("system");

    if !has_system {
        let base = "You are a helpful AI assistant.";
        let system_content = match tool_context {
            Some(ctx) => format!("{base}\n\n{ctx}"),
            None => base.to_string(),
        };
        api_msgs.push(ApiMsg {
            role: "system".into(),
            content: serde_json::Value::String(system_content),
        });
    }

    for m in messages {
        if m.role == "system" {
            let content = match tool_context {
                Some(ctx) => format!("{}\n\n{ctx}", m.content),
                None => m.content.clone(),
            };
            api_msgs.push(ApiMsg {
                role: "system".into(),
                content: serde_json::Value::String(content),
            });
            continue;
        }
        if m.role == "user" && m.id == last_id {
            if let Some(img) = image_path {
                let content = serde_json::json!([
                    {"type": "text", "text": &m.content},
                    {"type": "image_url", "image_url": {"url": format!("file://{img}")}}
                ]);
                api_msgs.push(ApiMsg {
                    role: "user".into(),
                    content,
                });
                continue;
            }
        }
        api_msgs.push(ApiMsg {
            role: m.role.clone(),
            content: serde_json::Value::String(m.content.clone()),
        });
    }

    serde_json::to_string(&api_msgs).map_err(|e| e.to_string())
}

fn read_wav_pcm(path: &str) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() > 44 && data.starts_with(b"RIFF") {
        Ok(data[44..].to_vec())
    } else {
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_messages_json_no_images() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                role: "user".into(),
                content: "Hello".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:00Z".into(),
                stats: None,
            },
        ];

        let json = build_messages_json(&messages, &None, &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();

        // First message should be the system prompt
        assert_eq!(parsed[0]["role"], "system");
        assert_eq!(parsed[0]["content"], "You are a helpful AI assistant.");

        // Second should be the user message
        assert_eq!(parsed[1]["role"], "user");
        assert_eq!(parsed[1]["content"], "Hello");
    }

    #[test]
    fn test_build_messages_json_with_existing_system_prompt() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                role: "system".into(),
                content: "You are a specialized coding assistant.".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:00Z".into(),
                stats: None,
            },
            ChatMessage {
                id: "2".into(),
                role: "user".into(),
                content: "Write a test".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:01Z".into(),
                stats: None,
            },
        ];

        let json = build_messages_json(&messages, &None, &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["role"], "system");
        assert_eq!(parsed[0]["content"], "You are a specialized coding assistant.");
        assert_eq!(parsed[1]["role"], "user");
    }

    #[test]
    fn test_build_messages_json_multi_turn() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                role: "user".into(),
                content: "Hi".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:00Z".into(),
                stats: None,
            },
            ChatMessage {
                id: "2".into(),
                role: "assistant".into(),
                content: "Hello!".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:01Z".into(),
                stats: None,
            },
            ChatMessage {
                id: "3".into(),
                role: "user".into(),
                content: "How are you?".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:02Z".into(),
                stats: None,
            },
        ];

        let json = build_messages_json(&messages, &None, &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.len(), 4); // system + 3 messages
        assert_eq!(parsed[0]["role"], "system");
        assert_eq!(parsed[1]["role"], "user");
        assert_eq!(parsed[1]["content"], "Hi");
        assert_eq!(parsed[2]["role"], "assistant");
        assert_eq!(parsed[3]["role"], "user");
        assert_eq!(parsed[3]["content"], "How are you?");
    }

    #[test]
    fn test_build_messages_json_with_image_attachment() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                role: "user".into(),
                content: "What's in this image?".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:00Z".into(),
                stats: None,
            },
        ];

        let json = build_messages_json(&messages, &Some("/path/to/img.png".into()), &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();

        // Last user message should have multimodal content
        let user_content = &parsed[1]["content"];
        assert!(user_content.is_array());
        assert_eq!(user_content[0]["type"], "text");
        assert_eq!(user_content[0]["text"], "What's in this image?");
        assert_eq!(user_content[1]["type"], "image_url");
        assert_eq!(
            user_content[1]["image_url"]["url"],
            "file:///path/to/img.png"
        );
    }

    #[test]
    fn test_read_wav_pcm_actually_reads_file() {
        // Create a minimal WAV file in a temp dir
        let dir = std::env::temp_dir().join("local_ai_test_wav");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.wav");

        // 44-byte WAV header + 16 bytes of silence
        let mut wav = vec![
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x24, 0x00, 0x00, 0x00, // file size - 8
            0x57, 0x41, 0x56, 0x45, // WAVE
            0x66, 0x6d, 0x74, 0x20, // fmt_ 0x20
            0x10, 0x00, 0x00, 0x00, // chunk size = 16
            0x01, 0x00,             // format = 1 (PCM)
            0x01, 0x00,             // channels = 1
            0x80, 0x3e, 0x00, 0x00, // sample rate = 16000
            0x00, 0x7d, 0x00, 0x00, // byte rate
            0x02, 0x00,             // block align
            0x10, 0x00,             // bits per sample = 16
            0x64, 0x61, 0x74, 0x61, // data
            0x04, 0x00, 0x00, 0x00, // data chunk size = 4
        ];
        // 4 bytes of PCM data (2 samples, 16-bit mono)
        wav.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]);

        std::fs::write(&path, &wav).unwrap();

        let pcm = read_wav_pcm(path.to_str().unwrap()).unwrap();
        assert_eq!(pcm.len(), 4); // Only PCM data, no header
        assert_eq!(pcm, vec![0x00, 0x00, 0x01, 0x00]);

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_read_wav_pcm_non_wav_file_returns_all_bytes() {
        let dir = std::env::temp_dir().join("local_ai_test_bin");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.bin");
        let data = vec![0x01, 0x02, 0x03];
        std::fs::write(&path, &data).unwrap();

        let result = read_wav_pcm(path.to_str().unwrap()).unwrap();
        assert_eq!(result, data);

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_build_messages_json_empty_messages() {
        let messages = vec![];
        let json = build_messages_json(&messages, &None, &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();

        // Even with empty messages, system prompt is added
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["role"], "system");
    }

    #[test]
    fn test_read_wav_pcm_nonexistent_file_returns_error() {
        let result = read_wav_pcm("/nonexistent/path/that/does/not/exist.wav");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_wav_pcm_riff_at_boundary_returns_all_bytes() {
        // Condition is `data.len() > 44`, so exactly 44 bytes falls through
        // to the else branch and all bytes are returned unchanged.
        let dir = std::env::temp_dir().join("local_ai_test_wav_boundary");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("boundary.wav");

        let mut wav = vec![0u8; 44];
        wav[..4].copy_from_slice(b"RIFF");
        std::fs::write(&path, &wav).unwrap();

        let result = read_wav_pcm(path.to_str().unwrap()).unwrap();
        assert_eq!(result.len(), 44, "exactly 44-byte RIFF should return all bytes");

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_build_messages_json_image_dropped_when_last_message_is_assistant() {
        // last_id is the id of the overall last message (assistant here).
        // No user message has that id, so the image cannot be attached and is silently dropped.
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                role: "user".into(),
                content: "What's in this image?".into(),
                image_path: None,
                audio_path: None,
                timestamp: "".into(),
                stats: None,
            },
            ChatMessage {
                id: "2".into(),
                role: "assistant".into(),
                content: "Let me check.".into(),
                image_path: None,
                audio_path: None,
                timestamp: "".into(),
                stats: None,
            },
        ];

        let json = build_messages_json(&messages, &Some("/img.png".into()), &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();

        // User message content stays a plain string — no image array
        assert!(parsed[1]["content"].is_string());
        assert_eq!(parsed[1]["content"], "What's in this image?");
    }

    #[test]
    fn test_build_messages_json_image_only_on_last_user_message() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                role: "user".into(),
                content: "First message".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:00Z".into(),
                stats: None,
            },
            ChatMessage {
                id: "2".into(),
                role: "assistant".into(),
                content: "First response".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:01Z".into(),
                stats: None,
            },
            ChatMessage {
                id: "3".into(),
                role: "user".into(),
                content: "Second message".into(),
                image_path: None,
                audio_path: None,
                timestamp: "2025-01-01T00:00:02Z".into(),
                stats: None,
            },
        ];

        // Image is attached — only the last message (which happens to be
        // the last user message) should get the multimodal content.
        let json = build_messages_json(&messages, &Some("/img.png".into()), &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.len(), 4); // system + 3 messages

        // First user message should NOT have image
        assert!(parsed[1]["content"].is_string());
        assert_eq!(parsed[1]["content"], "First message");

        // Last user message should have image
        let last = &parsed[3];
        assert!(last["content"].is_array());
        assert_eq!(last["content"][0]["type"], "text");
        assert_eq!(last["content"][0]["text"], "Second message");
        assert_eq!(last["content"][1]["type"], "image_url");
    }
}


#[tauri::command]
pub async fn load_model(
    model_id: String,
    backend: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if *state.inference_running.lock().unwrap() {
        return Err("Cannot change model during active inference".into());
    }

    {
        let guard = state.model.lock().unwrap();
        if guard
            .as_ref()
            .map(|l| {
                l.model_id == model_id
                    && backend.as_ref().map_or(true, |b| &l.backend_name == b)
            })
            .unwrap_or(false)
        {
            // Already loaded — notify frontend so it can clear modelLoading
            let backend_name = guard.as_ref().unwrap().backend_name.clone();
            let _ = app.emit("model-loaded", ModelLoadedPayload { model_id, backend: backend_name });
            return Ok(());
        }
    }

    let reg_path = app
        .path()
        .app_local_data_dir()
        .unwrap()
        .join("models.json");
    let reg: serde_json::Value = std::fs::read_to_string(&reg_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({"installed": []}));

    let entry = reg["installed"]
        .as_array()
        .and_then(|arr| arr.iter().find(|m| {
            m["id"].as_str() == Some(&model_id)
                && backend.as_ref().map_or(true, |b| m["backend"].as_str() == Some(b))
        }))
        .ok_or_else(|| format!("Model {model_id} not found in registry"))?;

    let filename = entry["filename"]
        .as_str()
        .ok_or_else(|| format!("Model {model_id} has no filename"))?
        .to_string();

    let backend_str = entry["backend"].as_str().unwrap_or("llama_cpp").to_string();
    let backend_kind = BackendKind::from_str(&backend_str);

    let model_path = models_dir(&app).join(&filename);
    if !model_path.exists() {
        return Err(format!("Model file not found: {}", model_path.display()));
    }
    let model_path_str = model_path
        .to_str()
        .ok_or("Invalid path encoding")?
        .to_string();

    // Spawn the heavy load on a blocking thread so the UI stays responsive.
    // The frontend clears modelLoading when it receives model-loaded/model-load-error.
    let app_bg = app.clone();
    let model_id_bg = model_id.clone();
    let backend_str_bg = backend_str.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state_ref = app_bg.state::<AppState>();
        match load_backend(backend_kind, &model_path_str) {
            Ok(loaded) => {
                let context_size = loaded.context_size();
                *state_ref.model.lock().unwrap() = Some(LoadedModel {
                    backend: Arc::from(loaded),
                    model_id: model_id_bg.clone(),
                    backend_name: backend_str_bg.clone(),
                    context_size,
                });
                let _ = app_bg.emit("model-loaded", ModelLoadedPayload {
                    model_id: model_id_bg,
                    backend: backend_str_bg,
                });
            }
            Err(e) => {
                let _ = app_bg.emit("model-load-error", ModelLoadErrorPayload {
                    model_id: model_id_bg,
                    backend: backend_str_bg,
                    error: e,
                });
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn unload_model(state: State<'_, AppState>) -> Result<(), String> {
    if *state.inference_running.lock().unwrap() {
        return Err("Cannot unload model during active inference".into());
    }
    state.model.lock().unwrap().take();
    Ok(())
}

#[tauri::command]
pub async fn send_message(
    session_id: String,
    messages: Vec<ChatMessage>,
    image_path: Option<String>,
    audio_path: Option<String>,
    tool_context: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let backend_arc = {
        let guard = state.model.lock().unwrap();
        guard
            .as_ref()
            .map(|l| Arc::clone(&l.backend))
            .ok_or("No model is loaded")?
    };

    {
        let mut running = state.inference_running.lock().unwrap();
        if *running {
            return Err("Inference already in progress".into());
        }
        *running = true;
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    *state.inference_cancel.lock().unwrap() = Some(Arc::clone(&cancel_flag));

    let messages_json = build_messages_json(&messages, &image_path, &tool_context)?;
    let pcm_data: Option<Vec<u8>> = if let Some(ref path) = audio_path {
        Some(read_wav_pcm(path)?)
    } else {
        None
    };

    let app_bg = app.clone();
    let sid = session_id.clone();
    let cancel_bg = Arc::clone(&cancel_flag);

    tauri::async_runtime::spawn_blocking(move || {
        let state_ref = app_bg.state::<AppState>();

        let result = backend_arc.complete(
            &messages_json,
            pcm_data.as_deref(),
            &|token| {
                if !cancel_bg.load(Ordering::Relaxed) {
                    let _ = app_bg.emit(
                        "token",
                        TokenPayload {
                            session_id: sid.clone(),
                            token: token.to_string(),
                        },
                    );
                }
            },
            Arc::clone(&cancel_bg),
        );

        // Drop before signalling done. If we drop after, a close handler that
        // races on inference_running==false sees Arc count 2 instead of 1,
        // so model.lock()=None only decrements to 1 (not 0) and the LlamaModel
        // stays alive past ggml_metal_device_free → GGML_ASSERT([rsets->data count]==0).
        drop(backend_arc);

        *state_ref.inference_running.lock().unwrap() = false;

        match result {
            Err(e) if !cancel_bg.load(Ordering::Relaxed) => {
                let _ = app_bg.emit(
                    "inference-error",
                    InferenceErrorPayload {
                        session_id: sid,
                        error: e,
                    },
                );
            }
            Ok(stats) => {
                let _ = app_bg.emit(
                    "inference-complete",
                    InferenceCompletePayload {
                        session_id: sid,
                        stats: MessageStats {
                            tokens_generated: stats.tokens_generated,
                            duration_ms: stats.duration_ms,
                            tokens_per_second: stats.tokens_per_second,
                        },
                    },
                );
            }
            _ => {
                let _ = app_bg.emit(
                    "inference-complete",
                    InferenceCompletePayload {
                        session_id: sid,
                        stats: MessageStats {
                            tokens_generated: 0,
                            duration_ms: 0,
                            tokens_per_second: 0.0,
                        },
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_inference(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(flag) = &*state.inference_cancel.lock().unwrap() {
        flag.store(true, Ordering::Relaxed);
    }
    let guard = state.model.lock().unwrap();
    if let Some(loaded) = &*guard {
        loaded.backend.stop();
    }
    Ok(())
}
