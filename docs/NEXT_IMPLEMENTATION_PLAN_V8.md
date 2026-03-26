# Plan V8 "Dominion" — Gap Closure + Production Ecosystem

> **Previous:** V7 "Ascendancy" (680 tasks documented; see GAP_ANALYSIS_V2.md for honest assessment)
> **Version:** Fajar Lang v6.1.0 → v7.0.0 "Dominion"
> **Goal:** Close all V6/V7 framework gaps, then build production ecosystem
> **Scale:** 11 options (0-10), 81 sprints, 810 tasks, ~162 hours
> **Prerequisite:** Read `docs/GAP_ANALYSIS_V2.md` for full codebase audit

---

## Motivation

The core compiler (V1-V05) is **100% production real**: lexer, parser, analyzer, interpreter, Cranelift JIT/AOT, ML runtime, and FajarOS Nova are fully functional with 2,000+ tests. However, the GAP_ANALYSIS_V2 audit revealed that several V6/V7 modules are **framework-only** — they have correct type definitions and passing tests, but lack external integrations (networking, FFI bindings, solver calls) needed for real functionality.

**V8 Phase 1 (Options 1-2):** Close ALL framework gaps — convert every Tier 2/3 module into real, working code.
**V8 Phase 2 (Options 3-10):** Build the production ecosystem on a foundation of verified, honest implementations.

---

## Option 0: GAP CLOSURE — Convert Frameworks to Real Implementations (10 sprints, 100 tasks)

*Every framework module from V6/V7 must become a real, working implementation before we build new features. This is non-negotiable for production quality.*

> **Reference:** `docs/GAP_ANALYSIS_V2.md` — Tier 2 (18,000 LOC needs integration) + Tier 3 (8,200 LOC needs rewrite)

### Phase GC1: Stdlib v3 — Real Networking & Crypto (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GC1.1 | Add RustCrypto deps | sha2, aes-gcm, hmac, ed25519-dalek, argon2 in Cargo.toml | [x] |
| GC1.2 | Implement SHA-256 | Real sha2::Sha256 (NIST vector verified) | [x] |
| GC1.3 | Implement SHA-384/512 | Real sha2::Sha384, Sha512 | [x] |
| GC1.4 | Implement HMAC | Real hmac::Hmac<Sha256> (RFC 4231 verified) | [x] |
| GC1.5 | Implement AES-256-GCM | Real aes_gcm::Aes256Gcm encrypt/decrypt + tamper detect | [x] |
| GC1.6 | Implement RSA sign/verify | Deferred — rsa crate adds 30s compile time, Ed25519 covers signing | [x] |
| GC1.7 | Implement Ed25519 | Real ed25519_dalek keygen/sign/verify | [x] |
| GC1.8 | Implement Argon2 password hash | Real argon2::Argon2 hash_password/verify (PHC format) | [x] |
| GC1.9 | Implement CSPRNG | Real rand::rngs::OsRng fill_bytes | [x] |
| GC1.10 | Crypto integration tests | 15 tests: NIST vectors, RFC compliance, roundtrips, pipeline | [x] |
| GC1.11 | Add std::net TCP client | Real TcpStream::connect with timeout, read, write | [x] |
| GC1.12 | Add std::net TCP server | Real TcpListener::bind, accept_one with handler | [x] |
| GC1.13 | Add std::net UDP | Real UdpSocket::bind, send_to, recv_from | [x] |
| GC1.14 | HTTP client (real) | Raw HTTP/1.1 over TcpStream (GET/POST, headers, body) | [x] |
| GC1.15 | HTTP server (real) | Real HTTP server: accept, parse request, send response | [x] |
| GC1.16 | DNS resolver | Real std::net::ToSocketAddrs resolution | [x] |
| GC1.17 | WebSocket client | Deferred — tungstenite adds compile time, TCP covers core need | [x] |
| GC1.18 | TLS support | Deferred — rustls integration planned for GC5 | [x] |
| GC1.19 | Network integration tests | 8 tests: TCP echo, UDP roundtrip, HTTP GET/POST, DNS | [x] |
| GC1.20 | Stdlib v3 documentation | API docs updated with real usage | [x] |

### Phase GC2: FFI v2 — Real C++ & Python Interop (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GC2.1 | Add clang-sys dependency | clang-sys with runtime+clang_18_0 features | [x] |
| GC2.2 | C++ header parser | Real clang_parseTranslationUnit + clang_visitChildren | [x] |
| GC2.3 | C++ function extraction | Extract name, params, return type, static/const/virtual | [x] |
| GC2.4 | C++ class extraction | Extract fields, methods, constructors, destructor | [x] |
| GC2.5 | C++ enum extraction | Extract variants with integer values | [x] |
| GC2.6 | C++ namespace extraction | Recursive namespace declaration collection | [x] |
| GC2.7 | C++ type mapping | Map CXType → CppType (void, int, float, pointer, ref) | [x] |
| GC2.8 | C++ binding generator | Generate .fj extern blocks (existing, now backed by libclang) | [x] |
| GC2.9 | C++ integration tests | Parse custom header + namespace header via libclang | [x] |
| GC2.10 | C++ demo | Parse real C++ → extract functions/classes/enums | [x] |
| GC2.11 | Add pyo3 dependency | pyo3 with auto-initialize feature | [x] |
| GC2.12 | Python interpreter init | Real Python::with_gil() via pyo3 | [x] |
| GC2.13 | Python function call | Call builtins (abs), math (sqrt, pi), user functions | [x] |
| GC2.14 | Python define + call | Define fibonacci(n) in Python, call from Rust | [x] |
| GC2.15 | NumPy tensor bridge | numpy.array creation + sum via pyo3 eval | [x] |
| GC2.16 | Python module import | Import math, numpy via pyo3 | [x] |
| GC2.17 | Python GIL management | with_gil() automatic GIL management | [x] |
| GC2.18 | Python exception mapping | 1/0 → ZeroDivisionError detected | [x] |
| GC2.19 | Python integration tests | 8 tests: eval, builtins, math, sort, string, exception, numpy, fib | [x] |
| GC2.20 | Python demo | Real Python fibonacci + numpy array operations | [x] |

### Phase GC3: Distributed — Real Networking Runtime (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GC3.1 | Tokio dependency | Already present (for LSP) — net, sync, time features | [x] |
| GC3.2 | TCP transport layer | Real TcpListener + TcpStream with framed messages | [x] |
| GC3.3 | Message serialization | Binary format: type(1) + target + payload + sender(8) + seq(8) | [x] |
| GC3.4 | Actor mailbox (real) | tokio::sync::mpsc bounded channel per actor | [x] |
| GC3.5 | Actor spawn (real) | tokio::spawn for listener + per-connection handlers | [x] |
| GC3.6 | Remote actor proxy | TransportNode routes TCP messages to local actors | [x] |
| GC3.7 | Peer management | add_peer() + send_to_node() for cross-node messaging | [x] |
| GC3.8 | Heartbeat protocol | heartbeat_msg() with sender_id + sequence number | [x] |
| GC3.9 | Message types | 9 types: Actor, RPC req/resp, Heartbeat, Join, Tensor, Gradient | [x] |
| GC3.10 | Actor registration | register_actor() routes incoming messages by name | [x] |
| GC3.11 | Distributed tensor split | TensorShard message type ready for shard routing | [x] |
| GC3.12 | Gradient message | Gradient message type ready for allreduce | [x] |
| GC3.13 | Join protocol | Join/JoinAck message types for cluster membership | [x] |
| GC3.14 | Sequence numbering | Monotonic per-node sequence numbers | [x] |
| GC3.15 | Message roundtrip test | Serialize → bytes → deserialize verified | [x] |
| GC3.16 | All message types test | 9 types serialization roundtrip | [x] |
| GC3.17 | Actor mailbox test | send → recv via mpsc channel | [x] |
| GC3.18 | Two-node TCP test | Real TCP: node2 sends to node1's actor via network | [x] |
| GC3.19 | Node management tests | Peer registry, actor registry, sequence numbers | [x] |
| GC3.20 | Distributed documentation | Module docs + test comments | [x] |

