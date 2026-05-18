use std::path::PathBuf;
use std::io::{BufReader, Read, Write};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::catalog::{self, ModelInfo};
use crate::downloader::{self, DownloadCompleteEvent, DownloadErrorEvent};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfFile {
    pub filename: String,
    pub size_bytes: u64,
    pub download_url: String,
    pub quant_name: String,
}

#[derive(Debug, Deserialize)]
struct HfLfs {
    size: u64,
}

#[derive(Debug, Deserialize)]
struct HfTreeEntry {
    path: String,
    size: Option<u64>,
    lfs: Option<HfLfs>,
}

fn extract_quant_name(filename: &str) -> String {
    let stem = filename.trim_end_matches(".gguf");
    let parts: Vec<&str> = stem.split('-').collect();
    let quant_start = parts.iter().position(|p| {
        p.starts_with('Q') || p.starts_with("IQ") || p.starts_with("BF") || p.starts_with("UD")
    });
    match quant_start {
        Some(i) => parts[i..].join("-"),
        None => stem.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_quant_name_q4_k_m() {
        assert_eq!(extract_quant_name("gemma-3-1b-it-Q4_K_M.gguf"), "Q4_K_M");
    }

    #[test]
    fn test_extract_quant_name_q8_0() {
        assert_eq!(extract_quant_name("model-Q8_0.gguf"), "Q8_0");
    }

    #[test]
    fn test_extract_quant_name_iq3_xxs() {
        assert_eq!(extract_quant_name("model-IQ3_XXS.gguf"), "IQ3_XXS");
    }

    #[test]
    fn test_extract_quant_name_bf16() {
        assert_eq!(extract_quant_name("model-BF16.gguf"), "BF16");
    }

    #[test]
    fn test_extract_quant_name_no_quant() {
        // If no quant pattern found, returns the full stem
        let result = extract_quant_name("bonsai.gguf");
        assert_eq!(result, "bonsai");

        let result = extract_quant_name("model-name.gguf");
        assert_eq!(result, "model-name");
    }

    #[test]
    fn test_extract_quant_name_ud_iq1_s1() {
        assert_eq!(extract_quant_name("model-UD-IQ1_S1.gguf"), "UD-IQ1_S1");
    }

    #[test]
    fn test_extract_quant_name_multiple_dashes_in_name() {
        assert_eq!(
            extract_quant_name("llama-3.2-3b-instruct-Q4_K_M.gguf"),
            "Q4_K_M"
        );
    }

    #[test]
    fn test_extract_quant_name_f16_not_recognized() {
        // F16 doesn't start with Q/IQ/BF/UD, so the full stem is returned
        let result = extract_quant_name("model-F16.gguf");
        assert_eq!(result, "model-F16");
    }

    #[test]
    fn test_extract_quant_name_with_version_numbers_in_name() {
        // Version numbers with dots should not confuse the parser
        assert_eq!(extract_quant_name("mistral-7b-v0.3-Q4_K_M.gguf"), "Q4_K_M");
    }

    #[test]
    fn test_extract_quant_name_lowercase_not_recognized() {
        // Matching is case-sensitive; HuggingFace always uploads uppercase quant names
        let result = extract_quant_name("model-q4_k_m.gguf");
        assert_eq!(result, "model-q4_k_m");
    }
}


#[tauri::command]
pub async fn fetch_hf_quants(repo: String) -> Result<Vec<HfFile>, String> {
    let api_url = format!("https://huggingface.co/api/models/{}/tree/main", repo);
    let client = reqwest::Client::builder()
        .user_agent("LocalAI/0.1")
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach HuggingFace API: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HuggingFace API returned {}", response.status()));
    }

    let entries: Vec<HfTreeEntry> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse HuggingFace response: {}", e))?;

    let mut files: Vec<HfFile> = entries
        .into_iter()
        .filter(|e| {
            if !e.path.ends_with(".gguf") || e.path.contains('/') { return false; }
            let lower = e.path.to_lowercase();
            // mmproj = vision projector weights, not a standalone model
            // ternary/TQ = ggml types 40/42, unsupported by current llama.cpp build
            !lower.contains("mmproj")
                && !lower.contains("ternary")
                && !lower.contains("tq1")
                && !lower.contains("tq2")
        })
        .map(|e| {
            let size_bytes = e.lfs.map(|l| l.size).or(e.size).unwrap_or(0);
            let filename = e.path;
            let quant_name = extract_quant_name(&filename);
            let download_url = format!(
                "https://huggingface.co/{}/resolve/main/{}",
                repo, filename
            );
            HfFile { filename, size_bytes, download_url, quant_name }
        })
        .collect();

    files.sort_by_key(|f| f.size_bytes);
    Ok(files)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModel {
    #[serde(flatten)]
    pub info: ModelInfo,
    pub file_size_bytes: u64,
    pub downloaded_at: String,
    pub backend: String,
}

#[derive(Serialize, Deserialize, Default)]
struct ModelRegistry {
    installed: Vec<InstalledModel>,
}

fn registry_path(app: &AppHandle) -> PathBuf {
    app.path().app_local_data_dir().unwrap().join("models.json")
}

pub fn models_dir(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_local_data_dir().unwrap().join("models");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn load_registry(app: &AppHandle) -> ModelRegistry {
    let path = registry_path(app);
    if !path.exists() {
        return ModelRegistry::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[derive(Clone, serde::Serialize)]
struct ExtractionProgressEvent {
    model_id: String,
    bytes_downloaded: u64,
    total_bytes: u64,
    speed_bps: u64,
    percentage: f64,
    phase: String,
}

fn extract_zip(zip_path: &PathBuf, dest_dir: &PathBuf, app: &AppHandle, model_id: &str, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(|e| e.to_string())?;

    // First pass: sum uncompressed sizes so we can report meaningful percentage
    let mut total_bytes: u64 = 0;
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            if !entry.is_dir() {
                total_bytes += entry.size();
            }
        }
    }

    let mut bytes_extracted: u64 = 0;
    let mut last_reported: u64 = 0;
    const REPORT_INTERVAL: u64 = 1024 * 1024; // emit every 1 MB

    for i in 0..archive.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Download cancelled".to_string());
        }
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        // Strip any leading directory component so files land directly in dest_dir
        let name = entry.name().to_string();
        let relative = name
            .split('/')
            .filter(|p| !p.is_empty() && *p != "..")
            .collect::<Vec<_>>()
            .join("/");
        if relative.is_empty() {
            continue;
        }
        let out_path = dest_dir.join(&relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            let mut buf = [0u8; 256 * 1024];
            loop {
                let n = entry.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                bytes_extracted += n as u64;
                if bytes_extracted - last_reported >= REPORT_INTERVAL {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Download cancelled".to_string());
                    }
                    last_reported = bytes_extracted;
                    let pct = if total_bytes > 0 {
                        bytes_extracted as f64 / total_bytes as f64 * 100.0
                    } else { 0.0 };
                    let _ = app.emit("download-progress", ExtractionProgressEvent {
                        model_id: model_id.to_string(),
                        bytes_downloaded: bytes_extracted,
                        total_bytes,
                        speed_bps: 0,
                        percentage: pct,
                        phase: "extracting".to_string(),
                    });
                }
            }
        }
    }

    // Emit 100% when done
    let _ = app.emit("download-progress", ExtractionProgressEvent {
        model_id: model_id.to_string(),
        bytes_downloaded: total_bytes,
        total_bytes,
        speed_bps: 0,
        percentage: 100.0,
        phase: "extracting".to_string(),
    });

    Ok(())
}

