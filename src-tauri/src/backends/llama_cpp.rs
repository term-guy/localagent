use std::num::NonZeroU32;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaChatMessage, LlamaModel},
    sampling::LlamaSampler,
    token::LlamaToken,
};

use crate::backend::{Backend, InferenceStats};

// Intentionally leaked: ggml_metal_free asserts all Metal residency sets are cleared,
// but AppKit's exit() path skips stack unwinding so LlamaModel (with GPU weights still
// registered) outlives this static's destructor. Leaking avoids the ordering crash;
// the OS reclaims all memory on process exit regardless.
static LLAMA_BACKEND: OnceLock<&'static LlamaBackend> = OnceLock::new();

fn init_backend() -> &'static LlamaBackend {
    LLAMA_BACKEND.get_or_init(|| {
        Box::leak(Box::new(LlamaBackend::init().expect("llama backend init failed")))
    })
}

pub struct LlamaCppBackend {
    model: LlamaModel,
}

unsafe impl Send for LlamaCppBackend {}
unsafe impl Sync for LlamaCppBackend {}

impl LlamaCppBackend {
    pub fn load(model_path: &str) -> Result<Self, String> {
        let backend = init_backend();
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(backend, model_path, &params)
            .map_err(|e| e.to_string())?;
        Ok(Self { model })
    }
}

// Build the prompt using the model's own embedded Jinja chat template.
// Falls back to ChatML when no template is present.
fn format_prompt(model: &LlamaModel, messages_json: &str) -> Result<String, String> {
    let raw: Vec<serde_json::Value> =
        serde_json::from_str(messages_json).map_err(|e| e.to_string())?;

    // Flatten multimodal content arrays to plain text for the prompt.
    let chat: Vec<LlamaChatMessage> = raw
        .iter()
        .filter_map(|msg| {
            let role = msg["role"].as_str()?.to_string();
            let content = match &msg["content"] {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(parts) => parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join(""),
                _ => return None,
            };
            LlamaChatMessage::new(role, content).ok()
        })
        .collect();

    // Prefer the template baked into the GGUF (correct for Gemma, Llama-3, etc.).
    if let Ok(ref tmpl) = model.chat_template(None) {
        match model.apply_chat_template(tmpl, &chat, true) {
            Ok(result) => return Ok(result),
            Err(_) => {
                // Gemma 4's Jinja template uses syntax not yet supported by this
                // llama-cpp-2 version. Detect by the presence of Gemma turn tokens and
                // fall back to a hardcoded format rather than surfacing an opaque error.
                if tmpl.to_str().unwrap_or("").contains("start_of_turn") {
                    let mut prompt = String::new();
                    for msg in &raw {
                        let role = msg["role"].as_str().unwrap_or("user");
                        let content = match &msg["content"] {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Array(parts) => parts
                                .iter()
                                .filter_map(|p| p["text"].as_str())
                                .collect::<Vec<_>>()
                                .join(""),
                            _ => String::new(),
                        };
                        let turn = if role == "assistant" { "model" } else { role };
                        prompt.push_str(&format!("<start_of_turn>{turn}\n{content}<end_of_turn>\n"));
                    }
                    prompt.push_str("<start_of_turn>model\n");
                    return Ok(prompt);
                }
                // For other families, fall through to ChatML below.
            }
        }
    }

    // Fallback: generic ChatML used by Qwen, Mistral-instruct, and others.
    let mut prompt = String::new();
    for msg in &raw {
        let role = msg["role"].as_str().unwrap_or("user");
        let content = msg["content"].as_str().unwrap_or("");
        prompt.push_str(&format!("<|im_start|>{role}\n{content}<|im_end|>\n"));
    }
    prompt.push_str("<|im_start|>assistant\n");
    Ok(prompt)
}

