# V13 "Beyond" — Post-Transcendence Production Plan

> **Previous:** V12 "Transcendence" (6 options, 600 tasks — ALL COMPLETE)
> **Version:** Fajar Lang v10.0.0 → v11.0.0 "Beyond"
> **Goal:** Close every remaining gap to world-class toolchain parity (Rust/Go/Zig level)
> **Scale:** 8 options, 71 sprints, 710 tasks, ~1,400 hours
> **Prerequisite:** V12 complete (LLVM, packages, macros, generators, WASI P1, LSP)
> **Date:** 2026-03-31
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **STATUS: IN PROGRESS**

---

## Motivation

V12 "Transcendence" proved Fajar Lang is **commercially deployable** — LLVM O3, package ecosystem, macros, generators, WASI, and LSP are all production-ready. But **world-class toolchain parity** requires more:

1. **CI stability** — Green CI across all platforms, nightly + stable, all feature flags
2. **WASI P2 + Component Model** — Full Preview 2 with async streams, resources, WIT tooling
3. **Incremental compilation** — Only recompile what changed, persistent disk cache, sub-second rebuilds
4. **Distributed runtime** — Real consensus (Raft), cluster-aware scheduling, distributed ML training
5. **FFI v2 full integration** — C++ templates, Python async, Rust trait objects, end-to-end wired
6. **SMT formal verification** — Z3-backed safety proofs, symbolic execution, @kernel/@device guarantees
7. **Self-hosting compiler** — Full Stage 2 bootstrap (compiler compiles itself in Fajar Lang)
8. **Const fn + compile-time eval** — Const generics, const trait bounds, compile-time allocation

**World-class** means: a developer can trust the compiler for safety-critical systems (automotive, aerospace, medical), deploy to any target (native, WASM, bare-metal, cluster), and interop seamlessly with C++/Python/Rust ecosystems — all verified by formal methods.

---

## Codebase Audit Summary (Pre-V13)

| Area | Existing LOC | Tests | Status | Gap |
|------|-------------|-------|--------|-----|
| CI/CD | 107 lines YAML | 5,955+ | 3 workflows, 11 jobs | Nightly clippy, keyword conflicts, feature lints |
| WASI | 3,323 | 12 | P1 100%, P2 40% framework | No P2 codegen, no WIT parsing, no component instantiation |
| Incremental | 2,869 | 40 | 80% real | No disk cache, no IR serialization, source-level only |
| Distributed | 4,235 | 78 | 70% real | No consensus, placeholder AllReduce, static discovery |
| FFI v2 | 3,149 | 69 | 90% real | Limited templates, no async Python, no complex Rust traits |
| SMT verify | 2,422 | 67 | 75% real | No proof caching, no symbolic exec, limited coverage |
| Self-hosting | 3,076 | 100+ | 60% real | No codegen, no Stage 2, flat arrays not tree AST |
| Const fn | 713 | 13 | 85% real | No const generics, no const trait bounds |

**Total existing:** ~22,059 LOC, 271+ tests across target areas

---

## Option A: CI Green — Cross-Platform Stability (1 sprint, 10 tasks)

### Context

CI has 3 workflows (CI, release, LLVM) with 11 jobs across 3 OS + 2 toolchains + 4 feature flags. Nightly clippy introduces new lints, `gen` keyword conflicts with V12 generators, and feature-gated code has stale warnings. This option ensures every push to main is green.

### Sprint A1: CI Stability & Hardening (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| A1.1 | Fix nightly `collapsible_match` | Collapse `if` into match guard in `type_check/check.rs` | 5 | Nightly clippy passes on all 3 OS |
| A1.2 | Fix `gen` keyword conflict in tests | Rename `gen` variable in `n2_inode_generation` to `generation` | 5 | `cargo test --test eval_tests n2_inode_generation` passes |
| A1.3 | Fix LLVM `unused-mut` warning | Remove `mut` from `compiler` in `llvm/mod.rs:6995` | 1 | `cargo test --lib --features llvm` passes |
| A1.4 | Fix cpp-ffi `useless_format` | Replace `format!("{e}")` with `e.to_string()` in `ffi_v2/cpp.rs` | 1 | `cargo clippy --features cpp-ffi -- -D warnings` passes |
| A1.5 | Audit all keyword-as-identifier usage | Grep for `let gen`, `let yield`, `let async` in tests + examples | 20 | No keyword conflicts in any .fj or test file |
| A1.6 | Pin nightly clippy allow-list | Add `#![allow()]` for known nightly-only lints with `// TODO: remove when stable` | 10 | Nightly + stable both pass clippy |
| A1.7 | Feature flag matrix test | Verify all 8 feature flags compile independently | 30 | `cargo check --features X` passes for each flag |
| A1.8 | Coverage job fix | Ensure tarpaulin runs after test fixes | 5 | Codecov upload succeeds |
| A1.9 | Add CI badge to README | `![CI](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml/badge.svg)` | 2 | Badge shows green |
| A1.10 | Verify full CI green | Push and confirm all 11 jobs pass | 0 | GitHub Actions: 11/11 green |

**Option A Total: 10 tasks, ~79 LOC, all CI jobs green**
**STATUS: ALL 10 TASKS COMPLETE (2026-03-31). Sprint A1 done.**

---

## Option B: WASI P2 + Component Model (10 sprints, 100 tasks)

### Context

WASI P1 is 100% production (8 syscalls wired into wasm compiler). P2 introduces the Component Model — typed interfaces (WIT), async streams, resources with ownership, and composable components. This is the future of WebAssembly deployment. Current state: type definitions exist in `wasi_v12.rs` but nothing is wired to codegen.

### Sprint W1: WIT Parser & Type System (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W1.1 | WIT lexer | Tokenize `.wit` files: `package`, `world`, `interface`, `resource`, `use` | 200 | Tokenizes `wasi:cli/command` world |
| W1.2 | WIT parser | Parse WIT into `WitDocument { interfaces, worlds, types }` | 300 | Parses WASI CLI world definition |
| W1.3 | WIT type system | Map WIT types to Fajar types: `u32→u32`, `string→str`, `list<T>→Array<T>` | 150 | All 15 WIT primitive types mapped |
| W1.4 | WIT record types | `record point { x: f64, y: f64 }` → Fajar struct | 80 | Record fields accessible in Fajar |
| W1.5 | WIT variant types | `variant error { timeout, refused(string) }` → Fajar enum | 100 | Variant matching works |
| W1.6 | WIT flags types | `flags permissions { read, write, exec }` → bitflags | 60 | Flag operations (or, and, contains) work |
| W1.7 | WIT resource types | `resource file { open: static func(...) → file }` → opaque handle | 120 | Resource creation and method calls |
| W1.8 | WIT tuple/option/result | Map built-in WIT generics to Fajar `Option<T>`, `Result<T,E>` | 80 | `option<string>` maps to `Option<str>` |
| W1.9 | WIT `use` imports | `use wasi:filesystem/types.{descriptor}` → type resolution | 100 | Cross-interface type references resolve |
| W1.10 | 10 WIT parser tests | Parse WASI CLI, HTTP, filesystem, sockets world definitions | 150 | All 10 pass |

### Sprint W2: Component Model Binary Format (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W2.1 | Component section emitter | Emit `component` section in Wasm binary (custom section type 0x0d) | 100 | `wasm-tools validate` accepts output |
| W2.2 | Component type encoding | Encode WIT types in canonical ABI format (core type → component type) | 150 | Types round-trip through encode/decode |
| W2.3 | Import section for interfaces | Emit `(import "wasi:filesystem/types")` in component | 80 | wasmtime resolves imports |
| W2.4 | Export section for interfaces | Emit `(export "run")` for command world entry point | 60 | wasmtime calls exported function |
| W2.5 | Canonical ABI lifting | Lower Fajar values to linear memory (strings → ptr+len, records → flat) | 200 | String passed correctly to host |
| W2.6 | Canonical ABI lowering | Lift host values from linear memory back to Fajar values | 200 | Host return value readable |
| W2.7 | Memory allocation protocol | `cabi_realloc` export for host-allocated memory | 80 | Host can allocate in guest memory |
| W2.8 | Post-return cleanup | `cabi_post_*` exports for freeing returned values | 60 | No memory leaks in component calls |
| W2.9 | Component validation | Validate component binary against WIT spec before output | 100 | Invalid components rejected with clear error |
| W2.10 | 10 binary format tests | Validate output with `wasm-tools component validate` | 150 | All 10 pass |

### Sprint W3: WASI P2 Filesystem Interface (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W3.1 | `wasi:filesystem/types` import | Descriptor, DirectoryEntry, Filestat types | 80 | Types compile in Fajar |
| W3.2 | `open-at` function | Open file relative to directory descriptor | 60 | `open_at(dir, "file.txt", READ)` works |
| W3.3 | `read-via-stream` | Streaming file read via `wasi:io/streams` | 100 | Read 1MB file in 4KB chunks |
| W3.4 | `write-via-stream` | Streaming file write via `wasi:io/streams` | 100 | Write file and verify contents |
| W3.5 | `stat` / `stat-at` | Get file metadata (size, timestamps, type) | 60 | File size matches written bytes |
| W3.6 | `readdir` | Read directory entries with cookie-based pagination | 80 | List directory contents |
| W3.7 | `path-create-directory` | Create nested directories | 40 | `mkdir -p` equivalent works |
| W3.8 | `path-remove-directory` / `unlink-file` | Delete files and directories | 40 | File removed, stat returns error |
| W3.9 | `path-rename` | Atomic rename across directory descriptors | 50 | Rename preserves content |
| W3.10 | 10 filesystem tests | Read/write/stat/readdir/rename with wasmtime | 150 | All 10 pass on wasmtime 18+ |

### Sprint W4: WASI P2 Streams & I/O (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W4.1 | `wasi:io/streams` input-stream | Blocking and pollable read from input stream | 100 | Read stdin line by line |
| W4.2 | `wasi:io/streams` output-stream | Blocking and pollable write to output stream | 100 | Write to stdout with flush |
| W4.3 | `wasi:io/poll` pollable | Create pollable from stream, poll-one, poll-many | 120 | Poll 2 streams, first ready returns |
| W4.4 | Stream splice | Splice input-stream to output-stream (zero-copy pipe) | 80 | Pipe stdin to file works |
| W4.5 | Blocking vs async streams | `subscribe()` → pollable for non-blocking I/O | 100 | Non-blocking read returns immediately |
| W4.6 | Error stream handling | `stream-error` type with last-operation-failed | 60 | EOF and permission errors propagated |
| W4.7 | `wasi:clocks/monotonic-clock` | `now()`, `resolution()`, `subscribe-duration()` | 60 | Monotonic timestamps correct |
| W4.8 | `wasi:clocks/wall-clock` | `now()` returning `datetime { seconds, nanoseconds }` | 40 | Wall clock within 1s of host |
| W4.9 | `wasi:random/random` | `get-random-bytes(len)`, `get-random-u64()` | 40 | Random bytes are non-zero |
| W4.10 | 10 stream/IO tests | Read/write streams, poll, clocks, random | 150 | All 10 pass |

### Sprint W5: WASI P2 HTTP Client (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W5.1 | `wasi:http/types` | Request, Response, Headers, Method, StatusCode types | 100 | Types compile in Fajar |
| W5.2 | `wasi:http/outgoing-handler` | `handle(request)` → future response | 120 | HTTP GET returns 200 |
| W5.3 | Request construction | `Request::new(method, url, headers, body?)` builder | 80 | Build GET/POST/PUT/DELETE requests |
| W5.4 | Response reading | Read status, headers, body stream from response | 80 | Parse JSON response body |
| W5.5 | Request body streaming | Write request body via output-stream (for POST) | 80 | POST with JSON body works |
| W5.6 | Response body streaming | Read response body via input-stream (chunked) | 80 | Stream large response in chunks |
| W5.7 | Header manipulation | Add, get, delete, iterate headers | 50 | Content-Type header set correctly |
| W5.8 | Error handling | Network error, timeout, DNS failure → Result | 60 | Timeout returns Err, not panic |
| W5.9 | HTTPS support | TLS via host (no guest TLS needed in WASI) | 30 | `https://` URLs work |
| W5.10 | 10 HTTP client tests | GET, POST, headers, streaming, errors | 150 | All 10 pass with mock server |

