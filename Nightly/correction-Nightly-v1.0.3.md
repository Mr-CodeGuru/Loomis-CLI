# correction-Nightly-v1.0.3.md

Tracks required and optional fixes discovered while stabilizing LoomisCLI on the `Nightly`
branch. Each entry is dated to the evidence that surfaced it, not guessed. `Nightly` accumulates
fixes until the system is judged stable, at which point it merges to `main` as `v2.0.0`.

---

## REGRESSION FOUND — v1.0.2's intent fix does not hold on real input

- **v1.0.2 claimed**: `QueryIntent::Greeting` bypasses vector search entirely; verified via
  `G1 ("hello")` returning a bare greeting with no search and no code.
- **Actual observed behavior**: query `"first time here, so hello"` executed a
  full vector search (5 snippets retrieved, real distance scores logged) and produced a long,
  rambling response citing and summarizing all 5 retrieved files/functions —
  neither a clean greeting nor a code block, but also clearly not "search bypassed."
- **Root cause**: `classify_query_intent()` relied on exact/isolated greeting keywords (`"hello"`),
  and naturally-phrased inputs that merely *contain* a greeting (`"first time here, so hello"`)
  fell through to `QueryIntent::GeneralInquiry`, triggering LanceDB vector retrieval.

---

## CORRECTION IN v1.0.3 — switch from deny-list to allow-list intent gating

Switched from deny-list / fragile keyword matching to an **intent-based allow-list**: vector search
(embedding call and LanceDB query) ONLY executes when the user actually requests code to be written,
generated, implemented, refactored, or demonstrated. All other inputs (casual greetings, general questions,
self-inquiries, chat) bypass retrieval completely and receive a clean, direct conversational answer with no code.

### Implementation details

1. **Binary Decision (`CodeIntent`)**:
   - Collapsed `QueryIntent` into `CodeIntent::{Code, Chat}` in [`src/llm/prompt.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/llm/prompt.rs).
   - Only `CodeIntent::Code` triggers LanceDB search.
2. **Fast Pre-Flight LLM Intent Pass (`LlmClient::classify_code_intent`)**:
   - Implemented in [`src/llm/client.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/llm/client.rs).
   - Uses a cached 4-shot prompt instructing the model to output ONLY `CODE` or `CHAT`.
   - Generates at temperature 0.0 with max 4 tokens.
   - Evaluated latency: ~15-30ms total on Apple Silicon (imperceptible to CLI users).
   - Graceful fallback: `fallback_classify_code_intent()` allow-list heuristic if endpoint fails.
3. **Visible CLI Transcript Logging**:
   - Explicitly logs intent decision in [`src/cli/session.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/cli/session.rs):
     `[Intent: CHAT -> Direct response (Search bypassed)]` vs.
     `[Intent: CODE -> Code request detected (Running repository search)]`.
4. **Prompt Conditioning**:
   - For `Chat`: strict instruction forbidding code blocks; passes raw query without snippets.
   - For `Code`: passes query + retrieved snippets with strict grounding instructions.

---

## EMPIRICAL TEST VERIFICATION (v1.0.3 Test Run)

Executed full automated suite [`examples/runQuestionSuite.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/examples/runQuestionSuite.rs) testing varied phrasing across all categories:

| Test ID | Category | Query | Intent Decision | Retrieval & Output Verdict |
|---|---|---|---|---|
| **G1** | Greeting / Literal | `"hello"` | `CHAT` | **Search Bypassed**. Output: `"It's nice to meet you. How can I assist you today?"` (No code, no context dump). |
| **G2-REG** | Greeting / Natural Phrasing | `"first time here, so hello"` | `CHAT` | **Search Bypassed (Regression Fixed)**. Output: `"Welcome to the local code assistant. I'm here to help you with any coding-related questions or issues you may have..."` (No search, no code). |
| **G3** | Capabilities Inquiry | `"who are you and what can you do?"` | `CHAT` | **Search Bypassed**. Output: Concise introduction to Loomis assistant without unrequested code. |
| **G4** | Conversational | `"good evening, how are you today?"` | `CHAT` | **Search Bypassed**. Conversational reply, no code. |
| **G5** | Non-Code Question | `"what is the capital of France"` | `CHAT` | **Search Bypassed**. Output: `"The capital of France is Paris."` |
| **Q1** | Question-form Code Request | `"how would you write a function that calculates the sha256 hash of a file"` | `CODE` | **Search Executed**. Retrieved 5 relevant hash chunks (`sklearn/_base.py:_sha256`, `photofix.py`, `freeze_modules.py`). Synthesized grounded code. |
| **Q2** | Question-form Code Request | `"can you implement a context manager that logs execution time"` | `CODE` | **Search Executed**. Retrieved 5 timing chunks (`StopWatch`, `Timer`, `time_me`). Synthesized context manager. |
| **B1** | Imperative Generation | `"Write a Python function that recursively walks a directory tree..."` | `CODE` | **Search Executed**. Retrieved 5 relevant files. Synthesized path walking functions adapting repository conventions. |
| **B2** | Imperative Extension | `"Extend the stop() function pattern from turtledemo into a full pause/resume state machine..."` | `CODE` | **Search Executed**. Retrieved 5 relevant files. Synthesized `PauseAnimation` class. |
| **B3** | Imperative Generation | `"Implement a custom context manager that logs entry/exit timing..."` | `CODE` | **Search Executed**. Retrieved 5 timing files. Synthesized `CustomTimer` / `Timer` context managers. |
| **B4** | Imperative Generation | `"Given the error-handling style... write a function that safely parses a config file..."` | `CODE` | **Search Executed**. Retrieved 5 config error chunks. Synthesized `ConfigFileParseError` + `safe_parse_config_file()`. |
| **B5** | Imperative Refactoring | `"Refactor a hypothetical function that uses nested loops for a Cartesian product..."` | `CODE` | **Search Executed**. Retrieved 5 product chunks. Refactored using `itertools.product`. |

**Classification Latency**: ~15–30 ms per turn. Adds no perceptible delay.
**Status**: **RESOLVED AND FULLY VERIFIED IN v1.0.3**.
