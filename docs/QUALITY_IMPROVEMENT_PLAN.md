# Quality Improvement Plan — Every Module to "Best in World"

> **Date:** 2026-03-26
> **Principle:** Setiap modul harus yang terbaik. Tidak ada "good enough".
> **Method:** Satu modul pada satu waktu. Verifikasi sebelum pindah ke modul berikutnya.
> **Rule:** Setiap task punya verification method. Tidak ada batch-marking.

---

## Assessment Scale

| Level | Meaning | Criteria |
|-------|---------|----------|
| ⭐⭐⭐⭐⭐ | Best in World | Rivals Rust/Go stdlib quality. Complete API, edge cases handled, comprehensive tests, documented |
| ⭐⭐⭐⭐ | Production Ready | Real users can depend on it. No crashes, good errors, tested edge cases |
| ⭐⭐⭐ | Functional | Works for common cases. Missing edge cases, limited API |
| ⭐⭐ | Basic | First implementation. Happy path only |
| ⭐ | Stub/Framework | Type definitions, no real behavior |

---

## Module-by-Module Assessment & Improvement Plan

### Module 1: Crypto (src/stdlib_v3/crypto.rs)

**Current: ⭐⭐⭐⭐ Production Ready**
- ✅ SHA-256/384/512 with NIST test vectors
- ✅ HMAC-SHA256 with RFC 4231 vectors
- ✅ AES-256-GCM encrypt/decrypt with tamper detection
- ✅ Ed25519 keygen/sign/verify
- ✅ Argon2id password hashing (PHC format)
- ✅ CSPRNG via OsRng
- ✅ Base64/Hex encode/decode
- ✅ constant_time_eq, secure_zero

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| CQ1.1 | SHA-256 streaming | Support incremental hashing (update + finalize) | Hash 1GB file in chunks == hash whole file |
| CQ1.2 | AES-128-GCM | Add AES-128 alongside AES-256 | Encrypt/decrypt roundtrip with 16-byte key |
| CQ1.3 | AES-CBC mode | Add CBC mode with PKCS7 padding | Roundtrip + known vector test |
| CQ1.4 | RSA signing | Add RSA-2048 sign/verify via rsa crate | Sign + verify + wrong-key-fails |
| CQ1.5 | X25519 key exchange | Add ECDH via x25519-dalek | Shared secret matches on both sides |
| CQ1.6 | PBKDF2 | Add PBKDF2-HMAC-SHA256 | RFC 6070 test vectors |
| CQ1.7 | HKDF | Add HKDF key derivation | RFC 5869 test vectors |
| CQ1.8 | Secure random range | random_u64(min, max) with uniform distribution | Chi-squared test on 100K samples |
| CQ1.9 | Timing attack test | Verify constant_time_eq is actually constant time | Measure 10K comparisons, variance < 1% |
| CQ1.10 | Crypto documentation | Every function with example + security notes | All examples compile and run |

---

### Module 2: Networking (src/stdlib_v3/net.rs)

**Current: ⭐⭐⭐ Functional**
- ✅ TCP connect/server/read/write
- ✅ UDP send/recv
- ✅ HTTP/1.1 GET/POST (raw socket)
- ✅ DNS resolver
- ✅ URL parser, rate limiter, circuit breaker
- ❌ No TLS/HTTPS
- ❌ No connection pooling
- ❌ No chunked transfer encoding
- ❌ No timeout on individual reads
- ❌ No keep-alive support

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| NQ2.1 | TLS support | Add rustls for HTTPS connections | GET https://httpbin.org/get returns 200 |
| NQ2.2 | Connection timeout | Per-connection timeout (not just read) | Connect to non-routable IP times out |
| NQ2.3 | Read timeout | Per-read timeout separate from connect | Slow server triggers read timeout |
| NQ2.4 | HTTP response parsing | Handle chunked transfer-encoding | Parse chunked response from httpbin |
| NQ2.5 | HTTP keep-alive | Reuse connection for multiple requests | 2 requests on same TcpStream |
| NQ2.6 | HTTP headers case | Case-insensitive header lookup | "Content-Type" == "content-type" |
| NQ2.7 | HTTP redirect | Follow 301/302 redirects (max 5) | httpbin.org/redirect/3 resolves |
| NQ2.8 | Connection pool | Pool with max connections and idle timeout | 10 concurrent requests share 3 connections |
| NQ2.9 | TCP nodelay | Set TCP_NODELAY option | Verify via getsockopt |
| NQ2.10 | IPv6 support | TCP/UDP work with IPv6 addresses | Connect to [::1]:port |
| NQ2.11 | Error types | Specific error variants (ConnTimeout, ReadTimeout, DnsError) | Each error type distinguishable |
| NQ2.12 | Network documentation | Every function with example | All examples compile and run |