### Sprint W6: WASI P2 HTTP Server (Proxy World) (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W6.1 | `wasi:http/incoming-handler` export | `handle(request, response-outparam)` entry point | 100 | Component exports handler |
| W6.2 | Request routing | Path + method matching in handler | 80 | `/api/users` routed to correct handler |
| W6.3 | Response construction | `response-outparam.set(status, headers, body-stream)` | 80 | Return 200 with JSON body |
| W6.4 | Middleware pipeline | Pre/post processing (logging, auth, CORS) | 100 | Middleware chain executes in order |
| W6.5 | JSON serialization | Serialize Fajar structs to JSON in response body | 60 | `{ "name": "fajar", "age": 42 }` output |
| W6.6 | Error responses | 400 Bad Request, 404 Not Found, 500 Internal Error | 40 | Correct status codes returned |
| W6.7 | Request body parsing | Parse JSON/form body from incoming request | 80 | POST body deserialized to struct |
| W6.8 | Static file serving | Serve files from preopened directory | 60 | HTML file served with correct MIME |
| W6.9 | wasmtime serve integration | Deploy with `wasmtime serve component.wasm` | 30 | HTTP server runs on wasmtime |
| W6.10 | 10 HTTP server tests | Route, middleware, JSON, error, static files | 150 | All 10 pass with wasmtime serve |

### Sprint W7: WASI P2 Sockets (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W7.1 | `wasi:sockets/tcp` types | TcpSocket, IpAddress, Network resource types | 80 | Types compile |
| W7.2 | TCP connect | `tcp.start-connect(addr)` → `tcp.finish-connect()` | 100 | Connect to remote TCP server |
| W7.3 | TCP listen & accept | `tcp.start-listen()`, `tcp.accept()` → connected socket | 100 | Accept incoming connections |
| W7.4 | TCP read/write streams | Get input/output streams from connected socket | 60 | Echo server works |
| W7.5 | `wasi:sockets/udp` | UDP datagram send/receive with address | 100 | UDP echo works |
| W7.6 | `wasi:sockets/ip-name-lookup` | DNS resolution: hostname → IP addresses | 80 | `resolve("example.com")` returns IPs |
| W7.7 | Socket options | `SO_REUSEADDR`, `TCP_NODELAY`, timeouts | 60 | Options affect socket behavior |
| W7.8 | Non-blocking socket I/O | Pollable sockets for async networking | 80 | Poll multiple sockets concurrently |
| W7.9 | Socket error handling | Connection refused, timeout, reset → Result | 40 | Errors propagated cleanly |
| W7.10 | 10 socket tests | TCP connect/listen, UDP, DNS, poll | 150 | All 10 pass on wasmtime |

### Sprint W8: Resource Lifecycle & Ownership (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W8.1 | Resource handle table | Guest-side handle→index table for WASI resources | 100 | Handles allocated/freed correctly |
| W8.2 | Resource drop protocol | `resource.drop(handle)` → host cleanup | 60 | Dropped resources freed on host |
| W8.3 | Resource borrow semantics | Borrowed handles cannot outlive owner | 80 | Borrow checker enforces resource lifetimes |
| W8.4 | Own vs borrow in WIT | `own<file>` vs `borrow<file>` in function signatures | 80 | Ownership transfer correct |
| W8.5 | Resource constructor | `[constructor]file(path: string)` → `resource.new()` | 60 | Constructor creates valid handle |
| W8.6 | Resource methods | `[method]file.read(len: u32) → list<u8>` dispatch | 60 | Method calls on resources work |
| W8.7 | Resource static methods | `[static]file.open(path: string) → result<file, error>` | 50 | Static factory methods work |
| W8.8 | Nested resources | Resource containing other resources (e.g., dir→file) | 80 | Nested drops in correct order |
| W8.9 | Resource in collections | `list<own<file>>` in function returns | 60 | Resource handles in arrays work |
| W8.10 | 10 resource lifecycle tests | Create, borrow, drop, nested, collection | 150 | All 10 pass, no leaks |

### Sprint W9: Component Composition & Linking (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W9.1 | Component instantiation | Instantiate component with provided imports | 120 | Component runs with host imports |
| W9.2 | Component linking | Link component A's export to component B's import | 150 | Two components communicate |
| W9.3 | Virtualized filesystem | Override `wasi:filesystem` with custom implementation | 100 | In-memory FS used instead of host |
| W9.4 | `fj build --target wasm32-wasi-p2` | CLI flag for WASI P2 component output | 40 | `.wasm` component binary produced |
| W9.5 | Component adapter | Wrap P1 module in P2 component using wasi_snapshot_preview1.reactor.wasm | 80 | Legacy P1 code runs as P2 component |
| W9.6 | Multi-component package | Single `.fj` project producing multiple components | 60 | Workspace builds multiple components |
| W9.7 | Import satisfaction check | Verify all imports satisfied before instantiation | 60 | Missing import → clear error |
| W9.8 | WIT dependency resolution | Resolve `use wasi:*` from WASI P2 spec packages | 80 | Standard WASI interfaces found |
| W9.9 | Component binary size | Strip debug info, optimize sections | 40 | Hello-world component < 100KB |
| W9.10 | 10 composition tests | Link, virtualize, adapt, multi-component | 150 | All 10 pass |

### Sprint W10: Validation & Deployment (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W10.1 | wasmtime compatibility | Verify all components run on wasmtime 18+ | 50 | `wasmtime run component.wasm` works |
| W10.2 | WAMR compatibility | Verify basic components run on WAMR | 50 | Alternative runtime works |
| W10.3 | Spin/Fermyon deployment | Deploy HTTP component to Fermyon Cloud | 30 | `spin up` serves Fajar HTTP handler |
| W10.4 | wasi-virt testing | Virtual filesystem + network for hermetic tests | 60 | Tests run without host access |
| W10.5 | Component size benchmarks | Measure hello/http/filesystem component sizes | 30 | Document sizes in RESULTS.md |
| W10.6 | Component startup benchmarks | Measure instantiation time for each component | 30 | < 10ms instantiation for CLI |
| W10.7 | WASI P2 conformance tests | Run WASI P2 test suite against Fajar output | 100 | 90%+ conformance |
| W10.8 | Documentation: WASI P2 guide | `book/wasi_p2_guide.md` with examples | 200 | Guide covers all interfaces |
| W10.9 | Example: WASI HTTP server | `examples/wasi_http_server.fj` deployed to Spin | 100 | Working HTTP server on cloud |
| W10.10 | Update GAP_ANALYSIS_V2 | Mark WASI P2 as 100% production | 20 | Audit reflects real status |

**Option B Total: 100 tasks, ~8,500 LOC, 100 tests, WASI P2 fully production**

---

## Option C: Incremental Compilation (10 sprints, 100 tasks)

### Context

Incremental compilation exists (2,869 LOC, 40 tests) with dependency graph, change detection via SHA256, and transitive recompilation. But it's in-memory only, source-level granularity, and not wired to the main `fj build` pipeline. For production: persistent disk cache, IR-level granularity, parallel compilation, sub-second rebuilds.

### Sprint I1: Persistent Disk Cache (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I1.1 | Cache directory layout | `target/incremental/{hash}/` with metadata.json + artifacts | 60 | Directory created on first build |
| I1.2 | Artifact serialization | Serialize analyzed AST + type info to bincode | 100 | Round-trip AST → bytes → AST matches |
| I1.3 | Artifact deserialization | Load cached artifacts on subsequent build | 80 | Cached build uses serialized data |
| I1.4 | Content hash persistence | Store SHA256 hashes in `target/incremental/hashes.json` | 50 | Hashes survive process restart |
| I1.5 | Cache invalidation | Invalidate when Cargo.toml, fj.toml, or compiler version changes | 60 | Config change triggers full rebuild |
| I1.6 | Cache size management | LRU eviction when cache exceeds `--cache-limit` (default 1GB) | 80 | Old artifacts evicted, newest kept |
| I1.7 | Cache corruption detection | Validate checksum on load, discard corrupt entries | 60 | Corrupted cache → clean rebuild |
| I1.8 | Atomic cache writes | Write to temp file, then atomic rename | 40 | Interrupted build doesn't corrupt cache |
| I1.9 | `fj clean --incremental` | CLI command to purge incremental cache | 20 | Cache directory removed |
| I1.10 | 10 persistence tests | Write/read/invalidate/corrupt/evict/clean | 150 | All 10 pass |

### Sprint I2: Fine-Grained Dependency Tracking (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I2.1 | Function-level tracking | Track which functions changed, not just files | 120 | Change one fn → only dependents recompile |
| I2.2 | Type-level tracking | Track struct/enum definition changes | 100 | Change struct field → dependents recompile |
| I2.3 | Import graph construction | Build use/mod import edges between modules | 80 | Import graph matches actual dependencies |
| I2.4 | Signature vs body change | Signature change → recompile callers; body-only → skip | 80 | Body-only change is fastest path |
| I2.5 | Trait impl tracking | Track which trait impls changed, invalidate users | 80 | New trait impl → monomorphization invalidated |
| I2.6 | Constant propagation tracking | Track const value changes | 50 | `const X = 5` → `6` triggers recompile of users |
| I2.7 | Macro expansion tracking | Track macro input → output mapping for invalidation | 80 | Macro change → all expansion sites recompile |
| I2.8 | Cross-module type inference | Track inferred types that depend on other modules | 60 | Type change in module A → module B reanalyzed |
| I2.9 | Dependency graph visualization | `fj build --dep-graph` outputs DOT format | 50 | `dot -Tsvg` renders readable graph |
| I2.10 | 10 dependency tracking tests | Function, type, signature, trait, const, macro | 150 | All 10 pass |

### Sprint I3: IR-Level Caching (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I3.1 | Cranelift IR serialization | Serialize Cranelift `Function` to binary format | 120 | IR round-trips correctly |
| I3.2 | LLVM bitcode caching | Cache `.bc` files per module for LLVM backend | 80 | LLVM backend uses cached bitcode |
| I3.3 | Bytecode VM caching | Cache compiled bytecode per function | 60 | VM backend uses cached bytecode |
| I3.4 | Object file caching | Cache `.o` files for unchanged modules | 60 | Linker uses cached objects |
| I3.5 | Cache key computation | Hash of (source hash + compiler version + flags + deps) | 80 | Different flags → different cache key |
| I3.6 | Incremental linking | Only re-link changed object files | 100 | Incremental link < 100ms for small change |
| I3.7 | Debug info caching | Cache DWARF debug info per module | 60 | Debug symbols correct after incremental build |
| I3.8 | Parallel cache reads | Load cached artifacts in parallel threads | 60 | Multi-core cache loading |
| I3.9 | Cache hit metrics | Report cache hit/miss ratio with `--verbose` | 40 | `Cache: 45/50 hit (90%)` displayed |
| I3.10 | 10 IR cache tests | Cranelift, LLVM, bytecode, object, parallel | 150 | All 10 pass |

### Sprint I4: Parallel Compilation (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I4.1 | Module-level parallelism | Compile independent modules on separate threads | 120 | 4 independent modules → 4 threads |
| I4.2 | Thread pool configuration | `--jobs=N` or `FJ_JOBS` env var (default = CPU count) | 40 | `fj build --jobs=8` uses 8 threads |
| I4.3 | Topological scheduling | Compile in dependency order, parallelize within levels | 80 | Level 0 (no deps) first, then level 1, etc. |
| I4.4 | Work stealing scheduler | Idle threads steal work from busy threads | 100 | All cores utilized |
| I4.5 | Thread-safe diagnostics | Collect errors from all threads without data races | 60 | Errors from parallel modules all reported |
| I4.6 | Parallel analysis | Run type checker on independent modules concurrently | 80 | Analysis parallelized |
| I4.7 | Parallel codegen | Run Cranelift/LLVM codegen on independent functions | 80 | Codegen parallelized |
| I4.8 | Progress reporting | `[1/50] Compiling module_a...` with progress bar | 40 | User sees real-time progress |
| I4.9 | Deadlock prevention | Detect and break circular compilation dependencies | 60 | No deadlocks on cyclic modules |
| I4.10 | 10 parallel compilation tests | 2/4/8 threads, scheduling, progress, deadlock | 150 | All 10 pass |

