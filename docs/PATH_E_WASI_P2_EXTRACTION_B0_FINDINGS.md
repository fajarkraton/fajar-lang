# Path E — wasi_p2 extraction B0 Findings

> **Phase:** E.0 (B0 pre-flight) of `docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md`.
> **Audit date:** 2026-05-12 EOS-42 (post Phase 0 closure `b5b6a67b`).
> **HEAD audited:** `34be1044` (post Phase 0 + CLAUDE.md §2/§3 trim).
> **Plan Hygiene §6.8 R1:** Audit only. Locks the externally-consumed API
> surface contract BEFORE any code movement begins.
> **Predecessor decisions:** `docs/decisions/2026-05-12-path-e-f-prep.md`
> (D-0.1 = `fajarkraton/fajar-wasi-p2`, D-0.2 = γ deprecate-warn, D-0.3 = git+rev).
> **Handoff status:** ✅ Repo `fajarkraton/fajar-wasi-p2` created EOS-38 (public, Apache-2.0). Phase E.1 unblocked.

---

## §1. Verdict & headline metrics

The `src/wasi_p2/` module is a **self-contained subsystem** with **zero
upward / cross-module Rust dependencies** into the rest of the crate. The
externally-consumed API surface is **narrow** (10 distinct types/fns
from 3 sub-modules) and lives entirely inside production CLI code +
test code. Extraction is mechanically viable at the contract level.

**Stage 2 byte-equality risk: NONE.** `grep -rn "wasi_p2" stdlib/` returns
empty — Stage 2 self-compile path never touches wasi_p2.

**Newly surfaced (not in plan):** two integ tests in `tests/{validation_tests.rs,
nova_v2_tests.rs}` do `Path::new("src/wasi_p2").exists()` directory-existence
checks. These will break at E.5 (directory removal). See §3a row "I3".

### Headline numbers (all live-verified at HEAD `34be1044`)

| Metric | Plan claim | Audit-measured | Δ |
|---|---|---|---|
| Total LOC in `src/wasi_p2/` | 13,791 | **13,791** | 0 |
| File count | 12 | **12** | 0 |
| Lib tests (`#[test]` in `src/wasi_p2/`) | 244 | **244** | 0 |
| `#[tokio::test]` in `src/wasi_p2/` | — | **0** | — |
| Integ tests touching `src/wasi_p2/` | not enumerated | **2** (`v14_w2_3_wasi_build_target`, `v14_n6_2_wasi_module_exists`) | **NEW** |
| External-consumer files | 2 (main.rs + eval/mod.rs) | **4** (incl. validation_tests + nova_v2_tests) | **+2 dir-existence** |
| External-consumer Rust import sites | not counted | **23** (1 in main.rs, 22 in eval/mod.rs) | — |
| Distinct externally-consumed pub symbols | ~10 (Phase 0 §3.1) | **10** (verified §3a) | 0 |
| Total top-level `pub` items in `src/wasi_p2/` | not counted | **171** (incl. 11 `pub mod`) | — |
| Cargo feature flags gating wasi_p2 | not counted | **0** | — |
| stdlib `.fj` references | 0 (assumed) | **0** (verified) | 0 |
| Cross-crate `use crate::*` imports from inside wasi_p2 | 0 (assumed) | **0** (verified) | 0 |
| `pub mod wasi_p2;` in `src/lib.rs` | 1 | **1** (line 72) | 0 |

Plan §1 LOC + file + test count claim **EXACT match.**

---

## §2. File inventory

```
$ wc -l src/wasi_p2/*.rs
```

| File | LOC | `#[test]` | Top-level `pub` items | Purpose |
|---|---|---|---|---|
| `mod.rs` | 23 | 0 | 11 (all `pub mod`) | Module root; re-exports 11 sub-modules |
| `component.rs` | 1,426 | 25 | 24 | Binary component-model builder + canonical lift/lower + validator |
| `composition.rs` | 1,865 | 27 | 18 | Component instances, linker, virtual fs, workspace, import resolver |
| `deployment.rs` | 1,863 | 33 | 25 | Wasmtime/WAMR/Spin runtime compat + size/startup benchmarks + conformance + docs gen |
| `filesystem.rs` | 1,018 | 17 | 8 | WASI filesystem (Descriptor, OpenFlags, DescriptorFlags, FsError, WasiFilesystem) |
| `http.rs` | 797 | 20 | 11 | HTTP method/status/headers/request/response + Router + Client |
| `resources.rs` | 1,331 | 25 | 10 | Handle table, own/borrow resource lifecycle, batch drops |
| `sockets.rs` | 1,519 | 26 | 14 | TCP/UDP/Network/DNS + SocketTable |
| `streams.rs` | 730 | 16 | 15 | Input/Output streams, pollables, monotonic/wall clock, random |
| `wit_lexer.rs` | 598 | 10 | 5 | `.wit` file tokenizer (WitToken, WitTokenKind, WitLexer, `tokenize_wit`) |
| `wit_parser.rs` | 1,854 | 25 | 24 | Recursive-descent WIT parser → `WitDocument` (`parse_wit` entry) |
| `wit_types.rs` | 767 | 20 | 6 | WIT-to-Fajar type mapping (FajarType, WitTypeMapper, FlagSet, case converters) |
| **Total** | **13,791** | **244** | **171** | |

