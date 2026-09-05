# LoomisCLI

A local-first, terminal-based Retrieval-Augmented Generation (RAG) code-assistant. Rust-first architecture with a minimal Python sidecar used only where Rust has no viable path (embedding model inference). No cloud dependency, no daemon-managed inference server — you run `llama-server` locally and point the CLI at it.

LoomisCLI offers **dual frontends** sharing the exact same RAG core:
1. **Claude Code-style TUI** (`cargo run -- --tui`): Alternate-screen terminal interface with real-time token streaming, true syntax highlighting (`syntect` Monokai), multi-line input editor, prompt history recall, and live status monitoring.
2. **Classic REPL Mode** (`cargo run`): Lightweight interactive terminal REPL with rounded ANSI code frames and streaming syntax color theme.

---

## Architecture

```
                                  ┌──────────────────────────┐
                                  │      LoomisCLI CLI       │
                                  │      (src/main.rs)       │
                                  └─────────────┬────────────┘
                                                │
                          ┌─────────────────────┴─────────────────────┐
                          │                                           │
                    [--tui flag]                               [default path]
                          │                                           │
                          ▼                                           ▼
            ┌───────────────────────────┐               ┌───────────────────────────┐
            │       TUI Frontend        │               │     Classic REPL Mode     │
            │     (src/tui/app.rs)      │               │   (src/cli/session.rs)    │
            │   - Ratatui Alt-Screen    │               │   - Standard Terminal IO  │
            │   - Syntect Monokai Theme │               │   - StreamingCodeFormatter│
            │   - Multi-line TextArea   │               │   - Colored ANSI Prompts  │
            │   - Up/Down History Recall│               │   - Lightweight Memory    │
            └─────────────┬─────────────┘               └─────────────┬─────────────┘
                          │                                           │
                          └─────────────────────┬─────────────────────┘
                                                │
                                                ▼
                                  ┌───────────────────────────┐
                                  │        LoomisCore         │
                                  │       (src/core.rs)       │
                                  └─────────────┬─────────────┘
                                                │
         ┌──────────────────────────────────────┼──────────────────────────────────────┐
         │                                      │                                      │
         ▼                                      ▼                                      ▼
┌───────────────────┐                  ┌───────────────────┐                  ┌───────────────────┐
│  LLM Intent Gate  │                  │  Sidecar IPC &    │                  │ In-Memory Session │
│ (src/llm/client)  │                  │  LanceDB Vector   │                  │  History Buffer   │
│ Fast Few-Shot Pass│                  │ (src/db/store.rs) │                  │ (ChatMessage Vec) │
└───────────────────┘                  └───────────────────┘                  └───────────────────┘
```

Why the split: LanceDB has a native Rust crate, so retrieval, config, the CLI shell, TUI rendering, and the LLM HTTP client all live in Rust. The one thing that doesn't have a workable Rust path is running `jina-embeddings-v2-base-code` (custom transformer architecture, no mature `candle`/`rust-bert` support) — that's the sidecar's entire job, and only that.

---

## Requirements