### Sprint I5: Pipeline Integration (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I5.1 | Wire into `fj build` | Default incremental builds for `fj build` | 40 | `fj build` uses incremental by default |
| I5.2 | `--no-incremental` flag | Disable incremental for clean builds | 10 | `fj build --no-incremental` does full rebuild |
| I5.3 | Wire into `fj check` | Incremental type checking (skip unchanged modules) | 40 | `fj check` faster on second run |
| I5.4 | Wire into `fj test` | Only recompile changed test targets | 40 | `fj test` faster when 1 file changed |
| I5.5 | Wire into `fj run` | Incremental build before run | 20 | `fj run` rebuilds only changed modules |
| I5.6 | Wire into LSP | LSP uses incremental analysis for real-time checking | 80 | LSP responds < 200ms after edit |
| I5.7 | Wire into `fj watch` | File watcher triggers incremental rebuild | 40 | Save file → rebuild in < 1s |
| I5.8 | Workspace incremental | Incremental across workspace members | 60 | Change one member → only affected members rebuild |
| I5.9 | `fj build --timings` | Show time spent in each phase (parse/analyze/codegen/link) | 60 | Timing breakdown displayed |
| I5.10 | 10 pipeline integration tests | build, check, test, run, watch, LSP, workspace | 150 | All 10 pass |

### Sprint I6: Rebuild Performance Benchmarks (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I6.1 | Cold build benchmark | Full build from scratch on 10K LOC project | 30 | Baseline measurement recorded |
| I6.2 | Warm build (no change) | Build with no changes (all cache hits) | 20 | < 100ms for 10K LOC |
| I6.3 | Single-file change | Change one file in 10K LOC project | 20 | < 500ms rebuild |
| I6.4 | Signature change | Change function signature (cascading rebuild) | 20 | < 2s rebuild |
| I6.5 | Type change | Change struct field (moderate cascade) | 20 | < 1s rebuild |
| I6.6 | New file added | Add new file to project | 20 | < 1s rebuild |
| I6.7 | File deleted | Remove file from project | 20 | < 500ms rebuild |
| I6.8 | 10 files changed | Batch change of 10 files | 20 | < 3s rebuild |
| I6.9 | Parallel speedup | Compare 1 vs 4 vs 8 threads on 50K LOC | 30 | 4x speedup with 8 threads |
| I6.10 | Document benchmarks | Write results to `docs/INCREMENTAL_BENCHMARKS.md` | 100 | Table with all measurements |

### Sprint I7: Error Recovery & Edge Cases (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I7.1 | Parse error recovery | Partially parsed file still caches successful modules | 60 | Error in file A doesn't invalidate file B |
| I7.2 | Type error recovery | Type errors don't prevent caching of valid modules | 60 | Valid modules cached despite errors elsewhere |
| I7.3 | Interrupted build recovery | Resume after Ctrl+C without corrupt cache | 60 | Build completes on retry |
| I7.4 | Clock skew handling | Handle system clock changes without false invalidation | 40 | Content hash (not mtime) is source of truth |
| I7.5 | Symlink handling | Follow symlinks for source files, cache by real path | 40 | Symlinked files tracked correctly |
| I7.6 | Case sensitivity | Handle case-insensitive filesystems (macOS/Windows) | 40 | No duplicate cache entries |
| I7.7 | Unicode paths | Handle Unicode file paths in cache keys | 30 | `src/日本語/mod.fj` cached correctly |
| I7.8 | Large file handling | Efficient hashing for files > 1MB | 30 | 10MB file hashed in < 10ms |
| I7.9 | Circular dependency detection | Detect and report circular module dependencies | 60 | Clear error: "circular dependency: A → B → A" |
| I7.10 | 10 edge case tests | Parse error, interrupt, clock, symlink, circular | 150 | All 10 pass |

### Sprint I8: LSP Integration (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I8.1 | Incremental analysis engine | LSP uses incremental pipeline for on-type checking | 120 | Type errors appear < 200ms after keystroke |
| I8.2 | Per-function reanalysis | Typing in function body only reanalyzes that function | 100 | Single function reanalysis < 50ms |
| I8.3 | Background indexing | Index workspace in background thread on startup | 80 | Full workspace indexed in < 5s |
| I8.4 | Incremental symbol index | Update symbol table incrementally on file change | 80 | Go-to-definition works after edit |
| I8.5 | Incremental diagnostics | Only recompute diagnostics for changed files + dependents | 60 | Diagnostics update in < 100ms |
| I8.6 | Incremental completion | Completion uses cached type information | 60 | Completion appears < 100ms |
| I8.7 | Memory-efficient caching | Share AST/type data between LSP and compiler | 60 | LSP memory < 200MB for 50K LOC |
| I8.8 | Cache warming on open | Pre-analyze opened files and their imports | 40 | First edit in file is fast |
| I8.9 | Stale cache indicator | StatusBar shows when analysis is stale vs fresh | 30 | User knows when results are up-to-date |
| I8.10 | 10 LSP incremental tests | Typing, completion, diagnostics, symbols, memory | 150 | All 10 pass |

### Sprint I9: Workspace & Multi-Target (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I9.1 | Workspace-level cache | Shared cache directory for all workspace members | 60 | `target/incremental/` at workspace root |
| I9.2 | Cross-member dependency tracking | Change in `core` member → rebuilds `cli` member | 80 | Cross-member invalidation correct |
| I9.3 | Per-target caching | Separate cache for `--target x86_64` vs `--target aarch64` | 60 | Different targets don't conflict |
| I9.4 | Per-profile caching | Separate cache for `debug` vs `release` vs `--features X` | 60 | Different profiles isolated |
| I9.5 | Shared dependency caching | Common dependencies compiled once across members | 80 | `fj-core` compiled once for 5 members |
| I9.6 | Workspace build order | Build workspace members in optimal parallel order | 60 | Maximum parallelism achieved |
| I9.7 | Workspace-level `--timings` | Show timing for each member in workspace | 40 | Per-member build times displayed |
| I9.8 | Remote cache (optional) | `--cache-dir=s3://bucket/` for CI shared cache | 100 | CI reuses local developer cache |
| I9.9 | Cache statistics | `fj cache stats` shows size, hit rate, age distribution | 60 | Useful diagnostics for developers |
| I9.10 | 10 workspace tests | Cross-member, multi-target, shared deps, remote | 150 | All 10 pass |

### Sprint I10: Optimization & Validation (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I10.1 | Correctness validation | Compare incremental output to clean build output | 100 | Byte-identical binaries |
| I10.2 | Deterministic builds | Same input always produces same cache key | 60 | No randomness in hashing |
| I10.3 | Memory profiling | Measure memory usage during incremental build | 40 | < 500MB for 50K LOC incremental build |
| I10.4 | Compile-time regression test | CI check that incremental doesn't slow clean builds > 5% | 40 | Clean build overhead < 5% |
| I10.5 | Self-hosting test | Fajar Lang compiles itself incrementally | 60 | Fajar stdlib rebuilds incrementally |
| I10.6 | Stress test | 1000 edit-rebuild cycles without corruption | 60 | No failures in 1000 cycles |
| I10.7 | Documentation | `book/incremental_compilation.md` | 200 | Guide with architecture + usage |
| I10.8 | Update CLAUDE.md | Document incremental in quick commands and architecture | 30 | CLAUDE.md reflects incremental |
| I10.9 | Update GAP_ANALYSIS_V2 | Mark incremental as 100% production | 20 | Audit reflects real status |
| I10.10 | Example: large project | `examples/workspace_demo/` with 10 modules | 200 | Incremental rebuild in < 500ms |

**Option C Total: 100 tasks, ~7,200 LOC, 100 tests, sub-second incremental builds**

---

## Option D: Distributed Runtime (10 sprints, 100 tasks)

### Context

Distributed runtime exists (4,235 LOC, 78 tests) with RPC, TCP transport, cluster scheduling, and fault tolerance framework. But discovery is static, fault tolerance has no real consensus, AllReduce is placeholder, and nothing is wired to `fj run --cluster`. For production: real Raft consensus, dynamic discovery, distributed ML training, and cluster deployment.

### Sprint D1: Raft Consensus Protocol (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D1.1 | Raft state machine | Leader/Follower/Candidate states with term tracking | 150 | State transitions correct per Raft paper |
| D1.2 | Leader election | RequestVote RPC, split brain prevention, randomized timeout | 200 | Leader elected within 2 election timeouts |
| D1.3 | Log replication | AppendEntries RPC, log consistency check, commit index | 200 | Log replicated to majority |
| D1.4 | Commit & apply | Apply committed entries to state machine | 80 | State machine reflects committed entries |
| D1.5 | Persistence | WAL (write-ahead log) for term, votedFor, log entries | 100 | Node recovers state after restart |
| D1.6 | Snapshot | Compact log with state snapshot | 100 | Snapshot reduces log size by 90%+ |
| D1.7 | Membership change | Joint consensus for adding/removing nodes | 120 | Add node without cluster downtime |
| D1.8 | Pre-vote extension | Pre-vote prevents disruption from partitioned nodes | 60 | Partitioned node doesn't trigger election |
| D1.9 | Lease-based reads | Leader leases for fast linearizable reads | 80 | Read latency < 1ms with lease |
| D1.10 | 10 Raft tests | Election, replication, partition, recovery, membership | 200 | All 10 pass with simulated network |

### Sprint D2: Dynamic Service Discovery (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D2.1 | mDNS discovery | Multicast DNS for local network node discovery | 100 | Nodes find each other on LAN |
| D2.2 | Seed node bootstrap | `--seed=host1:9000,host2:9000` for WAN bootstrap | 60 | Cluster forms from seed list |
| D2.3 | DNS-based discovery | `--discovery=dns:cluster.fajarlang.dev` SRV records | 80 | Nodes discovered via DNS |
| D2.4 | Gossip protocol | SWIM-based failure detection + membership dissemination | 150 | Membership converges within 5 gossip rounds |
| D2.5 | Node metadata | Advertise capabilities: CPU, memory, GPU, features | 50 | Scheduler sees node capabilities |
| D2.6 | Health checking | Periodic health probes with configurable interval | 60 | Unhealthy node removed from pool |
| D2.7 | Auto-scaling events | Emit events on node join/leave for external orchestration | 40 | Kubernetes integration possible |
| D2.8 | Service registry | Map service names to node addresses | 60 | `fj_rpc_connect("my-service")` resolves |
| D2.9 | Discovery configuration | `[cluster.discovery]` section in `fj.toml` | 40 | TOML config controls discovery method |
| D2.10 | 10 discovery tests | mDNS, seed, DNS, gossip, metadata, health | 150 | All 10 pass |

### Sprint D3: Distributed Task Scheduler (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D3.1 | Task definition | `@distributed fn process(data: Tensor) -> Tensor` annotation | 60 | Distributed functions parsed |
| D3.2 | Task placement | Schedule task on node with best resource match | 80 | GPU task → GPU node |
| D3.3 | Data locality | Prefer nodes that already have input data | 60 | Data-local scheduling reduces network |
| D3.4 | Load balancing | Round-robin, least-loaded, weighted strategies | 80 | Load evenly distributed |
| D3.5 | Task queue | Priority queue with fairness across users | 60 | High-priority tasks execute first |
| D3.6 | Task cancellation | Cancel running task with cleanup | 40 | Resources freed on cancel |
| D3.7 | Task retry | Retry failed tasks with exponential backoff | 50 | Transient failures recovered |
| D3.8 | Task dependencies | DAG-based task scheduling (A before B) | 80 | Dependency order respected |
| D3.9 | Resource reservations | Reserve CPU/memory/GPU before task start | 60 | No over-commitment |
| D3.10 | 10 scheduler tests | Placement, locality, balance, cancel, retry, DAG | 150 | All 10 pass |

### Sprint D4: Distributed Data Plane (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D4.1 | Data partitioning | Shard tensor across nodes by rows/columns | 100 | 1000x100 tensor → 4 shards of 250x100 |
| D4.2 | Data transfer protocol | Zero-copy TCP send/recv for large tensors | 120 | 100MB tensor transferred in < 1s on LAN |
| D4.3 | Serialization format | Efficient binary format for tensors (header + raw data) | 80 | Serialize/deserialize preserves shape + dtype |
| D4.4 | Compression | Optional LZ4 compression for network transfer | 60 | 2x compression ratio for sparse tensors |
| D4.5 | Scatter operation | Distribute data to N workers | 60 | Data arrives at all workers |
| D4.6 | Gather operation | Collect results from N workers | 60 | Results assembled in correct order |
| D4.7 | Broadcast operation | Send same data to all workers | 40 | All workers receive identical data |
| D4.8 | AllReduce operation | Real ring-allreduce for gradient aggregation | 150 | Gradients summed across 4 nodes correctly |
| D4.9 | Pipeline parallelism | Stream data through stages on different nodes | 100 | Pipeline throughput > single-node |
| D4.10 | 10 data plane tests | Shard, transfer, scatter, gather, allreduce, pipeline | 150 | All 10 pass |