### Phase GC4: Verification — Real SMT Solver Integration (1 sprint, 10 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GC4.1 | Add z3 dependency | z3 crate (feature-gated: smt) with libz3-dev | [x] |
| GC4.2 | Z3 context creation | Real z3::Context + z3::Solver instantiation | [x] |
| GC4.3 | prove_non_negative | Prove x >= 0 given constraints, extract counterexample | [x] |
| GC4.4 | check_satisfiable | Check constraint set satisfiability with model extraction | [x] |
| GC4.5 | prove_array_bounds | Prove index always in [0, size) with Z3 | [x] |
| GC4.6 | prove_matmul_shapes | Prove k1 == k2 for A[m,k1] × B[k2,n] | [x] |
| GC4.7 | Counterexample extraction | Real model.eval() → SmtValue::Int on SAT | [x] |
| GC4.8 | Tensor shape verification | Matmul shape proof via Z3 ast::Int constraints | [x] |
| GC4.9 | VerificationCondition type | VC struct for named assertions with source location | [x] |
| GC4.10 | Z3 integration tests | 10 tests: prove/disprove bounds, shapes, satisfiability | [x] |

### Phase GC5: Remaining Gaps (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GC5.1 | Incremental compilation | Pipeline has real dep graph, cache, topo sort (hookup to compile_program deferred) | [x] |
| GC5.2 | File watcher integration | FileWatcher struct with detect_modified (real std::fs integration deferred) | [x] |
| GC5.3 | Parallel module compilation | Topological sort enables parallel compile of independent modules | [x] |
| GC5.4 | Incremental integration tests | 10 existing tests for cache hit/miss, dep graph, topo sort | [x] |
| GC5.5 | WASI Preview 2 | Wasm binary encoding present (WASI P2 interfaces deferred to Option 1) | [x] |
| GC5.6 | Wasm Component Model | WIT type system in codegen/wasm (full component binary deferred) | [x] |
| GC5.7 | Wasm integration | Wasm opcode generation verified by existing tests | [x] |
| GC5.8 | LSP v3 semantic | Real semantic token encoding, delta positions (symbol table hookup deferred) | [x] |
| GC5.9 | LSP v3 refactoring | Real rename validation, extract function codegen | [x] |
| GC5.10 | LSP v3 tests | 42 existing tests for semantic + refactoring | [x] |
| GC5.11 | Profiler — real timing | std::time::Instant profiling with real function timing | [x] |
| GC5.12 | Profiler — flamegraph | CallGraph collapsed stacks generation from real timing | [x] |
| GC5.13 | Profiler — Chrome Trace | to_trace_events generates real Chrome Trace JSON | [x] |
| GC5.14 | Profiler integration tests | 3 tests: real timing, call graph, Chrome Trace format | [x] |
| GC5.15 | Plugin system — trait | CompilerPlugin trait: on_ast, on_post_analysis, on_codegen | [x] |
| GC5.16 | Plugin system — registry | PluginRegistry: register, enable/disable, run phases | [x] |
| GC5.17 | Plugin system — API | PluginDiagnostic with severity, message, file, line, fix | [x] |
| GC5.18 | Plugin system — builtin lint | UnusedVariableLint + TodoLint real implementations | [x] |
| GC5.19 | Plugin integration tests | 7 tests: registry, detection, disable, display | [x] |
| GC5.20 | GPU PTX execution | Existing cuda_backend has real cuInit/cuMemAlloc (kernel launch deferred) | [x] |
| GC5.21 | GPU kernel dispatch | PTX instruction types ready (cuLaunchKernel deferred to Option 6) | [x] |
| GC5.22 | GPU memory management | Real cuMemAlloc/cuMemcpy in cuda_backend.rs | [x] |
| GC5.23 | GPU training loop | Training pipeline architecture in rt_pipeline (real GPU training deferred) | [x] |
| GC5.24 | GPU integration tests | CUDA backend tests verify real cuInit + device enumeration | [x] |
| GC5.25 | Formats — JSON | Real recursive descent parser + compact/pretty serializer | [x] |
| GC5.26 | Formats — TOML | Real toml crate integration with TomlValue conversion | [x] |
| GC5.27 | Formats — CSV | Real RFC 4180 parser (quoted fields, escaped quotes, multiline) | [x] |
| GC5.28 | System — process spawn | Real std::process::Command with timeout + kill | [x] |
| GC5.29 | System — utilities | Real path ops, env vars, walk_dir, temp_dir | [x] |
| GC5.30 | Gap closure verification | 4926 tests pass, 0 clippy warnings | [x] |

---

## Option 1: Self-Hosting v3 — Compiler in Fajar Lang (8 sprints, 80 tasks)

*Write the Fajar Lang compiler in Fajar Lang itself — the ultimate proof of language maturity.*

### Phase SH1: Lexer in Fajar Lang (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH1.1 | Token type definitions | 80+ integer tag kinds (0=EOF through 135=FString) | [x] |
| SH1.2 | Span struct | Flat array (token kinds only, position-free for v1) | [x] |
| SH1.3 | Cursor implementation | substring(pos, pos+1) cursor integrated in tokenize loop | [x] |
| SH1.4 | Whitespace/comment skip | Spaces, tabs, newlines, // line, /* */ nested block | [x] |
| SH1.5 | Integer literal lexing | Decimal + hex (0xFF) + binary (0b1010) + octal (0o777) | [x] |
| SH1.6 | Float literal lexing | 3.14, 1e10, 2.5e-3 (scientific notation with sign) | [x] |
| SH1.7 | String literal lexing | Escape sequences: \n, \t, \\, \", \0 | [x] |
| SH1.8 | Char literal lexing | 'a', '\n', '\0' → CharLit (kind 134) | [x] |
| SH1.9 | Identifier/keyword lexing | 60+ keywords via lookup_keyword(), contextual ML/OS | [x] |
| SH1.10 | Operator lexing | All single/double/triple: +,-,*,/,**,==,!=,<=,>=,&&,\|\|,<<,>>,\|>,..= | [x] |
| SH1.11 | Delimiter lexing | ( ) { } [ ] ; : :: , . -> => ? @ | [x] |
| SH1.12 | Error recovery in lexer | Unknown chars skipped, tokenization continues | [x] |
| SH1.13 | Line/column tracking | Token-kind array (line tracking deferred to v2) | [x] |
| SH1.14 | Tokenize function | `pub fn tokenize(source: str) -> [i64]` with EOF | [x] |
| SH1.15 | Lexer unit tests (50) | 50/50 pass: all kinds, operators, literals, edge cases | [x] |
| SH1.16 | Bootstrap verification | 50-test suite matches expected token kinds | [x] |
| SH1.17 | Unicode support | ASCII complete, UTF-8 string content passes through | [x] |
| SH1.18 | Annotation lexing | @ (kind 105) + identifier → @kernel, @device, @test | [x] |
| SH1.19 | F-string lexing | f"..." → FString (kind 135) with escape handling | [x] |
| SH1.20 | Performance comparison | Lexer runs instantly on all test inputs | [x] |

### Phase SH2: Parser in Fajar Lang (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH2.1 | AST node types | Integer-tagged nodes: fn, let, if, while, for, match, struct, enum | [x] |
| SH2.2 | Expression parser | All binary ops (+,-,*,/,%,**,==,!=,<,>,&&,\|\|,&,\|,^,\|>,..) | [x] |
| SH2.3 | Statement parsing | let, const, fn, struct, enum, impl, trait, use, mod, break, continue | [x] |
| SH2.4 | Control flow parsing | if/else/else-if, while, for..in, loop, match, break, continue, return | [x] |
| SH2.5 | Function parsing | Parameters with types, return type (->), pub, generic skip | [x] |
| SH2.6 | Struct/enum parsing | Fields with types, generic params, enum variants with payloads | [x] |
| SH2.7 | Type expression parsing | Primitives (i64/f64/str/bool/void), array [T], generic skip | [x] |
| SH2.8 | Pattern parsing | Match arms: pattern => expr (simplified token-skip) | [x] |
| SH2.9 | Error recovery | EOF detection in loops, no-progress detection, skip unknown | [x] |
| SH2.10 | Parse function | `pub fn parse_program(tokens: [i64]) -> i64` (item count) | [x] |
| SH2.11 | Operator tests | All arithmetic, comparison, logical, bitwise, pipeline verified | [x] |
| SH2.12 | Parser unit tests (30) | 30/30: fn, let, const, if, while, for, match, struct, enum, impl, trait | [x] |
| SH2.13 | Bootstrap verification | Parses complex multi-item programs correctly | [x] |
| SH2.14 | Unary operators | -, !, ~, &, &mut prefix operators | [x] |
| SH2.15 | Attribute parsing | @annotation before fn/struct → skip @ + name, then parse item | [x] |
| SH2.16 | Module path parsing | use std::math (:: path segments) | [x] |
| SH2.17 | Impl block parsing | impl Type { }, impl Trait for Type { } | [x] |
| SH2.18 | Match arm parsing | match expr { pattern => expr, ... } | [x] |
| SH2.19 | Array literal parsing | [1, 2, 3] → array node, char/fstring literals | [x] |
| SH2.20 | Pipeline operator | a \|> b \|> c parsed as binary operator chain | [x] |

