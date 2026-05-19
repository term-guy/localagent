use reqwest::Client;
use scraper::{Html, Selector};
use serde::Serialize;
use std::time::Duration;
use tauri::State;

use crate::state::AppState;

const FETCH_TIMEOUT_SECS: u64 = 15;
// Tokens reserved for system prompt, conversation history, and response headroom.
const RESERVED_TOKENS: u32 = 1_500;
// Conservative chars-per-token estimate (handles multilingual content).
const CHARS_PER_TOKEN: u32 = 3;
// Hard ceiling so even 128k-context models don't fetch absurdly large pages.
const MAX_TEXT_CHARS_CEILING: usize = 12_000;

#[derive(Debug, Serialize)]
pub struct BrowseResult {
    pub url: String,
    pub title: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[tauri::command]
pub async fn fetch_url(url: String, state: State<'_, AppState>) -> Result<BrowseResult, String> {
    let context_size = {
        let guard = state.model.lock().unwrap();
        guard.as_ref().map(|m| m.context_size).unwrap_or(4096)
    };

    let max_chars = (context_size.saturating_sub(RESERVED_TOKENS) * CHARS_PER_TOKEN)
        .clamp(1_000, MAX_TEXT_CHARS_CEILING as u32) as usize;

    Ok(match do_fetch(&url, max_chars).await {
        Ok((title, text)) => BrowseResult { url, title, text, error: None },
        Err(e) => BrowseResult { url, title: String::new(), text: String::new(), error: Some(e) },
    })
}

async fn do_fetch(url: &str, max_chars: usize) -> Result<(String, String), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent("Mozilla/5.0 (compatible; localagent/1.0; +https://github.com/localagent)")
        .build()
        .map_err(|e| e.to_string())?;

    let response = client.get(url).send().await.map_err(|e| e.to_string())?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body = response.text().await.map_err(|e| e.to_string())?;

    if !content_type.contains("text/html") && !content_type.is_empty() {
        let truncated: String = body.chars().take(max_chars).collect();
        return Ok((String::new(), truncated));
    }

    let (title, text) = extract_text(&body);
    let text: String = text.chars().take(max_chars).collect();
    Ok((title, text))
}

fn extract_text(html: &str) -> (String, String) {
    let doc = Html::parse_document(html);

    let title = Selector::parse("title")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .map(|e| e.text().collect::<String>())
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Select leaf-level content elements; these rarely contain nested block elements,
    // so we avoid duplicating text that appears at multiple levels of the DOM.
    let content_sel = Selector::parse(
        "p, h1, h2, h3, h4, h5, h6, li, td, th, dt, dd, figcaption, blockquote, pre",
    )
    .unwrap();

    let mut seen = std::collections::HashSet::new();
    let mut lines: Vec<String> = Vec::new();

    for elem in doc.select(&content_sel) {
        let text: String = elem
            .text()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.len() < 4 || !seen.insert(text.clone()) {
            continue;
        }
        lines.push(text);
    }

    (title, lines.join("\n"))
}
