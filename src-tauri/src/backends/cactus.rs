use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::backend::{Backend, InferenceStats};

pub struct CactusBackend {
    ptr: *mut c_void,
}

unsafe impl Send for CactusBackend {}
unsafe impl Sync for CactusBackend {}

impl Drop for CactusBackend {
    fn drop(&mut self) {
        unsafe { cactus_sys::cactus_destroy(self.ptr) };
    }
}

impl CactusBackend {
    pub fn load(model_path: &str) -> Result<Self, String> {
        let path = CString::new(model_path).map_err(|e| e.to_string())?;
        let ptr = unsafe { cactus_sys::cactus_init(path.as_ptr(), std::ptr::null(), false) };
        if ptr.is_null() {
            return Err("cactus_init failed".into());
        }
        Ok(Self { ptr })
    }
}

// Passed through the C callback's user_data pointer.
// Safety: all pointers remain valid for the entire (blocking) cactus_complete call.
struct CallbackCtx {
    on_token: *const dyn Fn(&str),
    cancel: *const AtomicBool,
    token_count: *mut u32,
}

unsafe impl Send for CallbackCtx {}

unsafe extern "C" fn token_callback(
    token: *const c_char,
    _token_id: u32,
    user_data: *mut c_void,
) {
    if token.is_null() || user_data.is_null() {
        return;
    }
    let ctx = &*(user_data as *const CallbackCtx);
    if (*ctx.cancel).load(Ordering::Relaxed) {
        return;
    }
    if let Ok(s) = CStr::from_ptr(token).to_str() {
        *ctx.token_count += 1;
        (&*ctx.on_token)(s);
    }
}

impl Backend for CactusBackend {
    fn complete(
        &self,
        messages_json: &str,
        pcm_data: Option<&[u8]>,
        on_token: &dyn Fn(&str),
        cancel: Arc<AtomicBool>,
    ) -> Result<InferenceStats, String> {
        let messages_cstr = CString::new(messages_json).map_err(|e| e.to_string())?;
        let options_cstr = CString::new(r#"{"max_tokens":4096}"#).unwrap();

        let mut token_count: u32 = 0;
        let ctx = CallbackCtx {
            // Safety: cactus_complete is a blocking FFI call; on_token remains valid throughout.
            on_token: unsafe {
                std::mem::transmute::<&dyn Fn(&str), &'static dyn Fn(&str)>(on_token)
                    as *const dyn Fn(&str)
            },
            cancel: Arc::as_ptr(&cancel),
            token_count: &mut token_count as *mut u32,
        };
        let ctx_ptr = &ctx as *const CallbackCtx as *mut c_void;

        let mut response_buf = vec![0u8; 16 * 1024 * 1024];

        let (pcm_ptr, pcm_len) = match pcm_data {
            Some(pcm) => (pcm.as_ptr(), pcm.len()),
            None => (std::ptr::null(), 0),
        };

        let start = std::time::Instant::now();
        let ret = unsafe {
            cactus_sys::cactus_complete(
                self.ptr,
                messages_cstr.as_ptr(),
                response_buf.as_mut_ptr() as *mut c_char,
                response_buf.len(),
                options_cstr.as_ptr(),
                std::ptr::null(),
                Some(token_callback),
                ctx_ptr,
                pcm_ptr,
                pcm_len,
            )
        };
        let duration_ms = start.elapsed().as_millis() as u64;

        if ret < 0 && !cancel.load(Ordering::Relaxed) {
            // response_buf contains JSON with an "error" field when ret < 0
            let detail = std::str::from_utf8(&response_buf)
                .ok()
                .and_then(|s| {
                    let s = s.trim_end_matches('\0');
                    // extract "error":"<message>" from the JSON
                    let key = "\"error\":\"";
                    s.find(key).and_then(|i| {
                        let rest = &s[i + key.len()..];
                        // find closing quote, respecting backslash escapes
                        let mut chars = rest.char_indices();
                        let mut end = None;
                        let mut escaped = false;
                        for (j, c) in &mut chars {
                            if escaped { escaped = false; continue; }
                            if c == '\\' { escaped = true; continue; }
                            if c == '"' { end = Some(j); break; }
                        }
                        end.map(|j| rest[..j].to_owned())
                    })
                })
                .unwrap_or_else(|| format!("code {ret}"));
            Err(format!("ffi error: {detail}"))
        } else {
            let tps = if duration_ms > 0 {
                token_count as f64 / (duration_ms as f64 / 1000.0)
            } else {
                0.0
            };
            Ok(InferenceStats {
                tokens_generated: token_count,
                duration_ms,
                tokens_per_second: tps,
            })
        }
    }

    fn stop(&self) {
        unsafe { cactus_sys::cactus_stop(self.ptr) };
    }

    fn context_size(&self) -> u32 {
        // cactus-sys doesn't expose n_ctx_train via FFI; use the same default
        // we'd configure for llama.cpp so the browser budget is consistent.
        4096
    }
}