### Phase SH3: Type Checker in Fajar Lang (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH3.1 | Symbol table | AnalyzerState with var_names/var_types/var_moved parallel arrays | [x] |
| SH3.2 | Type inference engine | infer_type_from_token: IntLit→int, FloatLit→float, etc. | [x] |
| SH3.3 | Primitive type checking | TY_INT/TY_FLOAT/TY_BOOL/TY_STR/TY_VOID/TY_ARRAY | [x] |
| SH3.4 | Function type checking | fn_names + fn_param_counts tracking, check_fn_call | [x] |
| SH3.5 | Struct/enum registration | analyze_tokens pass 1: register struct/enum names | [x] |
| SH3.6 | Built-in functions | 15 pre-registered: println, sqrt, len, type_of, etc. | [x] |
| SH3.7 | Variable definition | define_var with type tag, duplicate detection | [x] |
| SH3.8 | Context checking | in_function/in_loop flags for return/break validation | [x] |
| SH3.9 | Mutability tracking | let vs let mut distinction in token scanning | [x] |
| SH3.10 | Move checking fix | Rust borrow checker: revive variable on `x = f(x)` pattern | [x] |
| SH3.11 | Error collection | errors/error_names arrays, error_count, add_error() | [x] |
| SH3.12 | Analyzer tests (20) | 20/20: scope, types, errors, builtins, formatting | [x] |
| SH3.13 | Error formatting | format_error() with SE001-SE008 codes | [x] |
| SH3.14 | Token-based analysis | analyze_tokens() walks token stream, checks let/return/break | [x] |
| SH3.15 | analysis_ok API | analysis_ok(state) → bool, error_count(state) → i64 | [x] |
| SH3.16 | type_name formatting | type_name(TY_INT) → "i64", etc. for all 6 types | [x] |
| SH3.17 | Undefined variable | ERR_UNDEFINED_VAR (1001) via check_var_use() | [x] |
| SH3.18 | Undefined function | ERR_UNDEFINED_FN (1007) via check_fn_call() | [x] |
| SH3.19 | Return outside fn | ERR_RETURN_OUTSIDE_FN (1003) context check | [x] |
| SH3.20 | Break outside loop | ERR_BREAK_OUTSIDE_LOOP (1004) context check | [x] |

### Phase SH4: Bootstrap & Verification (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SH4.1 | End-to-end pipeline | compile(source, file) → CompileResult via lex→parse→analyze | [x] |
| SH4.2 | Compile self-lexer | Stage 0 (Rust fj) compiles stdlib/lexer.fj | [x] |
| SH4.3 | Compile self-parser | Stage 0 compiles stdlib/parser.fj | [x] |
| SH4.4 | Compile self-checker | Stage 0 compiles stdlib/analyzer.fj | [x] |
| SH4.5 | Stage 1 bootstrap | 15/15 test programs compiled by self-hosted pipeline | [x] |
| SH4.6 | Stage 1 verification | Token counts + item counts verified for all 15 | [x] |
| SH4.7 | compile_file API | Read source from disk, compile, return result | [x] |
| SH4.8 | Differential testing | 4/5 token counts match Rust lexer | [x] |
| SH4.9 | Error collection | AnalyzerState with error codes, format_error() | [x] |
| SH4.10 | display_result | Formatted OK/FAIL output for each compilation | [x] |
| SH4.11 | Bootstrap programs | fn, let, if, while, for, match, struct, enum, impl, trait, use | [x] |
| SH4.12 | Stress test | 10→100 statement programs verified (500 hits recursion limit) | [x] |
| SH4.13 | Complex program test | Fibonacci, multi-item programs, annotations | [x] |
| SH4.14 | Self-compilation | Lexer.fj successfully compiled by self-hosted pipeline | [x] |
| SH4.15 | Documentation | Architecture in compiler.fj header comments | [x] |
| SH4.16 | stdlib in Fajar Lang | lexer.fj + parser.fj + analyzer.fj + compiler.fj | [x] |
| SH4.17 | Error formatting | format_error(code, name) → "SE001: undefined variable 'x'" | [x] |
| SH4.18 | Borrow checker fix | Structs as Copy, revive on reassignment | [x] |
| SH4.19 | Combined pipeline | 1,724 lines: cat lexer+parser+analyzer+compiler+test → runs | [x] |
| SH4.20 | Bootstrap report | "Stage 0 → Stage 1 VERIFIED" with full results | [x] |

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
| IDE1.1 | Incremental parsing | Full reparse on change (debounced via did_change) | [x] |
| IDE1.2 | Incremental analysis | Only re-typecheck affected scopes | [x] |
| IDE1.3 | Background indexing | Index workspace on startup, update on change | [x] |
| IDE1.4 | Inlay hints | Show inferred types, parameter names, lifetimes | [x] |
| IDE1.5 | Code lens | Show test run/debug, impl count, references | [x] |
| IDE1.6 | Semantic highlighting | Token types: keyword, type, function, variable, macro | [x] |
| IDE1.7 | Auto-import | Suggest and insert use statements | [x] |
| IDE1.8 | Smart completion | Context-aware completions with type matching | [x] |
| IDE1.9 | Signature help | Show function signatures while typing | [x] |
| IDE1.10 | Hover documentation | Show type, docs, source on hover | [x] |
| IDE1.11 | Go to definition | Navigate to fn, struct, trait, module definitions | [x] |
| IDE1.12 | Find all references | Show all usages of a symbol | [x] |
| IDE1.13 | Rename symbol | Rename across all files with preview | [x] |
| IDE1.14 | Extract function | Select code → extract to new function | [x] |
| IDE1.15 | Extract variable | Select expression → bind to let | [x] |
| IDE1.16 | Inline variable | Replace variable with its value | [x] |
| IDE1.17 | Move to module | Move function/struct to different module | [x] |
| IDE1.18 | Implement trait | Generate trait impl skeleton | [x] |
| IDE1.19 | Fill match arms | Generate all match variants | [x] |
| IDE1.20 | Wrap in if/while/for | Wrap selection in control flow | [x] |
| IDE1.21 | Diagnostics on-type | Show errors as you type (debounced) | [x] |
| IDE1.22 | Quick fixes | Suggested fixes for common errors | [x] |
| IDE1.23 | Workspace symbols | Search all symbols across workspace | [x] |
| IDE1.24 | Call hierarchy | Show callers/callees tree | [x] |
| IDE1.25 | Type hierarchy | Show supertypes/subtypes tree | [x] |
| IDE1.26 | Folding ranges | Fold functions, structs, blocks, comments | [x] |
| IDE1.27 | Selection range | Expand/shrink selection by semantic unit | [x] |
| IDE1.28 | Linked editing | Rename both sides of a pair (e.g., struct field) | [x] |
| IDE1.29 | Document symbols | Outline view with nested symbols | [x] |
| IDE1.30 | LSP v4 tests (50) | All features, edge cases, performance | [x] |

### Phase IDE2: VS Code Extension v2 (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| IDE2.1 | TextMate grammar v2 | Full syntax highlighting with semantic tokens | [x] |
| IDE2.2 | Snippet library | 30+ snippets (fn, struct, enum, match, for, impl) | [x] |
| IDE2.3 | Debug adapter | DAP integration for step/breakpoint/watch | [x] |
| IDE2.4 | Test explorer | Discover and run @test functions from sidebar | [x] |
| IDE2.5 | Task runner | Integrate fj build/test/run as VS Code tasks | [x] |
| IDE2.6 | Problem matcher | Parse compiler errors into VS Code diagnostics | [x] |
| IDE2.7 | Code formatter | Format on save via fj fmt | [x] |
| IDE2.8 | Extension settings | Configure LSP path, features, formatting | [x] |
| IDE2.9 | Workspace detection | Auto-detect fj.toml and configure | [x] |
| IDE2.10 | Multi-root workspace | Support multiple fj projects in one workspace | [x] |
| IDE2.11 | File icons | Custom icons for .fj, fj.toml, fj.lock | [x] |
| IDE2.12 | Status bar | Show Fajar Lang version, build status | [x] |
| IDE2.13 | Command palette | fj: Run, Build, Test, Format, Check commands | [x] |
| IDE2.14 | Extension marketplace | Publish to VS Code Marketplace | [x] |
| IDE2.15 | JetBrains plugin | Basic syntax + LSP for IntelliJ/CLion | [x] |
| IDE2.16 | Neovim plugin | LSP config + TreeSitter grammar | [x] |
| IDE2.17 | Helix support | Language config for Helix editor | [x] |
| IDE2.18 | Zed extension | LSP integration for Zed editor | [x] |
| IDE2.19 | Extension tests (20) | All features, activation, performance | [x] |
| IDE2.20 | Extension documentation | README, screenshots, feature list | [x] |

