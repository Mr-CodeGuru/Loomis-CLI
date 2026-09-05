# correction-Nightly-v1.0.2.md

Tracks required and optional fixes discovered while stabilizing LoomisCLI on the `Nightly`
branch. Each entry is dated to the evidence that surfaced it, not guessed. `Nightly` accumulates
fixes until the system is judged stable, at which point it merges to `main` as `v2.0.0`.

Version format: `v1.i.j` — bump `j` for a correction/fix commit, bump `i` for a structural change
(new component, changed architecture decision). Reset both when merged to `main` as `v2.0.0`.

---

## CORRECTIONS & ENHANCEMENTS IN v1.0.2

### 1. Intent Routing & Unrequested Code Generation Fix
- **Evidence**: On simple greetings (e.g. `"hello"` or `"who are you?"`) or general conceptual questions (e.g. `"function that stops a running loop"`), Loomis executed a full vector search and forced code generation because the system and user prompts unconditionally instructed: `"Provide a single code solution adapting the codebase conventions above..."`.
- **Root Cause**: Lack of query intent classification. Every input was treated as a code generation task, causing the 1B model to fabricate Python functions even for `"hello"`.
- **Correction Applied**:
  - Implemented `QueryIntent` enum (`Greeting`, `GeneralInquiry`, `CodeGeneration`) and `classify_query_intent()` in [`src/llm/prompt.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/llm/prompt.rs).
  - **Greeting handling**: Bypasses LanceDB vector search, provides a polite conversational response, and strictly forbids code generation.
  - **General inquiry handling**: Executes vector retrieval, explains repository patterns and concepts citing file paths/symbols, and strictly forbids unrequested code blocks.
  - **Code generation handling**: Triggers only when the user explicitly requests code (`write`, `implement`, `refactor`, `extend`, `generate`, etc.), providing a single grounded implementation following repo conventions.
  - Updated [`src/cli/session.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/cli/session.rs) to use intent classification and skip vector search for greetings.
- **Status**: **RESOLVED / VERIFIED**.

### 2. LLM Completion Token Budget Increase
- **Evidence**: In [`QTest/TQ1v1.0.2.md`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/QTest/TQ1v1.0.2.md), responses containing both a synthesized code solution and an explanation were occasionally truncated mid-sentence due to a 1024 token limit.
- **Correction Applied**: Increased `max_tokens` from 1024 to 1536 in [`src/llm/client.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/llm/client.rs).
- **Status**: **RESOLVED / VERIFIED**.

### 3. Empirical Test Suite Execution (`TQ1v1.0.2`)
- **Evidence**: Ran full automated question suite ([`examples/runQuestionSuite.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/examples/runQuestionSuite.rs)) across all Part A (A1–A5), Part B (B1–B5), and greeting (G1–G2) queries against local `llama-server` and LanceDB.
- **Results**:
  - **G1 (`hello`)**: `QueryIntent::Greeting` detected. Vector search bypassed. Output: `"Hello. How can I assist you today?"` (No code generated).
  - **G2 (`who are you`)**: `QueryIntent::GeneralInquiry`. Explains assistant capabilities without unrequested code block.
  - **A1–A5 (Lookup queries)**: 100% top-5 relevant retrieval. System explains mechanisms citing symbols (e.g. `turtledemo:stop`, `_get_if_exist`, `rekall_lib:memoize`) without dumping unrequested code.
  - **B1–B5 (Code generation queries)**: 100% relevant retrieval. Cleanly synthesizes single implementations adapting repository conventions with path citations.
- **Status**: **VERIFIED & LOGGED IN `QTest/TQ1v1.0.2.md`**.
