# correction-Nightly-v1.0.1.md

Tracks required and optional fixes discovered while stabilizing LoomisCLI on the `Nightly`
branch. Each entry is dated to the evidence that surfaced it, not guessed. `Nightly` accumulates
fixes until the system is judged stable, at which point it merges to `main` as `v2.0.0`.

Version format: `v1.i.j` — bump `j` for a correction/fix commit, bump `i` for a structural change
(new component, changed architecture decision). Reset both when merged to `main` as `v2.0.0`.

---

## STATUS UPDATES ON v1.0.0 ITEMS

### 1. Prompt assembly snippet content & plumbing bug
- **Evidence / Investigation**: Built and ran an inspection harness (`examples/inspectPromptPayload.rs`)
  querying `llama-server` with the exact walk-dir inquiry. Confirmed that full snippet source code
  (up to 1500 chars per chunk) **is** present in the prompt payload sent to the model.
- **Root Cause**:
  1. *Plumbing Bug*: In `src/llm/prompt.rs`, the format string used literal escaped backslashes
     `\n```{}\\n{}\\n```\n` (`\\n`), causing literal `\n` characters to be emitted instead of
     real linebreaks right after the code fence.
  2. *Codebase Grounding*: 4 of the 5 retrieved snippets in the dataset (`pygettext.py`,
     `check_c_api_usage.py`, `file_download.py`, `utils.py`) *actually do* use `os.walk()`. The
     model was not ignoring the codebase; the retrieved codebase snippets heavily rely on `os.walk()`.
  3. *Prompt Structure*: The original prompt lacked negative constraints, leading the 1B model to
     blindly copy imports from snippets (`import glob`, `import datetime`) that were unused in its
     generated function.
- **Correction Applied**: Fixed code fence linebreaks in `src/llm/prompt.rs`, added strict instructions
  disallowing unused imports, and mandated explicit citation of snippet paths and symbols.
- **Status**: **FIXED (v1.0.1)**.

### 2. Model hallucinations (explanation vs. generated code)
- **Evidence / Investigation**: In the test run, the model imported `glob` and `datetime` and listed
  `glob.glob()`, `datetime`, and `collections.defaultdict` under "relevant functions used in this code"
  despite none of them being invoked in the output function.
- **Root Cause**: The prompt instructed the model to "cite the relevant file paths and functions"
  without constraining the explanation to the code actually generated, leading the 1B model to
  summarize every symbol found in the context window.
- **Correction Applied**: Updated `src/llm/prompt.rs` with explicit constraints:
  - Explain ONLY what was written in the generated code.
  - Do NOT list or describe functions/modules not present in the generated code.
  - Explicitly cite the specific file path and function symbol adapted.
  Verified with live completion: model now outputs clean code and attributes it directly to the
  retrieved file (`numpy-main/tools/ci/check_c_api_usage.py` and `app/file_download.py`).
- **Status**: **FIXED (v1.0.1)**.

### 3. LanceDB persistence across fresh process
- **Evidence / Investigation**: Executed `cargo run --example testLanceDbMethods` and
  `cargo run --example testRetrievalPipeline` in isolated, fresh process invocations.
- **Result**: Successfully connected to existing table at `dbe/lancedb/chunks.lance`, verified
  persisted row count of 70,163, and executed vector search returning nearest neighbors with
  expected distance metrics (~187.35).
- **Status**: **CONFIRMED & VERIFIED**.

### 4. `setup.sh` validation on clean macOS environment
- **Evidence / Investigation**: Validated directly on macOS (Darwin arm64). Confirmed syntax (`bash -n setup.sh`),
  directory fallback logic (`dbe/` and `models/`), BSD `sed -i ''` fallback handling, and Rust
  compilation steps.
- **Status**: **CONFIRMED & VERIFIED**.

---

## MUST UPDATE ITEMS (5–10) STATUS AUDIT

All items previously marked under "MUST UPDATE" in v1.0.0 have been confirmed implemented in the codebase:
- **5. Python sidecar script**: Implemented at `sidecar/embed.py` (persistent stdin/stdout JSON-line process, schema `v: 1`).
- **6. Rust↔sidecar IPC wiring**: Implemented at `src/sidecar/client.rs` with automatic process supervision, crash detection, and restart recovery verified via `examples/testSidecarIpc.rs`.
- **7. LanceDB ingestion in `src/`**: Implemented at `src/db/store.rs` (`VectorStore::connect_or_create()`).
- **8. Real sidecar-embedded query pipeline**: Wired in `src/cli/session.rs` and verified via `examples/testRetrievalPipeline.rs`.
- **9. Real Rust HTTP client for `llama-server`**: Implemented at `src/llm/client.rs` (`LlmClient` with streaming SSE and connection validation).
- **10. CLI/REPL shell**: Implemented at `src/cli/session.rs`, `src/cli/commands.rs`, and `src/config.rs` with first-run config prompting, persistent config file, slash commands (`/help`, `/clear`, `/stats`, `/config`, `/exit`).

---

## NEW OBSERVATIONS & REMAINING ITEMS (v1.0.1)

1. **Distance Metric Scaling**:
   - `dist: 187.35`–`194.34`: LanceDB vector search returns squared L2 / Euclidean distance by default.
   - For 768-dimensional unnormalized embeddings, L2 distance values in the ~180–195 range are
     expected. If cosine distance is preferred in the future, embeddings can be normalized or the
     metric specified explicitly during table creation / search.
2. **Context Window Token Budgeting**:
   - 5 snippets capped at 1,500 characters each ~ 1,875 tokens.
   - Fits comfortably within `llama-server -c 4096` without exceeding context limits.

---

## Stabilization Criteria for v2.0.0 Merge to `main`

1. [x] All MUST FIX items (1–4) resolved or verified with durable evidence.
2. [x] Core architecture components (5–10) implemented, wired, and tested end-to-end.
3. [x] End-to-end RAG REPL interactive session verified.
4. [ ] Run 5 multi-turn interactive session queries to confirm prompt stability across conversational turns.