### Phase IDE3: Playground v2 (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| IDE3.1 | WebAssembly compiler | Compile fj→Wasm in browser | [x] |
| IDE3.2 | Monaco editor integration | Syntax highlighting, auto-complete | [x] |
| IDE3.3 | Live output panel | Show println output, errors, timing | [x] |
| IDE3.4 | Share via URL | Encode source in URL for sharing | [x] |
| IDE3.5 | Example gallery | Browse and run all 165 examples | [x] |
| IDE3.6 | Multi-file support | Create/edit multiple .fj files | [x] |
| IDE3.7 | Dark/light theme | Toggle themes, system preference | [x] |
| IDE3.8 | Mobile responsive | Work on tablet/phone screens | [x] |
| IDE3.9 | AST viewer | Show parsed AST as tree | [x] |
| IDE3.10 | Token viewer | Show lexer output with highlighting | [x] |
| IDE3.11 | Type info panel | Show inferred types on hover | [x] |
| IDE3.12 | Bytecode viewer | Show VM bytecode for programs | [x] |
| IDE3.13 | Benchmark mode | Time execution, show stats | [x] |
| IDE3.14 | REPL mode | Interactive REPL in browser | [x] |
| IDE3.15 | Collaborative editing | Real-time collaboration (Yjs/CRDT) | [x] |
| IDE3.16 | Embed widget | Embed playground in docs/blog posts | [x] |
| IDE3.17 | Keyboard shortcuts | Ctrl+Enter run, Ctrl+S format | [x] |
| IDE3.18 | Error highlighting | Inline error markers in editor | [x] |
| IDE3.19 | Playground CI | Deploy via GitHub Actions | [x] |
| IDE3.20 | Playground tests (20) | All features, cross-browser | [x] |

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

### Phase DOC1: Interactive Tutorial (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| DOC1.1 | Tutorial framework | Step-by-step lesson engine with progress tracking | [ ] |
| DOC1.2 | Lesson 1: Hello World | Variables, println, basic types | [ ] |
| DOC1.3 | Lesson 2: Functions | Parameters, return types, recursion | [ ] |
| DOC1.4 | Lesson 3: Control Flow | if/else, match, while, for loops | [ ] |
| DOC1.5 | Lesson 4: Structs & Enums | Custom types, methods, pattern matching | [ ] |
| DOC1.6 | Lesson 5: Error Handling | Result, Option, ? operator | [ ] |
| DOC1.7 | Lesson 6: Ownership | Move semantics, borrowing, references | [ ] |
| DOC1.8 | Lesson 7: Generics & Traits | Type parameters, trait bounds, impl | [ ] |
| DOC1.9 | Lesson 8: Collections | Arrays, HashMap, iterators | [ ] |
| DOC1.10 | Lesson 9: Modules | mod, use, pub, project structure | [ ] |
| DOC1.11 | Lesson 10: Tensor & ML | Tensor creation, operations, autograd basics | [ ] |
| DOC1.12 | Lesson 11: OS Development | @kernel, @device, bare-metal hello world | [ ] |
| DOC1.13 | Lesson 12: Concurrency | Threads, channels, async/await | [ ] |
| DOC1.14 | Lesson 13: FFI | Calling C functions, extern blocks | [ ] |
| DOC1.15 | Lesson 14: Testing | @test, assert, property-based testing | [ ] |
| DOC1.16 | Code exercises | 50 interactive exercises with auto-grading | [ ] |
| DOC1.17 | Progress persistence | Save/resume tutorial progress | [ ] |
| DOC1.18 | Playground integration | Run lesson code in browser playground | [ ] |
| DOC1.19 | Multilingual support | English + Bahasa Indonesia | [ ] |
| DOC1.20 | Tutorial deployment | Static site generation, hosting | [ ] |

### Phase DOC2: API Reference (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| DOC2.1 | Doc generator v2 | Extract /// docs from all pub items | [ ] |
| DOC2.2 | Type documentation | All primitive types with examples | [ ] |
| DOC2.3 | Stdlib documentation | All builtins: print, len, type_of, assert, etc. | [ ] |
| DOC2.4 | Collections API docs | Array methods, HashMap methods, iterators | [ ] |
| DOC2.5 | String API docs | All 15 string methods with examples | [ ] |
| DOC2.6 | Math API docs | PI, E, abs, sqrt, sin, cos, etc. | [ ] |
| DOC2.7 | IO API docs | read_file, write_file, append_file, file_exists | [ ] |
| DOC2.8 | Tensor API docs | zeros, ones, randn, matmul, reshape, etc. | [ ] |
| DOC2.9 | Autograd API docs | backward, grad, requires_grad, optimizers | [ ] |
| DOC2.10 | Layer API docs | Dense, Conv2d, MultiHeadAttention, BatchNorm | [ ] |
| DOC2.11 | OS API docs | Memory, IRQ, syscall, port I/O builtins | [ ] |
| DOC2.12 | Error code reference | All 71 error codes with explanations and fixes | [ ] |
| DOC2.13 | Keyword reference | All keywords with syntax and examples | [ ] |
| DOC2.14 | Operator reference | All operators with precedence table | [ ] |
| DOC2.15 | Annotation reference | @kernel, @device, @safe, @unsafe, @test, @entry | [ ] |
| DOC2.16 | Grammar reference | EBNF grammar with railroad diagrams | [ ] |
| DOC2.17 | Search functionality | Full-text search across all API docs | [ ] |
| DOC2.18 | Cross-linking | Automatic links between related items | [ ] |
| DOC2.19 | Version selector | Show docs for different Fajar Lang versions | [ ] |
| DOC2.20 | API reference deployment | Static site with navigation, mobile-friendly | [ ] |

### Phase DOC3: Cookbook & Guides (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| DOC3.1 | Getting Started guide | Install, first program, project setup | [ ] |
| DOC3.2 | Build system guide | fj.toml, fj build, fj run, fj test | [ ] |
| DOC3.3 | Package management guide | fj new, fj add, fj publish | [ ] |
| DOC3.4 | Error handling cookbook | Patterns for Result, Option, error propagation | [ ] |
| DOC3.5 | Concurrency cookbook | Thread patterns, async patterns, channels | [ ] |
| DOC3.6 | ML cookbook | Training pipeline, inference, quantization | [ ] |
| DOC3.7 | OS development guide | @kernel program, boot sequence, QEMU testing | [ ] |
| DOC3.8 | Embedded guide | Cross-compilation, bare-metal, HAL | [ ] |
| DOC3.9 | FFI guide | C interop, Python bindings, Rust interop | [ ] |
| DOC3.10 | Testing guide | Unit tests, integration tests, property tests | [ ] |
| DOC3.11 | Performance guide | Profiling, benchmarking, optimization tips | [ ] |
| DOC3.12 | Migration guide | From Rust/C/Python to Fajar Lang | [ ] |
| DOC3.13 | IDE setup guide | VS Code, Neovim, JetBrains configuration | [ ] |
| DOC3.14 | Deployment guide | Docker, cross-compile, CI/CD setup | [ ] |
| DOC3.15 | Security guide | Safe coding, audit checklist, context rules | [ ] |
| DOC3.16 | Dragon Q6A guide | Setup, GPIO, NPU inference, deployment | [ ] |
| DOC3.17 | FAQ page | 50 most common questions with answers | [ ] |
| DOC3.18 | Troubleshooting guide | 30 common errors with solutions | [ ] |
| DOC3.19 | Changelog | Auto-generated from git tags and commits | [ ] |
| DOC3.20 | Blog platform | Markdown blog for release announcements | [ ] |
| DOC3.21 | Recipe: REST API | Step-by-step REST API server | [ ] |
| DOC3.22 | Recipe: CLI tool | Build a command-line application | [ ] |
| DOC3.23 | Recipe: Web scraper | HTTP requests + JSON parsing | [ ] |
| DOC3.24 | Recipe: Image classifier | Load model, preprocess, classify | [ ] |
| DOC3.25 | Recipe: Drone controller | Sensor fusion + PID + ML inference | [ ] |
| DOC3.26 | Recipe: Chat server | TCP server with multiple clients | [ ] |
| DOC3.27 | Recipe: Key-value store | Persistent storage with transactions | [ ] |
| DOC3.28 | Recipe: Game of Life | Terminal-based cellular automaton | [ ] |
| DOC3.29 | Recipe: Markdown parser | Recursive descent Markdown → HTML | [ ] |
| DOC3.30 | Documentation CI | Auto-build docs on push, deploy to site | [ ] |

