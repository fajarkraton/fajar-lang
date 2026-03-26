# Plan V8 "Dominion" — Production Ecosystem & Self-Hosting

> **Previous:** V7 "Ascendancy" (680/680 tasks, ALL COMPLETE)
> **Version:** Fajar Lang v6.1.0 → v7.0.0 "Dominion"
> **Goal:** Transform Fajar Lang from a working compiler into a production ecosystem
> **Scale:** 10 options, 70 sprints, 700 tasks, ~140 hours

---

## Motivation

V7 completed distributed computing, WASI, GPU pipelines, advanced types, incremental compilation, cloud deployment, plugins, CI/CD, FajarOS Surya, and quality audit. The compiler is feature-complete. **V8 focuses on the ecosystem**: self-hosting, package registry, IDE experience, real-world deployment, and hardening for production use.

---

## Option 1: Self-Hosting v3 — Compiler in Fajar Lang (8 sprints, 80 tasks)

*Write the Fajar Lang compiler in Fajar Lang itself — the ultimate proof of language maturity.*

### Phase SH1: Lexer in Fajar Lang (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH1.1 | Token type definitions | TokenKind enum with all 82+ variants | [ ] |
| SH1.2 | Span struct | start/end byte offsets | [ ] |
| SH1.3 | Cursor implementation | peek/advance/is_eof in pure Fajar | [ ] |
| SH1.4 | Whitespace/comment skip | Skip spaces, tabs, newlines, // and /* */ | [ ] |
| SH1.5 | Integer literal lexing | Decimal, hex (0x), binary (0b), octal (0o) | [ ] |
| SH1.6 | Float literal lexing | 3.14, 1e10, 2.5e-3 | [ ] |
| SH1.7 | String literal lexing | Escape sequences: \n, \t, \\, \", \0 | [ ] |
| SH1.8 | Char literal lexing | 'a', '\n', '\0' | [ ] |
| SH1.9 | Identifier/keyword lexing | 60+ keywords, contextual keywords | [ ] |
| SH1.10 | Operator lexing | Single/double/triple char operators (19 precedence levels) | [ ] |
| SH1.11 | Delimiter lexing | ( ) [ ] { } , ; : :: -> => | [ ] |
| SH1.12 | Error recovery in lexer | Continue after invalid chars, collect all errors | [ ] |
| SH1.13 | Line/column tracking | Compute line:col from byte offset | [ ] |
| SH1.14 | Tokenize function | pub fn tokenize(source: str) -> Result<Array<Token>, Array<LexError>> | [ ] |
| SH1.15 | Lexer unit tests (50) | Test each token kind, edge cases, errors | [ ] |
| SH1.16 | Bootstrap verification | fj lexer output == rust lexer output for all examples | [ ] |
| SH1.17 | Unicode support | UTF-8 identifiers, string content | [ ] |
| SH1.18 | Annotation lexing | @kernel, @device, @safe, @unsafe, @test | [ ] |
| SH1.19 | F-string lexing | f"Hello {name}" tokenization | [ ] |
| SH1.20 | Performance comparison | Self-hosted lexer within 5x of Rust lexer | [ ] |