fn save_registry(app: &AppHandle, reg: &ModelRegistry) {
    let path = registry_path(app);
    if let Ok(json) = serde_json::to_string_pretty(reg) {
        std::fs::write(path, json).ok();
    }
}

#[tauri::command]
pub fn list_catalog() -> Vec<ModelInfo> {
    catalog::get_catalog()
}

#[tauri::command]
pub fn list_installed(app: AppHandle) -> Vec<InstalledModel> {
    let dir = models_dir(&app);
    load_registry(&app)
        .installed
        .into_iter()
        .filter(|m| dir.join(&m.info.filename).exists())
        .collect()
}

#[tauri::command]
pub async fn download_model(
    model_id: String,
    backend: Option<String>,
    filename: Option<String>,
    url: Option<String>,
    size_bytes: Option<u64>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let model = catalog::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let selected_backend = backend.unwrap_or_else(|| model.default_backend.clone());

    let resolved_url = if let Some(u) = url {
        u
    } else {
        match selected_backend.as_str() {
            "llama_cpp" => model.llama_cpp_url.as_ref()
                .ok_or_else(|| format!("No llama.cpp URL configured for {model_id}"))?,
            "cactus" => model.cactus_url.as_ref()
                .ok_or_else(|| format!("No cactus URL configured for {model_id}"))?,
            other => return Err(format!("Unknown backend: {other}")),
        }.clone()
    };

    let resolved_filename = filename.unwrap_or_else(|| {
        resolved_url
            .split('/')
            .last()
            .unwrap_or(&model.filename)
            .to_string()
    });

    let dir = models_dir(&app);
    let dest = dir.join(&resolved_filename);

    if dest.exists() {
        let reg = load_registry(&app);
        let already_registered = reg.installed.iter()
            .any(|m| m.info.id == model_id && m.backend == selected_backend);
        if already_registered {
            return Err(format!("Model {} is already downloaded", model_id));
        }
        // File exists but not in registry under this id/backend — adopt it
        let file_size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        let mut info = model;
        info.filename = resolved_filename;
        let mut reg = load_registry(&app);
        reg.installed.retain(|m| m.info.id != model_id || m.backend != selected_backend);
        reg.installed.push(InstalledModel {
            info,
            file_size_bytes: file_size,
            downloaded_at: Utc::now().to_rfc3339(),
            backend: selected_backend,
        });
        save_registry(&app, &reg);
        let _ = app.emit("download-complete", downloader::DownloadCompleteEvent { model_id });
        return Ok(());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut downloads = state.downloads.lock().unwrap();
        downloads.insert(model_id.clone(), Arc::clone(&cancel));
    }

    let mid = model_id.clone();
    let app2 = app.clone();
    let size_mb = size_bytes
        .map(|b| b / (1024 * 1024))
        .or_else(|| match selected_backend.as_str() {
            "cactus" => model.cactus_size_mb,
            _ => model.llama_cpp_size_mb,
        })
        .unwrap_or(0);
    let url = resolved_url;
    let filename = resolved_filename;

    tauri::async_runtime::spawn(async move {
        match downloader::download_file(app2.clone(), mid.clone(), url, &dest, size_mb, Arc::clone(&cancel)).await {
            Ok(file_size) => {
                let mut info = model;
                info.filename = filename.clone();

                // Extract zip archives and store the directory name instead
                if dest.extension().map_or(false, |e| e == "zip") {
                    let stem = dest.file_stem().unwrap().to_string_lossy().to_string();
                    let extract_dir = dir.join(&stem);
                    if let Err(e) = extract_zip(&dest, &extract_dir, &app2, &mid, &cancel) {
                        let cancelled = e.contains("cancelled");
                        std::fs::remove_file(&dest).ok();
                        std::fs::remove_dir_all(&extract_dir).ok();
                        let _ = app2.emit("download-error", DownloadErrorEvent {
                            model_id: mid,
                            error: if cancelled { String::new() } else { format!("Extraction failed: {e}") },
                        });
                        return;
                    }
                    std::fs::remove_file(&dest).ok();
                    info.filename = stem;
                }

                let mut reg = load_registry(&app2);
                // Only replace the entry for this specific backend; leave other backends intact.
                reg.installed.retain(|m| m.info.id != mid || m.backend != selected_backend);
                reg.installed.push(InstalledModel {
                    info,
                    file_size_bytes: file_size,
                    downloaded_at: Utc::now().to_rfc3339(),
                    backend: selected_backend,
                });
                save_registry(&app2, &reg);

                let _ = app2.emit("download-complete", DownloadCompleteEvent { model_id: mid });
            }
            Err(e) => {
                let err = e.to_string();
                let _ = app2.emit("download-error", DownloadErrorEvent {
                    model_id: mid,
                    error: if err.contains("cancelled") { String::new() } else { err },
                });
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_download(model_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut downloads = state.downloads.lock().unwrap();
    if let Some(flag) = downloads.remove(&model_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn remove_model(
    model_id: String,
    backend: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let is_active = state
        .model
        .lock()
        .unwrap()
        .as_ref()
        .map(|l| l.model_id == model_id && l.backend_name == backend)
        .unwrap_or(false);

    if is_active {
        // Cancel any in-flight inference so the backend can be dropped cleanly.
        if *state.inference_running.lock().unwrap() {
            if let Some(flag) = &*state.inference_cancel.lock().unwrap() {
                flag.store(true, Ordering::Relaxed);
            }
            let guard = state.model.lock().unwrap();
            if let Some(loaded) = &*guard {
                loaded.backend.stop();
            }
        }
        *state.model.lock().unwrap() = None;
    }

    let mut reg = load_registry(&app);
    let entry = reg
        .installed
        .iter()
        .find(|m| m.info.id == model_id && m.backend == backend)
        .ok_or_else(|| format!("Model {model_id} ({backend}) is not installed"))?
        .clone();

    let file = models_dir(&app).join(&entry.info.filename);
    if file.is_dir() {
        std::fs::remove_dir_all(&file).map_err(|e| e.to_string())?;
    } else if file.exists() {
        std::fs::remove_file(&file).map_err(|e| e.to_string())?;
    }

    reg.installed.retain(|m| m.info.id != model_id || m.backend != backend);
    save_registry(&app, &reg);

    Ok(())
}

#[tauri::command]
pub fn get_model_file_size(model_id: String, app: AppHandle) -> Result<u64, String> {
    let reg = load_registry(&app);
    let entry = reg
        .installed
        .iter()
        .find(|m| m.info.id == model_id)
        .ok_or_else(|| format!("Model {model_id} not installed"))?;
    Ok(entry.file_size_bytes)
}

#[tauri::command]
pub async fn download_hf_model(
    repo: String,
    filename: String,
    url: String,
    size_bytes: u64,
    backend: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let model_id = repo.replace('/', "--");
    let display_name = repo.split('/').last().unwrap_or(&repo).to_string();
    let provider = repo.split('/').next().unwrap_or("Unknown").to_string();

    let model = ModelInfo {
        id: model_id.clone(),
        display_name,
        provider,
        repo: repo.clone(),
        capabilities: vec!["chat".to_string()],
        filename: filename.clone(),
        description: format!("Custom model from {}", repo),
        default_backend: backend.clone(),
        llama_cpp_url: if backend == "llama_cpp" { Some(url.clone()) } else { None },
        llama_cpp_size_mb: None,
        llama_cpp_quant: None,
        cactus_url: if backend == "cactus" { Some(url.clone()) } else { None },
        cactus_size_mb: None,
    };

    let dir = models_dir(&app);
    let dest = dir.join(&filename);

    if dest.exists() {
        return Err(format!("File {} already exists", filename));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut downloads = state.downloads.lock().unwrap();
        downloads.insert(model_id.clone(), Arc::clone(&cancel));
    }

    let mid = model_id.clone();
    let app2 = app.clone();
    let size_mb = size_bytes / (1024 * 1024);
    let selected_backend = backend.clone();

    tauri::async_runtime::spawn(async move {
        match downloader::download_file(app2.clone(), mid.clone(), url, &dest, size_mb, Arc::clone(&cancel)).await {
            Ok(file_size) => {
                let mut info = model;
                info.filename = filename.clone();

                if dest.extension().map_or(false, |e| e == "zip") {
                    let stem = dest.file_stem().unwrap().to_string_lossy().to_string();
                    let extract_dir = dir.join(&stem);
                    if let Err(e) = extract_zip(&dest, &extract_dir, &app2, &mid, &cancel) {
                        let cancelled = e.contains("cancelled");
                        std::fs::remove_file(&dest).ok();
                        std::fs::remove_dir_all(&extract_dir).ok();
                        let _ = app2.emit("download-error", DownloadErrorEvent {
                            model_id: mid,
                            error: if cancelled { String::new() } else { format!("Extraction failed: {e}") },
                        });
                        return;
                    }
                    std::fs::remove_file(&dest).ok();
                    info.filename = stem;
                }

                let mut reg = load_registry(&app2);
                reg.installed.retain(|m| m.info.id != mid || m.backend != selected_backend);
                reg.installed.push(InstalledModel {
                    info,
                    file_size_bytes: file_size,
                    downloaded_at: Utc::now().to_rfc3339(),
                    backend: selected_backend,
                });
                save_registry(&app2, &reg);
                let _ = app2.emit("download-complete", DownloadCompleteEvent { model_id: mid });
            }
            Err(e) => {
                let err = e.to_string();
                let _ = app2.emit("download-error", DownloadErrorEvent {
                    model_id: mid,
                    error: if err.contains("cancelled") { String::new() } else { err },
                });
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn get_models_dir(app: AppHandle) -> String {
    models_dir(&app).to_string_lossy().to_string()
}

#[tauri::command]
pub fn reveal_models_dir(app: AppHandle) -> Result<(), String> {
    let dir = models_dir(&app);
    open::that(dir).map_err(|e| e.to_string())
}
