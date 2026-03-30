# V11 "Genesis" — Full Production & Commercial Readiness Plan

## Context

Fajar Lang v9.0.1 "Ascension" is 100% production-ready at the code level. But a programming language needs more than working code — it needs **discoverability** (website), **developer experience** (IDE), **credibility** (benchmarks), **maturity** (self-hosting), **safety** (borrow checker), and **onboarding** (tutorials).

This plan covers all 6 initiatives with detailed tasks. Key discovery: most infrastructure already exists — the work is **polishing and publishing**, not building from scratch.

---

## Option 1: Website fajarlang.dev (Effort: 2-3 days)

### What Already Exists
- Full playground at `playground/` — Monaco editor, WASM execution, examples, sharing
- `build-playground.sh` — WASM build script
- `.github/workflows/docs.yml` — deploys mdBook + playground to GitHub Pages
- No custom domain (CNAME) configured

### Tasks

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 1.1 | Create landing page `website/index.html` with hero section, feature cards, code samples, benchmark summary, download links | `website/index.html` | ~300 | Opens in browser |
| 1.2 | Style with Tailwind CSS (CDN) — dark theme, responsive, gradient hero | `website/style.css` | ~200 | Mobile + desktop |
| 1.3 | Embed playground iframe or link to `/playground/` | `website/index.html` | ~20 | Playground loads |
| 1.4 | Add "Getting Started" section with install commands + first program | `website/index.html` | ~50 | Copy-paste works |
| 1.5 | Add feature comparison table (vs Rust/Go/Python/Zig) | `website/index.html` | ~50 | Table renders |
| 1.6 | Add CNAME file for custom domain `fajarlang.dev` | `CNAME` | 1 | DNS resolves |
| 1.7 | Update `docs.yml` to deploy website/ as root, playground/ as /playground, book/ as /docs | `.github/workflows/docs.yml` | ~30 | All 3 deploy |
| 1.8 | Add Open Graph meta tags (title, description, image) for social sharing | `website/index.html` | ~10 | Preview on Twitter/LinkedIn |
| 1.9 | Add Google Analytics or Plausible analytics script | `website/index.html` | ~5 | Tracking works |
| 1.10 | Test: full deployment — website + playground + docs all accessible | — | — | 3 URLs work |

**Dependencies:** Domain purchase (fajarlang.dev) — user action required.
**Alternative:** Use GitHub Pages at fajarkraton.github.io/fajar-lang (no domain purchase needed).

---

## Option 2: Tutorial Series (Effort: 2 days)

### What Already Exists
- `book/src/` — 130 markdown files, comprehensive reference docs
- `book/src/tutorials/` — 2 tutorials (embedded ML, kernel module)
- `book/src/demos/` — 4 demos (drone, MNIST, mini OS, package)
- 178 example .fj programs

### Tasks

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 2.1 | Tutorial: "Build a REST API in 5 Minutes" — step-by-step using http_server/route/start, request_json, db_open | `book/src/tutorials/rest-api.md` | ~200 | mdBook renders |
| 2.2 | Tutorial: "Real-Time IoT Dashboard" — MQTT sensor → regex validation → SQLite → WebSocket display | `book/src/tutorials/iot-dashboard.md` | ~200 | mdBook renders |
| 2.3 | Tutorial: "ML Image Classifier" — tensor create, Dense layer, softmax, training loop, accuracy | `book/src/tutorials/ml-classifier.md` | ~200 | mdBook renders |
| 2.4 | Tutorial: "Async Web Scraper" — async_http_get, regex_find_all, concurrent crawling with async_spawn/join | `book/src/tutorials/async-scraper.md` | ~150 | mdBook renders |
| 2.5 | Tutorial: "GUI Calculator App" — gui_window, gui_button with callbacks, gui_label for display | `book/src/tutorials/gui-calculator.md` | ~150 | mdBook renders |
| 2.6 | Update `book/src/SUMMARY.md` to include 5 new tutorials | `book/src/SUMMARY.md` | ~10 | Book builds |
| 2.7 | Create matching example .fj files for each tutorial | `examples/tutorial_*.fj` | ~400 | All pass `fj check` |
| 2.8 | Add "Try in Playground" links where applicable | `book/src/tutorials/*.md` | ~10 | Links work |

---

## Option 3: Publish VS Code Extension (Effort: 1 day)

### What Already Exists
- `editors/vscode/` — complete extension (v3.0.0)
- `package.json` with publisher `primecore`, VS Code 1.75.0+
- TextMate grammar, snippets, debug adapter, LSP config
- Ready for marketplace publishing

