# LoomisCLI — TUI Frontend Build Prompt (NightlyTUI branch)

## Context

LoomisCLI already has a working, tested plain-REPL frontend (`src/cli/session.rs` +
`src/cli/formatter.rs`), with confirmed-working core logic: pure LLM intent classification
(CHAT vs CODE, 18/18 accuracy), sidecar IPC with crash recovery, LanceDB retrieval, streaming
`llama-server` responses, in-memory conversation history. None of that core logic is being
rewritten — this branch adds a new frontend on top of it.

## Decision: alternate mode, not a replacement

The TUI runs behind a `--tui` CLI flag. The existing plain REPL stays the default and stays
untouched. Rationale: the TUI introduces new, unproven surface area (rendering, event loop,
terminal compatibility) — gating it behind a flag means it can be developed and stabilized
incrementally without risking the already-working REPL. Do not remove or replace
`session.rs`'s REPL loop as part of this work.

Both frontends should share the same underlying core (intent classification, sidecar calls,
LanceDB search, `llama-server` client) — the TUI is a new rendering/input layer, not a parallel
reimplementation of the RAG pipeline. If sharing requires refactoring `session.rs` to expose its
core logic as callable functions/methods independent of its current stdout-printing REPL loop,
that refactor is in scope — but the refactor should preserve the existing REPL's behavior
exactly, verified by re-running the existing test suite (`testConversationHistory`,
`testCodeBlockFormatting`, etc.) against the unchanged `--tui`-less path after the refactor.

## Stack

- **`ratatui`** — widget/layout/rendering.
- **`crossterm`** — terminal backend (input events, raw mode).
- **`syntect`** — real syntax highlighting via actual language grammars. This replaces the
  current hand-rolled keyword-matching approach in `formatter.rs`'s Monokai highlighter — the
  hand-rolled version is fragile and language-specific; `syntect` handles arbitrary languages via
  real tokenization. Migrate the TUI's code rendering to `syntect` rather than porting the
  existing keyword-list approach as-is.
- **`pulldown-cmark`** — markdown parsing, so bold/italic/inline-code/lists render properly
  instead of showing raw markdown syntax characters.
- **`tui-textarea`** (or a custom `ratatui`-primitive-based input widget) — multi-line input with
  cursor movement and history recall (up-arrow for previous prompts).

Check current versions of all of these on crates.io before pinning — don't guess version numbers.

## Layout

```
+-----------------------------------------------+
| Scrollable conversation pane                   |
|  - user turns, assistant turns                 |
|  - streaming text updates live as tokens arrive|
|  - code blocks rendered via syntect, boxed     |
+-----------------------------------------------+
| Status bar: model name | context used | spinner while generating |
+-----------------------------------------------+
| Input box (multi-line capable)                 |
+-----------------------------------------------+
```

Optional, in scope if time allows but not blocking for a first working version: a
collapsible/expandable panel per assistant turn showing retrieved sources (file paths +
distance scores), matching the source-citation behavior already present in the plain REPL.

## Visual formatting fidelity — the actual bar to hit

"Like Claude Code" means full markdown rendering fidelity, not just colored code blocks. Specific
requirements:

- **Headers** (`#`, `##`, `###`) rendered with distinct bold/size-equivalent styling (terminal
  can't do font size, so use bold + color weight + possibly a leading marker to distinguish
  levels).
