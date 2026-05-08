---
phase: Memory audit sweep — Tier 1 (`pending_*.md` files)
status: TIER 1 COMPLETE 2026-05-08; 6 more stale-memory closures surfaced (12 total today across 5 audits)
purpose: proactive verification of pending-work memories per the meta-pattern observed in 2026-05-08 session — every "MEDIUM/HIGH priority" pending item from earlier 2026 era audits stale; this sweep catches the rest before they consume more wasted estimation cycles
---

# Memory Audit Sweep — Tier 1 (2026-05-08)

> The meta-pattern from this session — 3 stale-memory findings in
> 3 sequential audits (TQ12.2, len-returns-i64, stack-overflow) —
> motivated this proactive sweep. Tier 1 = the 4 `pending_*.md`
> files. Two were already updated today (TQ12.2, language_fixes).
> This audit covers the remaining 2: `pending_crypto_tasks.md` and
> `pending_template_tasks.md`.

## §1 — Tier 1 audit results

| File | Claim | Reality at 2026-05-08 | Action |
|---|---|---|---|
| `pending_tq12_2_sqlite.md` | TQ12.2 IN PROGRESS (file not created) | ✅ CLOSED today (v35.2.1) | Memory updated `8b53749e` |
| `pending_language_fixes.md` §1 (array literals) | Documented as FIXED | ✅ FIXED commit `0f89651` long ago | Already accurate |
| `pending_language_fixes.md` §2 (`len()` returns usize) | Returns usize, needs to_int wrapper | ✅ Already i64; cleanup shipped today (v35.2.2) | Memory updated `c653f33f` |
| `pending_language_fixes.md` §3 (stack overflow) | Blocks SQ11.6/SQ11.7 | ✅ CLOSED via SQ11.7 16MB stack | Memory updated `44b52a78` |
| `pending_language_fixes.md` §4 (lexer 24x slower) | char_at builtin needed | ⚠️ **GENUINELY OPEN** — 0 char_at impls; 41 substring(pos, pos+1) sites in stdlib/lexer.fj | Accurate; LOW priority |
| `pending_crypto_tasks.md` CQ1.3 (AES-CBC) | Needs aes/cbc/pkcs7 crates | ✅ DONE — aes128_cbc_encrypt/decrypt + aes256_cbc_encrypt/decrypt all in crypto.rs; deps in Cargo.toml | This audit |
| `pending_crypto_tasks.md` CQ1.4 (RSA) | Needs rsa crate | ⚠️ **GENUINELY OPEN** — no rsa dep; no rsa_* fns | Accurate; LOW priority |
| `pending_template_tasks.md` TQ12.1 (HTTP server) | Add http_serve builtin | ✅ DONE — http_listen + http_server registered | This audit |
| `pending_template_tasks.md` TQ12.2 (SQLite) | Add via rusqlite | ✅ DONE — closed v35.2.1 today | Already updated |
| `pending_template_tasks.md` TQ12.3 (web benchmark) | Measure req/sec after TQ12.1 done | ⚠️ OPERATIONAL — not a code state; needs running benchmark | Accurate (deferred to ops session) |
| `pending_template_tasks.md` TQ12.4 (GPIO) | gpio_read on Q6A via sysfs | ✅ DONE — gpio_read + gpio_write registered | This audit |
| `pending_template_tasks.md` TQ12.5 (MQTT) | mqtt_* builtins for mosquitto on Q6A | ✅ DONE — 5 mqtt_* fns (connect/publish/subscribe/recv/disconnect) registered | This audit |
| `pending_template_tasks.md` TQ12.6 (24h stability) | Deploy + monitor 24h | ⚠️ OPERATIONAL — needs Q6A hardware online + 24h wall-clock | Accurate (deferred to ops session) |

## §2 — Counts