### Tasks

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 3.1 | Update `package.json` version to match v9.0.1, update description | `editors/vscode/package.json` | ~5 | JSON valid |
| 3.2 | Add icon (128x128 PNG) — Fajar Lang logo | `editors/vscode/icon.png` | — | Image displays |
| 3.3 | Update README.md for marketplace listing with screenshots | `editors/vscode/README.md` | ~100 | Markdown renders |
| 3.4 | Add CHANGELOG.md for extension | `editors/vscode/CHANGELOG.md` | ~30 | Changelog valid |
| 3.5 | Test extension locally: `code --extensionDevelopmentPath=./editors/vscode` | — | — | Syntax highlighting works |
| 3.6 | Package with `vsce package` | — | — | .vsix generated |
| 3.7 | Publish with `vsce publish` (requires Personal Access Token) | — | — | Listed on marketplace |
| 3.8 | Add marketplace badge to README.md | `README.md` | ~2 | Badge renders |

**Prerequisite:** Azure DevOps PAT (Personal Access Token) for publishing — user action.

---

## Option 4: Performance Benchmarks (Effort: 1 day)

### What Already Exists
- `benches/` — Criterion benchmarks (interpreter, embedded, concurrency, arm64)
- `benches/baselines/` — C and Rust comparison programs (fibonacci, bubble_sort, sum_loop)
- `benches/baselines/RESULTS.md` — template with TBD values
- Criterion configured with HTML report generation

### Tasks

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 4.1 | Run Criterion benchmarks: `cargo bench` — collect interpreter baseline numbers | — | — | HTML report generated |
| 4.2 | Run Cranelift JIT benchmarks: fibonacci(30), bubble_sort(10K), matrix_mul | `benches/native_bench.rs` | ~100 | Results collected |
| 4.3 | Run LLVM backend benchmarks with -O2: same workloads | — | — | Results collected |
| 4.4 | Compile and run C baselines: `gcc -O2 baselines/*.c` | — | — | Times recorded |
| 4.5 | Compile and run Rust baselines: `rustc -O baselines/*.rs` | — | — | Times recorded |
| 4.6 | Run Python equivalents: `python3 baselines/*.py` | `benches/baselines/*.py` | ~50 | Times recorded |
| 4.7 | Run Go equivalents: `go run baselines/*.go` | `benches/baselines/*.go` | ~50 | Times recorded |
| 4.8 | Populate `RESULTS.md` with collected numbers + comparison table | `benches/baselines/RESULTS.md` | ~100 | Table complete |
| 4.9 | Add benchmark summary to README.md "Performance" section | `README.md` | ~30 | Renders correctly |
| 4.10 | Add benchmark summary to website landing page | `website/index.html` | ~30 | Chart renders |

---

## Option 5: Self-Hosting (Effort: 1-2 weeks)

### What Already Exists
- `src/selfhost/bootstrap.rs` — 3-stage bootstrap infrastructure (Stage0/1/2)
- `stdlib/lexer.fj` — tokenizer in Fajar Lang (partial)
- `stdlib/parser.fj` — recursive descent parser (partial)
- `stdlib/ast.fj` — AST node types
- `stdlib/analyzer.fj`, `stdlib/codegen.fj` — stubs

### Tasks

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 5.1 | Complete `stdlib/lexer.fj` — all 82+ token kinds, keyword map, string/number literals | `stdlib/lexer.fj` | ~500 | `fj run stdlib/lexer.fj` tokenizes hello.fj |
| 5.2 | Complete `stdlib/parser.fj` — statements (let, fn, struct, enum, if, while, for, match, return) | `stdlib/parser.fj` | ~800 | Parses hello.fj |
| 5.3 | Complete `stdlib/parser.fj` — expressions (Pratt parser, 19 precedence levels) | `stdlib/parser.fj` | ~400 | Parses complex exprs |
| 5.4 | Complete `stdlib/ast.fj` — all Expr/Stmt/Item variants matching Rust AST | `stdlib/ast.fj` | ~300 | Types defined |
| 5.5 | Implement `stdlib/analyzer.fj` — scope resolution, type checking basics | `stdlib/analyzer.fj` | ~500 | Catches type errors |
| 5.6 | Bootstrap test: `fj run stdlib/lexer.fj < hello.fj` produces same tokens as Rust lexer | `tests/selfhost_*.rs` | ~50 | Token-for-token match |
| 5.7 | Bootstrap test: `fj run stdlib/parser.fj < hello.fj` produces same AST as Rust parser | `tests/selfhost_*.rs` | ~50 | AST match |
| 5.8 | Wire Stage1 in `bootstrap.rs`: Rust compiler → compile self-hosted lexer+parser → Stage1 binary | `src/selfhost/bootstrap.rs` | ~100 | Stage1 binary produced |
| 5.9 | Stage2: Stage1 binary → compile self-hosted code → Stage2 binary | `src/selfhost/bootstrap.rs` | ~100 | Stage2 matches Stage1 |
| 5.10 | Document bootstrap process in `book/src/compilation/self-hosting.md` | `book/src/compilation/self-hosting.md` | ~100 | mdBook renders |