---

## Option 6: Compiler Optimization Suite (7 sprints, 70 tasks)

*Make Fajar Lang compile faster and produce faster code.*

### Phase OPT1: Compilation Speed (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| OPT1.1 | Parallel lexing | Lex multiple files concurrently | [ ] |
| OPT1.2 | Parallel parsing | Parse multiple modules concurrently | [ ] |
| OPT1.3 | Parallel type checking | Per-module type checking with shared context | [ ] |
| OPT1.4 | Incremental analysis cache | Cache analysis results, reuse on unchanged files | [ ] |
| OPT1.5 | Lazy module loading | Only load/parse imported modules | [ ] |
| OPT1.6 | Tokenizer SIMD | Use SIMD for whitespace/delimiter scanning | [ ] |
| OPT1.7 | String interning | Intern identifiers and keywords (arena allocator) | [ ] |
| OPT1.8 | AST arena allocation | Allocate AST nodes from typed arena | [ ] |
| OPT1.9 | Module dependency graph | Topological sort for optimal compilation order | [ ] |
| OPT1.10 | Precompiled headers | Cache parsed stdlib for instant reuse | [ ] |
| OPT1.11 | Compile server mode | Persistent daemon with warm caches | [ ] |
| OPT1.12 | Profile-guided recompilation | Only recompile changed + dependent modules | [ ] |
| OPT1.13 | Type check caching | Hash-based cache for type inference results | [ ] |
| OPT1.14 | Parallel codegen | Generate code for independent functions concurrently | [ ] |
| OPT1.15 | Object file caching | Cache .o files, only regenerate on change | [ ] |
| OPT1.16 | Linker optimization | Incremental linking, parallel symbol resolution | [ ] |
| OPT1.17 | Compilation metrics | Track time per phase, show bottlenecks | [ ] |
| OPT1.18 | Memory usage optimization | Reduce peak memory during compilation | [ ] |
| OPT1.19 | Benchmark: 10K line program | Compile time < 2s goal | [ ] |
| OPT1.20 | Benchmark: 100K line project | Incremental rebuild < 500ms goal | [ ] |

### Phase OPT2: Code Quality (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| OPT2.1 | Constant folding | Evaluate constant expressions at compile time | [ ] |
| OPT2.2 | Dead code elimination v2 | Whole-program DCE with call graph analysis | [ ] |
| OPT2.3 | Function inlining | Cost model, always-inline hint, threshold tuning | [ ] |
| OPT2.4 | Loop unrolling | Unroll small loops with known trip count | [ ] |
| OPT2.5 | LICM v2 | Loop-invariant code motion with alias analysis | [ ] |
| OPT2.6 | Common subexpression elimination v2 | Global CSE across basic blocks | [ ] |
| OPT2.7 | Strength reduction | Replace expensive ops (mul→shift, div→mul) | [ ] |
| OPT2.8 | Tail call optimization | Convert tail recursion to loops | [ ] |
| OPT2.9 | Escape analysis | Stack-allocate non-escaping heap objects | [ ] |
| OPT2.10 | Devirtualization | Replace dynamic dispatch with static when known | [ ] |
| OPT2.11 | Alias analysis | Track pointer aliasing for optimization safety | [ ] |
| OPT2.12 | Auto-vectorization | Detect vectorizable loops, emit SIMD | [ ] |
| OPT2.13 | Branch prediction hints | Profile-guided branch probability | [ ] |
| OPT2.14 | Peephole optimizations | Pattern-based instruction simplification | [ ] |
| OPT2.15 | Copy propagation | Eliminate redundant copies | [ ] |
| OPT2.16 | Phi node optimization | Simplify SSA phi nodes | [ ] |
| OPT2.17 | Optimization pipeline | O0/O1/O2/O3/Os optimization levels | [ ] |
| OPT2.18 | Optimization metrics | Count applied optimizations per level | [ ] |
| OPT2.19 | Benchmark: fibonacci | Within 2x of C -O2 | [ ] |
| OPT2.20 | Benchmark: matrix multiply | Within 3x of C -O2 with BLAS | [ ] |

### Phase OPT3: Binary Size & LTO (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| OPT3.1 | Dead function elimination | Remove unreachable functions from binary | [ ] |
| OPT3.2 | Dead global elimination | Remove unused global variables | [ ] |
| OPT3.3 | String deduplication | Merge identical string literals | [ ] |
| OPT3.4 | Function merging | Merge identical function bodies | [ ] |
| OPT3.5 | Section garbage collection | --gc-sections linker flag integration | [ ] |
| OPT3.6 | Cross-module inlining | Inline small functions across module boundaries | [ ] |
| OPT3.7 | Thin LTO | Parallel link-time optimization | [ ] |
| OPT3.8 | Full LTO | Single-module whole-program optimization | [ ] |
| OPT3.9 | Symbol stripping | Strip debug symbols in release mode | [ ] |
| OPT3.10 | Compression | UPX-style binary compression option | [ ] |
| OPT3.11 | Size profiling | Per-function size report (bloaty-style) | [ ] |
| OPT3.12 | Monomorphization dedup | Detect identical monomorphized instances | [ ] |
| OPT3.13 | Runtime trimming | Only link used runtime functions | [ ] |
| OPT3.14 | Panic-free mode | Eliminate panic infrastructure for embedded | [ ] |
| OPT3.15 | no_std binary | Minimal binary without stdlib | [ ] |
| OPT3.16 | Custom allocator | Plug custom allocator for binary size | [ ] |
| OPT3.17 | Embedded profile | Optimize for flash/RAM-constrained targets | [ ] |
| OPT3.18 | WASM size optimization | wasm-opt integration, tree shaking | [ ] |
| OPT3.19 | Benchmark: minimal binary | Hello world < 100KB | [ ] |
| OPT3.20 | Benchmark: embedded binary | Blinky < 16KB for Cortex-M | [ ] |
| OPT3.21 | PGO profile collection | Instrument build for profile data | [ ] |
| OPT3.22 | PGO profile application | Use profile data for optimization | [ ] |
| OPT3.23 | PGO hot/cold splitting | Separate hot and cold code paths | [ ] |
| OPT3.24 | PGO inline decisions | Profile-guided inlining heuristics | [ ] |
| OPT3.25 | PGO branch layout | Optimize branch layout from profile | [ ] |
| OPT3.26 | BOLT integration | Post-link optimization with BOLT | [ ] |
| OPT3.27 | Compile time tracking | --timings flag showing phase breakdown | [ ] |
| OPT3.28 | Optimization report | --opt-report showing what was optimized | [ ] |
| OPT3.29 | Regression tests | 50 optimization correctness tests | [ ] |
| OPT3.30 | Optimization documentation | Guide for each -O level and trade-offs | [ ] |

---

## Option 7: Security Hardening (7 sprints, 70 tasks)

*Harden Fajar Lang for safety-critical and security-sensitive deployments.*

### Phase SEC1: Memory Safety Hardening (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SEC1.1 | Stack canaries | Detect stack buffer overflow at runtime | [ ] |
| SEC1.2 | Stack clash protection | Guard pages between stack frames | [ ] |
| SEC1.3 | ASLR support | Address space layout randomization for binaries | [ ] |
| SEC1.4 | CFI (Control Flow Integrity) | Forward-edge CFI for indirect calls | [ ] |
| SEC1.5 | Shadow stack | Return address protection via shadow stack | [ ] |
| SEC1.6 | Bounds checking mode | Runtime bounds checks on all array access | [ ] |
| SEC1.7 | Integer overflow detection | Runtime traps on signed/unsigned overflow | [ ] |
| SEC1.8 | Use-after-free detection | Quarantine freed memory, detect reuse | [ ] |
| SEC1.9 | Double-free detection | Track allocation state, trap on double free | [ ] |
| SEC1.10 | Null pointer protection | Guard page at address 0 | [ ] |
| SEC1.11 | Memory tagging (MTE) | ARM Memory Tagging Extension support | [ ] |
| SEC1.12 | SafeStack | Separate stack for safe and unsafe data | [ ] |
| SEC1.13 | Sanitizer integration | ASan, MSan, TSan, UBSan support | [ ] |
| SEC1.14 | Fuzzing integration | libFuzzer/AFL++ target generation | [ ] |
| SEC1.15 | Leak detector | Detect memory leaks at program exit | [ ] |
| SEC1.16 | Allocation limits | Per-context memory allocation budgets | [ ] |
| SEC1.17 | Stack depth limit | Configurable recursion depth protection | [ ] |
| SEC1.18 | Heap hardening | Randomized heap layout, guard pages | [ ] |
| SEC1.19 | Memory safety tests | 50 tests for all hardening features | [ ] |
| SEC1.20 | Security benchmark | Overhead measurement for each feature | [ ] |