---

### Module 3: C++ FFI (src/ffi_v2/cpp.rs)

**Current: ⭐⭐⭐ Functional**
- ✅ libclang header parsing
- ✅ Function/class/enum/namespace extraction
- ✅ CXType → CppType mapping
- ❌ No template instantiation detection
- ❌ No C++ exception → Result mapping
- ❌ No std::string ↔ str bridge
- ❌ No actual function call generation

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| CQ3.1 | Template detection | Detect template<T> and extract parameters | Parse vector<int> header |
| CQ3.2 | Inheritance extraction | Extract base classes and virtual methods | Parse class Derived : Base |
| CQ3.3 | Method const/static | Track const and static method qualifiers | Parse const method, verify flag |
| CQ3.4 | Include resolution | Follow #include directives | Parse header that includes another |
| CQ3.5 | Macro expansion | Resolve simple #define constants | Parse header with #define MAX 100 |
| CQ3.6 | Error handling | Return structured errors for unparseable headers | Malformed header returns ParseError |
| CQ3.7 | Binding code generation | Generate .fj extern blocks from parsed C++ | Generated code compiles |
| CQ3.8 | std::string bridge | Map std::string to Fajar str | Round-trip string conversion |
| CQ3.9 | Parse real-world header | Parse a real OpenCV/Eigen header | Extract 10+ functions from opencv2/core.hpp |
| CQ3.10 | FFI documentation | Usage guide with real-world example | Guide works end-to-end |

---

### Module 4: Python FFI (src/ffi_v2/python.rs)

**Current: ⭐⭐⭐ Functional**
- ✅ pyo3 interpreter init
- ✅ Eval expressions, call builtins
- ✅ Import modules (math, numpy)
- ✅ Define + call Python functions
- ✅ Exception detection
- ❌ No ndarray ↔ numpy zero-copy
- ❌ No proper GIL management for threads
- ❌ No Fajar→Python callback
- ❌ No Python exception → Fajar Result mapping

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| PQ4.1 | NumPy zero-copy | Convert ndarray::Array2 ↔ numpy.ndarray without copy | 1M float array, verify same memory |
| PQ4.2 | Exception → Result | Map Python exceptions to Fajar Result::Err with traceback | ZeroDivisionError → Err("ZeroDivisionError: ...") |
| PQ4.3 | Type conversion | Auto-convert Fajar types ↔ Python types (int, float, str, list, dict) | Round-trip 5 types |
| PQ4.4 | Module introspection | List functions/classes in a Python module | List all functions in math module |
| PQ4.5 | Call with kwargs | Support keyword arguments | math.log(100, base=10) |
| PQ4.6 | Python → Fajar callback | Register Fajar function callable from Python | Python calls Fajar fn, gets result |
| PQ4.7 | Virtual env support | Detect and use virtualenv | Import from venv package |
| PQ4.8 | Async bridge | Call async Python functions from Fajar | await asyncio coroutine |
| PQ4.9 | Error messages | Clear error when Python not installed | "Python 3.x required" with install hint |
| PQ4.10 | Python FFI documentation | Complete guide with PyTorch example | Guide works end-to-end |

---

### Module 5: Distributed (src/distributed/transport.rs)

**Current: ⭐⭐⭐ Functional**
- ✅ TCP transport with framed messages
- ✅ Actor mailbox via mpsc
- ✅ Message serialization (9 types)
- ✅ 2-node message passing test
- ❌ No reconnection on disconnect
- ❌ No service discovery
- ❌ No cluster consensus
- ❌ No backpressure

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| DQ5.1 | Reconnection | Auto-reconnect on TCP disconnect | Kill node, restart, messages resume |
| DQ5.2 | Heartbeat timeout | Detect dead nodes via heartbeat timeout | Stop heartbeat → node marked dead in 5s |
| DQ5.3 | Backpressure | Slow consumer doesn't OOM sender | Send 1M messages to slow consumer, no OOM |
| DQ5.4 | Message ordering | Guarantee FIFO per-sender ordering | 1000 messages arrive in order |
| DQ5.5 | Connection pool | Reuse connections to same node | 100 messages use 1 connection |
| DQ5.6 | TLS transport | Encrypted node-to-node communication | Wireshark shows encrypted traffic |
| DQ5.7 | Service discovery | UDP multicast for node discovery | Start 3 nodes, all discover each other |
| DQ5.8 | Graceful shutdown | Drain messages before disconnect | All in-flight messages delivered |
| DQ5.9 | Metrics | Message count, latency, error rate | Prometheus-compatible metrics |
| DQ5.10 | Distributed documentation | Cluster setup guide + architecture | 3-node cluster tutorial works |

