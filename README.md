# LocalAgent

A fully offline AI chat desktop app built with Tauri v2 (Rust) and Vue 3. No cloud calls are made during inference — models run entirely on your machine.

## Features

- **Fully offline inference** — no data leaves your device
- **Dual backend support** — run models via [llama.cpp](https://github.com/ggerganov/llama.cpp) (with Metal acceleration on macOS) or [Cactus](https://github.com/cactus-compute/cactus)
- **Model catalog** — one-click download and install for curated models
- **HuggingFace quant picker** — browse and select quantizations directly from HF repos
- **Streaming responses** — tokens stream in real time as they are generated
- **Session management** — persistent chat history stored locally as JSON
- **Multiple sessions** — sidebar for creating, switching, and deleting conversations

## Bundled Models

| Model | Provider | Size (llama.cpp) | Backends |
|---|---|---|---|
| Gemma-3-1B | Google | ~806 MB | llama.cpp, Cactus |
| Gemma-4-E2B | Google | ~3.1 GB | llama.cpp, Cactus |
| Bonsai-8B | Prisma ML | ~1.2 GB | llama.cpp |

Any GGUF model from HuggingFace can also be installed by browsing quants directly in the app.

## Requirements

- macOS (primary target; Metal GPU acceleration on Apple Silicon and AMD/Intel Macs)
- [Rust](https://rustup.rs/) toolchain
- [Node.js](https://nodejs.org/) 18+
- Tauri CLI v2: `npm install -g @tauri-apps/cli`

## Development

```bash
# Install frontend dependencies
npm install

# Start the full dev environment (Vite + Rust hot-reload)
npm run tauri dev

# Frontend only (http://localhost:1420)
npm run dev

# Type-check + build frontend bundle
npm run build

# Rust compilation only
cargo build --manifest-path src-tauri/Cargo.toml

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend tests
npm test
```

## Production Build

```bash
npm run tauri build
```

Produces a signed `.app` bundle (macOS) in `src-tauri/target/release/bundle/`.

## Architecture

```
localagent/
├── src/                  # Vue 3 frontend
│   ├── views/            # SetupView, ChatView, SettingsView
│   ├── stores/           # Pinia stores (modelStore, chatStore)
│   ├── components/       # Reusable UI components
│   └── types/            # Shared TypeScript types
└── src-tauri/            # Rust backend
    ├── src/
    │   ├── backends/     # CactusBackend, LlamaCppBackend
    │   ├── commands/     # Tauri IPC commands (inference, models, sessions)
    │   ├── catalog.rs    # Hardcoded model catalog
    │   └── state.rs      # AppState (loaded model, inference guards, downloads)
    └── Cargo.toml
```

Model files and session data are stored in the OS app local data directory (`~/Library/Application Support/localagent/` on macOS).

## License

MIT
