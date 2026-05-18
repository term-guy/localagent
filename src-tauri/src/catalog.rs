use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub repo: String,
    pub capabilities: Vec<String>,
    pub filename: String,
    pub description: String,
    pub default_backend: String,
    // llama.cpp
    pub llama_cpp_url: Option<String>,
    pub llama_cpp_size_mb: Option<u64>,
    pub llama_cpp_quant: Option<String>,
    // Cactus
    pub cactus_url: Option<String>,
    pub cactus_size_mb: Option<u64>,
}

pub fn get_catalog() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gemma-3-1b-it".into(),
            display_name: "Gemma-3-1B".into(),
            provider: "Google".into(),
            repo: "unsloth/gemma-3-1b-it-GGUF".into(),
            capabilities: vec!["chat".into()],
            filename: "gemma-3-1b-it-Q4_K_M.gguf".into(),
            description: "Google's 1B instruction-tuned model from the Gemma 3 family. Extremely lightweight — runs on almost any hardware with minimal RAM.".into(),
            default_backend: "llama_cpp".into(),
            llama_cpp_url: Some("https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf".into()),
            llama_cpp_size_mb: Some(806),
            llama_cpp_quant: Some("Q4_K_M".into()),
            cactus_url: Some("https://huggingface.co/Cactus-Compute/gemma-3-1b-it/resolve/main/weights/gemma-3-1b-it-int4.zip".into()),
            cactus_size_mb: Some(653),
        },
        ModelInfo {
            id: "gemma-4-e2b-it".into(),
            display_name: "Gemma-4-E2B".into(),
            provider: "Google".into(),
            repo: "unsloth/gemma-4-e2b-it-GGUF".into(),
            capabilities: vec!["chat".into()],
            filename: "gemma-4-e2b-it-Q4_K_M.gguf".into(),
            description: "Google's Gemma 4 Edge 2B instruction-tuned model. Improved reasoning over Gemma 3 in a compact footprint suited for edge devices.".into(),
            default_backend: "llama_cpp".into(),
            llama_cpp_url: Some("https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-Q4_K_M.gguf".into()),
            llama_cpp_size_mb: Some(3111),
            llama_cpp_quant: Some("Q4_K_M".into()),
            cactus_url: Some("https://huggingface.co/Cactus-Compute/gemma-4-E2B-it/resolve/main/weights/gemma-4-e2b-it-int4.zip".into()),
            cactus_size_mb: Some(4680),
        },
        ModelInfo {
            id: "Bonsai-8B".into(),
            display_name: "Bonsai-8B".into(),
            provider: "Prisma ML".into(),
            repo: "prism-ml/Bonsai-8B-gguf".into(),
            capabilities: vec!["chat".into()],
            filename: "bonsai.gguf".into(),
            description: "Prisma ML's 8B chat model optimized for efficiency. Stronger reasoning than smaller models while remaining practical on mid-range hardware.".into(),
            default_backend: "llama_cpp".into(),
            llama_cpp_url: Some("https://huggingface.co/prism-ml/Bonsai-8B-gguf/resolve/main/Bonsai-8B.gguf".into()),
            llama_cpp_size_mb: Some(1160),
            llama_cpp_quant: None,
            cactus_url: None,
            cactus_size_mb: None,
        },
    ]
}

pub fn find_model(id: &str) -> Option<ModelInfo> {
    get_catalog().into_iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_catalog_contains_expected_models() {
        let catalog = get_catalog();
        // Should have at least 2 models
        assert!(catalog.len() >= 2);

        // Gemma-3-1B should be present
        let gemma = catalog.iter().find(|m| m.id == "gemma-3-1b-it");
        assert!(gemma.is_some());
        let gemma = gemma.unwrap();
        assert_eq!(gemma.display_name, "Gemma-3-1B");
        assert_eq!(gemma.provider, "Google");
        assert!(gemma.capabilities.contains(&"chat".to_string()));
        assert_eq!(gemma.default_backend, "llama_cpp");

        // Bonsai-8B should be present
        let bonsai = catalog.iter().find(|m| m.id == "Bonsai-8B");
        assert!(bonsai.is_some());
        let bonsai = bonsai.unwrap();
        assert_eq!(bonsai.display_name, "Bonsai-8B");
        assert_eq!(bonsai.provider, "Prisma ML");
        assert!(bonsai.llama_cpp_url.is_some());
        assert!(bonsai.cactus_url.is_none());
    }

    #[test]
    fn test_find_model_finds_existing() {
        let model = find_model("gemma-3-1b-it");
        assert!(model.is_some());
        assert_eq!(model.unwrap().id, "gemma-3-1b-it");
    }

    #[test]
    fn test_find_model_returns_none_for_unknown() {
        let model = find_model("nonexistent-model");
        assert!(model.is_none());
    }

    #[test]
    fn test_gemma_has_both_backends() {
        let gemma = find_model("gemma-3-1b-it").unwrap();
        assert!(gemma.llama_cpp_url.is_some());
        assert!(gemma.llama_cpp_size_mb.is_some());
        assert!(gemma.cactus_url.is_some());
        assert!(gemma.cactus_size_mb.is_some());
    }

    #[test]
    fn test_all_models_have_non_empty_fields() {
        for model in get_catalog() {
            assert!(!model.id.is_empty(), "Model id is empty");
            assert!(!model.display_name.is_empty(), "{} has empty display_name", model.id);
            assert!(!model.provider.is_empty(), "{} has empty provider", model.id);
            assert!(!model.repo.is_empty(), "{} has empty repo", model.id);
            assert!(!model.filename.is_empty(), "{} has empty filename", model.id);
            assert!(!model.default_backend.is_empty(), "{} has empty default_backend", model.id);
            assert!(!model.capabilities.is_empty(), "{} has empty capabilities", model.id);
        }
    }

    #[test]
    fn test_model_ids_are_unique() {
        let catalog = get_catalog();
        let ids: std::collections::HashSet<&str> = catalog.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids.len(), catalog.len(), "Duplicate model IDs found");
    }

    #[test]
    fn test_gemma_4_e2b_has_both_backends() {
        let model = find_model("gemma-4-e2b-it").unwrap();
        assert!(model.llama_cpp_url.is_some());
        assert!(model.llama_cpp_size_mb.is_some());
        assert!(model.cactus_url.is_some());
        assert!(model.cactus_size_mb.is_some());
    }

    #[test]
    fn test_bonsai_has_only_llama_cpp_backend() {
        let bonsai = find_model("Bonsai-8B").unwrap();
        assert!(bonsai.llama_cpp_url.is_some());
        assert!(bonsai.cactus_url.is_none());
        assert!(bonsai.cactus_size_mb.is_none());
    }

    #[test]
    fn test_all_model_urls_use_https() {
        for model in get_catalog() {
            if let Some(url) = &model.llama_cpp_url {
                assert!(url.starts_with("https://"), "{} llama_cpp_url is not https: {}", model.id, url);
            }
            if let Some(url) = &model.cactus_url {
                assert!(url.starts_with("https://"), "{} cactus_url is not https: {}", model.id, url);
            }
        }
    }

    #[test]
    fn test_find_model_is_case_sensitive() {
        assert!(find_model("GEMMA-3-1B-IT").is_none());
        assert!(find_model("gemma-3-1b-it").is_some());
    }
}
