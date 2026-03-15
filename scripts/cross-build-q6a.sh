#!/usr/bin/env bash
# Cross-build Fajar Lang for Radxa Dragon Q6A (aarch64-unknown-linux-gnu)
#
# Prerequisites:
#   rustup target add aarch64-unknown-linux-gnu
#   sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
#
# Usage:
#   ./scripts/cross-build-q6a.sh          # release build
#   ./scripts/cross-build-q6a.sh --strip  # stripped release build

set -euo pipefail

TARGET="aarch64-unknown-linux-gnu"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="${PROJECT_DIR}/target/${TARGET}/release"
BINARY="${OUTPUT_DIR}/fj"

echo "=== Fajar Lang Cross-Build for Dragon Q6A ==="
echo "Target: ${TARGET}"
echo ""

# Verify toolchain
if ! command -v aarch64-linux-gnu-gcc &>/dev/null; then
    echo "ERROR: aarch64-linux-gnu-gcc not found."
    echo "Install: sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu"
    exit 1
fi

if ! rustup target list --installed | grep -q "${TARGET}"; then
    echo "Adding Rust target: ${TARGET}"
    rustup target add "${TARGET}"
fi

# Build
echo "Building release binary..."
cd "${PROJECT_DIR}"
cargo build --release --target "${TARGET}"

# Verify
echo ""
echo "Binary: ${BINARY}"
file "${BINARY}"
SIZE=$(du -h "${BINARY}" | cut -f1)
echo "Size: ${SIZE}"

# Optional strip
if [[ "${1:-}" == "--strip" ]]; then
    STRIPPED="${OUTPUT_DIR}/fj-stripped"
    aarch64-linux-gnu-strip -o "${STRIPPED}" "${BINARY}"
    SSIZE=$(du -h "${STRIPPED}" | cut -f1)
    echo "Stripped: ${STRIPPED} (${SSIZE})"
fi

echo ""
echo "=== Build complete ==="
echo "Deploy: scp ${BINARY} radxa@<ip>:~/bin/fj"
