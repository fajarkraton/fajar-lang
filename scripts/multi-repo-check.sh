#!/usr/bin/env bash
# scripts/multi-repo-check.sh — V26 Plan Hygiene Rule 8 prevention layer
#
# Mechanizes the cross-repo state check that Rule 8 mandates before any
# session that touches more than one V26 repo. Without it, multi-repo
# work accumulates silently — the V26 Phase A4 audit found fajaros-x86
# had 40 commits unpushed for 5 days of major FajarQuant Phase 1-8 +
# Gemma 3 + SmolLM-135M v3-v6 work, all at risk of total loss on disk
# failure. Discovered only by manual GitHub audit.
#
# This script makes that audit one command:
#   bash scripts/multi-repo-check.sh           # human-readable, exit 1 if attention needed
#   bash scripts/multi-repo-check.sh --quiet   # one line per repo, CI-friendly
#   bash scripts/multi-repo-check.sh --json    # machine-readable JSON output
#   bash scripts/multi-repo-check.sh --help    # this header
#
# Exit codes:
#   0 — all repos clean and in sync with origin
#   1 — at least one repo has unpushed commits OR a dirty working tree
#   2 — at least one repo is missing or not a git repo (config error)
#
# Per CLAUDE.md §6.8 Rule 8 + V26 plan §10.5 Rule 8.

set -uo pipefail

# ─────────────────────────────────────────────────────────────────
# Canonical list of V26 local repos
# Update this list when a new repo joins the V26 effort.
# ─────────────────────────────────────────────────────────────────
REPOS=(
    "$HOME/Documents/Fajar Lang"
    "$HOME/Documents/fajarquant"
    "$HOME/Documents/fajaros-x86"
)

# ─────────────────────────────────────────────────────────────────
# ANSI colors (degrade gracefully if not a tty)
# ─────────────────────────────────────────────────────────────────
if [ -t 1 ] && [ "${NO_COLOR:-}" = "" ]; then
    RED=$'\033[31m'
    GREEN=$'\033[32m'
    YELLOW=$'\033[33m'
    BLUE=$'\033[34m'
    BOLD=$'\033[1m'
    RESET=$'\033[0m'
else
    RED="" GREEN="" YELLOW="" BLUE="" BOLD="" RESET=""
fi

MODE="${1:-human}"

# ─────────────────────────────────────────────────────────────────
# Per-repo inspection
# ─────────────────────────────────────────────────────────────────
inspect_repo() {
    local repo_path="$1"
    local repo_name
    repo_name=$(basename "$repo_path")

    # Existence check
    if [ ! -d "$repo_path" ]; then
        echo "MISSING|$repo_name|$repo_path|||"
        return 2
    fi
    if [ ! -d "$repo_path/.git" ]; then
        echo "NOT_GIT|$repo_name|$repo_path|||"
        return 2
    fi

    # HEAD short hash
    local head
    head=$(git -C "$repo_path" rev-parse --short HEAD 2>/dev/null || echo "?")

    # Current branch
    local branch
    branch=$(git -C "$repo_path" symbolic-ref --short HEAD 2>/dev/null || echo "DETACHED")

    # Ahead count vs origin/main (or origin/<branch> if branch != main)
    local upstream
    upstream=$(git -C "$repo_path" rev-parse --abbrev-ref --symbolic-full-name "@{upstream}" 2>/dev/null || echo "")
    local ahead behind
    if [ -n "$upstream" ]; then
        ahead=$(git -C "$repo_path" rev-list --count "$upstream..HEAD" 2>/dev/null || echo "?")
        behind=$(git -C "$repo_path" rev-list --count "HEAD..$upstream" 2>/dev/null || echo "?")
    else
        ahead="-"
        behind="-"
    fi

    # Dirty tree check (modified, staged, untracked tracked-class files)
    # Ignore untracked .venv/, target/, etc. — those are typical noise
    local dirty_count
    dirty_count=$(git -C "$repo_path" status --porcelain | grep -vE '^\?\? (\.venv|target|node_modules|\.cache|build|paper/\*\.(aux|log|bbl|blg|out|toc|synctex\.gz))/?$' | wc -l)

    # Status: clean | ahead | dirty | both
    local status="clean"
    if [ "$ahead" != "0" ] && [ "$ahead" != "-" ]; then
        if [ "$dirty_count" -gt 0 ]; then
            status="ahead+dirty"
        else
            status="ahead"
        fi
    elif [ "$dirty_count" -gt 0 ]; then
        status="dirty"
    fi

    echo "$status|$repo_name|$head|$branch|$ahead|$dirty_count"
    return 0
}