### Sprint D5: Distributed ML Training (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D5.1 | Data-parallel training | Same model on N workers, different data batches | 150 | MNIST accuracy matches single-node |
| D5.2 | Gradient synchronization | AllReduce gradients after each batch | 80 | Gradients averaged correctly |
| D5.3 | Model-parallel training | Split large model across multiple nodes | 120 | GPT-style model split across 2 nodes |
| D5.4 | Parameter server | Centralized parameter storage with async updates | 100 | Workers push/pull parameters |
| D5.5 | Learning rate scaling | Linear scaling rule for multi-node training | 30 | LR = base_lr * num_workers |
| D5.6 | Checkpoint saving | Distributed checkpoint to shared storage | 80 | Checkpoint saved from all workers |
| D5.7 | Checkpoint loading | Resume training from distributed checkpoint | 60 | Training resumes from saved state |
| D5.8 | Mixed precision | FP16 communication, FP32 computation | 60 | 2x communication speedup |
| D5.9 | Elastic training | Add/remove workers during training | 100 | Add worker → training continues |
| D5.10 | 10 distributed ML tests | Data-parallel MNIST, gradient sync, checkpoint | 150 | All 10 pass |

### Sprint D6: RPC Framework Completion (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D6.1 | Bidirectional streaming | Client and server stream simultaneously | 100 | Chat-style bidirectional stream works |
| D6.2 | RPC timeout | Per-call timeout with deadline propagation | 50 | Timeout → `Err(Timeout)` after deadline |
| D6.3 | RPC compression | gzip/lz4 compression for large payloads | 60 | 10x reduction for text payloads |
| D6.4 | RPC authentication | TLS mutual auth + bearer token validation | 100 | Unauthenticated calls rejected |
| D6.5 | RPC interceptors | Pre/post call hooks for logging, metrics, auth | 80 | Interceptor chain executes |
| D6.6 | RPC load balancing | Client-side load balancing across server replicas | 60 | Requests distributed across 3 servers |
| D6.7 | RPC service reflection | List available methods and their types | 40 | `rpc.list_methods()` returns method list |
| D6.8 | RPC health service | Standard health check endpoint | 30 | `rpc.check_health()` returns status |
| D6.9 | RPC metrics | Request count, latency histogram, error rate | 60 | Prometheus-compatible metrics export |
| D6.10 | 10 RPC tests | Streaming, timeout, auth, interceptors, metrics | 150 | All 10 pass |

### Sprint D7: Fault Tolerance (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D7.1 | Leader failover | Raft leader failure → new leader within 5s | 60 | Client reconnects to new leader |
| D7.2 | Worker failover | Failed worker's tasks reassigned to healthy workers | 80 | Training continues after worker loss |
| D7.3 | Network partition handling | Split-brain prevention via Raft majority | 60 | Minority partition becomes read-only |
| D7.4 | Graceful shutdown | `SIGTERM` → drain tasks → deregister from cluster | 60 | No lost work on shutdown |
| D7.5 | Data replication | Replicate critical data to N nodes | 80 | Data survives 1 node failure (N=3) |
| D7.6 | Circuit breaker | Stop sending to failing nodes, retry after cooldown | 60 | Cascading failure prevented |
| D7.7 | Backpressure | Slow consumer → producer backs off | 50 | No OOM from unbounded queues |
| D7.8 | Idempotent operations | Retry-safe task execution | 40 | Duplicate execution has no side effect |
| D7.9 | Split-brain recovery | Automatic reconciliation after partition heals | 80 | Cluster reconverges |
| D7.10 | 10 fault tolerance tests | Failover, partition, shutdown, replication, circuit | 150 | All 10 pass with chaos testing |

### Sprint D8: CLI & Deployment (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D8.1 | `fj run --cluster` | Run distributed job on cluster | 40 | `fj run --cluster train.fj` dispatches |
| D8.2 | `fj cluster status` | Show cluster nodes, health, load | 60 | Node table displayed |
| D8.3 | `fj cluster join` | Join current node to existing cluster | 30 | Node joins and receives work |
| D8.4 | `fj cluster leave` | Gracefully leave cluster | 20 | Node deregisters |
| D8.5 | `[cluster]` config in fj.toml | Cluster configuration section | 40 | Config controls cluster behavior |
| D8.6 | Docker deployment | Dockerfile for cluster node | 50 | `docker run fj-node` joins cluster |
| D8.7 | Kubernetes deployment | Helm chart + StatefulSet for cluster | 100 | `helm install fj-cluster` works |
| D8.8 | Monitoring dashboard | Grafana dashboard template for cluster metrics | 100 | Dashboard shows nodes, tasks, latency |
| D8.9 | Log aggregation | Structured JSON logging for cluster events | 40 | Logs parseable by fluentd/loki |
| D8.10 | 10 deployment tests | CLI, Docker, K8s, config, monitoring | 150 | All 10 pass |

### Sprint D9: Security & Multi-Tenancy (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D9.1 | TLS everywhere | All cluster communication over mTLS | 80 | Plaintext connections rejected |
| D9.2 | Certificate rotation | Automatic cert renewal before expiry | 60 | Rotation without downtime |
| D9.3 | RBAC | Role-based access: admin, scheduler, worker, reader | 80 | Worker can't access admin endpoints |
| D9.4 | Resource quotas | Per-user CPU/memory/GPU limits | 60 | Over-quota task queued, not rejected |
| D9.5 | Audit logging | Log all cluster operations with user attribution | 40 | Who did what, when |
| D9.6 | Secrets management | Encrypted storage for tokens and keys | 60 | Secrets encrypted at rest |
| D9.7 | Network policies | Limit inter-node communication to cluster ports | 40 | External access blocked |
| D9.8 | Sandboxed execution | Tasks run in isolated environment (process/container) | 80 | Task can't access host filesystem |
| D9.9 | Data encryption in transit | Encrypt tensor data during network transfer | 50 | Wireshark shows encrypted payload |
| D9.10 | 10 security tests | TLS, RBAC, quotas, audit, sandbox, encryption | 150 | All 10 pass |

### Sprint D10: Benchmarks & Documentation (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D10.1 | Single-node baseline | MNIST training time on 1 node | 30 | Baseline recorded |
| D10.2 | 2-node speedup | MNIST training on 2 nodes | 20 | > 1.8x speedup |
| D10.3 | 4-node speedup | MNIST training on 4 nodes | 20 | > 3.2x speedup |
| D10.4 | Communication overhead | Measure AllReduce latency for 10MB gradient | 30 | < 100ms on LAN |
| D10.5 | Failure recovery time | Measure time from node failure to task reassignment | 20 | < 10s recovery |
| D10.6 | Raft election benchmark | Measure election convergence time | 20 | < 3s election |
| D10.7 | Scalability test | Test with 16 simulated nodes | 40 | Linear scaling up to 16 nodes |
| D10.8 | Documentation | `book/distributed_runtime.md` | 200 | Architecture + deployment guide |
| D10.9 | Example: distributed MNIST | `examples/distributed_mnist.fj` | 150 | End-to-end distributed training |
| D10.10 | Update GAP_ANALYSIS_V2 | Mark distributed as 100% production | 20 | Audit reflects real status |

**Option D Total: 100 tasks, ~8,500 LOC, 100 tests, production distributed runtime**

---

## Option E: FFI v2 Full Integration (10 sprints, 100 tasks)

### Context

FFI v2 has solid foundations (3,149 LOC, 69 tests) — real libclang for C++, real PyO3 for Python, basic Rust bridge. But it's limited to primitive types and simple structs. For production: C++ template instantiation, Python async interop, Rust trait object marshalling, automatic binding generation, and end-to-end pipeline integration.

### Sprint E1: C++ Template Support (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E1.1 | Template class detection | Detect `template<class T>` in libclang AST | 80 | `std::vector<T>` detected as template |
| E1.2 | Template instantiation | Generate bindings for specific instantiations (`vector<int>`) | 120 | `vector_i32` type usable in Fajar |
| E1.3 | Nested templates | Handle `map<string, vector<int>>` | 80 | Nested template types correct |
| E1.4 | Template methods | Bind template class methods (`.push_back()`, `.at()`) | 80 | Method calls compile and run |
| E1.5 | SFINAE/concepts | Handle conditional template methods | 60 | Only valid methods exposed |
| E1.6 | Template aliases | `using Vec = vector<int>` resolved | 40 | Aliases map to concrete types |
| E1.7 | Variadic templates | `template<typename... Args>` basic support | 60 | `tuple<int, float, string>` works |
| E1.8 | Partial specialization | Handle specialized template variants | 60 | `vector<bool>` uses bool specialization |
| E1.9 | Default template args | `template<class T = int>` default handling | 40 | Defaults applied when not specified |
| E1.10 | 10 template tests | Vector, map, nested, methods, variadic | 150 | All 10 pass with real g++ |

### Sprint E2: C++ Smart Pointer & RAII Bridge (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E2.1 | `unique_ptr<T>` ownership | Map to Fajar ownership (move-only) | 80 | `unique_ptr<Widget>` → owned `Widget` |
| E2.2 | `shared_ptr<T>` reference counting | Map to Fajar `Rc<T>` | 80 | Ref count tracks correctly |
| E2.3 | `weak_ptr<T>` weak references | Map to Fajar `Weak<T>` | 60 | Weak ref doesn't prevent deallocation |
| E2.4 | RAII bridge | C++ destructor called when Fajar value dropped | 80 | No resource leaks across FFI boundary |
| E2.5 | Move semantics | `std::move()` mapped to Fajar moves | 60 | C++ object moved, not copied |
| E2.6 | Copy semantics | Copy constructor called for copyable types | 50 | Deep copy across FFI boundary |
| E2.7 | Custom deleters | Support `unique_ptr<T, Deleter>` | 40 | Custom cleanup runs on drop |
| E2.8 | Ref-qualified methods | `void method() &` vs `void method() &&` | 40 | Correct overload selected |
| E2.9 | Exception safety | Catch C++ exceptions across FFI boundary | 80 | Exception → Fajar `Err(msg)` |
| E2.10 | 10 smart pointer tests | unique, shared, weak, RAII, move, exception | 150 | All 10 pass |

### Sprint E3: C++ STL Container Bridge (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E3.1 | `std::string` ↔ `str` | Zero-copy where possible, copy when needed | 80 | String round-trip preserves content |
| E3.2 | `std::vector<T>` ↔ `Array<T>` | Automatic conversion with element copy | 80 | `[1,2,3]` → `vector<int>` → `[1,2,3]` |
| E3.3 | `std::map<K,V>` ↔ `HashMap<K,V>` | Key-value pair conversion | 80 | Map entries preserved |
| E3.4 | `std::set<T>` ↔ `HashSet<T>` | Unique element collection | 60 | Set operations work |
| E3.5 | `std::optional<T>` ↔ `Option<T>` | None/Some mapping | 40 | `nullopt` → `None`, value → `Some(v)` |
| E3.6 | `std::variant<T...>` ↔ `enum` | Tagged union conversion | 80 | Variant → Fajar enum |
| E3.7 | `std::tuple<T...>` ↔ `tuple` | Positional element access | 60 | Tuple elements accessible |
| E3.8 | `std::array<T,N>` ↔ `[T; N]` | Fixed-size array conversion | 40 | Compile-time size preserved |
| E3.9 | `std::span<T>` ↔ slice | View into contiguous memory | 60 | No-copy view works |
| E3.10 | 10 STL container tests | String, vector, map, optional, variant, tuple | 150 | All 10 pass |

### Sprint E4: Python Async Interop (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E4.1 | Python coroutine calling | Call `async def` Python function from Fajar | 120 | `await py_async_fn()` returns result |
| E4.2 | asyncio event loop bridge | Run Python asyncio loop alongside Fajar tokio | 100 | Both event loops coexist |
| E4.3 | Fajar async → Python awaitable | Expose Fajar async fn as Python awaitable | 100 | Python `await fj_fn()` works |
| E4.4 | Async generator bridge | Python `async for` consumes Fajar async generator | 80 | Async iteration works |
| E4.5 | GIL management | Release GIL during Fajar computation | 60 | Python threads not blocked |
| E4.6 | Python exception → Fajar error | Convert `asyncio.TimeoutError` → `Err(Timeout)` | 50 | Async exceptions propagated |
| E4.7 | Cancellation support | Cancel Python coroutine from Fajar | 50 | Cancelled task raises CancelledError |
| E4.8 | Timeout support | Set timeout for Python async calls | 40 | Timeout triggers cancellation |
| E4.9 | Connection pooling | Share HTTP/DB connections between Fajar and Python | 60 | Single connection pool for both |
| E4.10 | 10 async Python tests | Coroutine, event loop, generator, GIL, cancel | 150 | All 10 pass |

