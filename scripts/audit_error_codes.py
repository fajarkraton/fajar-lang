#!/usr/bin/env python3
"""Audit error-code coverage: every code in docs/ERROR_CODES.md must
have a coverage test in tests/error_code_coverage.rs.

This is the mechanical decision gate for FAJAR_LANG_PERFECTION_PLAN P4.C2
per CLAUDE.md §6.8 R3 (prevention layer per phase).

Usage:
    python3 scripts/audit_error_codes.py             # report
    python3 scripts/audit_error_codes.py --strict    # exit 1 on any gap

Output (stdout):
    cataloged: N
    covered:   M
    gap:       K (codes cataloged but not covered)
    bonus:     P (codes covered but not in catalog — possible drift)

Forward-compat codes (catalog-only, no source emission) are listed in the
"forward-compat" tracking and do NOT count as gaps in --strict mode if
they appear in catalog with the literal substring "forward-compat" in
the same row.
"""
import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "docs" / "ERROR_CODES.md"
TESTS = ROOT / "tests" / "error_code_coverage.rs"

CODE_RE = re.compile(r"\b(LE|PE|SE|KE|DE|TE|RE|ME|CE|EE|GE|LN|CT|NS)\d{3,4}\b")
TEST_FN_RE = re.compile(r"^\s*fn coverage_([a-z]{2})(\d{3,4})_", re.MULTILINE)
BATCH_TUPLE_RE = re.compile(r'"((?:LE|PE|SE|KE|DE|TE|RE|ME|CE|EE|GE|LN|CT|NS)\d{3,4})"')


def extract_catalog(path: Path) -> tuple[set[str], set[str]]:
    """Return (all_cataloged, forward_compat_only)."""
    text = path.read_text(encoding="utf-8")
    all_codes: set[str] = set()
    forward_compat: set[str] = set()
    for line in text.splitlines():
        if not line.startswith("|"):
            continue
        codes_in_line = set(CODE_RE.findall_iter(line) if hasattr(CODE_RE, "findall_iter") else [])
        codes_in_line = {m.group(0) for m in CODE_RE.finditer(line)}
        if not codes_in_line:
            continue
        all_codes |= codes_in_line
        if "forward-compat" in line.lower() or "metadata only" in line.lower():
            forward_compat |= codes_in_line
    # Also tag the LN section's forward-compat status: codes only inside the
    # LN forward-compat table.
    ln_block_match = re.search(
        r"## 11\. Linear Type Errors.*?(?=\n## )", text, flags=re.DOTALL
    )
    if ln_block_match and "forward-compat" in ln_block_match.group(0).lower():
        forward_compat |= {c for c in all_codes if c.startswith("LN")}
    # Codes with "(forward-compat" inline in the row description.
    for m in re.finditer(r"\|\s*(\w+)\s*\|.*forward-compat", text, re.IGNORECASE):
        c = m.group(1)
        if CODE_RE.fullmatch(c):
            forward_compat.add(c)
    return all_codes, forward_compat


def extract_covered(path: Path) -> set[str]:
    """Return set of codes covered by individual `coverage_<code>_*` tests
    plus codes referenced inside batch tuples (e.g., `("CE001", ...)`)."""
    text = path.read_text(encoding="utf-8")
    covered: set[str] = set()
    # Per-test fn names: coverage_le001_unexpected_char → LE001
    for prefix, num in TEST_FN_RE.findall(text):
        covered.add(prefix.upper() + num)
    # Batch tests use `("CE001", CodegenError::...)` tuples — also count.
    for m in BATCH_TUPLE_RE.finditer(text):
        covered.add(m.group(1))
    return covered


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--strict", action="store_true", help="exit 1 on gap")
    parser.add_argument("--verbose", action="store_true", help="list codes")
    args = parser.parse_args()

    cataloged, forward_compat = extract_catalog(CATALOG)
    covered = extract_covered(TESTS)

    gap = cataloged - covered - forward_compat
    bonus = covered - cataloged

    print(f"cataloged: {len(cataloged)}")
    print(f"forward-compat: {len(forward_compat)} (skipped from gap check)")
    print(f"covered:   {len(covered)}")
    print(f"gap:       {len(gap)}")
    print(f"bonus:     {len(bonus)} (covered but not cataloged — possible new code)")

    if args.verbose or gap or bonus:
        if forward_compat:
            print(f"\nForward-compat: {sorted(forward_compat)}")
        if gap:
            print(f"\nGap (cataloged, not covered, not forward-compat):")
            for c in sorted(gap):
                print(f"  - {c}")
        if bonus:
            print(f"\nBonus (covered, not cataloged):")
            for c in sorted(bonus):
                print(f"  - {c}")

    if args.strict and gap:
        print(f"\nFAIL: {len(gap)} codes lack coverage. Add tests to "
              f"tests/error_code_coverage.rs (one `coverage_<code>_*` fn per code).",
              file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
