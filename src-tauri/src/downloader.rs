use std::path::Path;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::Serialize;
use tauri::AppHandle;
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Serialize)]
pub struct DownloadProgressEvent {
    pub model_id: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub percentage: f64,
}

#[derive(Clone, Serialize)]
pub struct DownloadCompleteEvent {
    pub model_id: String,
}

#[derive(Clone, Serialize)]
pub struct DownloadErrorEvent {
    pub model_id: String,
    pub error: String,
}

pub async fn download_file(
    app: AppHandle,
    model_id: String,
    url: String,
    dest: &Path,
    expected_size_mb: u64,
    cancel: Arc<AtomicBool>,
) -> Result<u64> {
    let client = reqwest::Client::builder()
        .user_agent("LocalAI/0.1")
        .build()?;

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to connect to download server")?;

    if !response.status().is_success() {
        anyhow::bail!("Server returned {}", response.status());
    }

    let total = response
        .content_length()
        .unwrap_or(expected_size_mb * 1024 * 1024);

    let tmp_dest = dest.with_extension("gguf.part");
    let mut file = tokio::fs::File::create(&tmp_dest)
        .await
        .context("Failed to create destination file")?;

    let mut downloaded: u64 = 0;
    let start = std::time::Instant::now();
    let mut stream = response.bytes_stream();

    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            tokio::fs::remove_file(&tmp_dest).await.ok();
            anyhow::bail!("Download cancelled");
        }
        match stream.next().await {
            Some(Ok(bytes)) => {
                downloaded += bytes.len() as u64;
                file.write_all(&bytes).await.context("Write error")?;

                let elapsed = start.elapsed().as_secs_f64().max(0.001);
                let speed = (downloaded as f64 / elapsed) as u64;
                let pct = if total > 0 { downloaded as f64 / total as f64 * 100.0 } else { 0.0 };

                let _ = app.emit("download-progress", DownloadProgressEvent {
                    model_id: model_id.clone(),
                    bytes_downloaded: downloaded,
                    total_bytes: total,
                    speed_bps: speed,
                    percentage: pct,
                });
            }
            Some(Err(e)) => {
                drop(file);
                tokio::fs::remove_file(&tmp_dest).await.ok();
                return Err(e.into());
            }
            None => break,
        }
    }

    file.flush().await?;
    drop(file);

    // Validate approximate size (allow 5% tolerance)
    let actual = tokio::fs::metadata(&tmp_dest).await?.len();
    if total > 0 && actual < (total * 95 / 100) {
        tokio::fs::remove_file(&tmp_dest).await.ok();
        anyhow::bail!("Download incomplete: got {} of {} bytes", actual, total);
    }

    tokio::fs::rename(&tmp_dest, dest).await.context("Failed to finalize file")?;

    Ok(actual)
}
