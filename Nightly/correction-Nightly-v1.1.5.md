# correction-Nightly-v1.1.5.md

Tracks required and optional fixes discovered while stabilizing LoomisCLI on the `Nightly`
branch. Each entry is dated to the evidence that surfaced it, not guessed. `Nightly` accumulates
fixes until the system is judged stable, at which point it merges to `main` as `v2.0.0`.

---

## ISSUE SURFACED — Naive word-matching causes unprompted code generation

- **Symptom / User Observation**:
  The assistant appeared to "generate anything even though no one tells it and it just match word."
  Heuristic word-matching lists (e.g. matching keywords like `checksum`, `def `, `class `, `function to`)
  forced code generation even on conceptual explanations or general questions (e.g. `"explain how a checksum works"`).
  Conversely, crude heuristics broke when the model encountered unexpected phrasing patterns.
- **Root Cause**:
  Relying on hardcoded string allow-lists or substring keyword matching is brittle and lacks semantic
  understanding. It confuses queries discussing code concepts conceptually with requests to write executable code.

---

## ARCHITECTURE CORRECTION — Pure LLM Intent Routing & Regeneration Pipeline

Implemented the strict two-phase semantic pipeline requested:

```
[User Prompt]
      │
      ▼
[1. LLM Intent Classification] ──► {CODE or CHAT}
      │
      ├───────────────────────────────┬───────────────────────────────┐
      ▼                               ▼                               ▼
[Intent == CHAT]               [Intent == CODE]
(Non-code query)               (Code request detected)
      │                               │
      ▼                               ▼
[2. DO NOT RAG]                [2. RAG IT]
(Vector search bypassed)       (Embed query -> Search LanceDB for Top-K chunks)
      │                               │
      ├───────────────────────────────┴───────────────────────────────┐
      │                                                               │
      ▼                                                               ▼
[3. Send to LLM for Regeneration] ◄───────────────────────────────────┘
   - CHAT: Conversational system prompt + query + session history (Zero code blocks)
   - CODE: RAG grounded system prompt + retrieved snippets + query + session history
      │
      ▼
[Streaming Response to Terminal]
```

### Detailed Pipeline Components

1. **Pure LLM Intent Classification (Zero Word-Matching)**:
   - Located in [`src/llm/client.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/llm/client.rs) (`classify_code_intent`).
   - All hardcoded substring pattern checks (`is_explicit_code_request`) have been **completely removed**.
   - Uses a high-precision few-shot classification prompt conditioning `llama-3.2-1b` to evaluate semantic intent:
     - `CODE`: user explicitly requests code implementation, generation, refactoring, or code modifications.
     - `CHAT`: conversational queries, greetings, conceptual explanations (e.g. `"explain how a checksum works"`), history inquiries, or general non-code questions.
   - Temperature `0.0`, `max_tokens: 4`.

2. **Conditional Retrieval Gate**:
   - Located in [`src/cli/session.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/cli/session.rs) (`handle_query`).
   - If `intent == CodeIntent::Chat`: RAG is completely bypassed (`chunks = Vec::new()`).
   - If `intent == CodeIntent::Code`: RAG runs embedding via Python sidecar and LanceDB vector search.

3. **Regeneration Stage**:
   - Prompt assembled via `build_rag_messages()` in [`src/llm/prompt.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/src/llm/prompt.rs).
   - Injects bounded ephemeral session history (in-memory only, dropped on exit).
   - If `CHAT`: generates concise, helpful answers without dumping unwanted code.
   - If `CODE`: grounds output directly in retrieved repository snippets with explicit symbol/file citations.

---

## EMPIRICAL VERIFICATION (v1.1.5 Suite)

Executed [`examples/testConversationHistory.rs`](file:///Users/aman/Desktop/EvaProjects/Loomis/CLI/LoomisCLI/examples/testConversationHistory.rs) against live `llama-server` and LanceDB:

### Part 1: Semantic Intent Classification Matrix (18/18 PASS — 100% Accuracy)

| Query | Expected | LLM Output | Verdict |
|---|---|---|---|
| `"give me code about m5 checksum"` | `CODE` | `CODE` | **PASS** (Code request with algorithm name) |
| `"give me code for md5 checksum"` | `CODE` | `CODE` | **PASS** |
| `"show me code to read a file"` | `CODE` | `CODE` | **PASS** |
| `"need code for quicksort"` | `CODE` | `CODE` | **PASS** |
| `"how would you write a function to parse json"` | `CODE` | `CODE` | **PASS** |
| `"can you write a script that deletes old logs"` | `CODE` | `CODE` | **PASS** |
| `"refactor this loop into a list comprehension"` | `CODE` | `CODE` | **PASS** |
| `"Write a Python function that recursively walks a directory tree..."` | `CODE` | `CODE` | **PASS** |
| `"Extend the stop() function pattern from turtledemo into a full pause/resume state machine"` | `CODE` | `CODE` | **PASS** |
| `"now make it faster"` | `CODE` | `CODE` | **PASS** (Multi-turn follow-up) |
| `"hello"` | `CHAT` | `CHAT` | **PASS** |
| `"first time here, so hello"` | `CHAT` | `CHAT` | **PASS** |
| `"what can you do"` | `CHAT` | `CHAT` | **PASS** |
| `"sooo, how are you?"` | `CHAT` | `CHAT` | **PASS** |
| `"what's my last prompt to you?"` | `CHAT` | `CHAT` | **PASS** |
| `"explain how a checksum works"` | `CHAT` | `CHAT` | **PASS (Coding term present, but NO code requested -> No RAG)** |
| `"what is the difference between list and tuple"` | `CHAT` | `CHAT` | **PASS (Conceptual Python question -> No RAG)** |
| `"what is the capital of France"` | `CHAT` | `CHAT` | **PASS** |

### Part 2: End-to-End Multi-Turn Pipeline Execution

1. **Turn 1 (Small Talk)**:
   - Query: `"sooo, how are you?"`
   - Classification: `[LLM Intent: Chat]`
   - RAG: **Bypassed**
   - Regeneration: Conversational, friendly greeting with 0 code blocks.
2. **Turn 2 (Session Context Awareness)**:
   - Query: `"what's my last prompt to you?"`
   - Classification: `[LLM Intent: Chat]`
   - RAG: **Bypassed**
   - Regeneration: `"Your last prompt to me was to ask how I'm doing."` (Accurately identified prior prompt).
3. **Turn 3 (Conceptual Explanation with Coding Keywords)**:
   - Query: `"explain how a checksum works"`
   - Classification: `[LLM Intent: Chat]`
   - RAG: **Bypassed** (No LanceDB vector search executed).
   - Regeneration: Clear mathematical explanation of block sums and message verification without unprompted code dumping.
4. **Turn 4 (Grounded Code Request)**:
   - Query: `"give me code about m5 checksum"`
   - Classification: `[LLM Intent: Code]`
   - RAG: **Initiated** (Retrieved 5 chunks from `test/pacman/util.py`, `django-main/django/db/models/functions/text.py`, etc.).
   - Regeneration: Grounded Python MD5 checksum functions (`mkmd5sum`, `getmd5sum`, `md5_checksum`, `_calculate_md5_checksum`) citing the retrieved snippets.

**Status**: **VERIFIED & RESOLVED in v1.1.5**.