### Phase SH2: Parser in Fajar Lang (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH2.1 | AST node types | Expr, Stmt, Item, TypeExpr, Pattern enums | [ ] |
| SH2.2 | Pratt expression parser | 19-level precedence, right/left associativity | [ ] |
| SH2.3 | Statement parsing | let, const, fn, struct, enum, impl, trait, use, mod | [ ] |
| SH2.4 | Control flow parsing | if/else, while, for..in, loop, match, break, continue, return | [ ] |
| SH2.5 | Function parsing | Parameters, return types, generic params, where clauses | [ ] |
| SH2.6 | Struct/enum parsing | Fields, variants, associated types, methods | [ ] |
| SH2.7 | Type expression parsing | Primitives, generics, arrays, tuples, function types | [ ] |
| SH2.8 | Pattern parsing | Literal, ident, tuple, struct, enum, wildcard, or-patterns | [ ] |
| SH2.9 | Error recovery | Synchronize on ; and }, collect all parse errors | [ ] |
| SH2.10 | Parse function | pub fn parse(tokens: Array<Token>) -> Result<Program, Array<ParseError>> | [ ] |
| SH2.11 | Operator precedence tests | Verify all 19 levels parse correctly | [ ] |
| SH2.12 | Parser unit tests (50) | Expressions, statements, items, patterns, errors | [ ] |
| SH2.13 | Bootstrap verification | fj parser AST == rust parser AST for all examples | [ ] |
| SH2.14 | Lambda/closure parsing | |args| body, move || { }, capture syntax | [ ] |
| SH2.15 | Attribute parsing | @annotation before fn/struct/enum | [ ] |
| SH2.16 | Module path parsing | mod::item, super::item, use std::io | [ ] |
| SH2.17 | Impl block parsing | impl Type { }, impl Trait for Type { } | [ ] |
| SH2.18 | Match arm parsing | Pattern guards, multiple patterns, exhaustiveness | [ ] |
| SH2.19 | Array/tuple parsing | [1, 2, 3], (a, b, c), indexing | [ ] |
| SH2.20 | Pipeline operator | a |> b |> c parsing and AST | [ ] |

### Phase SH3: Type Checker in Fajar Lang (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH3.1 | Symbol table | Scope stack, name resolution, shadowing | [ ] |
| SH3.2 | Type inference engine | Bidirectional type inference, unification | [ ] |
| SH3.3 | Primitive type checking | i8-i128, u8-u128, f32/f64, bool, char, str | [ ] |
| SH3.4 | Function type checking | Parameter types, return type, generic instantiation | [ ] |
| SH3.5 | Struct/enum type checking | Field access, variant construction, pattern matching | [ ] |
| SH3.6 | Trait checking | Trait bounds, impl satisfaction, method dispatch | [ ] |
| SH3.7 | Generic instantiation | Monomorphization decisions, type substitution | [ ] |
| SH3.8 | Context checking | @kernel/@device restrictions (KE001-KE004, DE001-DE003) | [ ] |
| SH3.9 | Mutability checking | let vs let mut, &T vs &mut T | [ ] |
| SH3.10 | Move/borrow checking | Ownership tracking, borrow regions | [ ] |
| SH3.11 | Error collection | Collect all semantic errors, don't stop at first | [ ] |
| SH3.12 | Type checker tests (50) | Type inference, errors, generics, traits, context | [ ] |
| SH3.13 | Bootstrap verification | fj analyzer errors == rust analyzer errors | [ ] |
| SH3.14 | Expression type inference | Binary ops, unary ops, method calls, field access | [ ] |
| SH3.15 | Match exhaustiveness | Verify all patterns covered | [ ] |
| SH3.16 | Return type checking | All paths return correct type | [ ] |
| SH3.17 | Unused variable warnings | SE009 detection | [ ] |
| SH3.18 | Unreachable code warnings | SE010 detection | [ ] |
| SH3.19 | Import resolution | use statements, module paths | [ ] |
| SH3.20 | Numeric coercion | Integer widening, float promotion rules | [ ] |

