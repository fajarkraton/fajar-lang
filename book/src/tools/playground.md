# Online Playground

Fajar Lang includes a browser-based playground for trying the language without installing anything.

## Features

- **Instant compilation** — Wasm-based compiler runs in the browser
- **Share code** — generate shareable URLs with base64-encoded source
- **Example library** — pre-loaded examples for all language features
- **Output panel** — see program output, errors, and timing

## How It Works

The Fajar Lang compiler is compiled to WebAssembly and runs entirely in the browser:

1. Source code is compiled in the browser sandbox (no server roundtrip)
2. The Wasm runtime executes the compiled program
3. Output (stdout, stderr) is captured and displayed

## Memory Sandbox

The playground runs in a sandboxed environment:
- **Memory limit** — 64MB per program
- **Execution timeout** — 10 seconds
- **No file system** — `read_file`/`write_file` disabled
- **No network** — HTTP/TCP disabled

## Sharing

```
https://playground.fajarlang.dev/?code=<base64-encoded-source>
```

Click "Share" to generate a permanent link to your code.

## Running Locally

```bash
cd website/
python3 -m http.server 8080
# Open http://localhost:8080
```

The playground HTML is at `website/index.html`.