# ─────────────────────────────────────────────────────────────────
# Output formatters
# ─────────────────────────────────────────────────────────────────
print_human() {
    local needs_attention=0
    printf "%s%s=== V26 Multi-Repo State Check ===%s\n" "$BOLD" "$BLUE" "$RESET"
    printf "%-7s %-20s %-10s %-10s %-7s %-6s\n" "STATUS" "REPO" "HEAD" "BRANCH" "AHEAD" "DIRTY"
    printf "%s\n" "─────────────────────────────────────────────────────────────────"
    for repo in "${REPOS[@]}"; do
        local result
        result=$(inspect_repo "$repo")
        IFS='|' read -r status name head branch ahead dirty <<< "$result"
        local color="$GREEN"
        if [ "$status" = "clean" ]; then
            color="$GREEN"
        elif [ "$status" = "MISSING" ] || [ "$status" = "NOT_GIT" ]; then
            color="$RED"
            needs_attention=1
        else
            color="$YELLOW"
            needs_attention=1
        fi
        printf "%s%-7s%s %-20s %-10s %-10s %-7s %-6s\n" \
            "$color" "$status" "$RESET" "$name" "$head" "$branch" "$ahead" "$dirty"
    done
    printf "%s\n" "─────────────────────────────────────────────────────────────────"
    if [ "$needs_attention" -eq 0 ]; then
        printf "%s✅ All %d repos clean and in sync with origin.%s\n" "$GREEN" "${#REPOS[@]}" "$RESET"
    else
        printf "%s⚠️  At least one repo needs attention. Push or commit before starting V26 work.%s\n" "$YELLOW" "$RESET"
    fi
    return $needs_attention
}

print_quiet() {
    local needs_attention=0
    for repo in "${REPOS[@]}"; do
        local result
        result=$(inspect_repo "$repo")
        IFS='|' read -r status name head branch ahead dirty <<< "$result"
        printf "%s\t%s\t%s\tahead=%s\tdirty=%s\n" "$status" "$name" "$head" "$ahead" "$dirty"
        if [ "$status" != "clean" ]; then needs_attention=1; fi
    done
    return $needs_attention
}

print_json() {
    local needs_attention=0
    printf '{\n  "v26_repos": [\n'
    local first=1
    for repo in "${REPOS[@]}"; do
        local result
        result=$(inspect_repo "$repo")
        IFS='|' read -r status name head branch ahead dirty <<< "$result"
        if [ "$first" -eq 0 ]; then printf ',\n'; fi
        first=0
        printf '    {\n'
        printf '      "name": "%s",\n' "$name"
        printf '      "status": "%s",\n' "$status"
        printf '      "head": "%s",\n' "$head"
        printf '      "branch": "%s",\n' "$branch"
        printf '      "ahead": "%s",\n' "$ahead"
        printf '      "dirty": %s\n' "$dirty"
        printf '    }'
        if [ "$status" != "clean" ]; then needs_attention=1; fi
    done
    printf '\n  ],\n'
    printf '  "needs_attention": %s\n' "$([ $needs_attention -eq 0 ] && echo false || echo true)"
    printf '}\n'
    return $needs_attention
}

print_help() {
    sed -n '2,18p' "$0" | sed 's/^# \?//'
}

# ─────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────
case "$MODE" in
    --quiet|-q|quiet)
        print_quiet
        ;;
    --json|-j|json)
        print_json
        ;;
    --help|-h|help)
        print_help
        ;;
    human|--human|"")
        print_human
        ;;
    *)
        echo "Unknown option: $MODE" >&2
        echo "Usage: bash scripts/multi-repo-check.sh [human|--quiet|--json|--help]" >&2
        exit 2
        ;;
esac
