# LoomisCLI

A local, terminal-based RAG code-assistant CLI. Rust-first architecture with a minimal Python
sidecar used only where Rust has no viable path (embedding model inference). No cloud
dependency, no daemon-managed inference server — you run `llama-server` yourself and point the
CLI at it.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Rust binary (LoomisCLI)                                 │
│  - Interactive REPL / slash commands (/help, /exit)     │
│  - Config load/persist (~/.loomiscli/config.json)       │
│  - Native LanceDB vector storage & hybrid search        │
│  - Streaming HTTP client → llama-server completions     │
│  - Spawns and manages Python sidecar supervisor & IPC   │
└───────────────────────────┬─────────────────────────────┘
                            │ JSON-lines over stdin/stdout (long-lived process)
┌───────────────────────────▼─────────────────────────────┐
│ Python sidecar (sidecar/embed.py)                       │
│  - Loads jina-embeddings-v2-base-code ONCE at startup   │
│  - Embeds query text on request (768-dim float vector)  │
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

### One-Shot Onboarding

**Windows:**
```powershell
powershell -ExecutionPolicy Bypass -File setup.ps1
```

**macOS/Linux:**
```bash
./setup.sh
```

### Manual Setup Steps

#### 1. Python venv (embedding sidecar dependencies)

**Windows (PowerShell):**
```powershell
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt
```

**macOS/Linux (bash):**
```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

#### 2. Verify the embedding model loads

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

#### 3. Download the inference model

**Windows:**
```powershell
python loadModels\loadLlamaQ8.py
```

**macOS/Linux:**
```bash
python3 loadModels/loadLlamaQ8.py
```

Downloads `Llama-3.2-1B-Instruct-Q8_0.gguf` (~1.32GB) directly to `models/` — a fixed,
predictable path, so `llama-server` can be pointed at it deterministically.

#### 4. Build the Rust binary

```bash
cargo build
```

#### 5. Start `llama-server` manually

**Windows:**
```powershell
llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
```

**macOS/Linux:**
```bash
llama-server -m models/Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
```

#### 6. Run LoomisCLI

```bash
cargo run
```

On first run, LoomisCLI will prompt you for the endpoint URL (default: `http://localhost:8080`)
and optional API key, persisting it to `~/.loomiscli/config.json` (`%USERPROFILE%\.loomiscli\config.json` on Windows).
Conversation history is ephemeral and never persisted.

## Project Layout

```
LoomisCLI/
├── src/                     # Modular Rust application architecture
│   ├── cli/                 # REPL session, terminal rendering, slash commands
│   │   ├── commands.rs      # /help, /stats, /config, /clear, /exit
│   │   ├── session.rs       # Interactive loop & prompt rendering
│   │   └── mod.rs
│   ├── db/                  # Native LanceDB vector storage & parquet ingestion
│   │   ├── store.rs         # Connection, table management, vector similarity search
│   │   └── mod.rs
│   ├── llm/                 # OpenAI-compatible streaming client & prompt assembly
│   │   ├── client.rs        # HTTP reqwest client with SSE line-buffered streaming
│   │   ├── prompt.rs        # RAG context formatter and system instruction
│   │   └── mod.rs
│   ├── sidecar/             # Rust IPC supervisor & Python process manager
│   │   ├── client.rs        # Stdin/stdout JSON-line client with auto-restart
│   │   ├── process.rs       # Python path & sidecar script auto-discovery
│   │   └── mod.rs
│   ├── config.rs            # Config persistence (~/.loomiscli/config.json)
│   ├── lib.rs               # Library root re-exporting modules
│   └── main.rs              # Application CLI entrypoint
├── sidecar/
│   └── embed.py             # Minimal Python IPC worker for jina-embeddings-v2-base-code
├── scripts/                 # Dev and maintenance utilities
│   ├── init-structure.ps1   # Recreates gitignored dirs from STRUCTURE.txt (Windows)
│   ├── init-structure.sh    # Recreates gitignored dirs from STRUCTURE.txt (macOS/Linux)
│   ├── tree.ps1             # Generates STRUCTURE.txt snapshot (Windows)
│   └── tree.sh              # Generates STRUCTURE.txt snapshot (macOS/Linux)
├── loadModels/              # Standalone model test & download scripts
│   ├── loadJina.py          # Standalone pre-flight test for embedding model
│   └── loadLlamaQ8.py       # Downloads Llama-3.2-1B-Instruct-Q8_0.gguf
├── dbe/                     # Vector database & parquet source files (gitignored)
│   ├── embeddings.parquet   # 70,163 precomputed embeddings
│   └── lancedb/             # Native LanceDB database directory
├── models/                  # Local model storage & HF cache (gitignored)
├── examples/                # Integration tests and verification examples
├── prompts/
│   └── claude.md            # Standalone project specification & architectural decisions
├── setup.ps1                # Idempotent onboarding script (Windows)
├── setup.sh                 # Idempotent onboarding script (macOS/Linux)
├── requirements.txt         # Pinned Python dependencies
├── Cargo.toml               # Rust dependencies & configuration
├── STRUCTURE.txt            # Auto-generated project tree snapshot
└── README.md
```

## Dev Utilities

Both platforms produce/consume the same `STRUCTURE.txt` format, so either can be used regardless
of which machine generated it:

**Windows:**
```powershell
powershell -ExecutionPolicy Bypass -File scripts\tree.ps1
powershell -ExecutionPolicy Bypass -File scripts\init-structure.ps1
```

**macOS/Linux:**
```bash
chmod +x scripts/*.sh
./scripts/tree.sh
./scripts/init-structure.sh
```

## Status

Fully operational. End-to-end RAG query pipeline, persistent LanceDB vector store (70,163 code chunks),
auto-recovering Python embedding sidecar, streaming SSE LLM client, and interactive REPL shell are complete
and verified.