### Phase SH4: Bootstrap & Verification (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH4.1 | End-to-end pipeline | Lex → Parse → Analyze in pure Fajar Lang | [ ] |
| SH4.2 | Compile self-lexer | Use Rust compiler to compile .fj lexer to native | [ ] |
| SH4.3 | Compile self-parser | Use Rust compiler to compile .fj parser to native | [ ] |
| SH4.4 | Compile self-checker | Use Rust compiler to compile .fj checker to native | [ ] |
| SH4.5 | Stage 1 bootstrap | fj₀ (Rust) compiles fj₁ (.fj source) | [ ] |
| SH4.6 | Stage 2 bootstrap | fj₁ compiles fj₂ (second generation) | [ ] |
| SH4.7 | Stage 3 verification | fj₂ output == fj₁ output (fixed point) | [ ] |
| SH4.8 | Differential testing | Run all 165 examples through both compilers | [ ] |
| SH4.9 | Error message parity | Same error codes, spans, suggestions | [ ] |
| SH4.10 | Performance benchmark | Self-hosted within 3x of Rust impl | [ ] |
| SH4.11 | Binary reproducibility | Same input → identical output | [ ] |
| SH4.12 | Stress test | 10K line programs, deeply nested, complex generics | [ ] |
| SH4.13 | Regression suite | 200 test programs covering all language features | [ ] |
| SH4.14 | Self-hosted CI | CI job that builds via self-hosted compiler | [ ] |
| SH4.15 | Documentation | Self-hosting guide, architecture doc | [ ] |
| SH4.16 | stdlib in Fajar Lang | Port core stdlib functions to .fj | [ ] |
| SH4.17 | Error formatting | miette-style error display in pure Fajar | [ ] |
| SH4.18 | Source map generation | Map compiled output back to source locations | [ ] |
| SH4.19 | Incremental self-host | Cache compiled modules for faster rebuilds | [ ] |
| SH4.20 | Release self-hosted binary | Ship `fj-selfhosted` alongside `fj` | [ ] |

---

## Option 2: Package Registry & Ecosystem (7 sprints, 70 tasks)

*Build a real package registry with versioning, search, and dependency resolution.*

### Phase PR1: Registry Server (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| PR1.1 | Registry API design | REST API: publish, download, search, yank | [ ] |
| PR1.2 | Package storage backend | S3-compatible object storage | [ ] |
| PR1.3 | Sparse index | Git-based sparse index (like crates.io) | [ ] |
| PR1.4 | Version resolution | PubGrub solver integration | [ ] |
| PR1.5 | Authentication | API tokens, OAuth2, publish permissions | [ ] |
| PR1.6 | Rate limiting | Per-token rate limits, abuse prevention | [ ] |
| PR1.7 | Package validation | Name rules, semver enforcement, size limits | [ ] |
| PR1.8 | Search engine | Full-text search across names, descriptions, keywords | [ ] |
| PR1.9 | Download counting | Per-version download statistics | [ ] |
| PR1.10 | Dependency graph | Transitive dependency resolution, cycle detection | [ ] |
| PR1.11 | Yanking support | Soft-delete versions, prevent new installs | [ ] |
| PR1.12 | Audit log | Who published what, when, from where | [ ] |
| PR1.13 | Webhook notifications | Notify on new versions, security advisories | [ ] |
| PR1.14 | Mirror support | Read-only mirrors for air-gapped environments | [ ] |
| PR1.15 | Registry tests (30) | API endpoints, auth, resolution, edge cases | [ ] |
| PR1.16 | Docker deployment | docker-compose for self-hosted registry | [ ] |
| PR1.17 | TLS/HTTPS | Certificate management, HSTS | [ ] |
| PR1.18 | Backup strategy | Automated backups, disaster recovery | [ ] |
| PR1.19 | Admin dashboard | Web UI for registry management | [ ] |
| PR1.20 | API documentation | OpenAPI spec, usage examples | [ ] |

### Phase PR2: CLI Integration (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| PR2.1 | `fj publish` command | Build, validate, upload to registry | [ ] |
| PR2.2 | `fj install` command | Download, extract, add to deps | [ ] |
| PR2.3 | `fj update` command | Update deps to latest compatible versions | [ ] |
| PR2.4 | `fj search` command | Search registry by name/keyword | [ ] |
| PR2.5 | `fj yank` command | Yank a published version | [ ] |
| PR2.6 | `fj login` command | Authenticate with registry | [ ] |
| PR2.7 | `fj audit` command | Check deps for known vulnerabilities | [ ] |
| PR2.8 | Lock file (fj.lock) | Reproducible dependency resolution | [ ] |
| PR2.9 | Workspace support | Multi-package workspaces | [ ] |
| PR2.10 | Private registries | Configure alternate registry URLs | [ ] |
| PR2.11 | Offline mode | Install from cache when offline | [ ] |
| PR2.12 | Dependency tree | `fj tree` shows full dependency graph | [ ] |
| PR2.13 | Checksum verification | SHA-256 integrity checks on downloads | [ ] |
| PR2.14 | Proxy support | HTTP/SOCKS proxy for corporate environments | [ ] |
| PR2.15 | Auto-completion | Shell completions for fj commands | [ ] |
| PR2.16 | Progress indicators | Download progress bars, spinners | [ ] |
| PR2.17 | Conflict resolution | Handle version conflicts with clear errors | [ ] |
| PR2.18 | Feature flags | Optional dependencies via fj.toml features | [ ] |
| PR2.19 | Build scripts | Pre/post build hooks in fj.toml | [ ] |
| PR2.20 | CLI tests (30) | All commands, edge cases, error handling | [ ] |

