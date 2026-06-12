#!/usr/bin/env python3
"""Mechanical drift gate for CLAUDE.md §6.4 unsafe hygiene.

Two checks over src/**/*.rs (production code only — #[cfg(test)] tail
modules are skipped):

1. ALLOWLIST — `unsafe` constructs (fn/block/impl/extern) may only appear
   in the enumerated FFI/hardware surface below. Adding unsafe to a new
   file fails the gate; extend ALLOWED in the same commit with a one-line
   justification.

2. SAFETY COVERAGE — every unsafe construct must have a `SAFETY:` comment
   within the preceding WINDOW lines (one comment may cover an adjacent
   cluster, matching existing repo convention) or, for `unsafe fn`, a
   `# Safety` doc section in the doc block above.

Exit 0 = clean. Exit 1 = violations (always strict; --strict accepted for
symmetry with audit_error_codes.py).

Introduced 2026-06-12 per HONEST_AUDIT_V36 F7: the previous §6.4 text
("ZERO unsafe outside src/codegen/ + src/runtime/os/") predated the
GPU/NPU/FFI/BSP modules and was structurally stale.
"""

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SRC = REPO / "src"

# Files/dirs where unsafe is a structural necessity. Keep justifications
# accurate — this list IS the §6.4 policy.
ALLOWED = (
    "codegen/",                     # Cranelift/LLVM JIT + AOT — §6.4 original
    "runtime/os/",                  # kernel-mode primitives — §6.4 original
    "runtime/gpu/",                 # CUDA/wgpu driver FFI (libcuda symbol loads)
    "runtime/ml/npu/",              # Qualcomm QNN driver FFI
    "runtime/ml/ops.rs",            # AVX2 SIMD intrinsics (FWHT fast path)
    "ffi_v2/",                      # C/C++ FFI layer — unsafe is its purpose
    "bsp/",                         # board support: MMIO + Vulkan loaders
    "hw/",                          # CPUID/GPU detection intrinsics
    "jit/runtime.rs",               # calling Cranelift-emitted native code
    "plugin/mod.rs",                # dlopen plugin loader (libloading)
    "interpreter/ffi.rs",           # interpreter-level C FFI dispatch
    "interpreter/eval/methods.rs",  # OpenCL device-query FFI
    "stdlib_v3/system.rs",          # libc process/env calls
    "stdlib_v3/crypto.rs",          # write_volatile key-material zeroize
    "compiler/performance.rs",      # SmallString from_utf8_unchecked
    "main.rs",                      # CLI-level process glue
)

WINDOW = 30  # lines of look-back for a covering SAFETY comment (clusters)

UNSAFE_RE = re.compile(r"\bunsafe\b\s*(\{|fn\b|impl\b|extern\b|$)")
STRING_RE = re.compile(r'"(?:[^"\\]|\\.)*"')
TEST_MOD_RE = re.compile(r"^\s*#\[cfg\((all\()?test")
# `unsafe extern "C" fn(...)` in type position (fn-pointer aliases, vtable
# fields, Symbol<...> params) declares a type, not an operation — the call
# site is where SAFETY is required. Scrub before matching.
FN_PTR_TYPE_RE = re.compile(r'\bunsafe\s+extern\s+"[^"]*"\s+fn\b')


def scan_file(path: Path):
    """Return list of (lineno, kind, has_safety) for production unsafe sites."""
    # Whole-file test modules (declared `#[cfg(test)] mod tests;` in the
    # parent, e.g. src/codegen/cranelift/tests.rs) are test code — skip.
    if path.name == "tests.rs":
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    sites = []
    in_test = False
    for i, raw in enumerate(lines):
        if TEST_MOD_RE.match(raw):
            # Convention: #[cfg(test)] mod tests sits at the file tail.
            j = i + 1
            while j < len(lines) and lines[j].lstrip().startswith("#["):
                j += 1
            if j < len(lines) and re.match(r"^\s*(pub\s+)?mod\b", lines[j].lstrip()):
                in_test = True
        if in_test:
            continue
        stripped = raw.strip()
        if stripped.startswith(("//", "///", "//!", "*")):
            continue
        scrubbed = FN_PTR_TYPE_RE.sub("fn", STRING_RE.sub('""', raw))
        m = UNSAFE_RE.search(scrubbed)
        if not m:
            continue
        kind = (m.group(1) or "expr").strip() or "expr"
        lo = max(0, i - WINDOW)
        context = "\n".join(lines[lo:i + 1])
        has_safety = "SAFETY" in context.upper() or "# Safety" in context
        sites.append((i + 1, kind, has_safety))
    return sites


def main():
    total = allowed_total = 0
    not_allowlisted = []
    missing_safety = []
    for path in sorted(SRC.rglob("*.rs")):
        rel = path.relative_to(SRC).as_posix()
        sites = scan_file(path)
        if not sites:
            continue
        total += len(sites)
        in_allow = any(rel == a or rel.startswith(a) for a in ALLOWED)
        if in_allow:
            allowed_total += len(sites)
        for lineno, kind, has_safety in sites:
            if not in_allow:
                not_allowlisted.append(f"src/{rel}:{lineno} (unsafe {kind})")
            elif not has_safety:
                missing_safety.append(f"src/{rel}:{lineno} (unsafe {kind})")

    print(f"unsafe sites (production): {total}")
    print(f"  in allowlisted surface:  {allowed_total}")
    print(f"  outside allowlist:       {len(not_allowlisted)}")
    print(f"  missing SAFETY coverage: {len(missing_safety)}")
    ok = True
    if not_allowlisted:
        ok = False
        print("\nVIOLATION — unsafe outside the §6.4 allowlist "
              "(extend ALLOWED with justification or remove the unsafe):")
        for s in not_allowlisted:
            print(f"  {s}")
    if missing_safety:
        ok = False
        print(f"\nVIOLATION — no SAFETY comment within {WINDOW} lines "
              "(add // SAFETY: or /// # Safety):")
        for s in missing_safety:
            print(f"  {s}")
    if ok:
        print("PASS")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.argv = [a for a in sys.argv if a != "--strict"]
    sys.exit(main())