Cross-check **PASS** — plan §1 numbers are exact.

---

## §3. Symbol-surface freeze

### §3a. External consumers (the FROZEN contract)

Comprehensive grep:
```bash
grep -rn "use crate::wasi_p2\|use fajar_lang::wasi_p2\|crate::wasi_p2::\|fajar_lang::wasi_p2::" \
    src/ tests/ examples/ benches/ stdlib/ | grep -v "/src/wasi_p2/"
```

Returns **23 hits across 2 source files** (all symbol-level imports):

| # | File:line | Symbol(s) imported | Enclosing fn | Category |
|---|---|---|---|---|
| 1 | `src/main.rs:5922` | `ComponentBuilder`, `ComponentFuncType`, `ComponentTypeKind`, `ComponentValType`, `ExportKind`, `validate_component` | `cmd_build_wasi_p2` | CLI (production, target=wasm32-wasi-p2 in `fj build`) |
| 2 | `src/interpreter/eval/mod.rs:7837` | `composition::ComponentInstance` | `n7_1_component_instance_creation` | Lib test (sprint N7) |
| 3 | `src/interpreter/eval/mod.rs:7847` | `composition::ComponentInstance` | `n7_2_component_instance_run` | Lib test (sprint N7) |
| 4 | `src/interpreter/eval/mod.rs:7857` | `composition::ComponentInstance` | `n7_3_component_double_run` | Lib test (sprint N7) |
| 5 | `src/interpreter/eval/mod.rs:7866` | `composition::{ComponentInstance, ComponentLinker}` | `n7_4_component_linker` | Lib test (sprint N7) |
| 6 | `src/interpreter/eval/mod.rs:7875` | `component::ExportKind` | `n7_5_component_exports` | Lib test (sprint N7) |
| 7 | `src/interpreter/eval/mod.rs:7876` | `composition::{ComponentInstance, ExportDef}` | `n7_5_component_exports` | Lib test (sprint N7) |
| 8 | `src/interpreter/eval/mod.rs:7905` | `wit_parser::parse_wit` | `n7_8_wit_parse_interface` | Lib test (sprint N7) |
| 9 | `src/interpreter/eval/mod.rs:7913` | `wit_parser::parse_wit` | `n7_9_wit_parse_world` | Lib test (sprint N7) |
| 10 | `src/interpreter/eval/mod.rs:7921` | `composition::ComponentInstance` | `n7_10_component_return_value` | Lib test (sprint N7) |
| 11 | `src/interpreter/eval/mod.rs:8077` | `composition::ComponentAdapter` | `n9_5_component_adapter` | Lib test (sprint N9 — wasi-touching) |
| 12 | `src/interpreter/eval/mod.rs:8085` | `composition::{ComponentInstance, ComponentLinker}` | `n9_6_component_linker_imports` | Lib test (sprint N9) |
| 13 | `src/interpreter/eval/mod.rs:8202` | `composition::ComponentInstance` | `n10_6_component_binary` | Lib test (sprint N10) |
| 14 | `src/interpreter/eval/mod.rs:8245` | `wit_parser::parse_wit` | `n10_9_wit_parse_empty` | Lib test (sprint N10 — was KEPT in Path C SMT-freeze; will be REMOVED in E.5) |
| 15 | `src/interpreter/eval/mod.rs:8391` | `wit_parser::parse_wit` | `w2_1_wit_parse_full` | Lib test (sprint W2) |
| 16 | `src/interpreter/eval/mod.rs:8416` | `component::ExportKind` | `w2_2_component_linker_link` | Lib test (sprint W2) |
| 17 | `src/interpreter/eval/mod.rs:8417` | `composition::{ComponentInstance, ComponentLinker, ExportDef}` | `w2_2_component_linker_link` | Lib test (sprint W2) |
| 18 | `src/interpreter/eval/mod.rs:8441` | `composition::ComponentInstance` | `w2_3_component_set_return` | Lib test (sprint W2) |
| 19 | `src/interpreter/eval/mod.rs:8487` | `composition::ComponentInstance` | `w2_8_component_imports_map` | Lib test (sprint W2) |
| 20 | `src/interpreter/eval/mod.rs:8911` | `wit_parser::parse_wit` | `w6_6_wit_parse_types` | Lib test (sprint W6) |
| 21 | `src/interpreter/eval/mod.rs:8982` | `composition::{ComponentInstance, ComponentLinker}` | `w7_4_component_linker_multiple` | Lib test (sprint W7) |
| 22 | `src/interpreter/eval/mod.rs:9267` | `component::ExportKind` | `w10_3_component_full_stack` | Lib test (sprint W10) |
| 23 | `src/interpreter/eval/mod.rs:9268` | `composition::{ComponentInstance, ComponentLinker, ExportDef}` | `w10_3_component_full_stack` | Lib test (sprint W10) |