### Phase PR3: Ecosystem Packages (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| PR3.1 | fj-async — async runtime | Tokio-like async executor for Fajar | [ ] |
| PR3.2 | fj-log — structured logging | Log levels, JSON output, file rotation | [ ] |
| PR3.3 | fj-test — advanced testing | Fixtures, mocking, parameterized tests | [ ] |
| PR3.4 | fj-cli — CLI framework | Argument parsing, subcommands, help gen | [ ] |
| PR3.5 | fj-db — database drivers | SQLite, PostgreSQL, MySQL bindings | [ ] |
| PR3.6 | fj-web — web framework | HTTP router, middleware, templates | [ ] |
| PR3.7 | fj-serial — serialization | Binary, MessagePack, Protocol Buffers | [ ] |
| PR3.8 | fj-image — image processing | PNG/JPEG decode, resize, filters | [ ] |
| PR3.9 | fj-regex — regular expressions | NFA/DFA regex engine | [ ] |
| PR3.10 | fj-toml — TOML parser | Full TOML v1.0 spec | [ ] |
| PR3.11 | fj-yaml — YAML parser | YAML 1.2 core schema | [ ] |
| PR3.12 | fj-csv — CSV parser | RFC 4180 compliant | [ ] |
| PR3.13 | fj-time — date/time | Chrono-like date, time, duration, timezone | [ ] |
| PR3.14 | fj-uuid — UUID generation | v4 random, v7 timestamp | [ ] |
| PR3.15 | fj-base64 — encoding | Base64 encode/decode | [ ] |
| PR3.16 | fj-url — URL parsing | RFC 3986 compliant URL parser | [ ] |
| PR3.17 | fj-fs — filesystem utils | Walk, glob, watch, temp dirs | [ ] |
| PR3.18 | fj-env — environment | Env vars, dotenv, config loading | [ ] |
| PR3.19 | fj-color — terminal colors | ANSI colors, styles, RGB | [ ] |
| PR3.20 | fj-rand — random numbers | PCG, MT19937, cryptographic RNG | [ ] |
| PR3.21 | fj-compress — compression | gzip, zstd, lz4 | [ ] |
| PR3.22 | fj-tls — TLS/SSL | rustls-based TLS for fj-http | [ ] |
| PR3.23 | fj-mqtt — IoT messaging | MQTT 3.1.1/5.0 client | [ ] |
| PR3.24 | fj-gpio — GPIO abstraction | Cross-platform GPIO (Linux sysfs, Q6A) | [ ] |
| PR3.25 | fj-sensor — sensor fusion | Accelerometer, gyroscope, Kalman filter | [ ] |
| PR3.26 | fj-onnx — ONNX runtime | ONNX model loading and inference | [ ] |
| PR3.27 | fj-plot — data visualization | Line, bar, scatter plots to SVG/PNG | [ ] |
| PR3.28 | fj-bench — benchmarking | Criterion-like benchmark framework | [ ] |
| PR3.29 | fj-doc — documentation | mdBook-like documentation generator | [ ] |
| PR3.30 | Package ecosystem tests | Cross-package compatibility, integration | [ ] |

---

