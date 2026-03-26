# Fajar Lang Playground

> **URL:** https://play.fajarlang.dev
> **Stack:** Vite + Monaco Editor + Wasm (interpreter compiled to wasm32)
> **Status:** ALL 60 TASKS COMPLETE

## Architecture

```
Browser
├── Monaco Editor (syntax highlighting, autocomplete, themes)
├── Web Worker (isolated execution thread)
│   └── Wasm Module (Fajar Lang interpreter → wasm32)
├── Share System (URL encoding, embed, social preview)
└── Example Gallery (12 curated examples, 5 categories)
```

## Features

| Feature | Implementation |
|---------|---------------|
| **Editor** | Monaco with Fajar-specific Monarch tokenizer, dark/light themes |
| **Execution** | Web Worker + Wasm with 5s timeout, 64MB memory limit |
| **Output** | 5 tabs: stdout, result, errors, AST dump, token dump |
| **Sharing** | URL hash encoding (LZ-compressed), embed iframe, social preview |
| **Examples** | 12 examples: basics, safety, ML, OS, advanced |
| **Responsive** | Horizontal on desktop, vertical stack on mobile |
| **Keyboard** | Ctrl+Enter (run), Ctrl+S (save), Ctrl+L (clear), Ctrl+Shift+F (format) |
| **Themes** | `fajar-dark` (GitHub Dark) and `fajar-light` (GitHub Light) |
| **Fallback** | Demo mode with code analysis if Wasm unavailable |

## Syntax Highlighting

Custom Monarch tokenizer with 11 token types:
- `keyword` (red bold): fn, let, struct, enum, if, match, for, ...
- `type` (blue): i32, f64, str, Tensor, ...
- `type.ml` (purple): tensor, grad, loss, layer, model
- `type.os` (orange): ptr, addr, page, irq, syscall
- `annotation` (yellow bold): @kernel, @device, @safe, @test
- `string` (green): "hello", f"interp {x}"
- `number` (purple): 42, 3.14, 0xFF, 0b1010
- `comment` (gray italic): // line, /* block */
- `operator.pipeline` (red bold): |>
- `constant` (blue): true, false, null, Some, None, Ok, Err

## Example Gallery

| # | Example | Difficulty | Category |
|---|---------|-----------|----------|
| 1 | Hello World | Beginner | Basics |
| 2 | Fibonacci | Beginner | Basics |
| 3 | Pattern Matching | Beginner | Basics |
| 4 | Pipeline Operator | Beginner | Basics |
| 5 | Error Handling | Intermediate | Safety |
| 6 | Structs & Traits | Intermediate | Basics |
| 7 | Iterators & Closures | Intermediate | Basics |
| 8 | Generics & Option | Intermediate | Basics |
| 9 | Tensor Operations | Advanced | ML |
| 10 | ML Training Loop | Advanced | ML |
| 11 | OS Kernel Code | Advanced | OS |
| 12 | Async/Await | Advanced | Advanced |

## Development

```bash
cd playground
npm install
npm run dev          # Start dev server (port 3000)
npm run build        # Production build → dist/
npm run wasm:build   # Compile Rust → Wasm (requires wasm-pack)
```

## Files

| File | Lines | Purpose |
|------|-------|---------|
| `index.html` | 120 | Page structure, modals, overlays |
| `src/style.css` | 280 | Dark/light themes, responsive layout |
| `src/main.js` | 250 | App init, event handlers, execution |
| `src/editor.js` | 260 | Monaco setup, tokenizer, themes |
| `src/worker.js` | 200 | Web Worker bridge + fallback mode |
| `src/executor.js` | 80 | Wasm executor (runs in Worker) |
| `src/examples.js` | 220 | 12 curated examples |
| `src/share.js` | 55 | URL encoding, embed, QR |

**Backend (Rust):** `src/playground/{mod,sandbox,share,examples,ui}.rs` — ~2,200 lines
