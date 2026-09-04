# LoomisCLI — Full Project Prompt (Standalone, Consolidated)

This document is self-contained. It covers what LoomisCLI is, everything confirmed working,
everything deliberately tried and rejected (with reasons — do not re-attempt these), open
decisions still awaiting a choice, and the ordered path forward. Treat every "confirmed" item as
settled fact; treat every "open" item as something to resolve with the user before proceeding past
it, not something to assume.

---

## 1. What LoomisCLI is

A local, terminal-based RAG code-assistant CLI, inspired by tools like Claude Code / Gemini CLI /
Codex CLI. Fully local — no cloud dependency, no daemon-managed inference server. Windows-primary
development (`C:\Users\Aman.Yadav\Desktop\Projects\LoomisCLI\`), with macOS compatibility
maintained in parallel as a secondary concern.

---

## 2. Architecture (locked — do not re-litigate)

- **Rust** owns: CLI/REPL shell, slash commands, config persistence, LanceDB access (native
  `lancedb` crate — vector + hybrid search via RRF fusion), parquet reading/ingestion, HTTP client
  to an OpenAI-compatible chat completions endpoint, spawning/managing the Python sidecar
  subprocess.
- **Python sidecar** owns exactly one thing: loading `jina-embeddings-v2-base-code` once at
  startup and embedding query text on request. Nothing else — no LanceDB access, no reranking, no
  LLM calls. Scope stays intentionally narrow.
- **IPC**: long-lived Python subprocess (started once, stays alive — not spawned per-request, to
  avoid model reload cost), newline-delimited JSON over stdin/stdout, versioned payload schema
  (`"v": 1`), Rust detects a dead sidecar process and restarts it, read timeout instead of
  indefinite blocking.
- **Inference backend**: `llama-server` (llama.cpp) serving `Llama-3.2-1B-Instruct-Q8_0.gguf`,
  started manually by the user — the CLI is not a daemon manager for it. HTTP client is
  provider-agnostic (OpenAI-compatible), not llama.cpp-specific, so any compatible endpoint works.
- **State**: conversation history is ephemeral, never persisted. Only endpoint config persists, to
  `~/.loomiscli/config.json` (macOS/Linux) / `%USERPROFILE%\.loomiscli\config.json` (Windows).
  First run prompts for base URL + optional API key.
- **LanceDB** exclusively for retrieval (hybrid vector + Tantivy FTS via RRF); no FAISS, no
  separate BM25 library, no CrossEncoder reranking stage — that earlier design was superseded.

---

## 3. Architecture decisions tried and explicitly rejected — do not re-attempt

### 3.1 Rust-only embedding inference (via `candle-transformers`)
Real option — `candle-transformers` has a native `jina_bert` module, and other projects
(`glowrs`) have run `jina-embeddings-v2-base-en` through it successfully. Attempted, then
deliberately abandoned because:
- `hf-hub` underwent a full breaking API rewrite at v1.0.0 (old `Api`/`ApiRepo` sync interface
  removed, replaced with incompatible `HFClient`/`HFClientSync`), requiring a version pin
  workaround (`hf-hub@0.3`).
- Multiple rounds of guess-and-fix compile errors (missing `Module` trait import,
  `lancedb`'s `Error::Http` variant missing without a feature flag, etc.) — signals of a
  faster-churning, less mature dependency surface than Python's equivalent.
- Full dependency compile time (`candle` + `lancedb`/`lance`/`datafusion` stack) ran ~9 minutes on
  first build, requiring native toolchain setup (`protoc`) that Python's prebuilt wheels don't need.

**Decision: embedding inference is Python-sidecar-based**, using `loadJina.py`'s already-confirmed
working load path. Parquet/LanceDB reading stays in Rust — that part of the attempt worked and is
kept (see §4).

### 3.2 Custom `lld-link` linker configuration
Attempted to speed up Rust compile times via `.cargo\config.toml` pointing at `lld-link.exe`.
Reverted — first attempt used the wrong flag syntax (`-fuse-ld=lld-link`, a clang/gcc convention
MSVC's `link.exe` silently ignored with `LNK4044`), second attempt (`linker = "lld-link.exe"`)
broke a previously-working build because `lld-link.exe` wasn't reliably found even after PATH
changes. Also, this was unlikely to meaningfully help the real bottleneck (codegen across a huge
dependency graph), even if it had worked cleanly. **Do not re-attempt custom linker
configuration.**

---

## 4. Confirmed working (verified, not assumed)

- **`loadModels\loadJina.py`** — loads `jina-embeddings-v2-base-code` via
  `trust_remote_code=True`, embeds a sample string, confirmed 768-dim output
  (`PASS: embedding dimension matches expected 768.`). Uses `os.environ["HF_HOME"]` set to
  `<project_root>\models` via `Path(__file__).parent.parent` (portable, not hardcoded) —
  **must be set before any `sentence_transformers`/`transformers` import**, since env vars are
  read at import time. This was necessary because `cache_folder` alone (an earlier attempt) only
  controls `sentence-transformers`' own files, not a nested dependency
  (`jinaai/jina-bert-v2-qk-post-norm`) that `transformers`' own loader pulls in separately and
  which ignores `cache_folder`.
- **`loadModels\loadLlamaQ8.py`** — downloads `Llama-3.2-1B-Instruct-Q8_0.gguf` (~1.32GB) from
  `bartowski/Llama-3.2-1B-Instruct-GGUF` directly to `models\Llama-3.2-1B-Instruct-Q8_0.gguf`
  (flat path via `local_dir`, not nested in HF cache blobs — deliberate, for a deterministic path
  `llama-server` can point at). Also sets `HF_HOME` the same way, for the same underlying-cache
  reason as above.
- **`llama-server` — confirmed end-to-end via a real Rust request.** `examples\testLlamaServer.rs`
  (a `reqwest`-based client) sent a real `/v1/chat/completions` request and received `200 OK` with
  an actual completion (`"OK"`), full `usage`/`timings` data. Confirmed response shape: standard
  OpenAI-compatible (`choices[0].message.content`, `finish_reason`, `usage`) plus
  `llama-server`-specific extras (`timings.predicted_per_second`, `timings.prompt_per_second`).
  **Streaming also confirmed working** — see §5.3, `examples\testLlamaStreaming.rs`.
- **`db\embeddings.parquet` schema — confirmed independently in both Python and Rust**, outputs
  match exactly. 70,163 rows, 1 row group. Full schema: `chunk_id` (string), `text` (string),
  `content_hash` (string), `token_len` (int64), `embedding_model` (string,
  `"jinaai/jina-embeddings-v2-base-code"`), `embedding_dim` (int64, 768), `vector`
  (`fixed_size_list<float>[768]`), `id` (string), `source` (string), `repo` (string), `path`
  (string), `language` (string), `_source_file` (string), `_content_hash` (string),
  `chunk_len_chars` (int64), `chunk_len_tokens_est` (int64), `extracted_name` (string).
  Rust confirmation via `examples\testParquet.rs` (uses the `parquet` crate directly; had to guard
  against calling `.get_physical_type()` on the non-primitive `vector` column, which panics —
  fixed by checking `field.is_primitive()` first).
- **LanceDB table creation and vector search — CONFIRMED WORKING END-TO-END.**
  `examples\convertLanceDB.rs` reads all 70,163 rows from `db\embeddings.parquet`, creates a real
  LanceDB table (`chunks`) on disk at `db\lancedb\`, and runs a vector search against it — 5 rows
  returned, all 18 columns intact (17 original parquet columns + LanceDB's own result column).
  This is the first confirmed instance of data actually going into and being retrieved from
  LanceDB in this project (everything before this only read parquet schema).

  Getting here required resolving a real Arrow-ecosystem version conflict: `loomiscli`'s directly
  added `arrow` and `parquet` crates had each independently resolved to different, incompatible
  versions (`59.3.0`) than the one `lancedb`'s internal `datafusion`/`lance` dependency chain
  actually uses (`58.4.0`) — since neither was version-pinned when added. Fixed by pinning both
  `arrow` and `parquet` explicitly to `58.4.0` to match. **Lesson for future `cargo add` calls
  touching Arrow-adjacent crates on this project: check `cargo tree | findstr arrow-array`
  afterward to confirm no version split was introduced, rather than assuming it auto-resolves
  compatibly.** Also required switching all Arrow type imports (`FixedSizeListArray`,
  `Float32Array`, `RecordBatchIterator`, `RecordBatchReader`) to `lancedb`'s own re-export path
  (`lancedb::arrow::arrow_array::*`) rather than a standalone `arrow-array` dependency, to
  guarantee the same crate instance is used throughout.

  **Still using a placeholder query** (the first row's own vector, not a real embedded user
  query) — this proves the mechanism works, not that retrieval quality is good. Real query
  embedding comes from the Python sidecar (§3.1, not yet built).

  **Reopening the table in a fresh process to confirm on-disk persistence has not been done** —
  everything so far ran within the same process that created the table. Worth doing before
  treating persistence as fully proven, not just "the files exist and look structurally sane"
  (which was confirmed via directory listing — `chunks.lance/data/` with one ~298MB `.lance`
  file, `_transactions/`, `_versions/` with a manifest — Lance's standard on-disk format).
- **Rust project skeleton**: `cargo init` done. Current dependencies (after removing the
  candle/hf-hub/tokenizers set from §3.1): `lancedb`, `arrow`, `parquet`, `serde`/`serde_json`,
  `tokio`, `reqwest`, `anyhow`, `futures-util`. `lancedb` required the `protoc` compiler installed system-wide to
  build (native toolchain dependency, not a pip-wheel-style silent install) and needed either the
  `remote` feature enabled or a version pin to work around a `lancedb-0.38.0` compile error
  (`Error::Http` variant not found — likely a feature-gated enum variant issue).
- **Python environment**: managed via `uv` (replacing plain `pip`/`venv` — faster, pip-compatible,
  same pinned `requirements.txt` semantics). `.venv\` created via `uv venv --python 3.12`,
  installed via `uv pip install -r requirements.txt`. Pinned dependencies (deliberately minimal,
  fragile `trust_remote_code` compatibility — **do not bump without re-verifying the model still
  loads**): `transformers==4.38.2`, `sentence-transformers>2.6.0,<3.0.0`, `einops`,
  `huggingface-hub>=0.20.0,<0.23.0`. `lancedb`/`pyarrow` deliberately excluded from this list — not
  the sidecar's job, LanceDB access is Rust's.
- **Project tooling, current and working**: `.gitignore` (excludes `venv`/`.venv`, `models/`,
  `db/*.parquet`, `target/`, `.cache/`, etc.), `.gitattributes` (LF for `.rs`/`.toml`, CRLF for
  `.ps1`/`.bat`, binary for model/data formats), bilingual `README.md` (Windows + macOS/Linux
  commands for every setup step), and parallel PowerShell/bash dev utility script pairs:
  - `tree.ps1` / `tree.sh` — write `STRUCTURE.txt` (plain text, not Markdown), always show
    directories, filter out large binary file extensions, collapse `models\` (HF cache internals
    shown as a placeholder, not recursed into — avoids the tree becoming an unreadable HF cache
    dump).
  - `init-structure.ps1` / `init-structure.sh` — recreate gitignored **directories only** (never
    files — those come from `git clone`) from `STRUCTURE.txt` on a fresh clone. The bash version
    explicitly strips trailing `\r` from lines to handle a Windows-generated (CRLF) `STRUCTURE.txt`
    correctly — a real bug that was caught and fixed, not hypothetical.

---

## 5. Open decisions — resolve with the user, do not assume either way

1. **RESOLVED.** `chunk_id`/`id` confirmed 100% identical across all 70,163 rows (verified via
   `examples\checkDuplicateColumns.rs`, not assumed from a single sampled row).
   `content_hash`/`_content_hash` confirmed to mismatch on every single row — not duplicate
   columns at all, two genuinely different hashes (`chunk_id` is a truncated prefix of
   `content_hash`; `_content_hash` is a different, likely file-level hash — pattern observed but
   not fully confirmed at the source-data level).
   **Decision: keep all four columns in the ingested LanceDB table** (`chunk_id`, `id`,
   `content_hash`, `_content_hash`) — explicitly chosen for forward-compatibility with future
   dataset extensions/re-ingestion, not for lack of a technical answer.
   **`chunk_id` is the designated LanceDB primary key.** `id` stays in the table as a kept column,
   just not the key.
2. **RESOLVED.** `llama-server --help` confirmed `-c`/`--ctx-size` and `--port` are current, valid
   flag names for the installed build — not deprecated, not renamed. README's documented command
   is accurate as-is.
3. **Streaming support on `llama-server`'s `/v1/chat/completions` — CONFIRMED WORKING.**
   `examples\testLlamaStreaming.rs` received real incrementally-streamed tokens via SSE
   (`data: {...}` lines), not just a single batched response.

No open decisions remain blocking forward progress. Everything past this point is "not yet
built," not "not yet decided."

---

## 6. Ordered next steps

1. Build the actual Python sidecar script (distinct from `loadJina.py`, which is a pre-flight test
   only) — implement the stdin/stdout JSON-line IPC protocol described in §2, reusing
   `loadJina.py`'s validated model-load path (including the `HF_HOME`-before-import pattern).
2. Wire the IPC: Rust spawns the sidecar, sends a test embedding request, confirms round-trip
   works end-to-end before building anything further on top of it. Test crash/restart behavior
   deliberately (kill the sidecar mid-run, confirm Rust detects and restarts it).
3. Wire real LanceDB ingestion into the actual application (not just `convertLanceDB.rs`'s
   proof-of-concept) — as a proper `src/` module, using `chunk_id` as the primary key, keeping all
   four id/hash columns per §5.1's resolved decision.
4. Replace the placeholder query vector (currently the first row's own vector, from
   `convertLanceDB.rs`) with a real embedded query from the sidecar — confirms the full retrieval
   path, not just the mechanism.
5. Build the real Rust HTTP client based on `testLlamaServer.rs`/`testLlamaStreaming.rs`'s
   confirmed request/response shapes (both non-streaming and streaming work).
6. Wire the CLI/REPL shell: first-run config prompt (endpoint URL + optional API key), config
   persistence to the path in §2, slash commands, and the full end-to-end flow: user query →
   sidecar embeds it → LanceDB hybrid search → prompt assembly with retrieved context → LLM call
   → response to the user.

---

## 7. Onboarding automation (setup.ps1 / setup.sh)

Both scripts exist for a fresh clone to get from zero to a ready environment. Idempotent — every
step checks a concrete on-disk signal (directory existence, `.venv` presence, the HF cache folder,
the `.gguf` file, `chunks.lance`) before deciding to skip, not just "did this run before."
Step order: directory structure -> Python venv/deps -> embedding model -> LLM model -> Rust build
-> parquet/LanceDB sanity checks -> **hard wait for the user to start `llama-server` manually**
(prints the exact command, blocks on Enter, does not guess readiness) -> connectivity test ->
summary table of OK/SKIPPED/FAILED per step.

- **`setup.ps1` — CONFIRMED WORKING** on the actual Windows dev machine, full run, correct
  skip-detection on a second run.
- **`setup.sh` — NOT YET VALIDATED on a real, clean macOS/Linux machine.** It was run once via
  WSL against the *same Windows-built project folder* (shared `.venv`, `target/`, `models/`) and
  failed — but that failure was caused by cross-environment contamination (a Windows-layout
  `.venv` is structurally incompatible with Linux's venv layout; `target/` had Windows-compiled
  artifacts), not a bug in the script itself. Two real bugs *were* found and fixed during this:
  the script had CRLF line endings (bash doesn't tolerate `\r`, now fixed and `.gitattributes` has
  an explicit `*.sh text eol=lf` rule to prevent recurrence), and it now self-heals CRLF on the
  other shell scripts it calls (`init-structure.sh`, `tree.sh`) as a defensive first step.
  **Still needs an actual test on a genuine clean environment** (a real Mac/Linux machine, or a
  WSL-native clone at a WSL-native path like `~/LoomisCLI`, not `/mnt/c/...`) before it can be
  considered confirmed — the WSL-into-Windows-folder run doesn't count as a valid test of it.

---

## 8. Retrieval quality — not evaluated

Everything confirmed so far proves the *mechanism* works (data goes into LanceDB, a search
returns rows, `llama-server` returns real completions). **Whether the hybrid vector+FTS search
actually returns good, relevant results for real queries has not been evaluated at all** — the
only search performed used a placeholder query (a row's own vector), which trivially returns
itself as the top match and proves nothing about real-world retrieval quality. This remains
untested until real queries (via the sidecar, once built) are tried against the corpus.

---

## 9. Guardrails — apply throughout, don't relax these for convenience

- Don't re-attempt Rust-only embedding inference (§3.1) without a new, explicit decision to
  revisit it.
- Don't re-attempt custom linker configuration (§3.2).
- Don't replicate LanceDB access in Python — that's Rust's job, kept deliberately separate from
  the sidecar's narrow scope.
- Don't persist chat history anywhere.
- Keep the LLM HTTP client provider-agnostic (OpenAI-compatible shape), not llama.cpp-specific,
  even though llama.cpp is the current target.
- Don't silently pin or change dependency versions (Rust or Python) to work around a compile/load
  error — surface the actual failure and fix the real cause, or flag it as a decision point.
- Don't hardcode machine-specific absolute paths (e.g. a specific Windows username) into any code
  that will ship as part of the sidecar or CLI — use `__file__`-relative resolution, matching the
  pattern already established in `loadJina.py`/`loadLlamaQ8.py`. This was tried once (a hardcoded
  path) and explicitly reverted.
- Don't design the LanceDB schema around assumed column names — the real schema is documented in
  §4, use it, and resolve §5.1 before finalizing.
- Project spec documents (if referenced elsewhere, e.g. `prompts/` folder contents) are working
  documents, not sources of truth for constraint-checking — don't flag mismatches against them as
  errors.