---

### Module 6: Z3 SMT (src/verify/smt.rs)

**Current: ⭐⭐⭐ Functional**
- ✅ Z3 context/solver creation
- ✅ Integer constraint proofs
- ✅ Array bounds verification
- ✅ Matmul shape compatibility
- ✅ Counterexample extraction
- ❌ No bitvector theory (overflow detection)
- ❌ No integration with analyzer pipeline
- ❌ No `fj verify` CLI command

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| VQ6.1 | Bitvector theory | Prove integer overflow safety using BV32/BV64 | Prove i32 add doesn't overflow for bounded inputs |
| VQ6.2 | Real arithmetic | Prove floating point bounds | Prove 0.0 <= sigmoid(x) <= 1.0 |
| VQ6.3 | Analyzer integration | Auto-generate VCs from @verify annotated functions | `@verify fn abs(x: i64) -> i64` generates VC |
| VQ6.4 | `fj verify` CLI | Run verification from command line | `fj verify program.fj` shows results |
| VQ6.5 | Timeout handling | Configurable solver timeout | 1s timeout, returns Unknown for complex VCs |
| VQ6.6 | Incremental solving | Push/pop for multiple VCs in one session | Verify 10 functions, reuse solver |
| VQ6.7 | Counterexample display | Pretty-print counterexample values | "x = -5 violates precondition x >= 0" |
| VQ6.8 | Proof caching | Cache proven VCs, skip on re-verify | Second run skips unchanged functions |
| VQ6.9 | Error localization | Point to exact line that fails | "line 42: array index may be out of bounds" |
| VQ6.10 | Verification documentation | Guide with examples for each proof type | All examples work |

---

### Module 7: JSON/TOML/CSV (src/stdlib_v3/formats.rs)

**Current: ⭐⭐⭐ Functional**
- ✅ JSON recursive descent parser
- ✅ JSON stringify (compact + pretty)
- ✅ TOML via toml crate
- ✅ CSV RFC 4180 (quoted fields)
- ❌ No JSON unicode escapes (\uXXXX)
- ❌ No JSON streaming parser
- ❌ No JSON schema validation
- ❌ No proper number parsing (scientific notation in JSON)

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| FQ7.1 | JSON unicode escapes | Parse \uXXXX and \uD800\uDC00 surrogate pairs | Parse "\u0041" == "A" |
| FQ7.2 | JSON number edge cases | Parse 1e+10, -0, very large numbers | JSONTestSuite compliance |
| FQ7.3 | JSON error messages | Line:col position in parse errors | "line 3, col 15: expected ':'" |
| FQ7.4 | JSON streaming | Parse JSON from iterator/reader | Parse 100MB JSON without loading all in memory |
| FQ7.5 | TOML datetime | Parse TOML datetime values | Parse 2026-03-26T12:00:00Z |
| FQ7.6 | TOML inline tables | Parse inline tables { key = "value" } | Parse and access inline table fields |
| FQ7.7 | CSV escape roundtrip | Write CSV with quotes → read back identical | Field with comma + newline survives roundtrip |
| FQ7.8 | CSV header parsing | First row as header, access by column name | data["name"] == "Fajar" |
| FQ7.9 | Format detection | Auto-detect JSON/TOML/CSV from content | Detect by first non-whitespace character |
| FQ7.10 | Formats documentation | Every parser with example + error handling | All examples compile and run |

---

### Module 8: System Utils (src/stdlib_v3/system.rs)

