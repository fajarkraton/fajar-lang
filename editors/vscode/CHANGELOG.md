# Changelog

## [11.0.0] - 2026-04-01

### Added — V13 "Beyond"
- Syntax: `async`, `await`, `comptime`, `where`, `gen`, `yield` keyword highlighting
- Syntax: `@distributed`, `@requires`, `@ensures`, `@invariant`, `@assert`, `@gpu` annotations
- Snippets: `constfn` (const function), `constgen` (const generic), `comptime` (comptime block)
- Snippets: `genfn` (generator), `asyncfn` (async function)
- Snippets: `wasi` (WASI P2 component), `fficpp` (C++ FFI), `ffipy` (Python FFI)
- Snippets: `@requires`, `@ensures` (formal verification), `@distributed` (distributed task)

### Changed
- Version aligned with Fajar Lang v11.0.0 "Beyond"
- Description updated to reflect 398K LOC, 7,402 tests, formal verification, WASI P2, FFI v2

---

## [10.0.0] - 2026-03-30

### Added — V12 "Transcendence"
- LSP: type-driven completion (dot context: struct fields + methods, `::` enum variants)
- LSP: scope-aware rename (function boundary tracking, local vs global)
- LSP: incremental analysis (content hashing, diagnostic caching, skip unchanged)
- LSP: cross-file go-to-definition (workspace symbol index)
- LSP: smart code actions for 11 error codes (ME001/003/004/005/010, SE004/007/009/010, SE001/002)
- LSP: enhanced hover (fn signatures + doc comments, struct/enum defs, variable types)
- LSP: call hierarchy depth (find_callers, find_callees)
- LSP: performance optimization (<500ms on 10K lines)
- Syntax: `yield` and `gen` keyword highlighting
- Syntax: `format!`, `matches!`, `println!`, `assert_eq!`, `cfg!` macro highlighting

### Changed
- Version aligned with Fajar Lang v10.0.0 "Transcendence"
- Description updated to reflect 350K LOC, 5,955+ tests, LLVM O3+LTO+PGO

## [9.0.1] - 2026-03-30

### Added
- LSP: inlay hints (type annotations, parameter names)
- LSP: signature help with active parameter + doc comments
- LSP: semantic tokens for rich syntax coloring
- LSP: code lens (reference counts, test runners)
- Debug: DAP protocol support with breakpoints, stepping, variables
- Snippets: expanded snippet library for async, GUI, HTTP patterns
- Icon: official Fajar Lang extension icon

### Changed
- Updated extension metadata for VS Code Marketplace
- Version aligned with Fajar Lang v9.0.1 "Ascension"
- Description updated to reflect 340K LOC, 7,468 tests

## [3.0.0] - 2026-03-28

### Added
- TextMate grammar for full syntax highlighting
- Language configuration (comments, brackets, indentation)
- Debug adapter factory for `fj debug --dap`
- Problem matcher for compiler error output
- Task definitions for build, test, run, check, fmt
- Keybinding: Ctrl+Shift+B for build

## [1.0.0] - 2026-03-10

### Added
- Initial release with basic syntax highlighting
- `.fj` file association