### Sprint E5: Python NumPy/PyTorch Bridge (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E5.1 | Zero-copy NumPy → Tensor | Share memory between numpy.ndarray and Fajar Tensor | 100 | No copy for compatible dtypes |
| E5.2 | Zero-copy Tensor → NumPy | Export Fajar Tensor as numpy view | 80 | Python can read Fajar tensor data |
| E5.3 | PyTorch tensor bridge | Convert torch.Tensor ↔ Fajar Tensor | 100 | GPU tensors stay on GPU |
| E5.4 | dtype mapping | Map all numpy dtypes to Fajar types (float32, int64, etc.) | 60 | All 12 standard dtypes supported |
| E5.5 | Shape/stride handling | Preserve non-contiguous array views | 60 | Transposed views work without copy |
| E5.6 | PyTorch model loading | Load `.pt` model in Fajar via Python bridge | 80 | Pre-trained model runs inference |
| E5.7 | Mixed training | Train in Fajar, evaluate in PyTorch (or vice versa) | 100 | Model weights shared between frameworks |
| E5.8 | ONNX export via Python | `fj_model.export_onnx("model.onnx")` via Python runtime | 60 | Valid ONNX model produced |
| E5.9 | Batch processing | Process batches with automatic numpy/tensor conversion | 60 | Batch pipeline works end-to-end |
| E5.10 | 10 NumPy/PyTorch tests | Zero-copy, dtype, model load, mixed training, ONNX | 150 | All 10 pass |

### Sprint E6: Rust Trait Object Marshalling (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E6.1 | Rust trait → Fajar trait | Import Rust trait definition as Fajar trait | 80 | `trait Display` → Fajar `trait Display` |
| E6.2 | Implement Fajar trait for Rust struct | Fajar implements trait that Rust code calls | 100 | Rust calls Fajar's `Display::fmt()` |
| E6.3 | `dyn Trait` across FFI | Pass trait objects between Fajar and Rust | 100 | vtable dispatch works across boundary |
| E6.4 | Generic function bridge | Call Rust generic fn with Fajar types | 80 | Monomorphization at FFI boundary |
| E6.5 | Lifetime handling | Map Rust lifetimes to Fajar borrow semantics | 80 | No dangling references across FFI |
| E6.6 | Error type bridge | `Result<T, E>` seamless conversion | 50 | Rust errors readable in Fajar |
| E6.7 | Iterator bridge | Consume Rust iterators in Fajar for-in loops | 60 | `for x in rust_iter() { ... }` works |
| E6.8 | Closure bridge | Pass Fajar closures to Rust higher-order functions | 80 | `rust_vec.sort_by(|a, b| a.cmp(b))` works |
| E6.9 | Async bridge | Bridge Rust futures to Fajar async/await | 80 | `await rust_async_fn()` works |
| E6.10 | 10 Rust bridge tests | Traits, dyn, generics, lifetimes, closures, async | 150 | All 10 pass |

### Sprint E7: Automatic Binding Generator (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E7.1 | `fj bindgen` CLI | Generate Fajar bindings from C/C++ headers | 60 | `fj bindgen opencv.hpp` → `opencv.fj` |
| E7.2 | C header parsing | Parse C headers (no libclang needed for simple C) | 120 | `stdio.h` → Fajar bindings |
| E7.3 | C++ header parsing | Use libclang for C++ headers | 80 | Class methods, namespaces resolved |
| E7.4 | Python stub generation | Generate from `.pyi` stub files | 80 | Type-safe Python bindings |
| E7.5 | Rust crate binding | Parse Rust `pub` items from source | 80 | Public API imported to Fajar |
| E7.6 | Binding customization | `bindgen.toml` for type overrides, skip patterns | 60 | Customize generated bindings |
| E7.7 | Documentation preservation | Copy doc comments from source to bindings | 40 | Hover shows original docs |
| E7.8 | Incremental regeneration | Only regenerate changed headers | 40 | Cached bindings reused |
| E7.9 | Safety annotations | Mark unsafe bindings, generate safe wrappers | 60 | `unsafe` boundaries clear |
| E7.10 | 10 bindgen tests | C, C++, Python, Rust, custom, incremental | 150 | All 10 pass |

### Sprint E8: Build System Integration (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E8.1 | `[ffi]` section in fj.toml | Configure FFI targets: `cpp = ["opencv"]`, `python = ["numpy"]` | 40 | fj.toml parsed |
| E8.2 | Auto-detect system libraries | `pkg-config` integration for C/C++ libs | 60 | OpenCV found via pkg-config |
| E8.3 | Python venv integration | Use `fj.toml` specified venv for Python FFI | 40 | `.venv` activated for builds |
| E8.4 | CMake integration | Build C++ dependencies with CMake | 80 | `cmake --build` runs as part of `fj build` |
| E8.5 | Cargo integration | Build Rust dependencies with Cargo | 60 | `cargo build` runs for Rust deps |
| E8.6 | Linker flag management | Auto-add `-lopencv_core`, `-lpython3.12`, etc. | 60 | Correct libraries linked |
| E8.7 | Cross-compilation support | FFI works with cross-compilation targets | 80 | ARM64 FFI bindings compile |
| E8.8 | Hermetic builds | Vendor all FFI dependencies | 60 | Build succeeds without internet |
| E8.9 | CI integration | GitHub Actions with FFI dependencies | 50 | CI builds with all FFI backends |
| E8.10 | 10 build system tests | fj.toml, pkg-config, venv, cmake, cross | 150 | All 10 pass |

### Sprint E9: Safety & Performance (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E9.1 | FFI boundary validation | Validate types at FFI boundary (no UB) | 80 | Invalid types → clear error |
| E9.2 | Memory leak detection | Detect leaked FFI objects with Drop tracking | 60 | Leak report on program exit |
| E9.3 | Thread safety verification | Ensure FFI calls respect thread safety | 60 | GIL held for Python, locks for C++ |
| E9.4 | Performance overhead measurement | Benchmark FFI call overhead | 40 | < 100ns per FFI call |
| E9.5 | Batch optimization | Amortize FFI overhead for batch calls | 60 | 1000 calls batched into 1 FFI transition |
| E9.6 | Zero-copy verification | Verify zero-copy paths have no hidden copies | 40 | Memory addresses match across boundary |
| E9.7 | Alignment handling | Handle type alignment differences across languages | 50 | No alignment faults |
| E9.8 | Endianness handling | Handle byte order for cross-platform FFI | 40 | Big/little endian conversion correct |
| E9.9 | Sanitizer integration | ASAN/MSAN/TSAN for FFI boundary | 60 | No sanitizer errors |
| E9.10 | 10 safety tests | Validation, leaks, threads, alignment, sanitizers | 150 | All 10 pass |

### Sprint E10: Documentation & Examples (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| E10.1 | C++ FFI tutorial | Step-by-step OpenCV usage from Fajar | 200 | Tutorial builds and runs |
| E10.2 | Python FFI tutorial | NumPy + scikit-learn from Fajar | 200 | Tutorial builds and runs |
| E10.3 | Rust FFI tutorial | Using Rust crate from Fajar | 150 | Tutorial builds and runs |
| E10.4 | Example: OpenCV image processing | `examples/opencv_ffi.fj` with face detection | 100 | Face detection works |
| E10.5 | Example: NumPy data science | `examples/numpy_ffi.fj` with data analysis | 100 | Data analysis pipeline works |
| E10.6 | Example: PyTorch inference | `examples/pytorch_ffi.fj` with model inference | 100 | Pre-trained model runs |
| E10.7 | Example: Rust interop | `examples/rust_ffi.fj` with serde_json | 80 | JSON parsing via Rust works |
| E10.8 | API reference | `book/ffi_v2_reference.md` with all types and functions | 200 | Complete API documented |
| E10.9 | Migration guide | "From C FFI to FFI v2" guide | 100 | Existing users can migrate |
| E10.10 | Update GAP_ANALYSIS_V2 | Mark FFI v2 as 100% production | 20 | Audit reflects real status |

**Option E Total: 100 tasks, ~8,000 LOC, 100 tests, full C++/Python/Rust interop**

---

## Option F: SMT Formal Verification (10 sprints, 100 tasks)

### Context

SMT verification exists (2,422 LOC, 67 tests) with Z3 integration, spec language, and tensor shape verification. But it lacks symbolic execution, proof caching, comprehensive property coverage, and integration with the compilation pipeline. For production: verify @kernel/@device safety at compile time, symbolic execution for path exploration, and automated property inference.

### Sprint V1: Symbolic Execution Engine (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V1.1 | Symbolic value representation | `SymValue { concrete, symbolic, constraints }` | 100 | Symbolic int with constraints |
| V1.2 | Symbolic expression tree | Build symbolic expressions from Fajar AST | 120 | `x + 1 > 0` → symbolic constraint |
| V1.3 | Path condition tracking | Collect path conditions at branches | 80 | If/else generates 2 path conditions |
| V1.4 | Symbolic memory model | Symbolic arrays for heap/stack modeling | 100 | `arr[sym_idx]` creates symbolic read |
| V1.5 | Loop handling | Bounded unrolling (default: 10 iterations) | 80 | Loop explored up to bound |
| V1.6 | Function summaries | Cache function input→output constraints | 80 | Avoid re-analyzing called functions |
| V1.7 | Path explosion mitigation | Merge paths with similar constraints | 60 | Exponential blowup controlled |
| V1.8 | Concolic execution | Concrete + symbolic execution for guided exploration | 100 | Concrete run guides symbolic search |
| V1.9 | Counterexample generation | Generate concrete input that violates property | 60 | `x=5, y=-3` causes array out-of-bounds |
| V1.10 | 10 symbolic execution tests | Expressions, paths, memory, loops, counterexamples | 150 | All 10 pass |

### Sprint V2: Property Specification Language (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V2.1 | `@requires` precondition | `@requires(n > 0)` on function entry | 60 | Precondition checked at call sites |
| V2.2 | `@ensures` postcondition | `@ensures(result >= 0)` on function exit | 60 | Postcondition verified |
| V2.3 | `@invariant` loop invariant | `@invariant(i >= 0 && i < len)` in loops | 80 | Invariant holds at each iteration |
| V2.4 | `@assert` inline assertion | `@assert(ptr != null)` in code | 40 | Assertion verified statically |
| V2.5 | Quantified properties | `@forall(i in 0..len, arr[i] >= 0)` | 80 | Universal quantifier checked |
| V2.6 | Temporal properties | `@eventually(lock_released)` for resource safety | 60 | Liveness property verified |
| V2.7 | Type state properties | `@typestate(File: Closed -> Open -> Closed)` | 80 | State machine enforced |
| V2.8 | Data flow properties | `@no_leak(secret_key)` — sensitive data doesn't flow to public | 60 | Information flow checked |
| V2.9 | Custom property macros | `@property("sorted", arr)` with user-defined checker | 60 | Extensible property system |
| V2.10 | 10 property spec tests | Requires, ensures, invariant, quantified, temporal | 150 | All 10 pass |

### Sprint V3: @kernel Safety Proofs (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V3.1 | No-heap proof | Verify @kernel functions never allocate heap memory | 80 | Heap alloc in @kernel → verification error |
| V3.2 | No-tensor proof | Verify @kernel functions never create tensors | 60 | Tensor creation in @kernel → error |
| V3.3 | Stack bound proof | Verify @kernel stack usage < configurable limit | 100 | Stack overflow impossible |
| V3.4 | Interrupt safety | Verify IRQ handlers don't hold locks | 80 | Lock in IRQ → verification warning |
| V3.5 | Memory-mapped I/O safety | Verify MMIO reads/writes to valid regions only | 80 | MMIO to invalid address → error |
| V3.6 | DMA buffer safety | Verify DMA buffers are properly aligned and sized | 60 | Misaligned DMA → error |
| V3.7 | Concurrency safety | Verify no data races in @kernel code | 100 | Shared mutable access → error |
| V3.8 | Panic-freedom | Verify @kernel functions cannot panic | 80 | Panic path in @kernel → error |
| V3.9 | Termination proof | Verify @kernel functions always terminate | 60 | Infinite loop in @kernel → warning |
| V3.10 | 10 kernel safety tests | No-heap, stack bound, IRQ, MMIO, panic-free | 150 | All 10 pass |

