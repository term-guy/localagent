mod cactus;
mod llama_cpp;

use crate::backend::{Backend, BackendKind};

pub fn load_backend(kind: BackendKind, model_path: &str) -> Result<Box<dyn Backend>, String> {
    match kind {
        BackendKind::Cactus => cactus::CactusBackend::load(model_path)
            .map(|b| Box::new(b) as Box<dyn Backend>),
        BackendKind::LlamaCpp => llama_cpp::LlamaCppBackend::load(model_path)
            .map(|b| Box::new(b) as Box<dyn Backend>),
    }
}
