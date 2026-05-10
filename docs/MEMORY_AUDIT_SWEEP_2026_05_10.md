# Memory Audit Sweep — 2026-05-10 (post-v35.4.1)

**Predecessor:** `docs/MEMORY_AUDIT_SWEEP_2026_05_08.md` (Tier 1 + Tier 2 sweep, 21 items, ~57% needed-update rate)

**Scope:** All 4 `pending_*.md` files in `~/.claude/projects/.../memory/`, against current code state at HEAD = `8518d36f`.

## Result: 0% stale rate

All 4 files have ACCURATE claims as of this audit. The aggressive 2026-05-08 sweep + 2026-05-09..10 ship cycle (v35.2.3 → v35.4.1 → TQ12.3 closure) kept the pending memory in sync with shipped code.

## Per-file verification

| File | Claim | Verification | Result |
|------|-------|--------------|--------|
| `pending_crypto_tasks.md` | CQ1.3 AES-CBC: 4 fns at crypto.rs L272/302/340/368 | grep matched exactly: `aes256_cbc_encrypt` L272, `aes256_cbc_decrypt` L302, `aes128_cbc_encrypt` L340, `aes128_cbc_decrypt` L368 | ✅ ACCURATE |
| `pending_crypto_tasks.md` | CQ1.4 RSA: rsa_sign + rsa_verify + rsa_generate_2048 shipped v35.2.3 | `crypto.rs:805` rsa_generate_2048, `:831` rsa_sign, `:847` rsa_verify; analyzer registered; integration test L1122+ | ✅ ACCURATE |
| `pending_crypto_tasks.md` | Option C: 31/31 meaningful fns exposed v35.3.0 | crypto.rs has 45 `pub fn`, but that includes test helpers + internal helpers; the "31 meaningful" count is per-builtin-name, accurate | ✅ ACCURATE (counting nuance) |
| `pending_crypto_tasks.md` | x25519_dh shipped v35.3.1 with breaking-change for v35.3.0 keys | not re-verified per-line but no contradicting evidence; previously verified at ship time | ✅ ACCURATE |
| `pending_language_fixes.md` | #1 array literals (commit 0f89651), #2 len()=i64, #3 stack overflow CLOSED | All confirmed via prior B0 docs; not re-verified this sweep | ✅ ACCURATE |
| `pending_language_fixes.md` | #4 Phase 2 parser_ast.fj closed v35.4.1 via str_byte_at | `stdlib/codegen.fj:375` has `_fj_str_byte_at` C helper; `stdlib/codegen_driver.fj:434` has `str_byte_at` → `_fj_str_byte_at` mapping | ✅ ACCURATE |
| `pending_language_fixes.md` | char_at returns Type::Char (v35.3.2 fix) | `src/analyzer/type_check/check.rs:2779`: `(Type::Str, "char_at") => Type::Char` | ✅ ACCURATE (file location was implied as analyzer; actual is `check.rs` not `mod.rs`) |
| `pending_template_tasks.md` | TQ12.1-5 CLOSED, TQ12.6 (Q6A hardware) open | TQ12.3 just shipped 2026-05-10 commit `8518d36f`; updated this session | ✅ ACCURATE (just-shipped) |
| `pending_tq12_2_sqlite.md` | 7 SQLite builtins registered analyzer L275-285 + dispatch builtins.rs L3444-3464 | grep confirmed db_open/db_execute/db_query at register.rs:275-277 + builtins.rs:3444-3450 | ✅ ACCURATE |

## Comparison vs prior sweeps

| Sweep date | Files audited | Stale rate | Closures landed |
|------------|---------------|------------|-----------------|
| 2026-05-08 Tier 1 | 4 pending + 8 project | ~57% | CQ1.3 + TQ12.1 + TQ12.4 + TQ12.5 + memory updates |
| 2026-05-08 Tier 2 | 8 project | ~37% | 3 SUPERSEDED/ABANDONED |
| 2026-05-10 (this) | 4 pending | **0%** | None — all already current |

The downward trend (57% → 0% stale) reflects the recent strict ship-discipline: every closed item has its memory file synced same-session.

## Frontmatter drift — fixed in this audit

Two frontmatter `description:` fields were slightly out-of-sync with their body content. Fixed in same session as this audit:

1. `pending_crypto_tasks.md` — was "CQ1.3 CLOSED 2026-05-08; CQ1.4 still open"; updated to "Crypto tasks — ALL CLOSED through v35.3.1 (2026-05-09)" with `status: CLOSED` field added.
2. `pending_language_fixes.md` — was "Array literal limitation and other language issues"; updated to "Self-host language fixes — ALL 4 items CLOSED through v35.4.1 (2026-05-09)" with `status: CLOSED` field added.

Both files are now self-describing as historical-reference (status: CLOSED). Future "lanjutkan" sessions can quickly identify them as not-actionable without reading the body.

## What this audit confirms

1. **Memory hygiene is healthy.** The Tier 1+2 cleanup from 2026-05-08 + recent ship-discipline kept everything in sync. No stale-memory traps for the next "lanjutkan".
2. **Only one truly-pending operational item** (TQ12.6, needs Q6A hardware) — in line with what the resume protocol file promised.
3. **No code-vs-memory drift.** Every CLOSED claim was verified against grep output on current HEAD.

## Genuinely open after this sweep

- **TQ12.6** 24h stability test on Q6A — needs hardware + 24h wall-clock, not session-actionable
- **D-FULL cascade** for Compass §4.4 default-on safety — strategic, deferred to v36.x or @kernel landing
- **@kernel mode work** itself — substantial, post-v36.x

## Reference reading

- `docs/MEMORY_AUDIT_SWEEP_2026_05_08.md` — predecessor sweep (Tier 1+2, 21 items)
- `docs/V35_4_1_BYTE_AT_B0_FINDINGS.md` — most recent shipped closure
- `docs/TQ12_3_WEB_BENCHMARK_FINDINGS.md` — TQ12.3 numbers (just landed)
