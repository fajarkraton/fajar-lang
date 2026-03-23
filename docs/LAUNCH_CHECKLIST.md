# Fajar Lang Public Launch Checklist

## Repository

- [x] CLAUDE.md — master reference (auto-loaded)
- [x] README.md / introduction — project overview
- [x] LICENSE — open source license
- [x] CONTRIBUTING.md — git workflow, code style
- [x] CHANGELOG.md — version history

## Documentation

- [x] The Fajar Lang Book (60+ chapters in book/)
- [x] Migration guides: Rust, C++, Python/ML developers
- [x] API reference: `fj doc` generates HTML from doc comments
- [x] Error codes reference (80+ codes, 10 categories)
- [x] Language specification (FAJAR_LANG_SPEC.md)
- [x] Grammar reference (GRAMMAR_REFERENCE.md)

## Technical Content

- [x] Blog post: BLOG_LAUNCH.md — introduction to Fajar Lang
- [x] Paper abstract: PAPER_ABSTRACT.md — conference submission ready
- [x] Architecture docs: ARCHITECTURE.md
- [x] Benchmarks: 7 Benchmarks Game programs + real-world suite
- [x] Performance report framework with targets

## Demo

- [x] Killer demo: drone_controller.fj (639 lines, 3 domains)
- [x] 130+ example programs
- [x] Playground: `fj playground` generates HTML
- [x] Pre-loaded examples: hello, fib, effects, comptime, macros, structs, matching, context

## Compiler

- [x] 5,547 tests (0 failures)
- [x] Clippy clean, fmt clean
- [x] Dual backend: Cranelift (dev) + LLVM (release)
- [x] `fj build --release` → LLVM O2
- [x] `fj build --incremental` → cached rebuilds
- [x] Wasm backend for browser
- [x] Cross-compilation: x86_64, ARM64, RISC-V

## IDE Support

- [x] VS Code extension (syntax, snippets, LSP, DAP debugger)
- [x] LSP: hover, completion, definition, rename, semantic tokens, inlay hints, references
- [x] DAP: breakpoints, step in/over/out, variable inspection

## Self-Hosting

- [x] lexer.fj (381 LOC) — tokenizer in Fajar Lang
- [x] parser.fj (397 LOC) — recursive descent parser
- [x] analyzer.fj (210 LOC) — type checker
- [x] codegen.fj (280 LOC) — C code emitter
- [x] Total: 1,268 LOC self-hosted compiler

## Community (Planned)

- [ ] Discord server
- [ ] GitHub Discussions enabled
- [ ] Weekly newsletter template
- [ ] First 3 beta users identified
- [ ] Hacker News post prepared
- [ ] Reddit r/programming post prepared
- [ ] Lobste.rs post prepared

## Deployment (Planned)

- [ ] GitHub Actions CI: Linux + macOS + Windows
- [ ] Release binaries for all platforms
- [ ] Homebrew formula
- [ ] Docker image
- [ ] Website: fajarlang.org
- [ ] Registry: registry.fajarlang.org
- [ ] Playground: play.fajarlang.org