### Sprint V4: @device Safety Proofs (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V4.1 | No-raw-pointer proof | Verify @device functions never use raw pointers | 60 | Raw pointer in @device → error |
| V4.2 | Tensor shape proof | Verify matmul dimensions compatible at compile time | 100 | `matmul(3x4, 5x6)` → error |
| V4.3 | Tensor dtype proof | Verify operation dtypes compatible | 60 | `add(f32_tensor, i64_tensor)` → error |
| V4.4 | Memory bound proof | Verify tensor operations stay within allocation | 80 | Out-of-bounds slice → error |
| V4.5 | Gradient tracking proof | Verify backward() called only on tracked tensors | 60 | Backward on non-tracked → error |
| V4.6 | Numerical stability | Verify no division by zero, log(0), sqrt(neg) | 100 | Unstable operation → warning |
| V4.7 | Shape inference proof | Verify inferred shapes match runtime shapes | 80 | Shape mismatch → error |
| V4.8 | Broadcast compatibility | Verify broadcast rules satisfied for binary ops | 60 | Incompatible broadcast → error |
| V4.9 | Memory layout proof | Verify tensor memory layout (row-major vs col-major) consistent | 50 | Layout mismatch → error |
| V4.10 | 10 device safety tests | No-pointer, shapes, dtype, gradient, numerical | 150 | All 10 pass |

### Sprint V5: Proof Caching & Incrementality (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V5.1 | Proof result cache | Cache verification results per function | 80 | Unchanged function → cached result |
| V5.2 | Cache invalidation | Invalidate on function change or dependency change | 60 | Modified function re-verified |
| V5.3 | Incremental verification | Only verify changed functions + dependents | 80 | 1 function changed → only related proofs re-run |
| V5.4 | Parallel verification | Run Z3 on multiple functions concurrently | 60 | 4 functions verified in parallel |
| V5.5 | Timeout management | Per-function timeout (default: 10s) with fallback | 40 | Complex proof → timeout → warning |
| V5.6 | Proof persistence | Store proofs in `target/verify/` for CI sharing | 60 | CI reuses developer proofs |
| V5.7 | Proof visualization | `fj verify --report` → HTML report with proof status | 80 | Visual proof coverage report |
| V5.8 | Counterexample display | Show concrete failing input in error message | 60 | "Counterexample: x=-1 violates x>=0" |
| V5.9 | Proof statistics | Track verification time, cache hit rate, coverage | 40 | Stats displayed with `--verbose` |
| V5.10 | 10 caching tests | Cache, invalidate, incremental, parallel, persist | 150 | All 10 pass |

### Sprint V6: Automated Property Inference (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V6.1 | Null safety inference | Infer "result is never null" from code analysis | 80 | Nullable path → warning |
| V6.2 | Bounds inference | Infer "array index in bounds" from loop structure | 80 | `for i in 0..len { arr[i] }` verified safe |
| V6.3 | Overflow inference | Infer "no integer overflow" from value ranges | 80 | `u8 + u8` → potential overflow warning |
| V6.4 | Division safety | Infer "denominator != 0" from context | 60 | `x / y` verified safe when `y != 0` proven |
| V6.5 | Resource cleanup | Infer "all resources dropped" from scope analysis | 80 | Leaked file handle → warning |
| V6.6 | Unreachable code | Infer "dead code after return/panic" | 40 | Code after `return` → warning |
| V6.7 | Type narrowing | Infer narrowed types after `if` checks | 60 | `if x != null { x.method() }` — x is non-null |
| V6.8 | Pattern exhaustiveness | Verify match arms cover all cases | 60 | Missing variant → error |
| V6.9 | Purity inference | Infer "function has no side effects" | 60 | Pure functions marked for optimization |
| V6.10 | 10 inference tests | Null, bounds, overflow, division, resource, purity | 150 | All 10 pass |

### Sprint V7: Pipeline Integration (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V7.1 | `fj verify` CLI command | Run verification on project | 30 | `fj verify` checks all annotated functions |
| V7.2 | `fj check --verify` | Integrate verification with type checking | 20 | `fj check --verify` includes proofs |
| V7.3 | LSP verification hints | Show verification status as inlay hints | 60 | Green checkmark for verified functions |
| V7.4 | CI verification step | `fj verify --ci` returns exit code on failure | 20 | CI fails on unverified @kernel code |
| V7.5 | Verification level config | `[verify]` section in fj.toml with level (off/warn/error) | 40 | Config controls verification strictness |
| V7.6 | Suppression comments | `// @suppress(no-overflow)` to suppress specific checks | 30 | Known-safe code not flagged |
| V7.7 | Verification in REPL | `fj repl --verify` checks expressions live | 40 | REPL shows verification warnings |
| V7.8 | IDE code actions | "Add @requires annotation" from LSP | 60 | One-click annotation addition |
| V7.9 | Verification diff | `fj verify --diff` shows only new issues since last run | 40 | Only new issues flagged |
| V7.10 | 10 integration tests | CLI, LSP, CI, config, suppression, diff | 150 | All 10 pass |

### Sprint V8: Advanced Theories (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V8.1 | Bitvector theory | Precise reasoning about u8/u16/u32/u64 overflow | 80 | Bit-exact overflow detection |
| V8.2 | Array theory | SMT array reasoning for buffer operations | 80 | Buffer bounds verified |
| V8.3 | Floating-point theory | IEEE 754 reasoning for float operations | 80 | NaN/Inf detection |
| V8.4 | String theory | SMT string reasoning for string operations | 60 | Buffer overflow in string ops detected |
| V8.5 | Nonlinear arithmetic | Polynomial constraint solving | 60 | `x*x >= 0` proven |
| V8.6 | Separation logic | Heap reasoning for pointer operations | 100 | Aliasing detected |
| V8.7 | Concurrent theory | Reasoning about lock ordering, atomics | 100 | Deadlock potential detected |
| V8.8 | Theory combination | Nelson-Oppen combination of multiple theories | 80 | Mixed int/float/array constraints solved |
| V8.9 | Custom theory plugins | User-defined theory for domain-specific reasoning | 60 | Custom theory loaded at verify time |
| V8.10 | 10 theory tests | Bitvec, array, float, string, concurrent, combined | 150 | All 10 pass |

### Sprint V9: Safety Certification Support (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V9.1 | MISRA-C compliance check | Subset of MISRA rules for @kernel code | 100 | MISRA violations reported |
| V9.2 | CERT-C compliance check | Memory safety rules from CERT-C | 80 | CERT violations reported |
| V9.3 | DO-178C evidence | Generate verification evidence for aerospace | 100 | Evidence document produced |
| V9.4 | ISO 26262 evidence | Generate evidence for automotive ASIL-D | 80 | ASIL analysis report generated |
| V9.5 | IEC 62304 evidence | Generate evidence for medical device software | 60 | Medical SW class report |
| V9.6 | Traceability matrix | Map requirements to verified properties | 60 | Requirements covered by proofs |
| V9.7 | Verification coverage report | MCDC-style coverage of verification conditions | 80 | Coverage percentage reported |
| V9.8 | Audit trail | Log all verification decisions with timestamps | 40 | Auditable verification history |
| V9.9 | Certificate generation | Machine-readable verification certificate | 60 | Certificate for deployment gate |
| V9.10 | 10 certification tests | MISRA, CERT, DO-178C, ISO 26262, traceability | 150 | All 10 pass |

### Sprint V10: Benchmarks & Documentation (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| V10.1 | Verification time benchmark | Measure per-function verification time | 30 | < 10s per function average |
| V10.2 | Scalability benchmark | Verify 1K/10K/50K LOC projects | 30 | Scales sub-linearly |
| V10.3 | False positive rate | Measure false positive rate on known-safe code | 40 | < 5% false positive rate |
| V10.4 | Bug detection rate | Test against known-buggy code samples | 40 | > 90% of planted bugs detected |
| V10.5 | Proof cache speedup | Measure incremental verification speedup | 20 | > 10x speedup with cache |
| V10.6 | Documentation | `book/formal_verification.md` | 200 | Guide with annotation syntax + examples |
| V10.7 | Example: verified kernel | `examples/verified_kernel.fj` with full proofs | 150 | All @kernel functions verified |
| V10.8 | Example: verified ML | `examples/verified_ml.fj` with shape proofs | 100 | All shapes verified |
| V10.9 | Update CLAUDE.md | Document verify CLI and annotations | 30 | CLAUDE.md reflects verification |
| V10.10 | Update GAP_ANALYSIS_V2 | Mark SMT as 100% production | 20 | Audit reflects real status |

**Option F Total: 100 tasks, ~7,500 LOC, 100 tests, formal verification pipeline**

---

## Option G: Self-Hosting Compiler (10 sprints, 100 tasks)

### Context

Self-hosting has real progress (3,076 LOC in stdlib/) — lexer (513 LOC, 80+ tokens), parser (784 LOC, 26 functions), and analyzer (432 LOC, 11 functions) all work in Fajar Lang. But the parser uses flat arrays instead of tree AST, codegen is incomplete, and Stage 2 bootstrap (compiler compiles itself) has not been achieved. For production: tree-based AST, complete codegen, Stage 2 bootstrap, and self-test.

### Sprint S1: Tree-Based AST (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S1.1 | AST node type hierarchy | `enum Expr { Int(i64), BinOp(Box<Expr>, Op, Box<Expr>), ... }` in Fajar | 150 | 25+ expression variants defined |
| S1.2 | Statement nodes | `enum Stmt { Let, Fn, Struct, Enum, While, For, Return, ... }` | 100 | All statement types represented |
| S1.3 | Type expression nodes | `enum TypeExpr { Name, Generic, Array, Reference, ... }` | 80 | Type AST matches Rust reference |
| S1.4 | Pattern nodes | `enum Pattern { Ident, Tuple, Struct, Enum, Wildcard, ... }` | 60 | Pattern matching AST |
| S1.5 | Program node | `struct Program { items: Array<Item> }` as root | 30 | Top-level structure |
| S1.6 | Span tracking | Every node carries `span: Span` for error reporting | 40 | Spans correct for all node types |
| S1.7 | AST pretty printer | `fn print_ast(node: Expr) -> str` for debugging | 80 | AST round-trips through printer |
| S1.8 | AST visitor pattern | `fn walk(node: Expr, visitor: fn(Expr) -> void)` | 60 | Visitor traverses entire tree |
| S1.9 | AST serialization | Serialize AST to JSON for debugging | 80 | AST → JSON → AST round-trip |
| S1.10 | 10 AST tests | Node creation, printer, visitor, serialization | 150 | All 10 pass |

### Sprint S2: Parser Upgrade (Flat → Tree) (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S2.1 | Pratt expression parser | Port Pratt parser with 19 precedence levels to Fajar | 200 | Operator precedence correct |
| S2.2 | Statement parser | Parse let/fn/struct/enum/impl/trait/while/for/match | 200 | All statement types parsed |
| S2.3 | Type expression parser | Parse `Array<i32>`, `&T`, `fn(i32) -> bool` | 100 | Complex type expressions |
| S2.4 | Pattern parser | Parse `Some(x)`, `(a, b)`, `Point { x, y }` | 80 | All pattern forms |
| S2.5 | Error recovery | Synchronize on `;`/`}` after parse error, continue | 80 | Multiple errors reported |
| S2.6 | Operator precedence table | Define all 19 levels in Fajar data structure | 40 | Table matches spec |
| S2.7 | Parenthesized expressions | Handle grouping `(a + b) * c` | 20 | Precedence override works |
| S2.8 | If/match as expressions | `let x = if cond { a } else { b }` | 40 | Expression position if/match |
| S2.9 | Lambda expressions | `\|x, y\| x + y` parsed as closure | 40 | Closure AST node correct |
| S2.10 | 10 parser tests | Compare output to Rust parser for 10 programs | 150 | All 10 match reference |

