use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc, Mutex,
};

use crate::backend::Backend;

pub struct LoadedModel {
    pub backend: Arc<dyn Backend>,
    pub model_id: String,
    pub backend_name: String,
    pub context_size: u32,
}

pub struct AppState {
    pub model: Mutex<Option<LoadedModel>>,
    /// Bumped inside `model.lock()` on every unload. `load_model`'s spawn_blocking
    /// compares against the value it read before spawning; a mismatch means an unload
    /// happened while loading was in flight, so the freshly-loaded model is discarded.
    pub model_version: AtomicU64,
    pub inference_running: Mutex<bool>,
    pub inference_cancel: Mutex<Option<Arc<AtomicBool>>>,
    pub shutdown_started: AtomicBool,
    pub downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            model: Mutex::new(None),
            model_version: AtomicU64::new(0),
            inference_running: Mutex::new(false),
            inference_cancel: Mutex::new(None),
            shutdown_started: AtomicBool::new(false),
            downloads: Mutex::new(HashMap::new()),
        }
    }
}