**Current: ⭐⭐⭐ Functional**
- ✅ Process spawn with stdout/stderr capture
- ✅ Spawn with timeout + kill
- ✅ Environment variables get/set
- ✅ Path operations (join, parent, extension)
- ✅ Recursive directory walking
- ✅ Temp directory
- ❌ No signal handling
- ❌ No pipe stdin to child process
- ❌ No file watching (notify)
- ❌ No process exit code constants

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| SQ8.1 | Pipe stdin | Write to child process stdin | `echo "input" \| program` equivalent |
| SQ8.2 | Stream stdout | Read child stdout line-by-line | Long-running process output streamed |
| SQ8.3 | Exit codes | Constants for common exit codes | EXIT_SUCCESS == 0, EXIT_FAILURE == 1 |
| SQ8.4 | File permissions | Get/set file permissions (Unix) | chmod 755, verify with stat |
| SQ8.5 | Symlink support | Create/read/detect symlinks | Create link, readlink matches target |
| SQ8.6 | File metadata | Size, modified time, is_dir, is_file | stat("/tmp") → is_dir == true |
| SQ8.7 | File watching | Detect file changes via notify crate | Modify file → callback triggered |
| SQ8.8 | Home directory | Cross-platform home dir detection | Returns valid path on Linux/macOS/Windows |
| SQ8.9 | Which/find executable | Find executable in PATH | which("cargo") returns path |
| SQ8.10 | System documentation | Every function with cross-platform notes | All examples work on Linux |

---

### Module 9: Plugin System (src/plugin/mod.rs)

**Current: ⭐⭐ Basic**
- ✅ CompilerPlugin trait
- ✅ PluginRegistry with enable/disable
- ✅ 2 built-in lints (unused var, TODO comments)
- ❌ No dynamic loading (.so/.dylib)
- ❌ No API versioning
- ❌ No plugin configuration
- ❌ No plugin discovery from fj.toml

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| PQ9.1 | Dynamic loading | Load plugin from .so/.dylib via libloading | Compile plugin as cdylib, load at runtime |
| PQ9.2 | Plugin API versioning | Reject plugins built for different API version | Old plugin → "incompatible API v1, need v2" |
| PQ9.3 | Plugin configuration | Load plugin config from fj.toml [plugins.name] | `[plugins.unused-vars]\nignore_prefix = "_"` |
| PQ9.4 | Plugin discovery | Auto-discover plugins in ~/.fj/plugins/ | Place .so in dir, auto-loaded on startup |
| PQ9.5 | AST read API | Plugin can read full AST (not just source text) | Plugin iterates all function definitions |
| PQ9.6 | Diagnostic API | Plugin emits warnings/errors with source spans | Warning appears in IDE with correct line |
| PQ9.7 | Auto-fix API | Plugin can suggest code fixes | "unused import" → auto-remove line |
| PQ9.8 | Performance budget | Plugin limited to 100ms per file | Slow plugin gets warning, can be killed |
| PQ9.9 | 5 built-in plugins | naming-convention, complexity, security, deprecated, doc-coverage | Each detects real issues in test code |
| PQ9.10 | Plugin documentation | How to write a plugin (template + guide) | Template compiles and loads |

---

### Module 10: Profiler (src/profiler/instrument.rs)

**Current: ⭐⭐ Basic**
- ✅ CallRecord with timing fields
- ✅ CallGraph analysis
- ✅ Chrome Trace format export
- ✅ std::time::Instant profiling
- ❌ No sampling profiler
- ❌ No SVG flamegraph generation
- ❌ No memory profiling
- ❌ No integration with interpreter

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| PQ10.1 | Interpreter hooks | Profile real .fj program execution | `fj run --profile program.fj` outputs timing |
| PQ10.2 | Flamegraph SVG | Generate SVG flamegraph from profile data | Open SVG in browser, see call stacks |
| PQ10.3 | Hotspot detection | Identify top 5 hottest functions | Report "fn fib: 95% of runtime" |
| PQ10.4 | Memory tracking | Track allocations per function | Report "fn process: allocated 15MB" |
| PQ10.5 | Profile comparison | Compare two profiles (before/after optimization) | "fn sort: 30% faster" |
| PQ10.6 | Sampling profiler | Sample call stack at fixed intervals | 1000 samples/sec for 10 seconds |
| PQ10.7 | Line-level profiling | Time per source line | "line 42: 500ms (hot)" |
| PQ10.8 | Profile export | Export to Chrome Trace, speedscope, pprof | All 3 formats open in their viewers |
| PQ10.9 | Profile CLI | `fj profile program.fj --output flame.svg` | Command works end-to-end |
| PQ10.10 | Profiler documentation | Guide with optimization workflow | Profile → identify → optimize → verify |

---

### Module 11: Self-Hosted Compiler (stdlib/*.fj)