- Rust toolchain (`cargo`, stable channel)
- Python 3.12+ with a virtual environment (`.venv`)
- [`llama-server`](https://github.com/ggerganov/llama.cpp) built and available, serving a GGUF model with an OpenAI-compatible endpoint (targets `Llama-3.2-1B-Instruct-Q8_0.gguf`, but provider-agnostic)

---

## Setup

Commands are given for both Windows (PowerShell) and macOS/Linux (bash).

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

#### 1. Python Virtual Environment (Embedding Sidecar)

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

#### 2. Verify the Embedding Model
Downloads model weights into `models/` (gitignored, project-local):

```bash
# Windows: python loadModels\loadJina.py
python3 loadModels/loadJina.py
```
Expected output ends with `PASS: embedding dimension matches expected 768.`

#### 3. Download the Inference Model
Downloads `Llama-3.2-1B-Instruct-Q8_0.gguf` (~1.32GB) directly to `models/`:

```bash
# Windows: python loadModels\loadLlamaQ8.py
python3 loadModels/loadLlamaQ8.py
```

#### 4. Build the Rust Binary

```bash
cargo build
```

#### 5. Start `llama-server`

**Windows:**
```powershell
llama-server -m models\Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
```

**macOS/Linux:**
```bash
llama-server -m models/Llama-3.2-1B-Instruct-Q8_0.gguf -c 4096 --port 8080
```

---

## Running LoomisCLI

### Mode 1: Terminal UI (Recommended)
Launch the Claude Code-inspired alternate screen interface:
```bash
cargo run -- --tui
```

**Keybindings & Controls in TUI:**
- <kbd>Enter</kbd>: Submit query
- <kbd>Shift</kbd> + <kbd>Enter</kbd>: Multi-line newline
- <kbd>↑</kbd> / <kbd>↓</kbd>: Recall previous queries from input history
- <kbd>PageUp</kbd> / <kbd>PageDown</kbd>: Scroll conversation viewport
- `/clear`: Clear conversation and in-memory history
- `/exit` or `/quit` or <kbd>Ctrl</kbd>+<kbd>C</kbd>: Clean exit

### Mode 2: Classic REPL Mode
Standard terminal streaming mode:
```bash
cargo run
```

On first run, LoomisCLI prompts for the endpoint URL (default: `http://localhost:8080`) and optional API key, persisting to `~/.loomiscli/config.json`. Conversation history is ephemeral and never persisted to disk.

---

## Project Layout

```
LoomisCLI/
├── src/
│   ├── cli/                   # Classic REPL frontend & streaming ANSI formatter
│   │   ├── commands.rs        # /help, /stats, /config, /clear, /exit
│   │   ├── formatter.rs       # StreamingCodeFormatter with Monokai terminal highlighting
│   │   ├── session.rs         # REPL session loop delegating to LoomisCore
│   │   └── mod.rs
│   ├── tui/                   # Ratatui alternate-screen TUI frontend
│   │   ├── app.rs             # TUI event loop, async Tokio channel receiver, layout
│   │   ├── markdown.rs        # Pulldown-cmark parser + Syntect syntax highlighter
│   │   └── mod.rs
│   ├── db/                    # Native LanceDB vector storage & similarity search
│   │   ├── store.rs           # Connect, table management, vector search
│   │   └── mod.rs
│   ├── llm/                   # OpenAI-compatible streaming client & prompt assembly
│   │   ├── client.rs          # HTTP reqwest client, few-shot pure LLM intent classifier
│   │   ├── prompt.rs          # Strict grounding prompt builder with markdown code fences
│   │   └── mod.rs
│   ├── sidecar/               # Rust IPC supervisor & Python process manager
│   │   ├── client.rs          # Stdin/stdout JSON-line client with auto-restart
│   │   ├── process.rs         # Python path & sidecar script auto-discovery
│   │   └── mod.rs
│   ├── config.rs              # Config persistence (~/.loomiscli/config.json)
│   ├── core.rs                # Shared engine orchestrating DB, Sidecar, LLM, and History
│   ├── lib.rs                 # Library root re-exporting modules
│   └── main.rs                # Application entrypoint with --tui flag dispatcher
├── sidecar/
│   └── embed.py               # Minimal Python IPC worker for jina-embeddings-v2-base-code
├── scripts/                   # Dev and maintenance utilities
│   ├── init-structure.ps1     # Base directory initializer (Windows)
│   ├── init-structure.sh      # Base directory initializer (macOS/Linux)
│   ├── tree.ps1               # Generates STRUCTURE.txt (Windows)
│   ├── tree.sh                # Generates STRUCTURE.txt (macOS/Linux)
│   ├── nightly-init-structure.ps1 # Full directory initializer including Nightly
│   ├── nightly-init-structure.sh  # Full directory initializer including Nightly
│   ├── nightly-tree.ps1       # Generates NightStructure.txt
│   └── nightly-tree.sh        # Generates NightStructure.txt
├── loadModels/                # Model download and pre-flight testing
│   ├── loadJina.py            # Standalone pre-flight test for embedding model
│   └── loadLlamaQ8.py         # Downloads Llama-3.2-1B-Instruct-Q8_0.gguf
├── dbe/                       # Vector database & parquet source files (gitignored)
│   ├── embeddings.parquet     # 70,163 precomputed embeddings
│   └── lancedb/               # Native LanceDB database directory
├── models/                    # Local model storage (gitignored)
├── examples/                  # Integration tests and verification examples
│   ├── testCodeBlockFormatting.rs # REPL streaming code block syntax test
│   ├── testConversationHistory.rs # Multi-turn history and pure LLM intent test
│   └── testTuiMarkdown.rs     # TUI markdown AST and syntect code framing test
├── prompts/                   # Technical specs, progress reports & build prompts
├── Nightly/                   # Nightly stabilization logs and corrections
├── setup.ps1                  # Idempotent onboarding script (Windows)
├── setup.sh                   # Idempotent onboarding script (macOS/Linux)
├── requirements.txt           # Pinned Python dependencies
├── Cargo.toml                 # Rust dependencies & configuration
├── STRUCTURE.txt              # Auto-generated project tree snapshot (base)
├── NightStructure.txt         # Auto-generated project tree snapshot (full)
└── README.md
```

---

## Dev Utilities

Both platforms produce/consume the same structure format:

**Windows:**
```powershell
powershell -ExecutionPolicy Bypass -File scripts\tree.ps1
powershell -ExecutionPolicy Bypass -File scripts\nightly-tree.ps1
```

**macOS/Linux:**
```bash
./scripts/tree.sh
./scripts/nightly-tree.sh
```

---

## Status

**`v1.2.61` Operational**:
- Shared `LoomisCore` engine with pure LLM semantic intent gating (`CODE` vs `CHAT`).
- High-fidelity Ratatui TUI with token-by-token streaming and Syntect Monokai syntax highlighting.
- Classic REPL mode fully preserved and tested with zero regressions.
- LanceDB vector storage indexing 70,163 code chunks with zero-copy retrieval.
- Automated tests passing 100% across all targets.
