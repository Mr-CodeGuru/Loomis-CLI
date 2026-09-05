# LoomisCLI — Test Question Set (v1)

Structured queries for evaluating retrieval quality and code-generation quality separately, since
a bad result can come from either stage. Log actual results here as they're run — don't just note
pass/fail, note *which stage* looked weak, per the instructions under each section.

For every query, record:
- **Retrieved sources** (filenames + which snippet, if shown)
- **Distance scores** (to build a baseline for what "good" separation looks like over time)
- **Generated output** (or retrieved match, for lookup-style queries)
- **Verdict**: does retrieval look relevant? does generation actually use it, or ignore it?

---

## Part A — Retrieval mechanics (simple lookup, isolate retrieval from generation)

Use these to check retrieval alone is working, before generation quality becomes a variable.

1. `function that stops a running loop`
   — Pure semantic query, no exact keyword overlap expected. Should surface something like a
   `stop()`-style function via vector similarity, not keyword match.

2. `def stop`
   — Near-exact literal match. Sanity floor: FTS/keyword side should trivially find this.

3. `how to handle a KeyError when accessing a dictionary`
   — Natural-language phrasing, not code-like. Tests whether the embedding model bridges
   conversational phrasing to code/docstring content.

4. `turtle graphics animation`
   — Domain-specific vocabulary. Tests whether retrieval correctly narrows to the relevant module
   subset rather than generic Python results.

5. `recursive function with memoization`
   — Abstract programming-pattern query with many possible surface forms in real code
   (decorators, `functools.lru_cache`, manual dict caching). Tests generalization across
   implementation styles of the same underlying concept.

---

## Part B — Code generation (retrieval + synthesis, harder to satisfy)

Each has a visible failure signature: generated code that doesn't reflect retrieved
style/conventions signals bad retrieval-to-generation plumbing; generic/textbook-looking output
signals the model isn't using retrieved context at all.

1. `Write a Python function that recursively walks a directory tree and returns all .py files,
   following the conventions used elsewhere in this codebase for path handling.`
   — **Already run once.** Retrieval was good (5 genuinely relevant snippets); generation was
   generic `os.walk()` boilerplate that didn't reflect any retrieved source's actual style, and
   the explanation described code (`glob.glob()` usage) that wasn't actually in the output. Rerun
   after investigating whether full snippet source is actually reaching the LLM's prompt.

2. `Extend the stop() function pattern from turtledemo into a full pause/resume state machine for
   an animation loop.`
   — Tests retrieval of a *specific* known function plus meaningful extension beyond it, not just
   echoing it back.

3. `Implement a custom context manager that logs entry/exit timing, following whatever
   context-manager patterns already exist in this codebase.`
   — Tests precision retrieval on a fairly specific structural pattern (`__enter__`/`__exit__` or
   `@contextmanager`).

4. `Given the error-handling style used in this codebase, write a function that safely parses a
   config file and raises a custom exception with a helpful message on failure.`
   — Tests whether retrieval surfaces actual exception-handling conventions rather than generic
   try/except boilerplate.

5. `Refactor a hypothetical function that uses nested loops for a Cartesian product into something
   more idiomatic, based on patterns you can find in this codebase (e.g. itertools usage).`
   — Tests retrieval's ability to surface idiomatic/library-usage patterns and whether generation
   actually applies them.

---

## Results log