## Option 3: IDE Experience & Language Server (7 sprints, 70 tasks)

*Make the Fajar Lang IDE experience rival Rust-Analyzer.*

### Phase IDE1: LSP v4 — Semantic Intelligence (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| IDE1.1 | Incremental parsing | Only reparse changed regions | [ ] |
| IDE1.2 | Incremental analysis | Only re-typecheck affected scopes | [ ] |
| IDE1.3 | Background indexing | Index workspace on startup, update on change | [ ] |
| IDE1.4 | Inlay hints | Show inferred types, parameter names, lifetimes | [ ] |
| IDE1.5 | Code lens | Show test run/debug, impl count, references | [ ] |
| IDE1.6 | Semantic highlighting | Token types: keyword, type, function, variable, macro | [ ] |
| IDE1.7 | Auto-import | Suggest and insert use statements | [ ] |
| IDE1.8 | Smart completion | Context-aware completions with type matching | [ ] |
| IDE1.9 | Signature help | Show function signatures while typing | [ ] |
| IDE1.10 | Hover documentation | Show type, docs, source on hover | [ ] |
| IDE1.11 | Go to definition | Navigate to fn, struct, trait, module definitions | [ ] |
| IDE1.12 | Find all references | Show all usages of a symbol | [ ] |
| IDE1.13 | Rename symbol | Rename across all files with preview | [ ] |
| IDE1.14 | Extract function | Select code → extract to new function | [ ] |
| IDE1.15 | Extract variable | Select expression → bind to let | [ ] |
| IDE1.16 | Inline variable | Replace variable with its value | [ ] |
| IDE1.17 | Move to module | Move function/struct to different module | [ ] |
| IDE1.18 | Implement trait | Generate trait impl skeleton | [ ] |
| IDE1.19 | Fill match arms | Generate all match variants | [ ] |
| IDE1.20 | Wrap in if/while/for | Wrap selection in control flow | [ ] |
| IDE1.21 | Diagnostics on-type | Show errors as you type (debounced) | [ ] |
| IDE1.22 | Quick fixes | Suggested fixes for common errors | [ ] |
| IDE1.23 | Workspace symbols | Search all symbols across workspace | [ ] |
| IDE1.24 | Call hierarchy | Show callers/callees tree | [ ] |
| IDE1.25 | Type hierarchy | Show supertypes/subtypes tree | [ ] |
| IDE1.26 | Folding ranges | Fold functions, structs, blocks, comments | [ ] |
| IDE1.27 | Selection range | Expand/shrink selection by semantic unit | [ ] |
| IDE1.28 | Linked editing | Rename both sides of a pair (e.g., struct field) | [ ] |
| IDE1.29 | Document symbols | Outline view with nested symbols | [ ] |
| IDE1.30 | LSP v4 tests (50) | All features, edge cases, performance | [ ] |

### Phase IDE2: VS Code Extension v2 (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| IDE2.1 | TextMate grammar v2 | Full syntax highlighting with semantic tokens | [ ] |
| IDE2.2 | Snippet library | 30+ snippets (fn, struct, enum, match, for, impl) | [ ] |
| IDE2.3 | Debug adapter | DAP integration for step/breakpoint/watch | [ ] |
| IDE2.4 | Test explorer | Discover and run @test functions from sidebar | [ ] |
| IDE2.5 | Task runner | Integrate fj build/test/run as VS Code tasks | [ ] |
| IDE2.6 | Problem matcher | Parse compiler errors into VS Code diagnostics | [ ] |
| IDE2.7 | Code formatter | Format on save via fj fmt | [ ] |
| IDE2.8 | Extension settings | Configure LSP path, features, formatting | [ ] |
| IDE2.9 | Workspace detection | Auto-detect fj.toml and configure | [ ] |
| IDE2.10 | Multi-root workspace | Support multiple fj projects in one workspace | [ ] |
| IDE2.11 | File icons | Custom icons for .fj, fj.toml, fj.lock | [ ] |
| IDE2.12 | Status bar | Show Fajar Lang version, build status | [ ] |
| IDE2.13 | Command palette | fj: Run, Build, Test, Format, Check commands | [ ] |
| IDE2.14 | Extension marketplace | Publish to VS Code Marketplace | [ ] |
| IDE2.15 | JetBrains plugin | Basic syntax + LSP for IntelliJ/CLion | [ ] |
| IDE2.16 | Neovim plugin | LSP config + TreeSitter grammar | [ ] |
| IDE2.17 | Helix support | Language config for Helix editor | [ ] |
| IDE2.18 | Zed extension | LSP integration for Zed editor | [ ] |
| IDE2.19 | Extension tests (20) | All features, activation, performance | [ ] |
| IDE2.20 | Extension documentation | README, screenshots, feature list | [ ] |

