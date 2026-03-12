# LSP Server

Fajar Lang includes a full Language Server Protocol implementation for rich editor integration.

## Starting the Server

```bash
fj lsp
```

## Features

### Core

- **Diagnostics** — real-time error highlighting (lex, parse, semantic, borrow errors)
- **Go-to-definition** — jump to function, type, and trait definitions
- **Find references** — find all usages (read/write/definition classification)
- **Hover** — type information, documentation, and signature on hover
- **Auto-completion** — context-aware suggestions (functions, keywords, types, struct fields, trait methods)

### Editing

- **Rename symbol** — rename across workspace with whole-word matching
- **Signature help** — parameter hints for 16+ builtins and user functions
- **Inlay hints** — show inferred types for `let` bindings
- **Semantic highlighting** — 16 token types, 8 modifiers (context annotation coloring)

### Code Actions

- **Make mutable** — add `mut` to a binding (quick fix for SE009)
- **Add type annotation** — insert inferred type for `let`
- **Extract function** — extract selected code into a new function
- **Inline variable** — replace variable with its definition
- **Convert if/else to match** — restructure branching
- **Add missing import** — auto-import unresolved identifiers
- **Add missing fields** — fill in struct literal fields
- **Generate trait impl** — scaffold missing trait methods

### Navigation

- **Document symbols** — outline view (functions, structs, enums, traits)
- **Workspace symbols** — fuzzy search across all files
- **Call hierarchy** — incoming and outgoing call graph

### Advanced (LSP v2)

- **Trait implementation index** — `goto_implementation` for trait methods
- **Macro expander** — expand macros with hygiene tracking
- **Type-driven completion** — suggest expressions matching expected type
- **Postfix completions** — `.if`, `.match`, `.let`, `.dbg` snippets

## VS Code Setup

1. Install the Fajar Lang extension from `editors/vscode/`
2. The extension automatically starts `fj lsp` in the background
3. Open any `.fj` file to see diagnostics

## Editor Support

The LSP server works with any editor that supports the Language Server Protocol:

- **VS Code** — full support (extension provided)
- **Neovim** — via nvim-lspconfig
- **Emacs** — via lsp-mode or eglot
- **Helix** — native LSP support
- **Sublime Text** — via LSP package
- **Zed** — native LSP support