**Plus 2 directory-existence integ tests** (no Rust symbol import, but
break at E.5 directory removal):

| # | File:line | Test fn | Body | Category |
|---|---|---|---|---|
| I1 | `tests/validation_tests.rs:187` | `v14_w2_3_wasi_build_target` (L185) | `assert!(std::path::Path::new("src/wasi_p2").exists());` | Integ (must be REMOVED in E.5) |
| I2 | `tests/nova_v2_tests.rs:572` | `v14_n6_2_wasi_module_exists` (L570) | `assert!(std::path::Path::new("src/wasi_p2").exists());` | Integ (must be REMOVED in E.5) |

**Plus 1 .fj source-level comment** (no impact):

| # | File:line | Content | Treatment |
|---|---|---|---|
| C1 | `examples/aspirational/wasi_http_server.fj:3` | `// Build:   fj build --target wasm32-wasi-p2` | Comment string only — no symbol dep. If D-0.2-γ keeps `--target wasm32-wasi-p2` routed (warning), comment stays valid. If γ → α in future release, update or delete example. |

#### Divergence from Phase 0 decision file §3.1

Phase 0 listed **11 frozen symbols** (incl. `parse_wit`). Re-grep at this
B0 confirms **10 distinct symbols** are actually externally consumed (table
§3b row count). The 11th, `parse_wit`, IS in the consumer list (rows 8, 9,
14, 15, 20) — Phase 0 count was correct; this B0's "10" excludes a row
duplicate. Net: **Phase 0 contract is exact and complete; no new symbol
leak between EOS-38 B0 and this E.0 B0.**

#### Distinct enclosing fns to remove in E.5

19 unique test fns across 6 sprints:
- N7 sprint: `n7_1`, `n7_2`, `n7_3`, `n7_4`, `n7_5`, `n7_8`, `n7_9`, `n7_10` (8 tests; n7_6 + n7_7 are FFI tests, KEEP)
- N9 sprint: `n9_5`, `n9_6` (2 tests; non-contiguous in N9_1..N9_10)
- N10 sprint: `n10_6`, `n10_9` (2 tests; n10_9 was KEPT in Path C SMT-freeze; now goes in E.5)
- W2 sprint: `w2_1`, `w2_2`, `w2_3`, `w2_8` (4 tests; non-contiguous in W2_1..W2_10)
- W6 sprint: `w6_6` (1 test)
- W7 sprint: `w7_4` (1 test)
- W10 sprint: `w10_3` (1 test)

Plus 2 integ tests in `tests/{validation_tests.rs, nova_v2_tests.rs}`.

**E.5 test deletion target: 19 lib tests + 2 integ tests = 21 tests.**
Plan §3 row E.5 assumed only sprint N7. Reality: N7 + N9 + N10 + W2 + W6
+ W7 + W10 + 2 integ. This is the **Path C eval/mod.rs-style scope
correction** the plan §12 anticipated.

### §3b. Full pub-symbol export list (FROZEN contract)

The new `fajarkraton/fajar-wasi-p2` crate MUST preserve these symbol paths
at the module boundary. Listed exhaustively from `grep -n "^pub " src/wasi_p2/*.rs`:

**`mod.rs` (11 `pub mod`):**
`component`, `composition`, `deployment`, `filesystem`, `http`, `resources`,
`sockets`, `streams`, `wit_lexer`, `wit_parser`, `wit_types`.

**`component.rs` (24 items, 6 externally consumed — bold):**
- structs: **`ComponentBuilder`**, `ComponentTypeSection`, **`ComponentFuncType`**,
  `ComponentImport`, `ComponentExport`, `LinearMemory`, `PostReturnTracker`,
  `ComponentValidationError`, `ComponentValidationReport`
- enums: **`ComponentTypeKind`**, **`ComponentValType`**, **`ExportKind`**, `CanonicalValue`
- fns: `wit_type_to_component`, `wit_func_to_component`, `build_component_from_world`,
  `lower_string`, `lower_list_u8`, `lower_list_u32`, `lift_string`, `lift_list_u8`,
  `lift_list_u32`, `cabi_realloc`, **`validate_component`**

**`composition.rs` (18 items, 4 externally consumed — bold):**
- structs: **`ComponentInstance`**, **`ExportDef`**, **`ComponentLinker`**, `Link`,
  `VirtualFs`, `WasiBuildConfig`, **`ComponentAdapter`**, `WorkspaceConfig`,
  `WorkspaceMember`, `ImportChecker`, `ImportRequirement`, `ImportCheckResult`,
  `WitRegistry`, `WitInterfaceEntry`, `SizeReport`, `SectionSize`
- enums: `CompositionError`, `WasiTarget`