### Phase IDE3: Playground v2 (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| IDE3.1 | WebAssembly compiler | Compile fj→Wasm in browser | [ ] |
| IDE3.2 | Monaco editor integration | Syntax highlighting, auto-complete | [ ] |
| IDE3.3 | Live output panel | Show println output, errors, timing | [ ] |
| IDE3.4 | Share via URL | Encode source in URL for sharing | [ ] |
| IDE3.5 | Example gallery | Browse and run all 165 examples | [ ] |
| IDE3.6 | Multi-file support | Create/edit multiple .fj files | [ ] |
| IDE3.7 | Dark/light theme | Toggle themes, system preference | [ ] |
| IDE3.8 | Mobile responsive | Work on tablet/phone screens | [ ] |
| IDE3.9 | AST viewer | Show parsed AST as tree | [ ] |
| IDE3.10 | Token viewer | Show lexer output with highlighting | [ ] |
| IDE3.11 | Type info panel | Show inferred types on hover | [ ] |
| IDE3.12 | Bytecode viewer | Show VM bytecode for programs | [ ] |
| IDE3.13 | Benchmark mode | Time execution, show stats | [ ] |
| IDE3.14 | REPL mode | Interactive REPL in browser | [ ] |
| IDE3.15 | Collaborative editing | Real-time collaboration (Yjs/CRDT) | [ ] |
| IDE3.16 | Embed widget | Embed playground in docs/blog posts | [ ] |
| IDE3.17 | Keyboard shortcuts | Ctrl+Enter run, Ctrl+S format | [ ] |
| IDE3.18 | Error highlighting | Inline error markers in editor | [ ] |
| IDE3.19 | Playground CI | Deploy via GitHub Actions | [ ] |
| IDE3.20 | Playground tests (20) | All features, cross-browser | [ ] |

---

## Option 4: Real-World Application Templates (7 sprints, 70 tasks)

*Prove Fajar Lang works for real projects with production-ready templates.*

### Phase APP1: Web Service Template (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| APP1.1 | HTTP server framework | Router, middleware, request/response | [ ] |
| APP1.2 | JSON API endpoints | CRUD operations with validation | [ ] |
| APP1.3 | Database integration | SQLite/PostgreSQL connection pool | [ ] |
| APP1.4 | Authentication | JWT tokens, password hashing | [ ] |
| APP1.5 | Rate limiting middleware | Token bucket, sliding window | [ ] |
| APP1.6 | CORS middleware | Configurable origin, methods, headers | [ ] |
| APP1.7 | Request logging | Structured access logs | [ ] |
| APP1.8 | Health check endpoint | /health with dependency checks | [ ] |
| APP1.9 | Graceful shutdown | Handle SIGTERM, drain connections | [ ] |
| APP1.10 | Configuration | Env vars, TOML config, CLI flags | [ ] |
| APP1.11 | Error handling | Consistent error responses with codes | [ ] |
| APP1.12 | Pagination | Cursor-based pagination for lists | [ ] |
| APP1.13 | WebSocket support | Real-time bidirectional communication | [ ] |
| APP1.14 | Static file serving | Serve HTML/CSS/JS with caching | [ ] |
| APP1.15 | Template rendering | HTML templates with variable substitution | [ ] |
| APP1.16 | Docker deployment | Dockerfile + docker-compose | [ ] |
| APP1.17 | Integration tests | API endpoint tests with test client | [ ] |
| APP1.18 | OpenAPI generation | Auto-generate API docs from routes | [ ] |
| APP1.19 | Performance benchmark | Requests/sec comparison vs Express/Actix | [ ] |
| APP1.20 | Template documentation | Getting started guide, architecture | [ ] |