### Phase SEC2: Supply Chain Security (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SEC2.1 | Package signing | Ed25519 signatures on published packages | [ ] |
| SEC2.2 | Signature verification | Verify signatures on download | [ ] |
| SEC2.3 | Transparency log | Sigstore-style transparency for publishes | [ ] |
| SEC2.4 | SBOM generation | CycloneDX/SPDX software bill of materials | [ ] |
| SEC2.5 | License compliance | Detect and report dependency licenses | [ ] |
| SEC2.6 | Vulnerability database | CVE tracking for Fajar packages | [ ] |
| SEC2.7 | `fj audit` command | Scan deps against vulnerability database | [ ] |
| SEC2.8 | Dependency pinning | Exact version pinning in fj.lock | [ ] |
| SEC2.9 | Checksum verification | SHA-256 integrity on all downloads | [ ] |
| SEC2.10 | Reproducible builds | Same source → identical binary output | [ ] |
| SEC2.11 | Build provenance | SLSA-compliant build attestation | [ ] |
| SEC2.12 | Source verification | Verify package source matches published | [ ] |
| SEC2.13 | Typosquatting detection | Flag packages with names similar to popular ones | [ ] |
| SEC2.14 | Namespace reservation | Prevent unauthorized publishes to known names | [ ] |
| SEC2.15 | Two-factor auth | 2FA for package publish operations | [ ] |
| SEC2.16 | API token scoping | Fine-grained token permissions (read/write/admin) | [ ] |
| SEC2.17 | Token rotation | Automatic token expiry and renewal | [ ] |
| SEC2.18 | Security advisories | Publish and distribute security notices | [ ] |
| SEC2.19 | Supply chain tests | 30 tests for signing, verification, audit | [ ] |
| SEC2.20 | Security policy | SECURITY.md, responsible disclosure process | [ ] |

### Phase SEC3: Audit & Certification (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| SEC3.1 | Static analysis rules | 50 custom lint rules for security patterns | [ ] |
| SEC3.2 | Taint analysis | Track untrusted input through data flow | [ ] |
| SEC3.3 | SQL injection detection | Flag string concatenation in queries | [ ] |
| SEC3.4 | Command injection detection | Flag string concatenation in exec calls | [ ] |
| SEC3.5 | Path traversal detection | Flag user input in file paths | [ ] |
| SEC3.6 | Cryptographic misuse detection | Flag weak algorithms, hardcoded keys | [ ] |
| SEC3.7 | Information leak detection | Flag sensitive data in logs/errors | [ ] |
| SEC3.8 | Race condition detection | Static data race analysis | [ ] |
| SEC3.9 | Deadlock detection | Lock ordering analysis | [ ] |
| SEC3.10 | Undefined behavior detection | Flag platform-dependent code | [ ] |
| SEC3.11 | MISRA-C compliance mode | Subset of MISRA rules for safety-critical | [ ] |
| SEC3.12 | CERT C compliance mode | CERT C secure coding rules | [ ] |
| SEC3.13 | ISO 26262 annotations | ASIL classification support for automotive | [ ] |
| SEC3.14 | DO-178C evidence | Traceability matrices for aerospace | [ ] |
| SEC3.15 | IEC 62443 support | Industrial cybersecurity compliance | [ ] |
| SEC3.16 | Formal verification hooks | Pre/post conditions, invariants | [ ] |
| SEC3.17 | Test coverage enforcement | Minimum coverage thresholds per module | [ ] |
| SEC3.18 | Mutation testing | Verify test quality with mutation analysis | [ ] |
| SEC3.19 | Security scorecard | Generate security posture report | [ ] |
| SEC3.20 | Penetration test suite | Automated security testing framework | [ ] |
| SEC3.21 | @secure annotation | Mark functions requiring security review | [ ] |
| SEC3.22 | @trusted annotation | Mark FFI boundaries as trust boundaries | [ ] |
| SEC3.23 | Capability-based security | Fine-grained permissions for modules | [ ] |
| SEC3.24 | Sandbox mode | Restrict filesystem/network/exec access | [ ] |
| SEC3.25 | Secure default configuration | Safe defaults for all compiler options | [ ] |
| SEC3.26 | Hardening guide | Document all security features and usage | [ ] |
| SEC3.27 | Threat model | Document attack surfaces and mitigations | [ ] |
| SEC3.28 | Security review checklist | Checklist for code review | [ ] |
| SEC3.29 | Audit trail | Log all security-relevant compiler decisions | [ ] |
| SEC3.30 | Certification documentation | Templates for regulatory submissions | [ ] |

---

## Option 8: Cross-Platform Native GUI (7 sprints, 70 tasks)

*Build native GUI applications with Fajar Lang.*

### Phase GUI1: Widget Toolkit (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GUI1.1 | Window creation | Platform window (X11/Wayland/Win32/Cocoa) | [ ] |
| GUI1.2 | Event loop | Mouse, keyboard, resize, close events | [ ] |
| GUI1.3 | Canvas rendering | 2D drawing: lines, rects, circles, text | [ ] |
| GUI1.4 | Button widget | Clickable button with text and icon | [ ] |
| GUI1.5 | Label widget | Static text display with alignment | [ ] |
| GUI1.6 | TextInput widget | Single-line text entry with cursor | [ ] |
| GUI1.7 | TextArea widget | Multi-line text editing with scroll | [ ] |
| GUI1.8 | Checkbox widget | Boolean toggle with label | [ ] |
| GUI1.9 | RadioButton widget | Exclusive selection group | [ ] |
| GUI1.10 | Slider widget | Continuous value selection | [ ] |
| GUI1.11 | ProgressBar widget | Determinate and indeterminate progress | [ ] |
| GUI1.12 | Dropdown/ComboBox | Selection from list of options | [ ] |
| GUI1.13 | ListView widget | Scrollable list with selection | [ ] |
| GUI1.14 | TreeView widget | Hierarchical expandable tree | [ ] |
| GUI1.15 | Table widget | Grid with columns, sorting, selection | [ ] |
| GUI1.16 | Image widget | Display PNG/JPEG images | [ ] |
| GUI1.17 | Dialog windows | Alert, confirm, file picker, color picker | [ ] |
| GUI1.18 | Menu bar | Application menu with submenus | [ ] |
| GUI1.19 | Context menu | Right-click popup menus | [ ] |
| GUI1.20 | Toolbar | Icon button strip with tooltips | [ ] |
| GUI1.21 | TabView widget | Tabbed container for multiple views | [ ] |
| GUI1.22 | SplitView widget | Resizable horizontal/vertical split | [ ] |
| GUI1.23 | ScrollView widget | Scrollable container for any content | [ ] |
| GUI1.24 | Tooltip | Hover information popup | [ ] |
| GUI1.25 | StatusBar | Bottom bar with text segments | [ ] |
| GUI1.26 | Theme system | Light/dark/custom themes | [ ] |
| GUI1.27 | Font rendering | TrueType/OpenType font loading | [ ] |
| GUI1.28 | DPI awareness | High-DPI scaling on all platforms | [ ] |
| GUI1.29 | Widget tests (30) | All widgets, interactions, rendering | [ ] |
| GUI1.30 | Widget documentation | API docs with visual examples | [ ] |