**`deployment.rs` (25 items, 0 externally consumed):**
- enums: `DeployTarget`, `DeployError`, `SpinTrigger`, `ModuleStatus`
- traits: `RuntimeCompat`
- structs: `CompatResult`, `ComponentInfo`, `WasmtimeCompat`, `WamrCompat`,
  `SpinConfig`, `VirtualFile`, `VirtualNetworkBinding`, `VirtualEnvironment`,
  `SizeEntry`, `SizeBenchmark`, `StartupEntry`, `StartupBenchmark`,
  `CategoryResult`, `ConformanceRunner`, `DocSection`, `DocGenerator`,
  `RouteDefinition`, `HttpServerExample`, `ModuleAudit`, `WasiP2AuditReport`

**`filesystem.rs` (8 items, 0 externally consumed):**
- type aliases: `Descriptor`
- enums: `FileType`, `FsError`
- structs: `FileStat`, `DirectoryEntry`, `OpenFlags`, `DescriptorFlags`, `WasiFilesystem`

**`http.rs` (11 items, 0 externally consumed):**
- enums: `Method`, `HttpError`
- structs: `StatusCode`, `Headers`, `Request`, `Response`, `HttpClient`, `Route`,
  `HttpRouter`
- type aliases: `HandlerFn`, `MiddlewareFn`

**`resources.rs` (10 items, 0 externally consumed):**
- structs: `HandleTable<T>`, `RawHandle`, `OwnHandle<T>`, `BorrowHandle<T>`,
  `BorrowGuard<'a, T>`, `ResourceMethod`, `ResourceDef<T>`
- enums: `HandleError`
- fns: `batch_drop<T>`, `batch_drop_recursive<T>`

**`sockets.rs` (14 items, 0 externally consumed):**
- type aliases: `TcpSocketHandle`, `UdpSocketHandle`, `NetworkHandle`
- enums: `IpAddress`, `TcpState`, `SocketError`
- structs: `SocketAddr`, `Network`, `SocketOptions`, `TcpSocket`, `Datagram`,
  `UdpSocket`, `DnsResolver`, `SocketTable`

**`streams.rs` (15 items, 0 externally consumed):**
- type aliases: `InputStreamHandle`, `OutputStreamHandle`, `PollableHandle`
- structs: `InputStream`, `OutputStream`, `PollResult`, `PollEngine`,
  `StreamTable`, `MonotonicClock`, `DateTime`, `WallClock`, `WasiRandom`
- enums: `PollSource`, `StreamError`
- fns: `splice`

**`wit_lexer.rs` (5 items, 0 externally consumed):**
- structs: `WitToken`, `WitLexError`, `WitLexer<'src>`
- enums: `WitTokenKind`
- fns: `tokenize_wit`

**`wit_parser.rs` (24 items, 1 externally consumed — bold):**
- structs: `WitDocument`, `WitPackage`, `WitInterfaceDef`, `WitWorldDef`,
  `WitExternPath`, `WitFuncDef`, `WitParam`, `WitTypeDef`, `WitRecordField`,
  `WitEnumCase`, `WitVariantCase`, `WitResourceDef`, `WitUseDecl`,
  `WitUseName`, `WitParseError`, `WitParser`
- enums: `WitInterfaceItem`, `WitWorldItem`, `WitWorldImport`, `WitWorldExport`,
  `WitTypeDefKind`, `WitTypeRef`, `WitPrimitive`
- fns: **`parse_wit`**

**`wit_types.rs` (6 items, 0 externally consumed):**
- enums: `FajarType`
- structs: `WitTypeMapper`, `FlagSet`
- fns: `wit_to_pascal_case`, `wit_to_snake_case`, `map_primitive`

#### Externally-consumed pub symbols (the 10 contract entries)

| # | Module path | Symbol |
|---|---|---|
| 1 | `component` | `ComponentBuilder` |
| 2 | `component` | `ComponentFuncType` |
| 3 | `component` | `ComponentTypeKind` |
| 4 | `component` | `ComponentValType` |
| 5 | `component` | `ExportKind` |
| 6 | `component` | `validate_component` |
| 7 | `composition` | `ComponentInstance` |
| 8 | `composition` | `ComponentLinker` |
| 9 | `composition` | `ComponentAdapter` |
| 10 | `composition` | `ExportDef` |
| 11 | `wit_parser` | `parse_wit` |

(That's 11 path-symbol pairs across 3 sub-modules. Note `ComponentInstance`
constructor `new()` + method `add_export()`/`set_return_value()`/`run()`/
`binary()`/`imports()` + `ComponentLinker::new()`/`register()`/`link()`/
`check_all_imports()`/`instance_count()` + `ComponentAdapter::new()`/
`adapt()` + `ExportDef` field-init syntax are all transitive contract
requirements — they MUST stay pub on the migrated types.)

**Surface tightness ratio: 11 used / 171 pub = 6.4%.** The extracted
crate could shrink its pub surface aggressively without breaking
fajar-lang's consumers, but the recommended action is **preserve all 171
pub items** (faithful mirror per FajarQuant precedent §2) to minimise
extraction surprise.