### Phase APP2: IoT Edge Device Template (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| APP2.1 | Sensor data collection | Read from I2C/SPI/ADC sensors | [ ] |
| APP2.2 | Local ML inference | Run quantized model on sensor data | [ ] |
| APP2.3 | MQTT telemetry | Publish readings to MQTT broker | [ ] |
| APP2.4 | OTA update support | Download and apply firmware updates | [ ] |
| APP2.5 | Watchdog integration | Hardware watchdog timer management | [ ] |
| APP2.6 | Power management | Sleep modes, wake-on-event | [ ] |
| APP2.7 | Local data buffering | Store readings when offline | [ ] |
| APP2.8 | Configuration via BLE | Bluetooth Low Energy configuration | [ ] |
| APP2.9 | Edge-cloud sync | Batch upload when connectivity restored | [ ] |
| APP2.10 | Alert system | Local alerting on anomaly detection | [ ] |
| APP2.11 | Sensor calibration | Auto-calibration routines | [ ] |
| APP2.12 | Data compression | Compress telemetry for bandwidth savings | [ ] |
| APP2.13 | Secure boot chain | Verified boot with signature checks | [ ] |
| APP2.14 | Device provisioning | Factory setup and key enrollment | [ ] |
| APP2.15 | Fleet management | Report status to central dashboard | [ ] |
| APP2.16 | GPIO abstraction | Cross-platform GPIO for Q6A/RPi/STM32 | [ ] |
| APP2.17 | Real-time scheduling | Priority-based task scheduling | [ ] |
| APP2.18 | Memory budget | Static memory allocation with budget | [ ] |
| APP2.19 | Integration tests | Hardware-in-loop test framework | [ ] |
| APP2.20 | Template documentation | Hardware setup, wiring, deployment | [ ] |

### Phase APP3: ML Training Pipeline Template (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| APP3.1 | Dataset loader | CSV, images, custom formats with batching | [ ] |
| APP3.2 | Data augmentation | Random crop, flip, normalize, noise | [ ] |
| APP3.3 | Model definition | Sequential, functional, custom forward() | [ ] |
| APP3.4 | Training loop | Epoch-batch-step with progress bar | [ ] |
| APP3.5 | Validation loop | Periodic evaluation on val set | [ ] |
| APP3.6 | Learning rate scheduler | StepLR, CosineAnnealing, OneCycleLR | [ ] |
| APP3.7 | Early stopping | Monitor val_loss, patience, restore best | [ ] |
| APP3.8 | Checkpoint saving | Save model + optimizer + epoch state | [ ] |
| APP3.9 | TensorBoard logging | Loss, accuracy, learning rate plots | [ ] |
| APP3.10 | Hyperparameter search | Grid search, random search, Bayesian | [ ] |
| APP3.11 | Mixed precision training | FP16/BF16 forward, FP32 backward | [ ] |
| APP3.12 | Gradient clipping | Max norm, value clipping | [ ] |
| APP3.13 | Weight initialization | Xavier, He, orthogonal, pretrained | [ ] |
| APP3.14 | Transfer learning | Load pretrained, freeze layers, fine-tune | [ ] |
| APP3.15 | Model export | ONNX, FJML, quantized INT8 | [ ] |
| APP3.16 | Deployment pipeline | Train → quantize → deploy to edge | [ ] |
| APP3.17 | A/B testing | Compare model versions with metrics | [ ] |
| APP3.18 | Data versioning | Track dataset versions with hashes | [ ] |
| APP3.19 | Reproducibility | Seed everything, deterministic training | [ ] |
| APP3.20 | MNIST example | End-to-end MNIST 99%+ with all features | [ ] |
| APP3.21 | CIFAR-10 example | Conv2d + BatchNorm + Dropout | [ ] |
| APP3.22 | Text classification | Embedding + LSTM + Dense | [ ] |
| APP3.23 | Time series | 1D Conv + LSTM for prediction | [ ] |
| APP3.24 | Anomaly detection | Autoencoder for anomaly scoring | [ ] |
| APP3.25 | Federated learning | Privacy-preserving distributed training | [ ] |
| APP3.26 | Model compression | Pruning, distillation, quantization | [ ] |
| APP3.27 | Inference server | HTTP API for model serving | [ ] |
| APP3.28 | Batch inference | Process large datasets offline | [ ] |
| APP3.29 | Pipeline tests (30) | All stages, edge cases, performance | [ ] |
| APP3.30 | Template documentation | Tutorial, architecture, deployment | [ ] |