---

## Option 6: V11 Language Features (Effort: 1+ month)

### What Already Exists
- `src/analyzer/borrow_lite.rs` — real move semantics + borrow rules (OwnershipState, BorrowState, MoveTracker)
- `src/analyzer/cfg.rs` — real NLL analysis (liveness, loop-aware)
- `src/dependent/` — dependent types, const generics (2,529 LOC)
- Parser already handles lifetime annotations syntax

### Tasks — Borrow Checker Enhancement

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 6.1 | Add reborrowing: `&mut T` → `&T` coercion in function arguments | `src/analyzer/borrow_lite.rs` | ~50 | Test: reborrow in call |
| 6.2 | Add two-phase borrows: temporary immutable borrow for method receiver | `src/analyzer/borrow_lite.rs` | ~80 | Test: `v.push(v.len())` |
| 6.3 | Improve NLL: per-point borrow liveness (not just scope-based) | `src/analyzer/cfg.rs` | ~100 | Test: borrow dies at last use |
| 6.4 | Add `Drop` order validation: destructors run in reverse declaration order | `src/analyzer/borrow_lite.rs` | ~50 | Test: drop order |
| 6.5 | 20 edge-case tests: double mutable borrow, borrow across loop, return reference | `tests/safety_tests.rs` | ~200 | All 20 pass |

### Tasks — Lifetime Annotations (optional, deferred)

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 6.6 | Parser: `'a` lifetime syntax in function signatures | `src/parser/` | ~50 | Parses `fn foo<'a>(&'a str)` |
| 6.7 | Analyzer: lifetime constraint tracking | `src/analyzer/` | ~200 | Catches dangling ref |
| 6.8 | Lifetime elision rules (like Rust) | `src/analyzer/` | ~100 | No annotation needed for simple cases |

### Tasks — Effect System (optional, deferred)

| # | Task | File(s) | LOC | Verify |
|---|------|---------|-----|--------|
| 6.9 | Parser: `effect` keyword + `can Throw, IO` syntax | `src/parser/` | ~100 | Parses effect annotations |
| 6.10 | Analyzer: effect propagation checking | `src/analyzer/` | ~200 | Effect must be declared or handled |

---

## Priority Order & Dependencies

```
Option 3 (VS Code) ──── 1 day, no deps, immediate value
Option 4 (Benchmarks) ── 1 day, no deps, credibility
Option 2 (Tutorials) ─── 2 days, no deps, onboarding
Option 1 (Website) ───── 2-3 days, needs Option 4 results for benchmark display
Option 5 (Self-Host) ─── 1-2 weeks, independent
Option 6 (V11 Lang) ──── 1+ month, independent
```

**Recommended execution order:** 3 → 4 → 2 → 1 → 5 → 6

---

## Summary

| Option | Tasks | Est. LOC | Days | What Already Exists |
|--------|-------|----------|------|---------------------|
| 1. Website | 10 | ~670 | 2-3 | Playground 95% done, docs.yml deployed |
| 2. Tutorials | 8 | ~1,320 | 2 | 130 book chapters, 178 examples |
| 3. VS Code | 8 | ~140 | 1 | Extension 100% done, needs publish |
| 4. Benchmarks | 10 | ~360 | 1 | Criterion + baselines exist, need data |
| 5. Self-Host | 10 | ~2,900 | 7-10 | Bootstrap infra 40%, lexer/parser partial |
| 6. V11 Lang | 10 | ~1,130 | 14-30 | Borrow checker 90% real, NLL exists |
| **Total** | **56** | **~6,520** | **~28-47** | |

### Verification (per option)
1. **Website:** 3 URLs load (/, /playground, /docs)
2. **Tutorials:** `mdbook build` succeeds, examples pass `fj check`
3. **VS Code:** Extension installs from .vsix, syntax highlighting works
4. **Benchmarks:** RESULTS.md populated with real numbers
5. **Self-Host:** `fj run stdlib/lexer.fj < hello.fj` matches Rust lexer output
6. **V11:** 20 borrow checker edge-case tests pass