fn token_to_str(model: &LlamaModel, token: LlamaToken) -> String {
    let bytes = model
        .token_to_piece_bytes(token, 256, true, None)
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

impl Backend for LlamaCppBackend {
    fn complete(
        &self,
        messages_json: &str,
        _pcm_data: Option<&[u8]>,
        on_token: &dyn Fn(&str),
        cancel: Arc<AtomicBool>,
    ) -> Result<InferenceStats, String> {
        let prompt = format_prompt(&self.model, messages_json)?;

        let tokens = self
            .model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| e.to_string())?;

        let n_ctx = 4096u32;

        // Truncate the prompt to fit the context window. Keeping the tail means the
        // most recent turns are preserved; the system message may be lost in extreme
        // cases, but that is better than crashing.
        let tokens = if tokens.len() >= n_ctx as usize {
            tokens[tokens.len() - (n_ctx as usize - 1)..].to_vec()
        } else {
            tokens
        };

        // n_batch must be >= the number of prompt tokens fed to a single llama_decode
        // call. Setting it equal to n_ctx guarantees the entire prompt can be
        // processed in one shot and avoids the GGML_ASSERT(n_tokens_all <= n_batch).
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(n_ctx).unwrap()))
            .with_n_batch(n_ctx);

        let mut ctx = self
            .model
            .new_context(init_backend(), ctx_params)
            .map_err(|e| e.to_string())?;

        let n_prompt = tokens.len();
        let mut batch = LlamaBatch::new(n_prompt.max(1), 1);
        for (i, &token) in tokens.iter().enumerate() {
            batch
                .add(token, i as i32, &[0], i == n_prompt - 1)
                .map_err(|e| e.to_string())?;
        }
        ctx.decode(&mut batch).map_err(|e: llama_cpp_2::DecodeError| e.to_string())?;

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::temp(0.8),
            LlamaSampler::dist(0),
        ]);

        // Stop strings that some models emit as text rather than registered EOG tokens.
        const STOP_STRINGS: &[&str] = &[
            "<|im_end|>", "<end_of_turn>", "<|end|>", "<|eot_id|>", "<|endoftext|>",
        ];
        let max_stop_len = STOP_STRINGS.iter().map(|s| s.len()).max().unwrap_or(0);

        let mut n_cur = n_prompt;
        let decode_start = std::time::Instant::now();
        // Pending buffer withholds the last `max_stop_len` chars so we never emit the
        // start of a stop string before confirming whether the rest follows.
        let mut pending = String::with_capacity(max_stop_len * 2);

        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let new_token = sampler.sample(&ctx, -1);
            sampler.accept(new_token);

            // is_eog_token catches EOS and, when the correct chat template is used,
            // the model's actual EOT token (e.g. <end_of_turn> for Gemma).
            if self.model.is_eog_token(new_token) {
                on_token(&pending);
                pending.clear();
                break;
            }
            if n_cur >= n_ctx as usize {
                break;
            }

            pending.push_str(&token_to_str(&self.model, new_token));

            // Check whether any stop string appears anywhere in pending.
            if let Some(stop) = STOP_STRINGS.iter().find(|&&s| pending.contains(s)) {
                let stop_pos = pending.find(stop).unwrap();
                if stop_pos > 0 {
                    on_token(&pending[..stop_pos]);
                }
                pending.clear();
                break;
            }

            // Emit the safe prefix — everything except the last max_stop_len chars,
            // which might be the start of an upcoming stop string.
            if pending.len() > max_stop_len {
                let mut safe_len = pending.len() - max_stop_len;
                while !pending.is_char_boundary(safe_len) {
                    safe_len -= 1;
                }
                on_token(&pending[..safe_len]);
                pending.drain(..safe_len);
            }

            batch.clear();
            batch
                .add(new_token, n_cur as i32, &[0], true)
                .map_err(|e| e.to_string())?;
            ctx.decode(&mut batch)
                .map_err(|e: llama_cpp_2::DecodeError| e.to_string())?;

            n_cur += 1;
        }

        // Flush whatever is left in the pending buffer (cancel / ctx limit paths).
        if !pending.is_empty() {
            on_token(&pending);
        }

        let tokens_generated = (n_cur - n_prompt) as u32;
        let duration_ms = decode_start.elapsed().as_millis() as u64;
        let tokens_per_second = if duration_ms > 0 {
            tokens_generated as f64 / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        Ok(InferenceStats { tokens_generated, duration_ms, tokens_per_second })
    }

    fn stop(&self) {}

    fn context_size(&self) -> u32 {
        // Cap at our configured n_ctx so callers see the actual usable window.
        (self.model.n_ctx_train() as u32).min(4096)
    }
}