---

## Option 5: Documentation & Learning Platform (7 sprints, 70 tasks)

*Create world-class documentation that makes Fajar Lang accessible to everyone.*

*(70 tasks: Interactive tutorial (20), API reference (20), Cookbook (30))*

---

## Option 6: Compiler Optimization Suite (7 sprints, 70 tasks)

*Make Fajar Lang compile faster and produce faster code.*

*(70 tasks: Compilation speed (20), Code quality (20), Binary size (30))*

---

## Option 7: Security Hardening (7 sprints, 70 tasks)

*Harden Fajar Lang for safety-critical and security-sensitive deployments.*

*(70 tasks: Memory safety (20), Supply chain (20), Audit & certification (30))*

---

## Option 8: Cross-Platform Native GUI (7 sprints, 70 tasks)

*Build native GUI applications with Fajar Lang.*

*(70 tasks: Widget toolkit (30), Layout engine (20), Platform integration (20))*

---

## Option 9: FajarOS Nova v2.0 "Supernova" (7 sprints, 70 tasks)

*Next-gen OS kernel with SMP, networking, and GPU.*

*(70 tasks: SMP scheduler (20), Network stack (20), GPU compositor (30))*

---

## Option 10: Community & Governance (7 sprints, 70 tasks)

*Build the community infrastructure for Fajar Lang adoption.*

*(70 tasks: Website (20), Community (20), Governance (30))*

---

## Execution Strategy

### Path A — "Self-Hosting First" (recommended)
```
1 (Self-Host) → 3 (IDE) → 2 (Registry) → 4 (Apps) → 5 (Docs)
```
Prove the language by writing its own compiler, then polish the ecosystem.

### Path B — "Ecosystem First"
```
2 (Registry) → 3 (IDE) → 4 (Apps) → 5 (Docs) → 1 (Self-Host)
```
Build the ecosystem first, self-host later.

### Path C — "Ship It"
```
4 (Apps) → 5 (Docs) → 2 (Registry) → 10 (Community) → 1 (Self-Host)
```
Focus on real-world use cases and adoption.

---

## Summary

```
Option 1:   Self-Hosting v3            8 sprints   80 tasks    ~16 hrs
Option 2:   Package Registry           7 sprints   70 tasks    ~14 hrs
Option 3:   IDE Experience             7 sprints   70 tasks    ~14 hrs
Option 4:   Application Templates      7 sprints   70 tasks    ~14 hrs
Option 5:   Documentation Platform     7 sprints   70 tasks    ~14 hrs
Option 6:   Compiler Optimization      7 sprints   70 tasks    ~14 hrs
Option 7:   Security Hardening         7 sprints   70 tasks    ~14 hrs
Option 8:   Cross-Platform GUI         7 sprints   70 tasks    ~14 hrs
Option 9:   FajarOS Nova v2.0          7 sprints   70 tasks    ~14 hrs
Option 10:  Community & Governance     7 sprints   70 tasks    ~14 hrs

Total:     70 sprints, 700 tasks, ~142 hours
```
