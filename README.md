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
┌───────────────▼─────────────────────────────────────────┐
│ Python sidecar                                          │
│  - Loads jina-embeddings-v2-base-code ONCE at startup   │
│  - Embeds query text on request, nothing else           │
└─────────────────────────────────────────────────────────┘
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

Commands are given for both Windows (PowerShell) and macOS/Linux (bash). Use whichever matches
your machine.

### 1. Python venv (embedding sidecar dependencies)

**Windows (PowerShell):**
```powershell
python -m venv venv
.\venv\Scripts\Activate.ps1
pip install -q 'transformers==4.38.2' 'sentence-transformers>2.6.0,<3.0.0' einops 'huggingface-hub>=0.20.0,<0.23.0'
```

**macOS/Linux (bash):**
```bash
python3 -m venv venv
source venv/bin/activate
pip install -q 'transformers==4.38.2' 'sentence-transformers>2.6.0,<3.0.0' einops 'huggingface-hub>=0.20.0,<0.23.0'
```

These are pinned deliberately — `jina-embeddings-v2-base-code`'s custom `trust_remote_code`
implementation is fragile across `transformers`/`sentence-transformers`/`huggingface-hub`
version combinations. Don't bump these without re-verifying the model still loads. Or install
directly from the pinned list:

```bash
pip install -r requirements.txt
```

### 2. Verify the embedding model loads

**Windows:**
```powershell
python loadModels\loadJina.py
```

**macOS/Linux:**
```bash
python3 loadModels/loadJina.py
```

Expected output ends with `PASS: embedding dimension matches expected 768.` This downloads model
weights into `models/` (gitignored, project-local — not your global HF cache) on first run.

### 3. Rust build

Same on both platforms:
```bash
cargo build
```

### 4. Download the inference model

**Windows:**
```powershell
python loadModels\loadLlamaQ8.py
```

**macOS/Linux:**
```bash
python3 loadModels/loadLlamaQ8.py
```

Downloads `Llama-3.2-1B-Instruct-Q8_0.gguf` (~1.32GB) directly to `models/` — a fixed,
predictable path (not nested in HF's cache blob structure), so `llama-server` can be pointed
at it deterministically.

### 5. Start `llama-server` manually

**Windows:**
```powershell
llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
```

**macOS/Linux:**
```bash
llama-server -m models/Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
```

Start it yourself, before running LoomisCLI. On first run, LoomisCLI will prompt you for the
endpoint URL (e.g. `http://localhost:8080`) and API key (if applicable), and persist it to
`~/.loomiscli/config.json` (`%USERPROFILE%\.loomiscli\config.json` on Windows). Conversation
history is never persisted.

> Flag names (`-c`, `--port`, etc.) depend on your `llama-server` build/version — run
> `llama-server --help` to confirm before relying on these exact flags.

## Project layout

```
LoomisCLI/
├── src/                    # Rust application source
│   └── main.rs
├── loadModels/
│   ├── loadJina.py         # Standalone pre-flight test for the embedding model
│   └── loadLlamaQ8.py      # Downloads the Llama 3.2 1B Q8_0 GGUF into models/
├── db/
│   └── embeddings.parquet  # Precomputed embeddings (gitignored)
├── models/                 # Local HF model cache + GGUF file (gitignored)
├── prompts/
│   └── claude.md           # Build/continuation prompt for this project
├── Cargo.toml
├── requirements.txt
├── .gitignore
├── .gitattributes
├── tree.ps1                # Dev utility (Windows): writes STRUCTURE.txt
├── tree.sh                 # Dev utility (macOS/Linux): writes STRUCTURE.txt
├── init-structure.ps1      # Dev utility (Windows): recreates gitignored dirs from STRUCTURE.txt
├── init-structure.sh       # Dev utility (macOS/Linux): recreates gitignored dirs from STRUCTURE.txt
└── README.md
```

## Dev utilities

Both platforms produce/consume the same `STRUCTURE.txt` format, so either can be used regardless
of which machine generated it (line-ending differences between Windows/macOS are handled).

**Windows:**
```powershell
powershell -ExecutionPolicy Bypass -File tree.ps1
powershell -ExecutionPolicy Bypass -File init-structure.ps1
```

**macOS/Linux:**
```bash
chmod +x tree.sh init-structure.sh
./tree.sh
./init-structure.sh
```

- `tree.*` — regenerates `STRUCTURE.txt`, a snapshot of the project layout. Skips large binaries
  by extension and collapses `models/` (HF cache internals aren't meaningful structure — shown as
  a placeholder, not recursed into).
- `init-structure.*` — on a fresh clone, recreates the gitignored **directories** (`models/`,
  `db/`, etc.) that git doesn't track. Directories only — files always come from `git clone`,
  never from this script.

## Status

Environment and tooling setup phase. Rust application logic, the actual sidecar script (distinct
from the `loadJina.py` pre-flight test), LanceDB ingestion, and the CLI shell are not yet built.