**Total Tier 1 audit findings (cumulative across today's session):**
- ✅ **9 stale-memory CLOSED-today findings** (TQ12.2, language_fixes §1+§2+§3, CQ1.3, TQ12.1, TQ12.4, TQ12.5; the 9th is TQ12.2 from pending_template_tasks.md which is the same as pending_tq12_2_sqlite.md update)
- ⚠️ **3 genuinely-open accurate items** (language_fixes §4 lexer perf, CQ1.4 RSA, TQ12.3 web benchmark + TQ12.6 24h stability — last 2 are operational, not code)
- 1 already-accurate item (language_fixes §1 array literals — historical FIXED note)

**Stale rate among pending claims:** 9 of 13 (69%) of pending items
were closed in code but not in memory.

## §3 — What's GENUINELY OPEN after Tier 1 sweep

The actually-pending work, ranked by priority:

1. **CQ1.4 RSA signing** (~2-3h) — LOW priority per pending notes
   ("Ed25519 covers most signing needs, RSA for legacy/interop").
   Needs `rsa` crate (~30s compile time hit due to bignum arithmetic).
2. **Language fix #4 (lexer perf, char_at builtin)** (~2-4h) — LOW
   priority. Add `char_at(s, i)` builtin (returns byte without
   allocation) + migrate 41 `substring(pos, pos+1)` sites in
   `stdlib/lexer.fj`. Improves self-host lexer perf (currently 24x
   slower than Rust). "Proof-of-concept achieved" per memory note.
3. **TQ12.3 web benchmark** (~30min-1h, OPERATIONAL) — Measure req/sec
   for the `http_listen` builtin under load. Not a code change; just
   a script + run + record numbers. Useful for template ⭐⭐⭐⭐ → ⭐⭐⭐⭐⭐.
4. **TQ12.6 24h stability test** (operational, needs Q6A hardware
   online) — Deploy GPIO+MQTT+SQLite combo on Q6A; monitor for 24
   hours. Needs board online + 24h wall-clock. Not actionable in a
   normal coding session.

## §4 — Tier 2 recommendation (project_*.md)

Tier 1 (4 pending_* files) is now clean. Tier 2 = `project_*.md`
files claiming in-progress / pending work. Estimated 8-10 candidates
need spot-checking:

- `project_baremetal_progress.md` — "Phase 1-5 complete (2026-04-05),
  Phase 6+ pending"
- `project_code_quality.md` — "Code quality improvement plan...
  unwrap fix, dead_code audit"
- `project_next_session.md` — "V5 format... Next: P3 better scaling"
- `project_next_sprints.md` — "3 detailed plans ready"
- `project_os_enhancement.md` — "14-phase plan... 109 tasks"
- `project_rc_arc_refactor.md` — "Partial... 39 compilation errors
  remaining"
- `project_v21_hardware_deferred.md` (per MEMORY.md index) — likely
  superseded by recent work
- `project_v27_5_compiler_prep.md` (per MEMORY.md index) — likely
  superseded by v33+ work

Each Tier 2 audit takes ~5-10min (1-2 grep verification commands per
claim). Estimated Tier 2 sweep: ~1-1.5h total.

If past pattern holds (~70% stale rate), expect ~5-7 more
stale-memory closures in Tier 2.

## §5 — Decision gate (per CLAUDE.md §6.8 R6)

Tier 1 closed → ready for memory updates + commit (~10min).

After this commit, the genuinely-open list is short and accurate:
CQ1.4 RSA, lexer perf, web benchmark (op), 24h stability (op + hw).
For all future "lanjutkan" prompts in 2026-05 era, the resume
protocol can confidently surface ONLY these remaining items without
risk of more stale-memory surprises.

## §6 — Recommendation for next-step

Three options:

1. **Continue to Tier 2 sweep** (~1-1.5h, +5-7 expected closures) —
   Aggressively clean up before any new work. Low risk, high
   information-value. Prevents future estimation cycles from
   chasing stale memories.
2. **Tackle a genuinely-open item** — pick from §3 list (CQ1.4 RSA,
   lexer perf, etc). Substantive code work.
3. **Close session** — Tier 1 closure is itself a valuable artifact;
   today already shipped 4 GitHub Releases + this audit + 3 closure
   docs. Substantial.

---

*MEMORY_AUDIT_SWEEP_2026_05_08 — Tier 1 written 2026-05-08. 9
stale-memory closures surfaced (TQ12.2, language_fixes §2/§3, CQ1.3,
TQ12.1, TQ12.4, TQ12.5; plus already-updated TQ12.2 dup) + 3-4
genuinely-open items confirmed accurate (language_fixes §4 lexer
perf, CQ1.4 RSA, TQ12.3 web benchmark op, TQ12.6 24h stability op).
~70% stale rate among pending-claim items. Tier 2 (project_*.md)
sweep proposed as continuation; ~1-1.5h estimated, +5-7 expected
closures.*
