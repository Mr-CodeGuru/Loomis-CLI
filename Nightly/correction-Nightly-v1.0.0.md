# correction-Nightly-v1.0.0.md

Tracks required and optional fixes discovered while stabilizing LoomisCLI on the `Nightly`
branch. Each entry is dated to the evidence that surfaced it, not guessed. `Nightly` accumulates
fixes until the system is judged stable, at which point it merges to `main` as `v2.0.0`.

Version format: `v1.i.j` — bump `j` for a correction/fix commit, bump `i` for a structural change
(new component, changed architecture decision). Reset both when merged to `main` as `v2.0.0`.

---

## MUST FIX — blocking, affects correctness or stability

1. **Prompt assembly likely isn't giving the LLM the retrieved snippets' actual content.**
   Evidence: a real test run (`"Write a Python function that recursively walks a directory tree
   and returns all .py files, following the conventions used elsewhere in this codebase..."`)
   retrieved 5 genuinely relevant snippets (`walk_dir`, `_find_recursive`, `getFilesForName`,
   `iter_source_files`, `recursive_glob`), but the generated code was generic `os.walk()`
   boilerplate with no traceable resemblance to any of the 5 sources' actual implementation
   style. Either the retrieved code text isn't being included in the context sent to the LLM
   (only filenames/summaries), or it's included but the model isn't using it. **Action: inspect
   the actual prompt payload sent to `llama-server` for a test query — confirm whether full
   snippet source code is present in it or not, before assuming this is a model-capability
   problem instead of a plumbing bug.**

2. **Model hallucinates explanations that don't match its own generated code.** Same test run:
   the response claimed the code uses `glob.glob()` "to filter files based on their extensions,"
   but the actual generated function never calls `glob` at all — it uses `file.endswith('.py')`,
   and `import glob` is dead code. This is a real correctness issue in the generation output
   (self-inconsistent explanation), separate from the retrieval issue above. **Action: decide
   whether this is tolerable for a 1B model (may need a stronger prompt structure telling the
   model to only describe what it actually wrote) or requires a post-generation consistency
   check.**

3. **LanceDB persistence has never been confirmed across a fresh process.** Everything tested so
   far queried the table within the same process that created it. **Action: write a minimal
   reopen-and-query test in a separate `cargo run` invocation before trusting on-disk persistence
   is actually correct**, not just "the files exist and look structurally sane."

4. **`setup.sh` has not been validated on a genuine clean macOS/Linux environment.** The only run
   so far was WSL against a Windows-built project folder, which is not a valid test (cross-
   environment `.venv`/`target` contamination, not a script bug). **Action: run it on an actual
   clean machine, or a WSL-native clone at a WSL-native path, before treating it as confirmed
   working the way `setup.ps1` already is.**

---

## MUST UPDATE — not yet built, blocking further real progress

5. The actual Python sidecar script (long-lived stdin/stdout JSON-line process) — doesn't exist
   yet. `loadJina.py` is a one-shot test only.
6. Rust↔sidecar IPC wiring, including deliberate crash/restart testing.
7. Real LanceDB ingestion code in `src/` (not just `convertLanceDB.rs`'s proof-of-concept),
   applying the resolved column decisions: `chunk_id` as primary key, all four id/hash columns
   kept.
8. Replacing the placeholder query vector (a row's own vector) with a real sidecar-embedded query
   — required before retrieval quality can be judged fairly at all, since the current test
   pipeline may already be doing this (the walk_dir test above implies query embedding is
   working) — **confirm whether steps 5-6 are actually already done given that test ran, and
   update this document's "done" status accordingly rather than leaving it ambiguous.**
9. Real Rust HTTP client for `llama-server`, built on the confirmed request/response shapes
   (`testLlamaServer.rs`/`testLlamaStreaming.rs`).
10. CLI/REPL shell: first-run config prompt, config persistence, slash commands.

---

## CAN UPDATE — optional, quality-of-life, not blocking

- `_content_hash`'s actual meaning is inferred (likely file-level hash vs. `content_hash`'s
  chunk-level hash) but never confirmed against the original data-generation pipeline. Not
  blocking since both columns are kept regardless, but worth documenting properly if the
  original ingestion scripts/notes are ever available.
- Whether `lancedb`'s `remote` feature or the specific `0.38.0` version pin was the actual fix for
  the earlier `Error::Http` compile error was never isolated — both were tried close together.
  Worth a clean-room retest if `lancedb` is ever upgraded, to know which one actually matters.
- Distance score interpretation (`dist: 189.12`–`189.63` in the walk_dir test) — all 5 results
  clustered tightly. Unclear whether that's expected behavior for this embedding model/metric or
  a sign of weak discrimination between strong and weak matches. No baseline established yet for
  what "good separation" looks like.
- Retrieval quality more broadly has only been spot-checked with one real query so far. A larger,
  structured test set (the 5 code-generation prompts already drafted in conversation) hasn't been
  run systematically or logged anywhere durable.

---

## Stabilization criteria for merging to `main` as v2.0.0

Not yet defined precisely — placeholder until the user specifies what "perfectly stable" means
concretely (e.g. all MUST FIX items resolved + N consecutive clean test runs + retrieval quality
judged acceptable across the 5-prompt test set). Update this section once criteria are agreed,
rather than merging on a vague/subjective judgment call.