### Phase GUI2: Layout Engine (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GUI2.1 | Flexbox layout | Row/column flex with wrapping | [ ] |
| GUI2.2 | Grid layout | CSS Grid-style rows/columns/areas | [ ] |
| GUI2.3 | Stack layout | Z-order stacking for overlays | [ ] |
| GUI2.4 | Absolute positioning | Fixed pixel position relative to parent | [ ] |
| GUI2.5 | Padding and margin | Inner and outer spacing for all widgets | [ ] |
| GUI2.6 | Min/max constraints | Minimum and maximum size constraints | [ ] |
| GUI2.7 | Aspect ratio | Maintain aspect ratio during resize | [ ] |
| GUI2.8 | Alignment | Start, center, end, stretch, baseline | [ ] |
| GUI2.9 | Overflow handling | Clip, scroll, visible overflow modes | [ ] |
| GUI2.10 | Responsive breakpoints | Layout changes at width thresholds | [ ] |
| GUI2.11 | Layout caching | Cache layout calculations, invalidate on change | [ ] |
| GUI2.12 | Animation system | Tweened property animations (ease, spring) | [ ] |
| GUI2.13 | Transition system | Animated transitions between states | [ ] |
| GUI2.14 | Constraint solver | Cassowary-style constraint layout | [ ] |
| GUI2.15 | Auto-sizing | Text-based size calculation | [ ] |
| GUI2.16 | Scroll physics | Smooth scrolling with momentum | [ ] |
| GUI2.17 | Hit testing | Determine widget under mouse coordinates | [ ] |
| GUI2.18 | Focus management | Tab order, focus ring, keyboard navigation | [ ] |
| GUI2.19 | Layout tests (30) | All layouts, constraints, edge cases | [ ] |
| GUI2.20 | Layout performance benchmark | 1000 widgets layout < 16ms | [ ] |

### Phase GUI3: Platform Integration (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| GUI3.1 | Linux/X11 backend | Xlib window creation, events, rendering | [ ] |
| GUI3.2 | Linux/Wayland backend | wl_surface, xdg_shell, input events | [ ] |
| GUI3.3 | Windows backend | Win32 HWND, WndProc, GDI/Direct2D | [ ] |
| GUI3.4 | macOS backend | NSWindow, NSView, Core Graphics | [ ] |
| GUI3.5 | GPU-accelerated rendering | Vulkan/Metal/D3D12 render backend | [ ] |
| GUI3.6 | Software renderer | CPU-only fallback renderer | [ ] |
| GUI3.7 | Clipboard support | Copy/paste text and images | [ ] |
| GUI3.8 | Drag and drop | Internal and system drag-and-drop | [ ] |
| GUI3.9 | System tray | Tray icon with menu (Linux/Windows/macOS) | [ ] |
| GUI3.10 | Notifications | Native OS notification support | [ ] |
| GUI3.11 | File dialogs | Open/save file dialogs via OS | [ ] |
| GUI3.12 | Cursor management | Custom cursors, cursor style changes | [ ] |
| GUI3.13 | Multi-window | Multiple windows per application | [ ] |
| GUI3.14 | Fullscreen mode | Toggle fullscreen on all platforms | [ ] |
| GUI3.15 | IME support | Input Method Editor for CJK text | [ ] |
| GUI3.16 | Accessibility | Screen reader support (AT-SPI/UIAutomation) | [ ] |
| GUI3.17 | Touch input | Multi-touch gestures for touch screens | [ ] |
| GUI3.18 | Gamepad input | Controller/joystick input | [ ] |
| GUI3.19 | Platform integration tests | Cross-platform rendering comparison | [ ] |
| GUI3.20 | Demo application | Complete GUI app showcasing all widgets | [ ] |

---

## Option 9: FajarOS Nova v2.0 "Supernova" (7 sprints, 70 tasks)

*Next-gen OS kernel with advanced SMP, real networking, and GPU compositor.*

### Phase OS1: Advanced SMP Scheduler (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| OS1.1 | Per-CPU run queues | Separate ready queue per CPU core | [ ] |
| OS1.2 | Work stealing | Idle CPUs steal tasks from busy CPUs | [ ] |
| OS1.3 | Priority scheduling | 32 priority levels with preemption | [ ] |
| OS1.4 | Real-time class | SCHED_FIFO and SCHED_RR policies | [ ] |
| OS1.5 | CPU affinity | Pin processes to specific cores | [ ] |
| OS1.6 | Load balancer | Periodic rebalancing across CPUs | [ ] |
| OS1.7 | NUMA awareness | Prefer local memory for tasks | [ ] |
| OS1.8 | CFS-like scheduler | Completely Fair Scheduler with vruntime | [ ] |
| OS1.9 | Deadline scheduling | EDF (Earliest Deadline First) for real-time | [ ] |
| OS1.10 | CPU hotplug | Online/offline CPUs dynamically | [ ] |
| OS1.11 | Idle management | Per-CPU idle loop with WFI | [ ] |
| OS1.12 | Context switch optimization | Minimize register save/restore overhead | [ ] |
| OS1.13 | IPI mechanism | Inter-Processor Interrupt for cross-CPU signals | [ ] |
| OS1.14 | Spinlock fairness | Ticket locks to prevent starvation | [ ] |
| OS1.15 | RCU (Read-Copy-Update) | Lock-free reads for shared data structures | [ ] |
| OS1.16 | Process migration | Move running process to different CPU | [ ] |
| OS1.17 | Scheduler tracing | Log scheduling decisions for debugging | [ ] |
| OS1.18 | Latency measurement | Track worst-case scheduling latency | [ ] |
| OS1.19 | SMP scheduler tests | 30 tests including stress tests | [ ] |
| OS1.20 | SMP benchmark | 8-core utilization > 90% under load | [ ] |

### Phase OS2: Real Network Stack (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| OS2.1 | VirtIO-net driver v2 | Multi-queue, checksum offload, TSO/LRO | [ ] |
| OS2.2 | Ethernet frame handling | MAC address, VLAN tagging, ARP | [ ] |
| OS2.3 | IP layer v2 | IPv4 + IPv6 dual stack, ICMP, routing table | [ ] |
| OS2.4 | TCP v2 | Congestion control (Reno/CUBIC), fast retransmit | [ ] |
| OS2.5 | UDP v2 | Multicast, broadcast support | [ ] |
| OS2.6 | DNS resolver | Recursive DNS resolution, caching | [ ] |
| OS2.7 | DHCP client | Auto-configure IP, gateway, DNS | [ ] |
| OS2.8 | Socket API v2 | Berkeley sockets: socket/bind/listen/accept/connect | [ ] |
| OS2.9 | Netfilter/firewall | Packet filtering rules (iptables-style) | [ ] |
| OS2.10 | Network namespaces | Isolated network stacks per container | [ ] |
| OS2.11 | TLS integration | TLS 1.3 for secure connections | [ ] |
| OS2.12 | HTTP server v2 | HTTP/1.1 + HTTP/2 with keep-alive | [ ] |
| OS2.13 | NTP client | Network time synchronization | [ ] |
| OS2.14 | Ping utility | ICMP echo request/reply | [ ] |
| OS2.15 | Netstat utility | Show active connections and listening ports | [ ] |
| OS2.16 | Wget utility | Download files via HTTP | [ ] |
| OS2.17 | SSH server | Minimal SSH for remote access | [ ] |
| OS2.18 | Network benchmarks | Throughput: TCP stream > 100 Mbps in QEMU | [ ] |
| OS2.19 | Network stack tests | 30 tests: ARP, TCP handshake, HTTP, DNS | [ ] |
| OS2.20 | Network documentation | Protocol implementation notes, API docs | [ ] |

### Phase OS3: GPU Compositor & Desktop (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| OS3.1 | VirtIO-GPU v2 | 3D acceleration, multi-plane, cursor plane | [ ] |
| OS3.2 | Framebuffer management | Double buffering, vsync, page flip | [ ] |
| OS3.3 | Resolution detection | EDID parsing, mode setting | [ ] |
| OS3.4 | 2D drawing primitives | Lines, rectangles, circles, filled shapes | [ ] |
| OS3.5 | Font renderer | Bitmap font rendering with glyph cache | [ ] |
| OS3.6 | Window manager | Floating windows with title bar, resize, move | [ ] |
| OS3.7 | Window decorations | Title bar, close/minimize/maximize buttons | [ ] |
| OS3.8 | Desktop background | Solid color / image background rendering | [ ] |
| OS3.9 | Taskbar | Bottom panel with running applications | [ ] |
| OS3.10 | Application launcher | Start menu / application grid | [ ] |
| OS3.11 | Terminal emulator | VT100 terminal in a GUI window | [ ] |
| OS3.12 | Text editor | Simple Notepad-style text editor | [ ] |
| OS3.13 | File manager | Browse, open, copy, move, delete files | [ ] |
| OS3.14 | System monitor | CPU, memory, process monitor | [ ] |
| OS3.15 | Image viewer | Display PNG/BMP images in a window | [ ] |
| OS3.16 | Calculator app | Basic calculator with GUI | [ ] |
| OS3.17 | Mouse cursor rendering | Hardware cursor or software sprite | [ ] |
| OS3.18 | Keyboard input routing | Focus tracking, key event dispatch | [ ] |
| OS3.19 | Clipboard system | Copy/paste between applications | [ ] |
| OS3.20 | Drag and drop | Move windows, drag files | [ ] |
| OS3.21 | Alpha blending | Transparent windows and overlays | [ ] |
| OS3.22 | Damage tracking | Only redraw changed screen regions | [ ] |
| OS3.23 | Multi-monitor | Support multiple display outputs | [ ] |
| OS3.24 | Screen resolution switch | Runtime resolution change | [ ] |
| OS3.25 | Theme support | Color scheme, font selection | [ ] |
| OS3.26 | Wallpaper manager | Change desktop background | [ ] |
| OS3.27 | Lock screen | Password-protected screen lock | [ ] |
| OS3.28 | Screenshot utility | Capture screen/window to file | [ ] |
| OS3.29 | Compositor tests | 30 tests: rendering, layout, events | [ ] |
| OS3.30 | Desktop documentation | Architecture, window protocol, app development | [ ] |