### Sprint S3: Semantic Analyzer Upgrade (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S3.1 | Symbol table | Scope stack with HashMap<str, Type> per scope | 100 | Nested scopes resolve correctly |
| S3.2 | Type inference | Infer types from initializers and return values | 120 | `let x = 42` → `x: i64` |
| S3.3 | Type checking | Binary ops, function calls, struct fields | 100 | Type mismatch detected |
| S3.4 | Scope resolution | Block scoping, function scoping, module scoping | 80 | Variable shadowing works |
| S3.5 | Use-after-move detection | Track moved values in scope | 60 | Use after move → error |
| S3.6 | Mutability checking | `let mut` required for mutation | 40 | Mutation without mut → error |
| S3.7 | Exhaustiveness checking | Match arms cover all enum variants | 80 | Missing variant → error |
| S3.8 | Generic type checking | Monomorphization-aware type checking | 80 | `fn add<T>(a: T, b: T)` checked |
| S3.9 | Trait bound checking | `where T: Display` enforced at call sites | 60 | Missing impl → error |
| S3.10 | 10 analyzer tests | Types, scopes, moves, generics, traits | 150 | All 10 pass |

### Sprint S4: Bytecode Codegen in Fajar (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S4.1 | Opcode definition | 45 opcodes matching VM specification | 60 | All opcodes defined |
| S4.2 | Expression compilation | Compile BinOp, UnaryOp, Call, Index to bytecode | 150 | Arithmetic expressions compile |
| S4.3 | Control flow compilation | Compile if/while/for/loop/break/continue | 120 | Control flow correct |
| S4.4 | Function compilation | Compile fn definitions with locals and returns | 100 | Function calls work |
| S4.5 | Variable compilation | Compile let/mut/assign with local slot allocation | 60 | Variables stored and loaded |
| S4.6 | String/array compilation | Compile string/array literals and operations | 80 | Composite types work |
| S4.7 | Struct compilation | Compile struct init, field access, method calls | 80 | Struct operations work |
| S4.8 | Match compilation | Compile match → jump table / if-else chain | 80 | Pattern matching works |
| S4.9 | Closure compilation | Compile closure capture and invocation | 60 | Closures capture free variables |
| S4.10 | 10 codegen tests | Arithmetic, control, functions, structs, closures | 150 | All 10 produce correct bytecode |

### Sprint S5: Standard Library in Fajar (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S5.1 | String operations | `len`, `contains`, `split`, `trim`, `replace` in Fajar | 100 | 10 string methods work |
| S5.2 | Array operations | `push`, `pop`, `map`, `filter`, `sort` in Fajar | 100 | 10 array methods work |
| S5.3 | HashMap in Fajar | `insert`, `get`, `contains`, `remove`, `keys` | 120 | HashMap with chaining |
| S5.4 | File I/O wrappers | `read_file`, `write_file` using builtins | 40 | File round-trip works |
| S5.5 | Math functions | `abs`, `min`, `max`, `pow`, `sqrt` | 60 | Math accuracy verified |
| S5.6 | Error types | `Result<T,E>`, `Option<T>` with methods | 60 | `?` operator works |
| S5.7 | Iterator protocol | `.iter()`, `.map()`, `.filter()`, `.collect()` | 100 | Iterator chain works |
| S5.8 | Formatting | `format("Hello {}", name)` string interpolation | 60 | Format strings work |
| S5.9 | Debug printing | `dbg(value)` with type and value display | 30 | Debug output helpful |
| S5.10 | 10 stdlib tests | String, array, map, file, math, iterator | 150 | All 10 pass |

### Sprint S6: Stage 1 Bootstrap (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S6.1 | Compile lexer.fj | Self-hosted compiler compiles lexer.fj to bytecode | 40 | Bytecode produced |
| S6.2 | Compile parser.fj | Self-hosted compiler compiles parser.fj to bytecode | 40 | Bytecode produced |
| S6.3 | Compile analyzer.fj | Self-hosted compiler compiles analyzer.fj to bytecode | 40 | Bytecode produced |
| S6.4 | Compile codegen.fj | Self-hosted compiler compiles codegen.fj to bytecode | 40 | Bytecode produced |
| S6.5 | Compile compiler.fj | Self-hosted compiler compiles itself to bytecode | 40 | Bytecode produced (Stage 1!) |
| S6.6 | Differential testing | Compare Stage 0 (Rust) vs Stage 1 (Fajar) output | 100 | Identical bytecode for test programs |
| S6.7 | Performance comparison | Benchmark Stage 0 vs Stage 1 compilation speed | 30 | Stage 1 within 5x of Stage 0 |
| S6.8 | Error message comparison | Compare error messages between stages | 40 | Same errors reported |
| S6.9 | 100-program test | Compile 100 test programs with Stage 1 | 60 | All 100 produce correct output |
| S6.10 | Document Stage 1 | Write `book/self_hosting.md` with bootstrap process | 100 | Bootstrap process documented |

### Sprint S7: Stage 2 Bootstrap (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S7.1 | Fix stack overflow | Increase recursion limit or use trampolining | 80 | Deep recursion handled |
| S7.2 | Fix closure capture bugs | Ensure all free variables captured correctly | 60 | Closures work in self-hosted |
| S7.3 | Fix generic instantiation | Self-hosted compiler handles its own generics | 80 | Generic functions compile |
| S7.4 | Fix pattern matching | Self-hosted match handles all compiler's own patterns | 60 | Complex patterns work |
| S7.5 | Stage 2 compilation | Stage 1 compiler compiles itself → Stage 2 | 40 | Stage 2 binary produced |
| S7.6 | Stage 2 validation | Stage 2 output matches Stage 1 output | 60 | Identical binaries (fixed point) |
| S7.7 | Triple bootstrap | Stage 2 → Stage 3, verify Stage 2 == Stage 3 | 30 | Fixed point reached |
| S7.8 | CI bootstrap test | GitHub Actions runs Stage 1 → Stage 2 bootstrap | 40 | Bootstrap succeeds in CI |
| S7.9 | Reproducible bootstrap | Bootstrap produces identical output across platforms | 40 | Linux == macOS == Windows |
| S7.10 | Bootstrap documentation | Document full bootstrap procedure | 100 | Step-by-step guide |

### Sprint S8: Performance Optimization (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S8.1 | String interning | Intern identifiers and keywords to reduce allocations | 80 | 50% fewer string allocations |
| S8.2 | Arena allocation | Allocate AST nodes from arena instead of individual alloc | 100 | 3x faster AST construction |
| S8.3 | Inline caching | Cache method lookup results | 60 | Method calls 2x faster |
| S8.4 | Tail call optimization | Detect and optimize tail recursion | 60 | Recursive descent doesn't stack overflow |
| S8.5 | Constant folding | Fold constant expressions at compile time | 60 | `2 + 3` → `5` at compile time |
| S8.6 | Dead code elimination | Remove unreachable code | 60 | Smaller bytecode output |
| S8.7 | Register allocation | Optimize local variable to register mapping | 80 | Fewer stack accesses |
| S8.8 | Peephole optimization | Optimize common bytecode patterns | 60 | 10% smaller bytecode |
| S8.9 | Benchmark: 10K LOC | Compile 10K LOC project with self-hosted compiler | 30 | < 5s compilation time |
| S8.10 | 10 optimization tests | Interning, arena, TCO, constant fold, peephole | 150 | All 10 pass |

### Sprint S9: Error Messages & UX (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S9.1 | Source snippets | Show source code snippet with error location | 80 | Error shows `^^^` underline |
| S9.2 | Error codes | All errors have codes (PE001, SE004, etc.) | 40 | Codes match Rust compiler |
| S9.3 | Suggestions | "Did you mean `println`?" for typos | 60 | Edit distance suggestions |
| S9.4 | Multi-error display | Show all errors, not just first | 40 | Multiple errors displayed |
| S9.5 | Color output | ANSI colors for error/warning/note | 40 | Colored terminal output |
| S9.6 | Error recovery | Continue parsing/analyzing after errors | 60 | Maximum number of errors reported |
| S9.7 | Warning system | Warnings for unused variables, dead code | 40 | Warnings don't block compilation |
| S9.8 | Help messages | `--explain PE001` shows detailed error explanation | 60 | Explanation displayed |
| S9.9 | JSON error output | `--error-format=json` for IDE integration | 40 | Errors as JSON array |
| S9.10 | 10 error UX tests | Snippets, suggestions, multi-error, colors, JSON | 150 | All 10 pass |

