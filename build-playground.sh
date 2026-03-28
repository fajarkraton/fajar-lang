#!/bin/bash
# Build the Fajar Lang playground WASM package.
#
# Prerequisites:
#   cargo install wasm-pack
#   rustup target add wasm32-unknown-unknown
#
# Output: playground/pkg/ (JavaScript + WASM bundle)
#
# Usage:
#   ./build-playground.sh          # build release
#   ./build-playground.sh --dev    # build debug (faster, larger)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

MODE="${1:---release}"
if [ "$MODE" = "--dev" ]; then
    echo "[playground] Building WASM (debug)..."
    wasm-pack build --target web --out-dir playground/pkg --dev \
        --features playground-wasm \
        -- --lib
else
    echo "[playground] Building WASM (release)..."
    wasm-pack build --target web --out-dir playground/pkg \
        --features playground-wasm \
        -- --lib
fi

echo "[playground] Build complete: playground/pkg/"
ls -lh playground/pkg/*.wasm 2>/dev/null || echo "(no .wasm file — check for errors above)"
echo ""
echo "To test locally:"
echo "  cd playground && npx vite"
