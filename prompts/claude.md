# Build Prompt: LoomisCLI — Continuation (v2)

## Current state (verified facts — don't re-derive or re-decide these)

- Project root: `C:\Users\Aman.Yadav\Desktop\Projects\LoomisCLI\`
- Rust: `cargo init` has been run. `Cargo.toml` and `src/main.rs` exist (default/near-empty
  skeleton). No dependencies added yet, no application logic written yet.
- Python venv exists with pinned deps: `transformers==4.38.2`,
  `sentence-transformers>2.6.0,<3.0.0`, `einops`, `huggingface-hub>=0.20.0,<0.23.0`.
- `loadModels\loadJina.py` exists — a standalone script that sets `HF_HOME` to
  `<project_root>\models` (via `Path(__file__).parent.parent`, portable, not hardcoded) and
  attempts to load `jina-embeddings-v2-base-code` with `trust_remote_code=True`, embedding a
  sample string and checking for a 768-dim output.
  **Not yet confirmed: whether this script actually passed on last run.** An earlier run produced
  a duplicated cache (both `models\hub\...` and flat `models\models--...`) caused by setting both
  `cache_folder` and `HF_HOME` simultaneously — that bug is fixed (only `HF_HOME` is set now), the
  cache was cleared, and a re-run was initiated, but the pass/fail result has not been reported
  back. **Do not assume this passed. Re-run it and check the actual output before proceeding past
  step 1 below.**
- `embeddings.parquet` exists at `db\embeddings.parquet`. Schema still not inspected.
- Project has `.gitignore` (excludes venv, models/, db/*.parquet, target/, etc.) and
  `.gitattributes` (normalizes line endings: LF for `.rs`/`.toml`, CRLF for `.ps1`/`.bat`, binary
  for model/data formats) in place.
- Two utility PowerShell scripts exist for documentation/scaffolding purposes only (not part of
  the application itself): `tree.ps1` (writes `STRUCTURE.md`, shows directories always, filters
  large file extensions, collapses `models\`/similar cache dirs without recursing into them) and
  `init-structure.ps1` (recreates gitignored directories from `STRUCTURE.md` on a fresh clone —
  directories only, never files, since files come from `git clone`).
- `llama-server` (llama.cpp) serving `Llama-3.2-1B-Instruct-Q8_0.gguf` with an OpenAI-compatible
  endpoint — start/verify manually before wiring the Rust HTTP client against it.

## Architecture (locked, do not re-litigate)

- **Rust** owns: CLI/REPL, slash commands, config persistence, LanceDB access (native `lancedb`
  crate), HTTP client to the OpenAI-compatible chat endpoint, spawning/managing the Python sidecar
  subprocess.
- **Python sidecar** (not yet built — `loadJina.py` is a pre-flight test, not the sidecar itself)
  owns exactly one thing: loading `jina-embeddings-v2-base-code` once and embedding query text on
  request. Nothing else — no LanceDB access, no reranking, no LLM calls.
- **IPC**: long-lived Python subprocess, newline-delimited JSON over stdin/stdout, versioned
  payload (`"v": 1`), Rust detects a dead process and restarts it, read timeout instead of
  indefinite block.
- Conversation history stays ephemeral, never persisted. Only endpoint config
  (`~/.loomiscli/config.json` / `%USERPROFILE%\.loomiscli\config.json`) persists.
- `llama-server` is started manually by the user; the CLI is not a daemon manager for it.

## Ordered next steps

1. **Confirm `loadModels\loadJina.py` actually passes** (`PASS: embedding dimension matches
   expected 768`). This is a gate — if it fails, diagnose the real error before continuing.
2. **Inspect `db\embeddings.parquet`'s real schema** — column names, vector dtype/dimensionality,
   any FTS-ready text column. Don't design the LanceDB table around assumptions.
3. **Add Rust dependencies** to `Cargo.toml` — check current versions on crates.io rather than
   guessing: `lancedb`, `reqwest` + `tokio`, a CLI/REPL crate (`rustyline` or `crossterm`),
   `serde`/`serde_json`.
4. **Build the LanceDB ingestion step in Rust** using the confirmed schema from step 2; confirm a
   basic vector search query returns rows before moving on.
5. **Build the actual Python sidecar script** (distinct from `loadJina.py`) implementing the
   stdin/stdout JSON-line protocol, reusing the validated model-load path.
6. **Wire the IPC**: Rust spawns the sidecar, sends a test embedding request, confirms round-trip
   end-to-end before building anything on top of it.
7. **Build the `llama-server` HTTP client** — confirm the server responds to a manual test request
   first; verify whether streaming is actually supported rather than assuming.
8. **Wire the CLI/REPL shell**: first-run config prompt, config persistence, slash commands, and
   the full flow (query → sidecar embeds → LanceDB search → prompt assembly → LLM call → response).

## Guardrails (carried over, still apply)

- Don't add a Rust ML inference dependency to avoid the Python sidecar — it's intentional.
- Don't replicate LanceDB access in Python.
- Don't persist chat history.
- Keep the LLM client provider-agnostic (OpenAI-compatible), not llama.cpp-specific.
- Don't silently pin/change dependency versions to work around an error — surface the actual
  failure first.
- Don't hardcode machine-specific absolute paths (e.g. a specific Windows username) into any code
  that will actually ship as part of the sidecar or CLI — that pattern was explicitly tried and
  reverted for `loadJina.py` in favor of a `__file__`-relative path.