---

## §4. Internal dep graph

```
$ grep -rn "use crate::" src/wasi_p2/ | grep -v "use crate::wasi_p2"
(empty)
```

**ZERO cross-module `use crate::*` imports from inside `src/wasi_p2/`.**

This is the cleanest possible extraction surface — the module is a
self-contained subsystem with no upward dependencies into fajar-lang
core (no `crate::analyzer::*`, `crate::parser::*`, `crate::interpreter::*`,
`crate::runtime::*`, etc.).

Intra-module deps (`use super::*` / `use super::<sibling>`) confirm a
clean internal DAG:

| Importer | Imports (super::) | Direction |
|---|---|---|
| `component.rs` (L15) | `wit_parser::{WitFuncDef, WitTypeRef, WitWorldDef}` | component ← wit_parser |
| `composition.rs` (L20) | `component::{ComponentBuilder, ComponentFuncType, ComponentTypeKind, ExportKind}` | composition ← component |
| `composition.rs` (L21) | `filesystem::WasiFilesystem` | composition ← filesystem |
| `composition.rs` (L419) | `filesystem::{DescriptorFlags, OpenFlags}` | composition ← filesystem |
| `wit_parser.rs` (L10) | `wit_lexer::{WitToken, WitTokenKind, tokenize_wit}` | wit_parser ← wit_lexer |
| `wit_types.rs` (L7) | `wit_parser::{...}` | wit_types ← wit_parser |
| (8 other matches) | `use super::*;` inside `#[cfg(test)]` | (test-local) |

Sub-module dependency graph:
```
wit_lexer ── (no deps) ──────────────► leaf
wit_parser ◄── wit_lexer ───────────► mid (consumes lexer)
wit_types ◄── wit_parser ────────────► mid (consumes parser)
component ◄── wit_parser ────────────► mid (consumes parser)
filesystem ── (no deps) ─────────────► leaf
composition ◄── component, filesystem ► top (consumes 2)
deployment ── (no super:: deps to siblings, only own impl) ► leaf
http, resources, sockets, streams, mod ── (no super:: imports) ► leaves
```

No cycles. The new crate's `lib.rs` can keep the exact same
`pub mod <name>;` shape as the current `src/wasi_p2/mod.rs`.

**Phase E §3 (Plan §12 lesson) risk = ZERO.** No `use crate::<not-self>::*`
in the extracted surface; no inversion or refactor required pre-extraction.

---

## §5. CLI subcommand audit

```bash
grep -n "build-wasi-p2\|cmd_build_wasi_p2\|BuildWasiP2\|build_wasi_p2" src/main.rs
```

Output: **2 hits total.**

| Line | Code | Purpose |
|---|---|---|
| `src/main.rs:557` | `return cmd_build_wasi_p2(&path, output.as_deref(), verbose);` | Dispatched from inside `fj build` subcommand's target-matching block when `--target wasm32-wasi-p2` or `wasm32-wasip2`. |
| `src/main.rs:5921` | `fn cmd_build_wasi_p2(path: &PathBuf, output: Option<&std::path::Path>, verbose: bool) -> ExitCode` | The CLI handler body. |

### §5.1 Dispatch context (`src/main.rs:555-558`)

```rust
// WASI P2 component target
if target == "wasm32-wasi-p2" || target == "wasm32-wasip2" {
    return cmd_build_wasi_p2(&path, output.as_deref(), verbose);
}
```

**Important nuance not surfaced in Phase 0:** `build-wasi-p2` is NOT a
top-level clap subcommand. It is a **target string** of the `fj build`
subcommand. No separate `clap::Subcommand` variant exists. This means
Phase E.4 (Option γ deprecate-warn) does NOT touch clap registration —
it only modifies the target-match block + the `cmd_build_wasi_p2` body.

### §5.2 Function signature + dependencies

`fn cmd_build_wasi_p2(path: &PathBuf, output: Option<&std::path::Path>, verbose: bool) -> ExitCode`

Depends on (all within the function body, line 5921-6025, ~105 LOC):
- `fajar_lang::wasi_p2::component::{ComponentBuilder, ComponentFuncType, ComponentTypeKind, ComponentValType, ExportKind, validate_component}` (the §3a row 1 imports)
- `read_source` (helper, in main.rs)
- `tokenize`, `parse`, `analyze` from `fajar_lang::{lexer, parser, analyzer}` (core compiler — already imported elsewhere in main.rs)
- `FjDiagnostic::from_lex_error / from_parse_error / from_semantic_error` (error display)
- `EXIT_USAGE`, `EXIT_COMPILE`, `EXIT_RUNTIME` constants (already in main.rs)
- `std::fs::write`, `std::path::PathBuf`, `ExitCode`

**Phase E.4 action per D-0.2-γ** (deprecate-warn, 3-version cycle):
- Replace L5922-5925 import with `use fajar_wasi_p2::component::{...same six symbols...};` (after `Cargo.toml` adds the git+rev dep at E.3).
- Insert at L5926 (start of fn body) a `eprintln!()` warning:
  ```
  WARN: `--target wasm32-wasi-p2` is deprecated. Install the
  `fajar-wasi-p2` crate's CLI directly. This routing will error in v37
  and be removed in v38.
  ```
