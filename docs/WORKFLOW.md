# Workflow — Fajar Lang Development

> Panduan workflow yang dioptimalkan untuk Claude Code + Claude Opus 4.6

## 1. Session Lifecycle

Setiap Claude Code session mengikuti lifecycle yang konsisten:

```
┌─────────────────────────────────────────────────────┐
│                   SESSION START                      │
│                                                      │
│  1. Auto-load CLAUDE.md                              │
│  2. Read PLANNING.md → current phase & sprint        │
│  3. Read TASKS.md → today's tasks                    │
│  4. Read RULES.md → coding conventions               │
│  5. Orient: "What is the next uncompleted task?"     │
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│              TASK EXECUTION LOOP                     │
│                                                      │
│  For each task:                                      │
│  ┌─────────────────────────────────────────────┐     │
│  │ 1. THINK  → analyze requirements (high effort)│   │
│  │ 2. DESIGN → write interface/types first       │   │
│  │ 3. TEST   → write test cases                  │   │
│  │ 4. IMPL   → implement to pass tests           │   │
│  │ 5. VERIFY → cargo test + cargo clippy          │   │
│  │ 6. UPDATE → mark task done in TASKS.md         │   │
│  └─────────────────────────────────────────────┘     │
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│                    SESSION END                       │
│                                                      │
│  1. Run full test suite: cargo test                  │
│  2. Run linter: cargo clippy -- -D warnings          │
│  3. Update TASKS.md with completion status            │
│  4. Note blockers in PLANNING.md if any              │
│  5. Commit changes with conventional commit msg      │
└─────────────────────────────────────────────────────┘
```

## 2. Task Execution Flow (TDD)

### Step 1: THINK (effort: high)

```
Read the task description in TASKS.md.
Check ARCHITECTURE.md for component contracts.
Check FAJAR_LANG_SPEC.md for language behavior.
Identify: What exactly needs to be built?
```

### Step 2: DESIGN (effort: high)

```
Write the public interface FIRST:
- pub fn signature
- pub struct fields
- pub enum variants
- doc comments

Do NOT write implementation yet.
```

### Step 3: TEST (effort: medium)

```
Write tests BEFORE implementation:
- Happy path tests
- Edge case tests
- Error case tests

All tests should FAIL at this point. This is correct.
```

### Step 4: IMPLEMENT (effort: medium)

```
Write minimal code to make tests pass.
Follow RULES.md strictly:
- No .unwrap() in src/
- No panics in library code
- Use thiserror for error types
```

### Step 5: VERIFY

```bash
cargo test                       # all tests pass
cargo clippy -- -D warnings      # no warnings
cargo fmt                        # consistent formatting
```

### Step 6: UPDATE

```
Mark task as [x] in TASKS.md
Move to next task
```

## 3. Effort Levels

| Effort | When to Use | Examples |
|--------|-------------|---------|
| `high` | Architecture decisions, complex algorithms | Parser design, autograd implementation |
| `medium` | Standard implementation, test writing | Lexer tokens, CLI subcommands |
| `low` | Simple boilerplate, formatting | Doc comments, import statements |

## 4. File Editing Rules

- **One file at a time** — complete one file before moving to next
- **Small changes** — prefer multiple small edits over one large rewrite
- **Test immediately** — run `cargo test` after every significant change
- **Commit often** — one commit per completed task

## 5. Error Handling Pattern

```
When a build error occurs:
1. Read the FULL error message
2. Check if it's a type error → fix types
3. Check if it's a borrow error → reconsider ownership
4. Check if it's a missing import → add use statement
5. If stuck → re-read ARCHITECTURE.md contracts
```

## 6. Documentation Pattern

Every public item must have documentation:

```rust
/// Tokenizes a Fajar Lang source string into a flat list of tokens.
///
/// # Arguments
/// * `source` - The complete source code as a UTF-8 string slice
///
/// # Returns
/// * `Ok(Vec<Token>)` - Successfully tokenized tokens including EOF
/// * `Err(Vec<LexError>)` - All lexing errors found (not just first)
///
/// # Examples
/// ```
/// let tokens = tokenize("let x = 42").unwrap();
/// assert_eq!(tokens[0].kind, TokenKind::Let);
/// ```
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>> {
```

## 7. Phase Transition Checklist

Before moving to next phase:

- [ ] All tasks in TASKS.md for current phase: **DONE**
- [ ] `cargo test`: 100% passing
- [ ] `cargo clippy -- -D warnings`: clean
- [ ] `cargo doc`: compiles cleanly
- [ ] `examples/` for current phase: all running correctly
- [ ] ARCHITECTURE.md: updated with any design changes
- [ ] PLANNING.md: current phase marked complete, next phase unlocked
- [ ] Git: all committed, branch merged to main

---

*Workflow Version: 1.0 | Optimized for: Claude Code + Opus 4.6*
