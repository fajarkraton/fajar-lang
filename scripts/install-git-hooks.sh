#!/usr/bin/env bash
# Install Fajar Lang git hooks into .git/hooks/
#
# Idempotent: re-run any time to refresh hooks.
# Run from repo root: bash scripts/install-git-hooks.sh

set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
SOURCE_DIR="$REPO_ROOT/scripts/git-hooks"
TARGET_DIR="$REPO_ROOT/.git/hooks"

if [ ! -d "$SOURCE_DIR" ]; then
    echo "❌ Source dir not found: $SOURCE_DIR"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    echo "❌ Target dir not found: $TARGET_DIR (not a git repo?)"
    exit 1
fi

INSTALLED=0
for hook in "$SOURCE_DIR"/*; do
    [ -f "$hook" ] || continue
    name=$(basename "$hook")
    cp "$hook" "$TARGET_DIR/$name"
    chmod +x "$TARGET_DIR/$name"
    echo "✅ Installed: $name"
    INSTALLED=$((INSTALLED + 1))
done

if [ "$INSTALLED" -eq 0 ]; then
    echo "⚠️  No hooks found in $SOURCE_DIR"
    exit 1
fi

echo ""
echo "Installed $INSTALLED hook(s) into .git/hooks/"
echo "Test with:  cargo fmt --check && git commit --allow-empty -m 'test'"