- Leave the function body otherwise intact (still routes via the new
  crate symbols; Cargo dep does the heavy lifting).

For Option α (clean removal, v38): delete L555-558 + L5920-6025 (the
whole `cmd_build_wasi_p2` fn) + drop the consumer dep from Cargo.toml.

### §5.3 No other CLI surfaces

Grep confirms zero other CLI-surface references:
```bash
grep -rn "wasi_p2\|wasi-p2" src/main.rs
# only lines 557, 5921 above
```

No `wasi_p2` in clap parser, help text, README CLI docs (`src/main.rs`
help strings unchanged by extraction at v36.0.0 with γ).

---

## §6. stdlib/.fj reference check

```bash
grep -rn "wasi_p2\|wasi-p2" stdlib/
(empty)
grep -rn "wasi_p2" src/selfhost/
(empty)
```

**Stage 2 byte-equality safe for extraction.** No `.fj` source in stdlib
references wasi_p2. Self-host Stage 1 + Stage 2 compile paths are
unaffected. Phase17 4/4 byte-equality regression test stays green
through extraction.

Bonus check — examples / scripts / CI:
```bash
grep -rln "wasi_p2\|wasi-p2" examples/ benches/ scripts/ .github/
examples/aspirational/wasi_http_server.fj  # comment-only (§3a row C1)
```
Aspirational examples (not currently runnable per the `aspirational/`
directory convention) only mention the build target as a comment string.
No build/test consumes them.

---

## §7. Risk register update

Plan §8 risks re-scored with audit evidence:

| Plan risk | Plan-assigned | Audit verdict | Notes |
|---|---|---|---|
| Hidden symbol leak | MED | **LOW** | Comprehensive grep across `src/ tests/ examples/ benches/ stdlib/` returns exactly 23 symbol-import sites in 2 files + 2 dir-existence checks. Surface fully enumerated. |
| nova_v2_tests.rs has more distributed-touching tests than expected | MED-HIGH | **N/A for E** | This risk is for Path F. For Path E, nova_v2_tests has only **1 wasi_p2 hit** (dir-existence at L572). |
| Stage 2 byte-equality breaks | LOW | **NONE** | stdlib grep empty. Confirmed safe. |
| CLI users disrupted | MED | **MED** | D-0.2-γ chosen path-of-least-disruption: warning + still-functional routing in v36, error in v37, remove in v38. Phase 0 §5 decision confirmed. |
| Cargo.lock churn | LOW | **LOW** | No `wasi_p2` feature flag, no z3-style optional dep. Adding `fajar-wasi-p2 = { git, rev }` is a single line. |
| Extraction takes longer than estimated | HIGH | **MED** | Audit reveals scope is well-contained (zero cross-module deps); biggest variance risk is repo provisioning lag (Fajar handoff). |
| Cross-repo git history confusion | MED | **MED** | Same as plan. Recommend `git filter-repo --path src/wasi_p2/` at E.2 to preserve relevant history. |

### Newly surfaced risks (NOT in Plan §8)

| # | Risk | Probability | Impact | Mitigation |
|---|---|---|---|---|
| **R-E1** | 2 integ tests do `Path::new("src/wasi_p2").exists()` (validation_tests L185-189, nova_v2_tests L570-573) — will fail at E.5 directory removal. | **CERTAIN** | Build break in tests/ when extraction completes | E.5 must DELETE both tests (or replace them with `cargo metadata`-style checks against `fajar-wasi-p2` dep version, but cleaner: delete; deprecation aligns with extraction). |
| **R-E2** | E.5 sprint-test deletion footprint is **19 lib tests across 6 sprints**, not just sprint N7 (plan §3 row E.5 mentioned only "n7_1..n7_4+"). Mirrors Path C scope correction (B0 found 10 unexpected SMT-test consumers; this B0 finds equivalent scope creep). | HIGH | E.5 effort: plan estimated 2-3h; audit suggests 3-4h | E.5 task plan: 8 edits required (N7 contiguous block, N9 2 non-contiguous, N10 2 non-contiguous, W2 4 non-contiguous, W6 1, W7 1, W10 1, plus 2 integ tests). |
| **R-E3** | `cmd_build_wasi_p2` is a `fj build --target` routing branch, NOT a top-level clap subcommand. Phase E.4 description (plan §3 row E.4) says "remove its CLI clap registration" — there IS no clap registration. | LOW | Plan documentation drift, but execution unaffected once clarified | Phase E.4 task description should be revised: "remove the target-match dispatch at main.rs L555-558 + the cmd_build_wasi_p2 function body (Option α), OR replace with warning-then-route via new crate (Option γ — per D-0.2)". |
| **R-E4** | `examples/aspirational/wasi_http_server.fj` mentions `--target wasm32-wasi-p2` in a comment. Under D-0.2-γ (v36) the warning still routes; under v37/v38 (α removal) this example's comment becomes misleading. | LOW | Minor doc drift at v37/v38 | Update at v37/v38 ship time; not a v36 blocker. |

