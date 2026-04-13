#!/usr/bin/env bash
# check_version_sync.sh — Verify Cargo.toml version matches CLAUDE.md claims
# V27 A4 prevention layer (§6.8 Rule 3)

set -euo pipefail

CARGO_VER=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
CLAUDE_VER=$(grep '^*CLAUDE.md Version:' CLAUDE.md | grep -oP 'Version: \K[0-9.]+')

echo "Cargo.toml version: $CARGO_VER"
echo "CLAUDE.md version:  $CLAUDE_VER"

# Compare major version
CARGO_MAJOR=$(echo "$CARGO_VER" | cut -d. -f1)
CLAUDE_MAJOR=$(echo "$CLAUDE_VER" | cut -d. -f1)

if [ "$CARGO_MAJOR" != "$CLAUDE_MAJOR" ]; then
    echo "FAIL: Cargo.toml major ($CARGO_MAJOR) != CLAUDE.md major ($CLAUDE_MAJOR)"
    exit 1
fi

echo "PASS: Version sync OK (major $CARGO_MAJOR)"
