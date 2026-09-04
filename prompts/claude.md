# Build Prompt: LoomisCLI — Continuation (v5)

## Architecture decision, closed — do not re-open

A Rust-only embedding path (via `candle-transformers`' `jina_bert` module) was attempted and
**deliberately abandoned**. It is technically possible in principle — `candle-transformers` does
have a native JinaBert implementation — but was rejected for practical reasons:

- `hf-hub` underwent a breaking full API rewrite at v1.0.0 (old `Api`/`ApiRepo` sync interface
  removed entirely, replaced with an incompatible `HFClient`/`HFClientSync` interface), requiring
  a version pin workaround.
- Multiple rounds of guess-and-fix compile errors (missing `Module` trait import, `Error::Http`
  variant missing from `lancedb` requiring a feature flag/version pin, etc.) — signal of a
  less mature, faster-churning dependency surface than the Python equivalent.
- Full dependency compile time (`candle` + `lancedb`/`lance`/`datafusion` stack) ran ~9 minutes on
  first build, with `protoc` and native toolchain requirements (unlike Python's prebuilt wheels).

**Decision: embedding inference goes back to the Python sidecar, as originally designed.**
`loadModels\loadJina.py` is already confirmed working with pinned, stable dependencies — that's
the actual embedding path going forward. Don't attempt the Rust-embedding path again without a
new, explicit decision to revisit it — this isn't an oversight to "fix," it was tried and rejected
on its merits.

**What stays in Rust, unaffected by this reversal:** `db\embeddings.parquet` schema reading
(`examples\testParquet.rs` — confirmed working, parquet is a mature, stable crate unlike
`candle`) and LanceDB access generally. Only the embedding *model inference* moved back to Python
— not database/retrieval logic.

## Current state (verified facts — don't re-derive or re-decide these)

- Project root: `C:\Users\Aman.Yadav\Desktop\Projects\LoomisCLI\` (Windows-primary; macOS
  compatibility maintained via `.sh` equivalents of dev utility scripts).
- Rust: `cargo init` done. `candle-core`/`candle-nn`/`candle-transformers`/`hf-hub`/`tokenizers`
  removed from `Cargo.toml` (dead-end embedding attempt). Remaining/kept dependencies: `lancedb`,
  `arrow`, `parquet`, `serde`/`serde_json`, `tokio`, `reqwest`, `anyhow`.
  `examples\testEmbeddeJina.rs` deleted. `examples\testParquet.rs` kept — confirmed compiling and
  running successfully against `db\embeddings.parquet`'s real schema.
- Python: dependency management moved to **`uv`** instead of plain `pip`/`venv` — same pinned
  `requirements.txt`, drop-in compatible, faster installs. `.venv\` (not `venv\`) is now the
  convention.
- **`loadModels\loadJina.py` — CONFIRMED PASSING.** This is the actual embedding path.
- **`loadModels\loadLlamaQ8.py` — CONFIRMED RUN.** `Llama-3.2-1B-Instruct-Q8_0.gguf` downloaded to
  `models\`.
- **`llama-server` status: still not fully confirmed end-to-end.** Reported as "working" earlier,
  but never confirmed via an actual observed `/v1/chat/completions` response payload. Treat as
  open until a real request/response has been seen.
- `db\embeddings.parquet` — schema confirmed (70,163 rows; columns include `text`, `vector`
  [768-dim fixed-size-list-float], plus others). **Open decision, still unresolved:** `chunk_id`
  vs `id` and `content_hash` vs `_content_hash` appear duplicated — canonical column not yet
  chosen. Resolve before LanceDB table design locks this in.
- `.gitignore`, `.gitattributes`, bilingual `README.md`, and both PowerShell/bash dev utility
  script pairs (`tree.*`, `init-structure.*`) are current and in place.

## Architecture (locked, do not re-litigate)

- **Rust** owns: CLI/REPL, slash commands, config persistence, LanceDB access (native `lancedb`
  crate), parquet schema/ingestion reading, HTTP client to the OpenAI-compatible chat endpoint,
  spawning/managing the Python sidecar subprocess.
- **Python sidecar** owns exactly one thing: loading `jina-embeddings-v2-base-code` once (reusing
  `loadJina.py`'s validated load path) and embedding query text on request. Nothing else.
- **IPC**: long-lived Python subprocess, newline-delimited JSON over stdin/stdout, versioned
  payload (`"v": 1`), Rust detects a dead process and restarts it, read timeout instead of
  indefinite block.
- Conversation history stays ephemeral, never persisted. Only endpoint config
  (`~/.loomiscli/config.json` / `%USERPROFILE%\.loomiscli\config.json`) persists.
- `llama-server` is started manually by the user; the CLI is not a daemon manager for it.

## Ordered next steps

1. **Resolve the `chunk_id`/`id` and `content_hash`/`_content_hash` duplicate-column question**
   with the user before it becomes a baked-in LanceDB schema decision.
2. **Confirm `llama-server` actually returns a real completion** — a concrete request/response,
   not just "the process started." Note whether streaming is supported.
3. **Build the LanceDB ingestion step in Rust**, building on `testParquet.rs`'s confirmed-working
   schema read, using the resolved column decision from step 1.
4. **Build the actual Python sidecar script** (distinct from `loadJina.py`) implementing the
   stdin/stdout JSON-line protocol, reusing `loadJina.py`'s validated model-load path.
5. **Wire the IPC**: Rust spawns the sidecar, sends a test embedding request, confirms round-trip
   end-to-end before building anything on top of it.
6. **Build the `llama-server` HTTP client** in Rust, using what was confirmed in step 2.
7. **Wire the CLI/REPL shell**: first-run config prompt, config persistence, slash commands, and
   the full flow (query → sidecar embeds → LanceDB search → prompt assembly → LLM call → response).

## Guardrails (carried over, still apply)

- Don't re-attempt Rust-only embedding inference without a new explicit decision — see closed
  decision above.
- Don't replicate LanceDB access in Python.
- Don't persist chat history.
- Keep the LLM client provider-agnostic (OpenAI-compatible), not llama.cpp-specific.
- Don't silently pin/change dependency versions to work around an error — surface the actual
  failure first.
- Don't hardcode machine-specific absolute paths — `__file__`-relative paths only, matching
  `loadJina.py`/`loadLlamaQ8.py`.