**Current: ⭐⭐⭐ Functional**
- ✅ Lexer: 469 lines, 50/50 tests, all token kinds
- ✅ Parser: 675 lines, 30/30 tests, all constructs
- ✅ Analyzer: 367 lines, 20/20 tests, scope + types
- ✅ Compiler pipeline: 243 lines, 15 bootstrap programs
- ❌ No real AST (parser returns position only)
- ❌ No real type inference (just type tags)
- ❌ Stage 1→2 bootstrap not achieved
- ❌ Stack overflow on large programs

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| SQ11.1 | Real AST nodes | Parser builds array-based AST tree | Parse "let x = 42" → ["let", "x", ["int", "42"]] |
| SQ11.2 | AST pretty print | Convert AST back to source code | Print AST → reparse → same AST |
| SQ11.3 | Real type inference | Infer types from expressions (not just tags) | Infer "x: i64" from "let x = 42" |
| SQ11.4 | Scope nesting | Push/pop scopes for blocks, functions | Inner var shadows outer, restored after block |
| SQ11.5 | Error recovery | Continue parsing after error | 3 errors in one file, all reported |
| SQ11.6 | Stage 2 bootstrap | Self-hosted compiler compiles itself | fj₁ output == fj₂ output |
| SQ11.7 | Stack depth fix | Handle 1000+ statement programs | 1000-line program compiles without overflow |
| SQ11.8 | Token spans | Track start/end position per token | Error messages show line:col |
| SQ11.9 | Performance | Within 3x of Rust implementation | Benchmark on stdlib/lexer.fj |
| SQ11.10 | Self-host documentation | Architecture + how to contribute | New developer can understand in 1 hour |

---

### Module 12: Application Templates (examples/template_*.fj)

**Current: ⭐⭐⭐ Functional**
- ✅ Web service: router, auth, CRUD, JSON, logging
- ✅ IoT edge: sensors, anomaly detection, telemetry
- ✅ ML pipeline: training, early stopping, metrics
- ❌ Web: doesn't actually listen on port
- ❌ IoT: doesn't read real hardware
- ❌ ML: only synthetic data

**To reach ⭐⭐⭐⭐⭐:**

| # | Task | Detail | Verification |
|---|------|--------|-------------|
| TQ12.1 | Web: real HTTP server | Listen on port, handle real HTTP requests | curl http://localhost:8080/health → 200 |
| TQ12.2 | Web: real database | SQLite via FFI for persistence | Restart server, data persists |
| TQ12.3 | Web: benchmark | Measure requests/second | >1000 req/sec for simple endpoint |
| TQ12.4 | IoT: real GPIO | Read GPIO on Q6A hardware | LED blinks, temperature sensor reads |
| TQ12.5 | IoT: real MQTT | Publish to real MQTT broker | mosquitto_sub receives messages |
| TQ12.6 | IoT: 24h stability | Run for 24 hours without crash/leak | Memory usage stable over time |
| TQ12.7 | ML: real dataset | Train on Iris or MNIST CSV | Accuracy > 90% on test set |
| TQ12.8 | ML: model export | Export trained model, load in new program | Exported model gives same predictions |
| TQ12.9 | ML: visualization | Generate loss curve as text/CSV | Plot loss curve, confirm convergence |
| TQ12.10 | Template documentation | Step-by-step tutorial for each template | New user follows guide, app runs |

---

## Summary

| Module | Current | Target | Tasks | Hours |
|--------|---------|--------|-------|-------|
| Crypto | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~10 |
| Networking | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 12 | ~15 |
| C++ FFI | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~10 |
| Python FFI | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~10 |
| Distributed | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~15 |
| Z3 SMT | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~10 |
| Formats | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~10 |
| System Utils | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~10 |
| Plugin System | ⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~15 |
| Profiler | ⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~15 |
| Self-Hosted | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~15 |
| Templates | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 10 | ~10 |

**Total: 122 tasks, ~145 hours**

### Priority Order (impact on users):

```
1. Networking (users need HTTP/TLS first)
2. Formats (JSON/CSV are essential for any app)
3. Templates (prove the language works for real projects)
4. Self-Hosted (prove the language can compile itself)
5. Crypto (security-critical, must be perfect)
6. System Utils (developers need file/process tools)
7. Python FFI (ML users need numpy/torch bridge)
8. C++ FFI (systems users need C++ interop)
9. Plugin System (ecosystem growth)
10. Profiler (optimization workflow)
11. Distributed (advanced use case)
12. Z3 SMT (niche but important for safety-critical)
```

### Execution Rule

- Work on ONE module at a time
- Complete ALL tasks in a module before moving to the next
- Each task individually verified before marking done
- No shortcuts. No batch-marking. Quality > speed.

---

*"Kita akan membuat sejarah di dunia IT" — every module must earn ⭐⭐⭐⭐⭐*