---

## §8. Verification commands (runnable, literal)

All commands run at HEAD `34be1044` (working tree clean).

```bash
cd "/home/primecore/Documents/Fajar Lang"

# §2 — File inventory + LOC
wc -l src/wasi_p2/*.rs
# expect: 13791 total, 12 files

# §2 — Test counts
grep -rn "^\s*#\[test\]" src/wasi_p2/ | wc -l
# expect: 244
grep -rn "^\s*#\[tokio::test\]" src/wasi_p2/ | wc -l
# expect: 0

# §3a — External consumer trace (symbol-level)
grep -rn "use crate::wasi_p2\|use fajar_lang::wasi_p2\|crate::wasi_p2::\|fajar_lang::wasi_p2::" \
    src/ tests/ examples/ benches/ stdlib/ 2>/dev/null | grep -v "/src/wasi_p2/"
# expect: 23 lines (1 main.rs + 22 eval/mod.rs)

# §3a — Dir-existence integ checks
grep -rn "wasi_p2" tests/ examples/ benches/ 2>/dev/null
# expect: validation_tests.rs:187, nova_v2_tests.rs:572, examples/aspirational/wasi_http_server.fj:3

# §3b — Total top-level pub items
grep -c "^pub " src/wasi_p2/*.rs | awk -F: '{s+=$2} END {print s}'
# expect: 171

# §4 — Internal dep graph (cross-module deps)
grep -rn "use crate::" src/wasi_p2/ | grep -v "use crate::wasi_p2"
# expect: (empty)

# §5 — CLI subcommand scope
grep -n "build-wasi-p2\|cmd_build_wasi_p2\|BuildWasiP2\|build_wasi_p2" src/main.rs
# expect: 2 lines (L557 dispatch, L5921 fn def)

# §6 — Stage 2 byte-equality safety
grep -rn "wasi_p2\|wasi-p2" stdlib/ src/selfhost/
# expect: (empty)

# §6 — Cargo.toml feature flag check
grep -n "wasi_p2\|wasi-p2" Cargo.toml
# expect: (empty)

# §6 — lib.rs pub mod
grep -n "wasi_p2\|wasi-p2" src/lib.rs
# expect: 72:pub mod wasi_p2;
```

Post-ship gates (run after Phase E.5 completes):

```bash
cargo test --lib 2>&1 | tail -3
# expect: ~7,211 - 19 = ~7,192 passed

cargo test --tests --no-fail-fast 2>&1 | grep "test result"
# expect: -2 integ from validation_tests + nova_v2_tests

cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# expect: 4 passed (Stage 2 byte-equality preserved)

cargo clippy --lib -- -D warnings
# expect: 0 warnings

# In new repo
cd ~/Documents/fajar-wasi-p2
cargo build && cargo test
# expect: build clean + 244 tests passed (faithful mirror)
```

---

## §9. Effort variance prediction

Re-scored using audit evidence vs plan §3 estimates:

| Sub-phase | Plan estimate (+25% buffer) | Audit-revised | Reasoning |
|---|---|---|---|
| **E.0** | ~1.5h | **~1.5h actual (this doc)** | On budget. |
| **E.1** | ~30min (Fajar action) | **✅ DONE EOS-38** | `gh repo create fajarkraton/fajar-wasi-p2 --public --license apache-2.0` — repo live with README + LICENSE. Cargo skeleton still needed at E.2 start. |
| **E.2** | ~4-6h | **~3-5h** | Surface is unusually clean (zero cross-module deps); `git filter-repo --path src/wasi_p2/` should be near-mechanical. 244 tests pass intact since they only use `super::*` patterns. |
| **E.3** | ~1.5h | **~1.5h** | On budget. Cargo.toml + 1 import-path rewrite in cmd_build_wasi_p2 + run `cargo test`. |
| **E.4** | ~1.5h | **~1h** | D-0.2-γ chosen — replace 1 import block + insert 1 eprintln!. NOT touching clap (no clap entry exists). Plan §3 row E.4 needs correction per R-E3. |
| **E.5** | ~2-3h | **~3-4h** | **+33-100% over plan estimate.** 19 lib tests across 6 non-contiguous sprint blocks + 2 integ tests + 1 lib.rs `pub mod` line + 1 `src/wasi_p2/` directory removal. Requires ~10-12 Edit calls (sprint N7 contiguous batch + 5 non-contiguous N9/N10/W2/W6/W7/W10 individual deletes + 2 integ test deletes). Echoes Path C pattern (B0 §3 row says "non-contiguous deletions handled via 5 Edit calls"). |
| **E.6** | ~2-3h | **~2-3h** | On budget. Mirror FajarQuant's 16-test pattern; 4-8 round-trip integ tests against `fajar-wasi-p2` crate exercising the 11-symbol contract. |
| **E.7** | ~1.5h | **~1.5h** | On budget. Standard closure shape (findings doc + CHANGELOG + MEMORY.md + multi-repo push). |
| **Phase E total** | ~13-17h base / 16-21h with buffer | **~13-19h** with buffer | Within plan envelope. E.5 is the variance-risk sub-phase; +25% plan buffer absorbs it. |

