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

pub(crate) fn extract_text(html: &str) -> (String, String) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_title_and_paragraph() {
        let html = "<html><head><title>Hello World</title></head><body><p>Some content here.</p></body></html>";
        let (title, text) = extract_text(html);
        assert_eq!(title, "Hello World");
        assert!(text.contains("Some content here."), "text: {text}");
    }

    #[test]
    fn test_extract_text_no_title_returns_empty_string() {
        let html = "<html><body><p>No title here.</p></body></html>";
        let (title, _) = extract_text(html);
        assert_eq!(title, "");
    }

    #[test]
    fn test_extract_text_headings() {
        let html = "<html><body><h1>Main Heading</h1><h2>Sub Heading</h2><p>Paragraph.</p></body></html>";
        let (_, text) = extract_text(html);
        assert!(text.contains("Main Heading"), "text: {text}");
        assert!(text.contains("Sub Heading"), "text: {text}");
        assert!(text.contains("Paragraph."), "text: {text}");
    }

    #[test]
    fn test_extract_text_deduplicates_repeated_content() {
        // The same text appearing in multiple elements should appear only once.
        let html = "<html><body><p>Repeated</p><p>Repeated</p><p>Unique</p></body></html>";
        let (_, text) = extract_text(html);
        let count = text.matches("Repeated").count();
        assert_eq!(count, 1, "duplicate lines should be deduplicated; text: {text}");
        assert!(text.contains("Unique"), "text: {text}");
    }

    #[test]
    fn test_extract_text_skips_short_fragments() {
        // Strings shorter than 4 chars should be filtered out.
        let html = "<html><body><p>OK</p><p>Yes</p><p>Long enough content</p></body></html>";
        let (_, text) = extract_text(html);
        assert!(!text.contains("OK"), "short fragment 'OK' should be skipped; text: {text}");
        assert!(!text.contains("Yes"), "short fragment 'Yes' should be skipped; text: {text}");
        assert!(text.contains("Long enough content"), "text: {text}");
    }

    #[test]
    fn test_extract_text_list_items() {
        let html = "<html><body><ul><li>Item one</li><li>Item two</li></ul></body></html>";
        let (_, text) = extract_text(html);
        assert!(text.contains("Item one"), "text: {text}");
        assert!(text.contains("Item two"), "text: {text}");
    }

    #[test]
    fn test_extract_text_table_cells() {
        let html = "<html><body><table><tr><th>Name</th><th>Value</th></tr><tr><td>Alpha cell</td><td>Beta cell</td></tr></table></body></html>";
        let (_, text) = extract_text(html);
        assert!(text.contains("Name"), "text: {text}");
        assert!(text.contains("Value"), "text: {text}");
        assert!(text.contains("Alpha cell"), "text: {text}");
        assert!(text.contains("Beta cell"), "text: {text}");
    }

    #[test]
    fn test_extract_text_title_whitespace_collapsed() {
        let html = "<html><head><title>  Hello   World  </title></head><body><p>Content</p></body></html>";
        let (title, _) = extract_text(html);
        assert_eq!(title, "Hello World");
    }

    #[test]
    fn test_extract_text_empty_document() {
        let (title, text) = extract_text("");
        assert_eq!(title, "");
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_text_nav_and_script_elements_not_selected() {
        // nav, script, style are not in our content selector — their text should NOT appear.
        let html = r#"<html><body>
            <nav>Nav link</nav>
            <script>var x = 1;</script>
            <style>.foo { color: red; }</style>
            <p>Real content here</p>
        </body></html>"#;
        let (_, text) = extract_text(html);
        assert!(!text.contains("Nav link"), "nav content should be excluded; text: {text}");
        assert!(!text.contains("var x"), "script content should be excluded; text: {text}");
        assert!(!text.contains(".foo"), "style content should be excluded; text: {text}");
        assert!(text.contains("Real content here"), "text: {text}");
    }

    #[test]
    fn test_max_chars_calculation() {
        // Verify the formula: (context_size - RESERVED_TOKENS) * CHARS_PER_TOKEN, clamped.
        // context_size=4096 → (4096-1500)*3 = 7788, within [1000, 12000]
        let context_size: u32 = 4096;
        let max_chars = (context_size.saturating_sub(1_500) * 3)
            .clamp(1_000, 12_000) as usize;
        assert_eq!(max_chars, 7788);

        // Very small context → clamped to minimum 1000
        let small: u32 = 100;
        let max_small = (small.saturating_sub(1_500) * 3).clamp(1_000, 12_000) as usize;
        assert_eq!(max_small, 1_000);

        // Very large context → clamped to ceiling 12000
        let large: u32 = 128_000;
        let max_large = (large.saturating_sub(1_500) * 3).clamp(1_000, 12_000) as usize;
        assert_eq!(max_large, 12_000);
    }
}
