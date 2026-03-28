# Fajar Lang Playground

Browser-based playground for writing, running, and sharing Fajar Lang code.

## Architecture

```
playground/
├── index.html          # UI: Monaco editor, output tabs, share modal
├── src/
│   ├── main.js         # App bootstrap
│   ├── editor.js       # Monaco editor setup + theme
│   ├── executor.js     # Web Worker: runs Wasm in background thread
│   ├── worker.js       # Worker message handler
│   ├── examples.js     # Example programs dropdown
│   ├── share.js        # Share via URL hash (base64 encoded)
│   └── style.css       # UI styles
├── pkg/                # Built by wasm-pack (gitignored)
│   ├── fajar_lang.js   # JS glue code
│   ├── fajar_lang_bg.wasm  # Compiled WASM binary
│   └── ...
├── vite.config.js      # Vite dev server config
├── package.json        # npm dependencies (vite, monaco)
└── README.md           # This file
```

## WASM API

The Rust-to-JS bridge exports four functions:

| Function | Input | Output |
|----------|-------|--------|
| `eval_source(code)` | Fajar Lang source | Captured stdout + return value |
| `tokenize_source(code)` | Fajar Lang source | JSON array of tokens |
| `format_source(code)` | Fajar Lang source | Formatted source code |
| `check_source(code)` | Fajar Lang source | "OK" or error messages |

## Building

```bash
# Prerequisites
cargo install wasm-pack
rustup target add wasm32-unknown-unknown

# Build WASM package
./build-playground.sh

# Run dev server
cd playground
npx vite
```

## Development

```bash
# Quick rebuild (debug, faster)
./build-playground.sh --dev

# Production build
./build-playground.sh
```

The dev server at `http://localhost:5173` auto-reloads when JS files change.
WASM changes require re-running `./build-playground.sh`.
