# Language Server v3

> **Module:** `src/lsp_v3/` (2,037 lines, 42 tests)
> **Status:** ALL 50 TASKS COMPLETE
> **Goal:** rust-analyzer-level IDE experience for Fajar Lang

## Architecture

```
LSP v3
├── semantic.rs (847 lines, 20 tests)
│   ├── Semantic Tokens (24 types, 8 modifiers, delta encoding)
│   ├── Go-to-Definition + Find References
│   ├── Go-to-Implementation + Type Hierarchy
│   ├── Call Hierarchy (incoming/outgoing)
│   ├── Workspace & Document Symbols (fuzzy search)
│   ├── Import Suggestions + Unused Detection
│   ├── Hover (markdown) + Signature Help
│   ├── Inlay Hints (type, parameter, chaining)
│   ├── Code Lens (test count, references, impls, run/debug)
│   └── Folding Ranges + Breadcrumbs
│
├── refactoring.rs (660 lines, 12 tests)
│   ├── Rename Symbol (cross-file, validation)
│   ├── Extract Function + Extract Variable
│   ├── Inline Variable + Inline Function
│   ├── if-else ↔ match conversion
│   ├── Generate Trait Impl Stubs
│   ├── Generate Constructor
│   ├── Organize Imports (sort + group)
│   ├── Wrap in Some/Ok/Err, add ? operator
│   ├── Generate Documentation Comment
│   └── Move to File + Refactoring Preview
│
└── diagnostics.rs (522 lines, 10 tests)
    ├── Quick Fix: add missing import
    ├── Quick Fix: add type annotation
    ├── Quick Fix: fix typo (Levenshtein distance)
    ├── Quick Fix: make mutable
    ├── Quick Fix: add missing struct field
    ├── Quick Fix: implement trait
    ├── Diagnostic: ownership suggestion (clone/borrow/Rc)
    ├── Diagnostic: type mismatch (with cast suggestion)
    ├── Diagnostic: unreachable code (gray out)
    └── Diagnostic: deprecated API (strikethrough + replacement)
```

## Key Features

| Feature | Capability |
|---------|-----------|
| **Semantic Tokens** | 24 token types + 8 modifiers, delta-encoded |
| **Navigation** | Definition, references, implementation, call hierarchy |
| **Symbols** | Document outline, workspace search (fuzzy matching) |
| **Hints** | Inlay type hints, parameter names, chaining types |
| **Lens** | Test count, reference count, impl count, run/debug |
| **Rename** | Cross-file rename with keyword validation |
| **Extract** | Function (with captured vars) and variable |
| **Code Actions** | 8 quick fix types, 7 refactoring types |
| **Typo Fix** | Levenshtein distance-based "Did you mean?" |
| **Deprecation** | Strikethrough + automatic replacement suggestion |
