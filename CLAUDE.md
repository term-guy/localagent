# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Run the full Tauri dev environment (starts Vite + Rust backend)
npm run tauri dev

# Build the production app (runs vue-tsc + vite build + Rust release)
npm run tauri build

# Frontend only (Vite dev server on http://localhost:1420)
npm run dev

# Type-check + build frontend bundle
npm run build

# Rust compilation only
cargo build --manifest-path src-tauri/Cargo.toml

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml
```

## Architecture

This is a **Tauri v2 desktop app** (Rust backend + Vue 3 frontend) for fully offline LLM inference. No cloud calls are made during inference; models run locally via one of two pluggable backends.

### Frontend (`src/`)

**Views** handle routing and page-level layout:
- `SetupView` — first-run model download wizard; router guards redirect here when no models are installed
- `ChatView` — main chat interface with session sidebar
- `SettingsView` — model management (install, switch, remove)

**Pinia stores** are the primary state layer:
- `modelStore` — catalog, installed models, active model, download progress. Watches `activeModelId` and preloads the model into the Rust backend whenever it changes.
- `chatStore` — sessions, messages, streaming state. Listens to Tauri events (`token`, `inference-complete`, `inference-error`) to stream tokens into the active message in real time.

**Multi-backend installed list**: `list_installed` returns one `InstalledModel` per `(model_id × backend)` pair — a model installed for both backends appears twice in the array. `availableModels` filters by `id` only, so a model disappears from the catalog list as soon as any backend is installed; the missing-backend download is offered inline in `SettingsView`'s Installed section. UI iterating over `installed` must group by `id` to avoid duplicate cards.

**Tauri IPC**: all backend calls use `invoke()` from `@tauri-apps/api/core`. Event payloads are typed in `src/types/index.ts`.

### Backend (`src-tauri/src/`)

**`AppState`** (managed by Tauri) holds:
- `model: Mutex<Option<LoadedModel>>` — currently loaded backend instance
- `inference_running: Mutex<bool>` + `inference_cancel: Mutex<Option<Arc<AtomicBool>>>` — guards concurrent inference
- `downloads: Mutex<HashMap<String, oneshot::Sender<()>>>` — cancellation channels for in-flight downloads

**`Backend` trait** (`backend.rs`) abstracts over inference engines. Two implementations live in `backends/`:
- `CactusBackend` — thin FFI wrapper around `cactus-sys` (a local path dep at `../cactus/rust/cactus-sys`). Supports chat, vision, and audio (WAV PCM).
- `LlamaCppBackend` — wraps `llama-cpp-2`; uses Metal on macOS.

`BackendKind::from_str` defaults anything unrecognized to Cactus. The registry stores which backend was used for each installed model.

**Commands** (`commands/`) are grouped by domain and registered in `lib.rs`:
- `inference` — `load_model`, `unload_model`, `send_message`, `cancel_inference`. Inference runs on `spawn_blocking` and emits Tauri events per token.
- `models` — catalog listing, download (with progress events + optional ZIP extraction), removal. Model files land in `app_local_data_dir/models/`; the registry is `app_local_data_dir/models.json`.
- `sessions` — CRUD for chat sessions stored as JSON files in `app_local_data_dir/sessions/`.

**Catalog** (`catalog.rs`) is a hardcoded `Vec<ModelInfo>`. Each entry can have separate `llama_cpp_url` and `cactus_url` fields; the download command picks the right URL based on the selected backend. Entries with a `repo` field support HuggingFace quant picking via `fetch_hf_quants` (queries the HF API and returns a sorted `Vec<HfFile>`).

**HuggingFace quant flow**: `download_model` accepts optional `filename`, `url`, `size_bytes` overrides so the frontend can pass a specific quant's details from `fetch_hf_quants`. `QuantPickerPanel.vue` handles the selection UI inline in `ModelCard`; it emits a `DownloadRequest` (`{ backend, filename?, url?, size_bytes? }`).
- `fetch_hf_quants` uses `/api/models/{repo}/tree/main` — **not** `/api/models/{repo}`; the model endpoint returns `null` for `size` and `lfs` on GGUF files so file sizes would all show as unknown.

**ZIP extraction**: Cactus models are `.zip` archives. After download the zip is extracted into a same-named directory and the archive deleted. Extraction progress is reported on the same `download-progress` event with `phase: "extracting"` (no `speed_bps`).

### Shutdown sequence (macOS-critical)

On macOS, dropping a Metal-backed llama.cpp model during AppKit termination triggers a ggml residency-set assert. The app intercepts `applicationShouldTerminate:` via ObjC runtime swizzling, cancels any running inference, waits for it to finish, then calls `_exit(0)` directly—bypassing the normal AppKit teardown path.

### Tauri events reference

| Event | Direction | Payload |
|---|---|---|
| `token` | Rust → JS | `{ session_id, token }` |
| `inference-complete` | Rust → JS | `{ session_id }` |
| `inference-error` | Rust → JS | `{ session_id, error }` |
| `download-progress` | Rust → JS | `{ model_id, bytes_downloaded, total_bytes, speed_bps, percentage, phase? }` — `phase: "extracting"` during ZIP extraction (speed_bps is 0) |
| `download-complete` | Rust → JS | `{ model_id }` |
| `download-error` | Rust → JS | `{ model_id, error }` |