### Sprint S10: Validation & Completion (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| S10.1 | Compile all examples | Self-hosted compiler handles all 178 examples | 60 | 178/178 examples compile |
| S10.2 | Compile stdlib | Self-hosted compiler compiles stdlib/*.fj | 40 | All stdlib files compile |
| S10.3 | Self-test suite | Run compiler's own tests with self-hosted compiler | 60 | Tests pass on self-hosted |
| S10.4 | Feature parity audit | Compare supported features vs Rust compiler | 100 | 90%+ feature parity |
| S10.5 | Binary size comparison | Compare self-hosted vs Rust compiler binary size | 20 | Document size difference |
| S10.6 | Memory usage comparison | Compare memory usage during compilation | 20 | Document memory difference |
| S10.7 | Compilation speed comparison | Side-by-side speed on 10K LOC | 20 | Document speed difference |
| S10.8 | Example: self-hosted build | `fj build --self-hosted examples/hello.fj` | 40 | End-to-end self-hosted build |
| S10.9 | Update CLAUDE.md | Document self-hosting in architecture section | 30 | CLAUDE.md reflects self-hosting |
| S10.10 | Update GAP_ANALYSIS_V2 | Mark self-hosting as 100% production | 20 | Audit reflects real status |

**Option G Total: 100 tasks, ~7,000 LOC, 100 tests, Stage 2 bootstrap achieved**

---

## Option H: Const Fn + Compile-Time Eval (10 sprints, 100 tasks)

### Context

Compile-time evaluation exists (713 LOC, 13 tests) with `comptime {}` blocks, arithmetic, control flow, and function calls. But it lacks const generics (`[T; N]`), const trait bounds, compile-time allocation, and integration with the type system. For production: full const generics, compile-time trait evaluation, and const-evaluated standard library.

### Sprint K1: Const Generics Foundation (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K1.1 | Const parameter syntax | `fn zeros<const N: usize>() -> [f64; N]` parsed | 80 | Parser handles const generics |
| K1.2 | Const parameter in type system | `N` usable as type-level integer | 100 | `[T; N]` has type `Array<T, N>` |
| K1.3 | Const parameter monomorphization | Instantiate `zeros<3>()` → concrete function | 80 | Monomorphized function correct |
| K1.4 | Const expressions in types | `[T; N + 1]`, `[T; N * 2]` in type positions | 60 | Arithmetic in type expressions |
| K1.5 | Const parameter bounds | `where N > 0` compile-time constraint | 60 | `zeros<0>()` → compile error |
| K1.6 | Const parameter inference | Infer N from context: `let arr: [i32; 3] = zeros()` | 60 | N inferred as 3 |
| K1.7 | Const parameter in struct | `struct Matrix<const R: usize, const C: usize>` | 60 | Struct parameterized by const |
| K1.8 | Const parameter in enum | `enum SmallVec<T, const N: usize>` | 40 | Enum with inline storage |
| K1.9 | Const parameter in impl | `impl<const N: usize> Matrix<N, N> { fn identity() }` | 50 | Impl blocks with const params |
| K1.10 | 10 const generic tests | Parse, monomorphize, infer, struct, enum, impl | 150 | All 10 pass |

### Sprint K2: Const Fn Enhancement (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K2.1 | `const fn` declaration | `const fn fib(n: usize) -> usize` syntax | 40 | Parser accepts `const fn` |
| K2.2 | Const fn type checking | Verify const fn body is const-evaluable | 80 | Non-const operations → error |
| K2.3 | Const fn recursion | Support recursive const fns with bounded depth | 60 | `const fn factorial(n)` works |
| K2.4 | Const fn with generics | `const fn size_of<T>() -> usize` | 60 | Generic const fn works |
| K2.5 | Const fn with structs | Construct structs at compile time | 60 | `const ORIGIN: Point = Point { x: 0.0, y: 0.0 }` |
| K2.6 | Const fn with arrays | Create and index arrays at compile time | 60 | `const PRIMES: [i32; 5] = compute_primes()` |
| K2.7 | Const fn with match | Pattern matching in const context | 60 | `const fn abs(x: i32) -> i32 { match ... }` |
| K2.8 | Const fn with loops | For/while loops in const context (bounded) | 60 | Loop unrolled at compile time |
| K2.9 | Const fn panic | `const_panic!("message")` at compile time | 30 | Compile-time panic → error message |
| K2.10 | 10 const fn tests | Recursion, generics, structs, arrays, match, loops | 150 | All 10 pass |

### Sprint K3: Const Trait Bounds (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K3.1 | `const trait` definition | `const trait ConstAdd { const fn add(self, other: Self) -> Self }` | 60 | Trait parsed |
| K3.2 | `const impl` | `const impl ConstAdd for i32 { const fn add... }` | 60 | Impl parsed and checked |
| K3.3 | Const trait bounds | `fn sum<T: const ConstAdd>(arr: [T; N])` | 60 | Bound enforced at call site |
| K3.4 | Const trait method dispatch | Call const trait method at compile time | 80 | Static dispatch in const context |
| K3.5 | Built-in const traits | `const ConstEq`, `const ConstOrd`, `const ConstDefault` | 60 | Built-in types implement const traits |
| K3.6 | Derive const traits | `#[derive(ConstEq, ConstDefault)]` for user types | 60 | Derived implementations work |
| K3.7 | Const trait objects | `&dyn const ConstAdd` for compile-time polymorphism | 80 | Const vtable dispatch |
| K3.8 | Const where clauses | `where T: const Add + const Mul` compound bounds | 40 | Multiple const bounds |
| K3.9 | Const associated types | `const type Output;` in const traits | 50 | Associated type resolved at compile time |
| K3.10 | 10 const trait tests | Definition, impl, bounds, dispatch, derive | 150 | All 10 pass |

### Sprint K4: Compile-Time Allocation (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K4.1 | Const array allocation | `const ARR: [i32; 1000] = fill(0)` | 60 | Large const array in .rodata |
| K4.2 | Const string allocation | `const GREETING: str = format("Hello {}", NAME)` | 60 | String in .rodata |
| K4.3 | Const struct allocation | `const CONFIG: Config = Config { ... }` | 40 | Struct in .rodata |
| K4.4 | Const HashMap | `const MAP: HashMap<str, i32> = build_map()` | 100 | Map precomputed, stored as static |
| K4.5 | Const slice | `const SLICE: &[i32] = &[1, 2, 3]` | 40 | Slice points to .rodata |
| K4.6 | Static promotion | Promote const expressions to static storage | 60 | Temporary const → static lifetime |
| K4.7 | Const allocator | Arena allocator for compile-time computations | 80 | No runtime allocation for const data |
| K4.8 | Const size verification | Verify const allocation fits in target memory | 40 | > 1MB const data → warning |
| K4.9 | Cross-compilation const | Const evaluation respects target pointer size | 40 | Const correct for ARM64 target |
| K4.10 | 10 allocation tests | Array, string, struct, map, slice, promotion | 150 | All 10 pass |

### Sprint K5: Compile-Time Reflection (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K5.1 | `type_name::<T>()` | Return type name as const string | 40 | `type_name::<i32>()` → `"i32"` |
| K5.2 | `size_of::<T>()` | Return byte size at compile time | 40 | `size_of::<i64>()` → `8` |
| K5.3 | `align_of::<T>()` | Return alignment at compile time | 30 | `align_of::<f64>()` → `8` |
| K5.4 | `field_count::<T>()` | Return struct field count | 40 | `field_count::<Point>()` → `2` |
| K5.5 | `field_names::<T>()` | Return field names as const array | 60 | `field_names::<Point>()` → `["x", "y"]` |
| K5.6 | `field_types::<T>()` | Return field type names | 60 | `field_types::<Point>()` → `["f64", "f64"]` |
| K5.7 | `variant_count::<T>()` | Return enum variant count | 40 | `variant_count::<Option<i32>>()` → `2` |
| K5.8 | `variant_names::<T>()` | Return variant names | 40 | `variant_names::<Option<i32>>()` → `["Some", "None"]` |
| K5.9 | `has_trait::<T, Trait>()` | Check if type implements trait at compile time | 60 | `has_trait::<i32, Display>()` → `true` |
| K5.10 | 10 reflection tests | Type name, size, align, fields, variants, traits | 150 | All 10 pass |

### Sprint K6: Const Evaluation in Macros (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K6.1 | `const_eval!()` macro | Evaluate expression at compile time in macro | 60 | `const_eval!(1 + 2)` → `3` |
| K6.2 | `static_assert!()` | Compile-time assertion | 40 | `static_assert!(size_of::<T>() <= 64)` |
| K6.3 | `include_str!()` | Include file contents as const string | 40 | File embedded at compile time |
| K6.4 | `include_bytes!()` | Include file as const byte array | 40 | Binary data embedded |
| K6.5 | `env!()` | Read environment variable at compile time | 30 | `env!("VERSION")` → build-time value |
| K6.6 | `concat!()` | Concatenate const strings | 30 | `concat!("v", env!("VERSION"))` |
| K6.7 | `cfg!()` enhancement | `cfg!(target_os = "linux")` as const bool | 30 | Conditional compilation |
| K6.8 | `option_env!()` | Optional env var as `Option<str>` | 30 | Missing var → `None` |
| K6.9 | `compile_error!()` | User-defined compile error | 20 | Custom error message at compile time |
| K6.10 | 10 macro const tests | eval, assert, include, env, cfg, compile_error | 150 | All 10 pass |

### Sprint K7: Const in Standard Library (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K7.1 | Const math functions | `const fn abs`, `const fn min`, `const fn max`, `const fn clamp` | 60 | Math at compile time |
| K7.2 | Const string operations | `const fn str_len`, `const fn str_eq` | 40 | String ops at compile time |
| K7.3 | Const array operations | `const fn array_len`, `const fn array_get` | 40 | Array ops at compile time |
| K7.4 | Const Option methods | `const fn unwrap_or`, `const fn is_some`, `const fn map` | 40 | Option at compile time |
| K7.5 | Const Result methods | `const fn unwrap_or`, `const fn is_ok`, `const fn map` | 40 | Result at compile time |
| K7.6 | Const hash functions | `const fn hash_str`, `const fn hash_bytes` | 60 | Hashing at compile time |
| K7.7 | Const formatting | `const fn format_int`, `const fn format_float` | 60 | Number → string at compile time |
| K7.8 | Const bit manipulation | `const fn count_ones`, `const fn leading_zeros` | 40 | Bit ops at compile time |
| K7.9 | Const conversion | `const fn i32_to_i64`, `const fn f64_to_f32` | 30 | Type conversion at compile time |
| K7.10 | 10 const stdlib tests | Math, string, array, option, hash, format, bits | 150 | All 10 pass |

### Sprint K8: Const Generics in Type System (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K8.1 | `[T; N]` fixed array type | First-class fixed-size array with const generic | 80 | `[i32; 3]` distinct from `[i32; 4]` |
| K8.2 | Matrix type | `struct Matrix<T, const R: usize, const C: usize>` | 60 | `Matrix<f64, 3, 3>` is a type |
| K8.3 | Const generic functions | `fn dot<const N: usize>(a: [f64; N], b: [f64; N]) -> f64` | 60 | Dimension-checked dot product |
| K8.4 | Const generic methods | `impl<const N: usize> [T; N] { fn len() -> usize { N } }` | 60 | Methods on const generic types |
| K8.5 | Const generic trait impl | `impl<const N: usize> Display for [T; N]` | 60 | Display for all array sizes |
| K8.6 | Const arithmetic in types | `fn concat<const A: usize, const B: usize>(x: [T; A], y: [T; B]) -> [T; A + B]` | 80 | Return type computed from consts |
| K8.7 | Dependent type verification | Verify `A + B` doesn't overflow usize | 40 | Overflow → compile error |
| K8.8 | Const generic enum | `enum SmallVec<T, const N: usize> { Inline([T; N]), Heap(Vec<T>) }` | 60 | Enum with inline buffer |
| K8.9 | Default const values | `struct Buffer<T, const N: usize = 1024>` | 40 | Default const parameter |
| K8.10 | 10 type system tests | Array, matrix, functions, methods, arithmetic | 150 | All 10 pass |

### Sprint K9: Pipeline Integration (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K9.1 | Analyzer integration | Const evaluation runs during semantic analysis | 60 | Const errors reported at check time |
| K9.2 | Cranelift integration | Const values emitted as immediates in IR | 40 | `const X = 42` → `iconst.i64 42` |
| K9.3 | LLVM integration | Const values as LLVM constants | 40 | `const X` → LLVM `i64 42` |
| K9.4 | VM integration | Const values precomputed in bytecode | 40 | VM loads const from constant pool |
| K9.5 | LSP integration | Const values shown in hover | 30 | Hover on `const X` shows value |
| K9.6 | LSP completion | Suggest const fns in const context | 30 | Completion filters non-const fns |
| K9.7 | Error messages | "cannot call non-const fn `read_file` in const context" | 40 | Clear error for const violations |
| K9.8 | REPL support | `comptime { ... }` evaluates in REPL | 20 | REPL shows const results |
| K9.9 | Documentation | `book/const_evaluation.md` | 200 | Const generics + const fn guide |
| K9.10 | 10 integration tests | Analyzer, Cranelift, LLVM, VM, LSP, REPL | 150 | All 10 pass |

### Sprint K10: Benchmarks & Validation (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| K10.1 | Const eval time benchmark | Measure compile-time cost of const evaluation | 30 | < 1% compilation overhead |
| K10.2 | Const vs runtime comparison | Compare const-evaluated vs runtime-computed results | 30 | Identical results |
| K10.3 | Large const data | 1MB lookup table generated at compile time | 40 | Table correct, in .rodata |
| K10.4 | Recursive const depth | Test 256-level recursive const fn | 20 | Evaluates without stack overflow |
| K10.5 | Const generics coverage | All 15 numeric types as const parameters | 30 | `u8` through `u128` work |
| K10.6 | Example: const physics | `examples/const_physics.fj` with compile-time unit checking | 100 | Units checked at compile time |
| K10.7 | Example: const LUT | `examples/const_lut.fj` with lookup table generation | 80 | LUT embedded in binary |
| K10.8 | Example: const matrix | `examples/const_matrix.fj` with dimension-checked ops | 80 | Matrix operations type-safe |
| K10.9 | Update CLAUDE.md | Document const features in language reference | 30 | CLAUDE.md reflects const features |
| K10.10 | Update GAP_ANALYSIS_V2 | Mark const fn as 100% production | 20 | Audit reflects real status |

**Option H Total: 100 tasks, ~6,000 LOC, 100 tests, full const generics + compile-time eval**

---

## Summary

| Option | Name | Sprints | Tasks | Est. LOC | Est. Tests | Status |
|--------|------|---------|-------|----------|------------|--------|
| A | CI Green | 1 | 10 | ~79 | — | **A1.1-A1.4 DONE** |
| B | WASI P2 + Component Model | 10 | 100 | ~8,500 | 100 | Not started |
| C | Incremental Compilation | 10 | 100 | ~7,200 | 100 | Not started |
| D | Distributed Runtime | 10 | 100 | ~8,500 | 100 | Not started |
| E | FFI v2 Full Integration | 10 | 100 | ~8,000 | 100 | Not started |
| F | SMT Formal Verification | 10 | 100 | ~7,500 | 100 | Not started |
| G | Self-Hosting Compiler | 10 | 100 | ~7,000 | 100 | Not started |
| H | Const Fn + Compile-Time Eval | 10 | 100 | ~6,000 | 100 | Not started |
| **TOTAL** | | **71** | **710** | **~52,779** | **700** | |

### Recommended Execution Order

**Phase 1 — Foundation (Options A, C, H):**
CI stability + incremental compilation + const fn — these improve every developer's daily experience and have no external dependencies.

**Phase 2 — Ecosystem (Options B, E):**
WASI P2 + FFI v2 — these expand deployment targets and language interop, attracting new users.

**Phase 3 — Differentiation (Options D, F, G):**
Distributed + verification + self-hosting — these are the features that make Fajar Lang unique among systems languages.

### Prerequisites & Dependencies

```
A ──────────────────────────────────────────────────── (none, do first)
C ──────────────────────────────────────────────────── depends on A (CI must be green)
H ──────────────────────────────────────────────────── depends on A
B ── W8 (resources) benefits from H (const generics)
E ── E4 (async Python) benefits from C (incremental for fast iteration)
F ── V5 (proof caching) benefits from C (incremental infrastructure)
D ── D5 (distributed ML) benefits from E (Python/NumPy bridge)
G ── S4 (bytecode codegen) benefits from H (const eval in compiler)
```

---

*V13 "Beyond" Plan — Version 1.0 | 710 tasks, 71 sprints, ~52,779 LOC | Created 2026-03-31*
*Author: Fajar (PrimeCore.id) + Claude Opus 4.6*
