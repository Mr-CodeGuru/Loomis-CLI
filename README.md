# LoomisCLI

A local, terminal-based RAG code-assistant CLI. Rust-first architecture with a minimal Python
sidecar used only where Rust has no viable path (embedding model inference). No cloud
dependency, no daemon-managed inference server — you run `llama-server` yourself and point the
CLI at it.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Rust binary (LoomisCLI)                                 │
│  - REPL / slash-command shell                           │
│  - Config load/persist                                  │
│  - LanceDB client (native `lancedb` crate) — vector +   │
│    hybrid search                                        │
│  - HTTP client → OpenAI-compatible chat endpoint        │
│  - Owns the persistent Python sidecar process handle    │
└───────────────┬─────────────────────────────────────────┘
                │ JSON-lines over stdin/stdout (long-lived process)
┌───────────────▼────────────────────────────────────────┐
│ Python sidecar                                         │
│  - Loads jina-embeddings-v2-base-code ONCE at startup  │
│  - Embeds query text on request, nothing else          │
└────────────────────────────────────────────────────────┘
```

Why the split: LanceDB has a native Rust crate, so retrieval, config, the CLI shell, and the LLM
HTTP client all live in Rust. The one thing that doesn't have a workable Rust path is running
`jina-embeddings-v2-base-code` (custom transformer architecture, no mature `candle`/`rust-bert`
support) — that's the sidecar's entire job, and only that.

## Requirements

- Rust toolchain (`cargo`, stable channel)
- Python 3.12+ with a venv
- [`llama-server`](https://github.com/ggerganov/llama.cpp) built and available, serving a
  GGUF model with an OpenAI-compatible endpoint (this project targets
  `Llama-3.2-1B-Instruct-Q8_0.gguf`, but the client is provider-agnostic — any
  OpenAI-compatible endpoint works)

## Setup

### 1. Python venv (embedding sidecar dependencies)

```powershell
python -m venv venv
.\venv\Scripts\Activate.ps1
pip install -q 'transformers==4.38.2' 'sentence-transformers>2.6.0,<3.0.0' einops 'huggingface-hub>=0.20.0,<0.23.0'
```

These are pinned deliberately — `jina-embeddings-v2-base-code`'s custom `trust_remote_code`
implementation is fragile across `transformers`/`sentence-transformers`/`huggingface-hub`
version combinations. Don't bump these without re-verifying the model still loads.

### 2. Verify the embedding model loads

```powershell
python loadModels\loadJina.py
```

Expected output ends with `PASS: embedding dimension matches expected 768.` This downloads model
weights into `models\` (gitignored, project-local — not your global HF cache) on first run.

### 3. Rust build

```powershell
cargo build
```

### 4. Download the inference model

```powershell
python loadModels\loadLlamaQ8.py
```

Downloads `Llama-3.2-1B-Instruct-Q8_0.gguf` (~1.32GB) directly to `models\` — a fixed,
predictable path (not nested in HF's cache blob structure), so `llama-server` can be pointed
at it deterministically.

### 5. Start `llama-server` manually

```powershell
llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
```

Start it yourself, before running LoomisCLI. On first run, LoomisCLI will prompt you for the
endpoint URL (e.g. `http://localhost:8080`) and API key (if applicable), and persist it to
`%USERPROFILE%\.loomiscli\config.json`. Conversation history is never persisted.

## Project layout

```
LoomisCLI/
├── src/                  # Rust application source
│   └── main.rs
├── loadModels/
│   └── loadJina.py       # Standalone pre-flight test for the embedding model
├── db/
│   └── embeddings.parquet  # Precomputed embeddings (gitignored)
├── models/                # Local HF model cache (gitignored)
├── Cargo.toml
├── requirements.txt
├── .gitignore
├── .gitattributes
├── tree.ps1              # Dev utility: writes STRUCTURE.md (dirs + filtered files)
├── init-structure.ps1    # Dev utility: recreates gitignored dirs from STRUCTURE.md
└── README.md
```

## Dev utilities

- `powershell -ExecutionPolicy Bypass -File tree.ps1` — regenerates `STRUCTURE.md`, a snapshot of
  the project layout. Skips large binaries and collapses `models\` (HF cache internals aren't
  meaningful structure).
- `powershell -ExecutionPolicy Bypass -File init-structure.ps1` — on a fresh clone, recreates the
  gitignored directories (`models\`, `db\`, etc.) that git doesn't track, so the project has
  somewhere to put downloaded/generated artifacts without manual mkdir-ing.

## Status

Environment and tooling setup phase. Rust application logic, the actual sidecar script (distinct
from the `loadJina.py` pre-flight test), LanceDB ingestion, and the CLI shell are not yet built.