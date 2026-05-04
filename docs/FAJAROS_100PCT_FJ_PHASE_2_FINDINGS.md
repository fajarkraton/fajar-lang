---
phase: 2 — boot/startup.S removal (Phase 2.A compiler fix + 2.B port/delete)
status: CLOSED 2026-05-04
budget: 1-1.5d (Option 2A from Phase 0.3 finding) + 25% surprise
actual: ~2.5h Claude time (≈ 0.3d)
variance: -70%
artifacts:
  - This findings doc
  - fajar-lang commit 4b115d45 (Phase 2.A: LLVM global_asm! emission)
  - fajar-lang commit pending (Phase 2.B: lexer r#"..."# + parser raw asm)
  - fajaros-x86 commit pending (Phase 2.B: delete boot/startup.S + Makefile cleanup)
prereq: Phase 1 closed (fajar-lang c44bdb05)
---

# Phase 2 Findings — boot/startup.S removal

> Phase 2 of `docs/FAJAROS_100PCT_FJ_PLAN.md`. Phase 0.3 had pre-flight
> identified Option 2A (port .S to .fj global_asm!) as the path. Phase 2
> execution surfaced two deeper findings that simplified AND broadened
> the work.

## Phase 2.A — fajar-lang compiler-side prep

### Gap G-G surfaced and FIXED

**Discovery:** Phase 2.B's plan was "wrap boot/startup.S in
`global_asm!(r#"..."#)`" — which assumed `global_asm!()` worked. Audit
revealed:

- `global_asm!()` parsed via `parser/expr.rs:1196 parse_global_asm`
- Stored as `Item::GlobalAsm(GlobalAsm { template: String, span })`
- Cranelift backend collects them in `global_asm_sections: Vec<String>`
  with a getter, BUT no code emits these to the output object file.
  Tests (`src/codegen/cranelift/tests.rs:12001+`) only verify collection.
- LLVM backend has **zero handling** of `Item::GlobalAsm`.

**Severity:** HIGH for Phase 2.B as planned (would silently drop
the asm). Real fj-lang capability gap; affects any future kernel work.

### Fix (committed `4b115d45` fajar-lang main)

Added Pass 0.4 in `LlvmCompiler::compile_program`:
```rust
let mut combined = String::new();
for item in &program.items {
    if let Item::GlobalAsm(ga) = item {
        if !combined.is_empty() { combined.push('\n'); }
        combined.push_str(&ga.template);
    }
}
if !combined.is_empty() {
    self.module.set_inline_assembly(&combined);
}
```

`Module::set_inline_assembly()` forwards to `LLVMSetModuleInlineAsm2`.
Concatenated asm emitted verbatim into the output `.o` file's text
section, with `.section` directives controlling final layout.

### Phase 2.A.2 — raw string lexer + parser asm template

**Discovery 2:** wrapping 515 LOC of asm in `global_asm!()` requires a
string literal that can hold `"`. Two fj-lang gaps:

1. **Lexer:** `r"..."` raw strings exist but no `r#"..."#` /
   `r##"..."##` Rust-style hash-delimited variants.
2. **Parser:** `parse_global_asm` and `parse_inline_asm` accept ONLY
   `TokenKind::StringLit`, not `TokenKind::RawStringLit`.

### Fix (committed pending fajar-lang main)

- `src/lexer/mod.rs`: extended raw-string scanner to support N hash
  delimiters (`r#"..."#` through arbitrary `r##...##"..."##...##`).
  Counts opening hashes, finds matching closing `"` followed by N hashes.
- `src/parser/expr.rs`: `parse_global_asm` AND `parse_inline_asm` now
  accept `StringLit | RawStringLit`.

3 new lexer tests + 3 new LLVM codegen tests added (single-block,
multi-block-concat, absence-leaves-empty for asm emission;
single-hash, double-hash, multiline for raw strings).

### Verification (Phase 2.A E2E)

```fajar
global_asm!(r#".section .test\n.global mark\nmark: .quad 0xCAFEBABE"#)
fn main() -> i32 { 0 }
```
→ `nm` shows `mark` symbol; `objdump -h` shows `.test` (8 bytes).

Total: 8,963 → 8,966+ lib tests pass.

## Phase 2.B — boot/startup.S removal (the actual goal)

### Discovery 3: boot/startup.S is DEAD CODE in main build

After Phase 2.A landed, I went to wrap `boot/startup.S` in a new
`kernel/boot/startup_x86_64.fj` file. Created it, `fj check` passed.
But before adding to SOURCES, I checked the Makefile dependency chain:

- `build-llvm: $(COMBINED) $(RUNTIME_O) $(VECMAT_O) $(TL2_O)` (line 310)
- `STARTUP_O` is **NOT** in build-llvm's dependency list.
- Only `build-llvm-custom` (line 473) depends on `STARTUP_O`.
- Main build link uses `combined.start.o.saved` — that's fj-lang's
  AUTO-GENERATED startup (verified via `nm`: contains `_start`,
  `_start64`, `__gdt64`, `multiboot_header_start/end`).

**Conclusion:** fajaros-x86's main `build-llvm` path has never used
`boot/startup.S`. The file existed only for the alternate
`build-llvm-custom` target (also dead code in production).

This **invalidates Phase 0.3's Option 2A recommendation** — there's
no port to do. Just delete.

### Phase 0.3 retrospective

Phase 0.3 said:
> "fajaros boot/startup.S has framebuffer tag (type=5, 1024×768×32)
> that auto-gen lacks; page tables differ; CPUID feature detection
> in fajaros startup.S — auto-gen lacks. Option 2A (port to global_asm!)
> recommended."

What I missed: I didn't check whether the auto-gen was actually
USED in the main build path. The auto-gen DOES lack framebuffer tag
(per linker.rs:1418), but **fajaros doesn't need it** — fajaros uses
VGA TEXT mode directly via `kernel/runtime_stubs.S`'s
`fj_rt_bare_console_putchar` writing to `0xB8000`, not framebuffer.
So the absence is harmless for fajaros's actual graphics path.

The Phase 0.3 recommendation was over-cautious. Better audit pattern
for future phases: trace the `make` dependency graph, not just diff
the file content.

### Phase 2.B execution

1. Renamed `boot/startup.S` → `boot/startup.S.bak` for safety.
2. `rm -f build/fajaros-llvm.elf && make build-llvm` →
   ELF size **identical** (1,504,519 bytes). Confirms dead code.
3. `make test-spinlock-smp-regression` → PASS in 25s.
4. `make test-security-triple-regression` → 6/6 invariants PASS in 25s.
5. Permanently deleted `boot/startup.S.bak`.
6. Removed Makefile dead-code:
   - `STARTUP_S` + `STARTUP_O` variable definitions
   - `build-llvm-custom` from `.PHONY` list
   - `$(STARTUP_O)` build rule (lines 468-471)
   - `build-llvm-custom` target (lines 473-482)
7. Replaced with a clear comment block explaining the removal +
   pointer to git history if anyone needs custom startup.
8. Final clean rebuild + regression gates → all green.

## Audit progress

```
$ make audit-100pct-fj
[INFO] 2 non-fj files remaining:
   768 LOC  ./kernel/compute/vecmat_v8.c
   912 LOC  ./boot/runtime_stubs.S
  TOTAL: 1680 LOC
Plan progress:
  Phase 0 baseline:  3 files, 2,195 LOC
  Plan target:       0 files, 0 LOC (end of Phase 4)
```

**515 LOC eliminated** (boot/startup.S). Mechanical audit confirms.

## Phase 2 summary

| Sub-task | Status | Surfaced |
|---|---|---|
| 2.A.1 LLVM global_asm! emission (Gap G-G) | ✅ CLOSED | New gap — fixed in 4b115d45 |
| 2.A.2 Lexer r#"..."# + parser raw asm template | ✅ CLOSED | Two gaps — fixed in pending commit |
| 2.B.1 Verify boot/startup.S unused by main build | ✅ CONFIRMED | Phase 0.3 was over-cautious |
| 2.B.2 Delete boot/startup.S + Makefile cleanup | ✅ CLOSED | -515 LOC; clean rebuild + gates green |

**Phase 2 effort:** ~2.5h (vs 1-1.5d planned). Variance: -70%. Saved
because boot/startup.S was dead code AND because Phase 2.A compiler
work (which I had budgeted for separately) closed Gap G-G + raw string
gaps in the same window.

## Surfaced fj-lang gaps (closed in this phase)

| Gap | Status | Commit |
|---|---|---|
| G-G LLVM backend doesn't emit `global_asm!()` | ✅ CLOSED | 4b115d45 fajar-lang |
| G-H Lexer lacks `r#"..."#` raw string with hash delimiters | ✅ CLOSED | (this commit) |
| G-I `parse_global_asm` + `parse_inline_asm` reject raw strings | ✅ CLOSED | (this commit) |

G-F (analyzer SE009 false-positive on asm operand uses) — surfaced in
Phase 1 — still pending. Defer to Phase 5 alongside LLVM atomics.

## Decision gate (§6.8 R6)

This file committed → satisfies pre-commit gate for Phase 3+ work.
Phase 3 (port `boot/runtime_stubs.S` → 4 .fj files using
`global_asm!()` blocks) UNBLOCKED. The G-G/G-H/G-I fixes from this
phase are exactly what Phase 3 needed.

---

*FAJAROS_100PCT_FJ_PHASE_2_FINDINGS — 2026-05-04. Closes Phase 2 with
3 fj-lang compiler gaps closed (G-G, G-H, G-I), boot/startup.S deleted,
Makefile cleaned. Plan progress: 2/9 phases CLOSED, 1,680 LOC non-fj
remaining (down from 2,195), plan target 0.*