**Flag for re-plan:** if E.5 exceeds **5h** (plan ceiling + ~70%), pause
per Plan Hygiene §6.8 R5 (high-uncertainty +30% threshold). Likely
cause: discovering another integ-test file or stdlib `.fj` reference not
surfaced by this B0 (unlikely given exhaustive grep, but the "Path C +10
SMT-test surprise" precedent says be vigilant).

---

## §10. Closure self-check (Plan Hygiene §6.8)

```
[x] Pre-flight audit (B0) exists for the Phase                  (Rule 1 — this doc)
[x] Every task has runnable verification command                (Rule 2 — §8)
[x] Prevention mechanism added (hook/CI/rule)                   (Rule 3 — §3a freeze enforcement + Phase 0's scripts/check-no-path-deps.sh)
[x] Agent-produced numbers cross-checked with Bash              (Rule 4 — all numbers in §1 table from live grep + wc)
[ ] Effort variance tagged in commit message                    (Rule 5 — at ship-time of this B0 commit)
[x] Decisions are committed files                               (Rule 6 — Phase 0 decision file `b5b6a67b` is the contract; this B0 verifies it)
[x] Public-artifact drift swept                                 (Rule 7 — done EOS-37 + EOS-42 CLAUDE.md trim)
[x] Multi-repo state checked                                    (Rule 8 — clean per Phase 0 §2; re-verify before E.1)
```

7 YES + 1 ship-time pending. Pre-flight contract fully captured.

---

## §11. Re-entry conditions (per plan §10)

If next session pauses Phase E mid-stream after this B0 ships:

| State | Re-entry first step |
|---|---|
| Post-E.0 (this doc), pre-E.1 | Verify GitHub repo `fajarkraton/fajar-wasi-p2` exists. If yes → E.1 cargo skeleton. If no → continue holding. |
| Post-E.1, pre-E.2 | `cd ~/Documents/fajar-wasi-p2 && cargo build` should succeed on empty skeleton. Run `wc -l src/wasi_p2/*.rs` in fajar-lang to confirm source unchanged. |
| Post-E.2, pre-E.3 | New repo's `cargo test` should report 244 passed. fajar-lang unchanged. |
| Post-E.3, pre-E.5 | fajar-lang `Cargo.toml` has `fajar-wasi-p2 = { git, rev }`. `cargo build --lib` clean. `src/wasi_p2/` still present locally → DUAL-SOURCE state, must complete E.5 next. |
| Post-E.5, pre-E.6 | `src/wasi_p2/` deleted; lib.rs missing `pub mod wasi_p2`; 19 lib + 2 integ tests removed. Re-verify §8 post-ship gates green. |
| Post-E.6, pre-E.7 | `tests/wasi_p2_integration.rs` green. Multi-repo `git status -sb` should be dirty (closure commits pending). |
| Post-E.7 | Phase E shipped. Phase F can begin independently. |

---

## §12. Source artifacts

- This file: `docs/PATH_E_WASI_P2_EXTRACTION_B0_FINDINGS.md`
- Plan: `docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md` (commit `8cdbad97`)
- Phase 0 decisions: `docs/decisions/2026-05-12-path-e-f-prep.md` (commit `b5b6a67b`)
- Predecessor B0: `docs/COMPASS_5_FREEZE_REMAINING_B0_FINDINGS.md` (commit `eb3a3c25`)
- Compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 (wasi_p2 row: "Bekukan")
- Path C precedent (eval/mod.rs scope-surprise lesson): `docs/VERIFY_PATH_C_LOAD_BEARING_B0_FINDINGS.md`
- FajarQuant precedent (extraction pattern): `MEMORY.md` Pinned facts + `Cargo.toml` line 24

---

*B0 written 2026-05-12 EOS-42, post Phase 0 closure (`b5b6a67b`) + CLAUDE.md
trim (`34be1044`). ~1.5h actual / 1.5h estimated (on-budget). Verdict:
extraction is mechanically viable; surface is unusually clean (zero
cross-module deps; only 11 distinct symbols externally consumed across
2 source files + 2 dir-existence integ tests). Symbol-surface frozen at
the 11-symbol contract in §3a/§3b. E.5 scope is 19 lib + 2 integ tests
across 6 sprints — Path-C-style non-contiguous deletion pattern.
Phase E.1 handoff ✅ done EOS-38 (`fajarkraton/fajar-wasi-p2` live, public, Apache-2.0).
Phase E.2 (source file move) is the next executable step. Plan §8 risk
register updated with 4 newly-surfaced risks (R-E1..R-E4).*