- **Bold**, *italic*, and `inline code` spans rendered with actual distinct styling (bold text,
  italic via terminal support where available, inline code with a background/color distinct from
  surrounding prose) — not left as raw `**`/`_`/`` ` `` characters.
- **Bullet and numbered lists** rendered with proper indentation and marker characters (`•` or
  similar for bullets, not raw `-`/`*`), correctly nested for sub-lists.
- **Blockquotes** rendered with a left border/indent marker, distinct from regular prose.
- **Horizontal rules** (`---`) rendered as an actual visual divider, not left as literal dashes.
- **Code blocks**: language-tagged rounded box (already planned), real `syntect` syntax
  highlighting, and — distinctly Claude-Code-like — a subtle visual separation between inline
  `code spans` and full fenced blocks so they're not visually confused.
- **Turn separation**: clear visual distinction between user and assistant turns (e.g. a
  left-margin marker or color accent per role, not just plain sequential text) so scrolling back
  through a long conversation is easy to scan.
- **Streaming/thinking indicator**: a visible state (spinner or similar) while waiting for the
  first token, distinct from the token-by-token rendering once streaming starts — Claude Code's
  feel comes partly from never leaving the user looking at a dead screen during generation.
- **Retrieved-source citations** (if the optional sources panel from the layout section is
  implemented): styled distinctly from the main response text, not just plain inline text — e.g.
  a dimmed/boxed reference list, similar to how the plain REPL already prints
  `Retrieved Context Sources:` but with proper TUI styling instead of plain stdout lines.

This raises `pulldown-cmark` from "handle basic bold/italic" to "drive full markdown-to-styled-
`ratatui`-widget rendering" — treat it as the actual rendering engine for all non-code prose, with
a `ratatui` `Text`/`Line`/`Span` tree built from the parsed markdown AST, not simple string
substitution for a couple of markdown tokens.

## Known hard parts — flag these explicitly during implementation, don't silently work around them

1. **Streaming into a TUI is harder than streaming into stdout.** Each token arriving means
   re-rendering (part of) the screen without flickering or losing scroll position. This needs
   `ratatui`'s immediate-mode redraw model driven by an async event loop
   (`tokio::select!` interleaving terminal input events, SSE token events, and periodic redraw
   ticks) — not a direct port of the current "print token, flush stdout" approach.
2. **The existing `StreamingCodeFormatter`'s fence-detection *logic* (identifying where a code
   block starts/ends within a token stream) is reusable — its *output mechanism* is not.** It
   currently returns ANSI-escaped strings for direct stdout printing; a `ratatui` app needs
   styled `Span`/`Line` objects instead. Port the fence-detection state machine, rewrite the
   output side.
3. **Terminal compatibility** — `ratatui`/`crossterm` behavior can vary across terminal emulators
   (especially relevant given this project's Windows + macOS dual-platform target). Test on both
   platforms before considering any milestone done, not just the platform being actively
   developed on.

## Ordered build steps

1. Add dependencies (`ratatui`, `crossterm`, and the others above) behind a scaffold — get a
   minimal TUI rendering (empty conversation pane + input box, no streaming, no LLM calls yet)
   wired behind `--tui`, and confirm the event loop takes input and can quit cleanly, before
   touching anything else.
2. Wire static (non-streaming) display of a full conversation turn — send a query, wait for the
   complete response, render it in the conversation pane. Proves the core-logic sharing works
   before adding streaming complexity on top.
3. Add token-by-token streaming rendering into the conversation pane.
4. Migrate code-block rendering to `syntect`, replacing the keyword-matching approach.
5. Add markdown rendering via `pulldown-cmark` for non-code formatting (bold, italic, lists).
6. Add the status bar (model name, context usage, generating indicator).
7. Add input history recall (up-arrow) via `tui-textarea` or equivalent.
8. Optional: retrieved-sources panel per turn.
9. Full regression pass: confirm the existing plain REPL (`--tui`-less path) still behaves
   identically to before this work started, via the existing test suite.

## Guardrails

- Don't remove or degrade the existing plain REPL as part of this work.
- Don't silently change core RAG/intent-classification logic while refactoring for shared use
  between frontends — if a change to `session.rs`'s structure is needed, it should be a pure
  refactor (same behavior, different code organization), verified against the existing test
  suite, not an opportunity to also "improve" the logic itself.
- Don't guess crate version numbers — check crates.io for current versions.
- Test on both Windows and macOS before considering any milestone complete.