| # | Part | Query (short) | Retrieval verdict | Generation verdict | Notes |
|---|------|----------------|--------------------|----------------------|-------|
| G1 | Greeting | `hello` | N/A (bypassed via intent router) | Excellent — polite greeting without code block | Intent classifier routed to `QueryIntent::Greeting`. Skipped redundant LanceDB vector search. Model returned: `"Hello. How can I assist you today?"` without dumping code blocks. |
| G2 | Greeting | `hi, who are you and what can you do?` | Good (5/5 relevant assistant snippets, dist ~165.0-167.4) | Good — conversational answer citing assistant modules | Explains Loomis capabilities as a terminal code assistant, answering directly without unrequested code generation blocks. |
| A1 | A | `function that stops a running loop` | Good (5/5 relevant, dist 187.17-193.38) | Good — conceptual explanation without code dump | Retrieved `turtledemo/round_dance.py:stop`, `InMoov2:stopit`, `compiler:loop_exit`, `qa:stop_node`, `psychopy:quit`. Explains how `stop()` sets `running = False`. |
| A2 | A | `def stop` | Good (5/5 exact/near-exact matches, dist 180.59-185.97) | Good — explains matching symbols | Retrieved `round_dance.py:stop`, `InMoov2:stopit`, `stopTracking`, `tkinter:stop`, `elogger:_stop`. Describes how each handles termination without generating unrequested code. |
| A3 | A | `how to handle a KeyError when accessing a dictionary` | Good (5/5 relevant dictionary/key error chunks, dist 217.68-218.77) | Good — explains pattern with minimal targeted example | Retrieved `main.py:_get_if_exist`, `exceptions.py:TypeOfKeyDoesNotExist`, `test_dicts.py`, `config.py:KeyNotFound`. Explains exception handling patterns in the repository. |
| A4 | A | `turtle graphics animation` | Excellent (5/5 domain specific, dist 195.67-197.96) | Good — describes classes and animation usage | All 5 retrieved chunks from `cpython-main/Lib/turtle.py` and `turtledemo/` (`start`, `TNavigator`, `Star`, `ColorTurtle`, `fractalcurves:main`). Perfect domain narrowing. |
| A5 | A | `recursive function with memoization` | Excellent (5/5 exact pattern matches, dist 205.82-207.50) | Good — explains decorator and caching patterns | Retrieved `rekall_lib:memoize`, `pylint:Fibonacci`, `pegen:memoize_left_rec`, `CodegenRust:memoize`, `pylint:cached_fibonacci`. Clearly explains memoization decorators in the codebase. |
| B1 | B | `walk .py files` | Good (5/5 relevant, dist 189.12-189.63) | Good — synthesizes functions adapting repo conventions | Retrieved `app/file_download.py:walk_dir`, `filesystem.py:_find_recursive`, `pygettext.py:getFilesForName`, `check_c_api_usage.py:iter_source_files`, `todo.py:recursive_glob`. Generates `get_python_files()` adapting `walk_dir` and `recursive_glob`. |
| B2 | B | `Extend stop() pattern into pause/resume` | Good (5/5 relevant animation/control chunks, dist 181.99-185.91) | Good — generates complete state machine class | Retrieved `round_dance.py:stop`, `minimal_hanoi.py:play`, `pause_resume.py:PauseAnimation`. Generates `PauseAnimation` class with `toggle_pause`, `pause`, `resume`, `stop`. |
| B3 | B | `Implement custom context manager logging timing` | Excellent (5/5 exact structural matches, dist 194.92-195.94) | Good — generates custom context manager with entry/exit timing | Retrieved `roam/utils.py:Timer`, `ftscalingbench.py:MyContextManager`, `django_mysql:StopWatch`. Generates `CustomContextManager` using `__enter__` and `__exit__` with elapsed timing calculation. |
| B4 | B | `safely parse config file and raise custom exception` | Good (5/5 relevant config error chunks, dist 181.84-184.73) | Good — creates custom error subclass and safe parser | Retrieved `cfg.py:ConfigFileParseError`, `configparser.py:ParsingError`, `config_file_parser.py:_ConfigurationFileParser`. Generates `ConfigFileParseError` subclass and `safe_parse_config_file()` function. |
| B5 | B | `Refactor nested loops for Cartesian product` | Excellent (5/5 itertools / cartesian product chunks, dist 175.47-179.66) | Good — refactors using `itertools.product` | Retrieved `kernel-generator.py:product`, `pandas:cartesian_product`, `multipleloop.py:_outer`, `tqdm:product`, `combination.py:combination`. Generates idiomatic `cartesian_product(*args)` using `itertools.product(*args)`. |
