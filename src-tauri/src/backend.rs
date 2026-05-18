use std::sync::{atomic::AtomicBool, Arc};

pub struct InferenceStats {
    pub tokens_generated: u32,
    pub duration_ms: u64,
    pub tokens_per_second: f64,
}

pub trait Backend: Send + Sync {
    fn complete(
        &self,
        messages_json: &str,
        pcm_data: Option<&[u8]>,
        on_token: &dyn Fn(&str),
        cancel: Arc<AtomicBool>,
    ) -> Result<InferenceStats, String>;

    fn stop(&self);
}

pub enum BackendKind {
    Cactus,
    LlamaCpp,
}

impl BackendKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "llama_cpp" => Self::LlamaCpp,
            _ => Self::Cactus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_kind_llama_cpp() {
        let kind = BackendKind::from_str("llama_cpp");
        assert!(matches!(kind, BackendKind::LlamaCpp));
    }

    #[test]
    fn test_backend_kind_cactus_default() {
        let kind = BackendKind::from_str("cactus");
        assert!(matches!(kind, BackendKind::Cactus));
    }

    #[test]
    fn test_backend_kind_unknown_defaults_to_cactus() {
        let kind = BackendKind::from_str("some_unknown_backend");
        assert!(matches!(kind, BackendKind::Cactus));
    }

    #[test]
    fn test_backend_kind_empty_string_defaults_to_cactus() {
        let kind = BackendKind::from_str("");
        assert!(matches!(kind, BackendKind::Cactus));
    }

    #[test]
    fn test_backend_kind_matching_is_case_sensitive() {
        // "LLAMA_CPP" doesn't match "llama_cpp" — falls through to Cactus default
        let kind = BackendKind::from_str("LLAMA_CPP");
        assert!(matches!(kind, BackendKind::Cactus));
    }
}