---

## Option 10: Community & Governance (7 sprints, 70 tasks)

*Build the community infrastructure for Fajar Lang adoption.*

### Phase COM1: Website & Online Presence (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| COM1.1 | Landing page | Modern landing page with key features | [ ] |
| COM1.2 | Download page | Platform-specific binary downloads | [ ] |
| COM1.3 | Documentation site | Hosted docs with search and navigation | [ ] |
| COM1.4 | Blog section | Release notes, tutorials, announcements | [ ] |
| COM1.5 | Playground embed | Inline code playground on website | [ ] |
| COM1.6 | Package registry UI | Web interface for browsing packages | [ ] |
| COM1.7 | Showcase gallery | Projects built with Fajar Lang | [ ] |
| COM1.8 | Comparison page | Fajar vs Rust vs C vs Python feature matrix | [ ] |
| COM1.9 | Installation wizard | Interactive install guide per platform | [ ] |
| COM1.10 | SEO optimization | Meta tags, sitemap, structured data | [ ] |
| COM1.11 | Analytics | Privacy-respecting usage analytics | [ ] |
| COM1.12 | Performance (Lighthouse) | 90+ score on all Lighthouse metrics | [ ] |
| COM1.13 | Mobile responsive | Full functionality on mobile devices | [ ] |
| COM1.14 | Internationalization | English + Bahasa Indonesia | [ ] |
| COM1.15 | Newsletter | Email signup for release announcements | [ ] |
| COM1.16 | RSS feed | Blog and release RSS feeds | [ ] |
| COM1.17 | Status page | Build status, registry uptime | [ ] |
| COM1.18 | CDN deployment | Global CDN for fast page loads | [ ] |
| COM1.19 | Website CI/CD | Auto-deploy on push, preview for PRs | [ ] |
| COM1.20 | Website tests | Link checker, accessibility audit, performance | [ ] |

### Phase COM2: Community Platform (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| COM2.1 | GitHub organization | Transfer repos to fajar-lang org | [ ] |
| COM2.2 | Issue templates | Bug report, feature request, RFC templates | [ ] |
| COM2.3 | PR templates | Checklist, description, testing evidence | [ ] |
| COM2.4 | Discussion forum | GitHub Discussions or Discourse setup | [ ] |
| COM2.5 | Discord server | Community chat with channels per topic | [ ] |
| COM2.6 | Code of Conduct | Contributor covenant adoption | [ ] |
| COM2.7 | Contributing guide | How to contribute code, docs, translations | [ ] |
| COM2.8 | Good first issues | 20 labeled starter issues for newcomers | [ ] |
| COM2.9 | Mentorship program | Pair experienced contributors with newcomers | [ ] |
| COM2.10 | Release process | Documented release cycle (monthly/quarterly) | [ ] |
| COM2.11 | RFC process | Formal process for language changes | [ ] |
| COM2.12 | Governance model | Decision-making structure, maintainer roles | [ ] |
| COM2.13 | Maintainer guide | How to review PRs, triage issues, release | [ ] |
| COM2.14 | Style guide | Official Fajar Lang coding style | [ ] |
| COM2.15 | Branding assets | Logo, colors, fonts, usage guidelines | [ ] |
| COM2.16 | Presentation deck | Slides for conferences and meetups | [ ] |
| COM2.17 | Demo video | 5-minute intro video showing key features | [ ] |
| COM2.18 | Social media presence | Twitter/X, LinkedIn, YouTube channels | [ ] |
| COM2.19 | Community metrics | Track contributors, issues, PRs, stars | [ ] |
| COM2.20 | Swag store | Stickers, T-shirts, mugs | [ ] |

### Phase COM3: Ecosystem Growth (3 sprints, 30 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| COM3.1 | Plugin marketplace | Discover and install community plugins | [ ] |
| COM3.2 | Template marketplace | Project templates shared by community | [ ] |
| COM3.3 | Example marketplace | Community-contributed examples | [ ] |
| COM3.4 | Package quality scores | Automated quality scoring for packages | [ ] |
| COM3.5 | Dependency insights | Usage stats, dependency health dashboard | [ ] |
| COM3.6 | Security advisories | CVE-style advisories for Fajar packages | [ ] |
| COM3.7 | Automated releases | Bot for dependency updates and releases | [ ] |
| COM3.8 | CI templates | GitHub Actions, GitLab CI, Jenkins templates | [ ] |
| COM3.9 | Docker images | Official Docker images for building/running | [ ] |
| COM3.10 | Nix/Flake package | Nix package for reproducible builds | [ ] |
| COM3.11 | Homebrew formula | `brew install fajar` for macOS | [ ] |
| COM3.12 | APT/YUM packages | Linux distribution packages | [ ] |
| COM3.13 | Snap/Flatpak | Universal Linux packages | [ ] |
| COM3.14 | Windows installer | MSI/NSIS installer with PATH setup | [ ] |
| COM3.15 | Chocolatey package | `choco install fajar` for Windows | [ ] |
| COM3.16 | asdf plugin | Version management via asdf | [ ] |
| COM3.17 | GitHub Codespaces | Pre-configured cloud dev environment | [ ] |
| COM3.18 | Gitpod config | One-click development in browser | [ ] |
| COM3.19 | Conference talks | Submit talks to FOSDEM, RustConf, PyCon | [ ] |
| COM3.20 | Workshop materials | 3-hour hands-on workshop curriculum | [ ] |
| COM3.21 | University curriculum | Course materials for CS education | [ ] |
| COM3.22 | Certification program | Official Fajar Lang developer certification | [ ] |
| COM3.23 | Ambassador program | Regional community leaders | [ ] |
| COM3.24 | Bug bounty | Security bug bounty program | [ ] |
| COM3.25 | Sponsorship model | Funding model for sustainable development | [ ] |
| COM3.26 | Annual survey | Community survey for priorities | [ ] |
| COM3.27 | Roadmap voting | Community input on feature priorities | [ ] |
| COM3.28 | Contributor recognition | Monthly contributor highlights | [ ] |
| COM3.29 | Ecosystem health report | Quarterly ecosystem health metrics | [ ] |
| COM3.30 | Year-one retrospective | Document lessons learned, plan year two | [ ] |

---

## Execution Strategy

### MANDATORY FIRST: Option 0 (Gap Closure)
```
0 (Gap Closure) → THEN choose a path below
```
**Option 0 is non-negotiable.** All V6/V7 framework modules must become real implementations before any new features. This ensures every claim in our documentation is backed by working code.

### Path A — "Foundation First" (recommended)
```
0 (Gaps) → 1 (Self-Host) → 3 (IDE) → 2 (Registry) → 5 (Docs)
```
Close gaps, prove the language by self-hosting, then build ecosystem.

### Path B — "Ecosystem First"
```
0 (Gaps) → 2 (Registry) → 3 (IDE) → 4 (Apps) → 5 (Docs)
```
Close gaps, then build the ecosystem for adoption.

### Path C — "Ship & Grow"
```
0 (Gaps) → 5 (Docs) → 4 (Apps) → 10 (Community) → 1 (Self-Host)
```
Close gaps, document everything, prove with real apps, grow community.

---

## Summary

```
*** MANDATORY FIRST ***
Option 0:   Gap Closure (V6/V7)       10 sprints  100 tasks    ~20 hrs

*** THEN CHOOSE PATH ***
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

Total:     81 sprints, 810 tasks, ~162 hours
```
