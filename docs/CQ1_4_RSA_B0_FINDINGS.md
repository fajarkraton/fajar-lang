---
phase: CQ1.4 RSA signing — B0 audit (2026-05-09)
status: B0 CLOSED — RSA itself genuinely pending; BUT major systemic gap surfaced: ALL 30 crypto.rs pub fns are unreachable from .fj source (analyzer name table + interpreter dispatch both missing). Same shape as TQ12.2 had. 3 scope options surfaced for user decision.
purpose: empirical verification of CQ1.4 RSA scope before code work. 5th stale-memory finding today (the CQ1.4 part is accurate; the surrounding crypto-not-exposed gap was hidden).
---

# CQ1.4 RSA — B0 Pre-Flight Audit Findings

> The pending memory `pending_crypto_tasks.md` (updated yesterday to
> "CQ1.4 RSA still open, LOW priority") said RSA needs the `rsa` crate
> + 2 fns. **Reality at 2026-05-09:** RSA itself IS still pending —
> only enum/struct scaffolding exists (`RsaKeySize`, `RsaKeyPair`),
> no fns, no Cargo dep. **Surrounding gap:** ALL 30 existing crypto.rs
> pub fns are unreachable from `.fj` source (analyzer name table has
> ZERO crypto entries; interpreter dispatch has ZERO `name == "sha256"`
> matches). Same systemic gap pattern as TQ12.2 had pre-v35.2.1.

## §1 — Headline numbers

| Probe | Number | Significance |
|---|---|---|
| `rsa = ` in Cargo.toml | ❌ MISSING | Genuinely pending |
| `pub fn rsa_*` in `src/stdlib_v3/crypto.rs` | **0** | Only `RsaKeySize` enum (L730) + `RsaKeyPair` struct (L747) scaffolding |
| `RsaKeySize` enum (Rsa2048, Rsa4096) | ✅ exists L730-745 | Scaffolding from earlier era |
| Total `pub fn` count in crypto.rs | **30** (sha256/sha384/sha512/hash + hmac_sha256 + 8 AES variants + 3 ed25519 + 2 argon2 + random_bytes + x25519_generate + pbkdf2 + hkdf + 4 encoding + 2 utility) | Full crypto surface |
| Crypto fns registered in `src/analyzer/type_check/register.rs` | **0** | Empty grep for "sha256" / "hmac" / "aes128" / "ed25519" / "chacha" |
| Crypto dispatches in `src/interpreter/eval/builtins.rs` | **0** | `grep -c 'name == "(sha\|hmac\|aes\|ed25519\|chacha\|rsa)'` returns 0 |
| `.fj` smoke `sha256("hello")` | ❌ SE001 "undefined variable" | confirms unreachable |

## §2 — The systemic gap

Every single function in `src/stdlib_v3/crypto.rs` (30 fns covering
the major crypto primitives a real-world program needs: hashing,
HMAC, AES-GCM/CBC, ed25519 signing, argon2 password hashing, x25519
key exchange, PBKDF2/HKDF KDFs, base64/hex encoding, constant-time
compare) is **callable from Rust code only**. None of them are
exposed to `.fj` source.

This is the same pattern as TQ12.2 had pre-v35.2.1:
- TQ12.2: 7 db_* fns dispatched in interpreter (`builtins.rs:3444+`)
  but only 3 registered in analyzer name table → `db_close` etc.
  rejected with SE001
- Crypto: 30 fns implemented in `crypto.rs` but NEITHER analyzer
  registration NOR interpreter dispatch exists → ALL 30 rejected
  with SE001

So even though the meta-pattern from today's session was "always
B0-audit because pending memories are stale", this case is the
INVERSE: the pending memory about CQ1.4 was technically accurate
(RSA IS still pending), but the surrounding context was much
worse — a much bigger gap exists.

## §3 — Three scope options

The original CQ1.4 plan was "add 2 RSA fns + register in analyzer".
But ed25519 (which IS implemented at L408-446 in crypto.rs) is also
not exposed; it's in the same systemic gap. So the scope question
is broader:

### Option A — CQ1.4 narrow (~1-2h, ship as v35.2.3)

- Add `rsa = "0.9"` (or latest) to Cargo.toml (~30s extra compile-time hit due to bignum)
- Implement `pub fn rsa_sign(privkey: &RsaPrivateKey, msg: &[u8]) -> Vec<u8>` and `pub fn rsa_verify(pubkey: &RsaPublicKey, msg: &[u8], sig: &[u8]) -> bool` in crypto.rs
- Add 2 unit tests (round-trip sign + verify; verify-rejects-tampered)
- Register `rsa_sign` + `rsa_verify` in analyzer (`register.rs`) + dispatch in interpreter (`builtins.rs`)
- Add 1 integration test through full pipeline
- v35.2.3 patch ship

Closes CQ1.4. Leaves the bigger systemic gap (28 other crypto fns
unreachable) unresolved.

### Option B — CQ1.4 + signing exposure (~2-3h, ship as v35.2.3)

- Everything in Option A
- PLUS register the 3 ed25519 fns + sha256 in analyzer + interpreter (signature primitives + the main hash needed for signing flows)
- Total ~6 fns exposed to `.fj` source (rsa_sign, rsa_verify, ed25519_generate, ed25519_sign, ed25519_verify, sha256)
- Coherent "fj source can sign+verify+hash" capability

Closes CQ1.4 + makes signing primitives usable from `.fj` source.

### Option C — Full crypto exposure (~5-8h, ship as v35.3.0 minor bump)

- Add RSA per Option A (2 fns)
- Register all 30 existing crypto fns in analyzer name table + interpreter dispatch
- Add integration tests for at least the big-3 categories (hashing, AEAD, signing, KDF)
- v35.3.0 minor bump (significant new user-visible capability — `.fj` source gets full crypto stdlib)

Closes the full systemic gap. Same shape as TQ12.2 closure but
~7-10× the surface area.

## §4 — Risks per option (CLAUDE.md §6.8)

| Risk | A | B | C |
|---|---|---|---|
| `rsa` crate adds compile time | ~30s extra (one-time) | ~30s | ~30s |
| Stage 2 byte-equality breaks | NONE — no stdlib/*.fj or codegen touched | NONE | NONE |
| stage1_full chain breaks | NONE | NONE | NONE |
| Lib test regressions | LOW (new fns are additive) | LOW | LOW |
| API surface mistakes (sig wrong) | LOW (mirror existing patterns) | LOW | MEDIUM (more decisions) |
| Scope creep / surprise effort | LOW | LOW | MEDIUM-HIGH (~30 fn signatures to design) |

All 3 options are LOW-risk for the engineering gates. Option C has
higher scope-uncertainty (~30 fn signatures need analyzer Type::*
choices: `[u8]` vs `Str` vs `I64` for byte-array params, etc.).

## §5 — Recommendation

**Lean Option B (CQ1.4 + signing exposure, ~2-3h, v35.2.3 patch).**

Reasoning:
- Option A alone leaves the bigger gap visible-but-unfixed; user
  who reads the v35.2.3 release notes "RSA signing added" will try
  ed25519 next (more common modern choice) and hit SE001
- Option B closes a coherent capability ("`.fj` source can do digital
  signing + hashing"), which is more useful than RSA alone
- Option C is substantial scope (~7h+ with surprise budget); doesn't
  fit the v35.2.3 patch shape; better suited for v35.3.0 minor with
  full design pass on Type::* signatures

## §6 — Closure plan if Option B chosen (~2-3h)

| Step | What | Effort |
|---|---|---|
| **1** | Add `rsa = "0.9"` to Cargo.toml; add `pub fn rsa_sign` + `pub fn rsa_verify` to crypto.rs; add 2 unit tests | ~45min |
| **2** | Register 6 fns in `src/analyzer/type_check/register.rs`: `rsa_sign`, `rsa_verify`, `ed25519_generate`, `ed25519_sign`, `ed25519_verify`, `sha256` | ~15min |
| **3** | Add interpreter dispatch in `src/interpreter/eval/builtins.rs` (mirror `db_open`/`db_execute` pattern); add to interpreter name list in `mod.rs` | ~30-45min |
| **4** | Add integration test `tests/stdlib_v3_crypto_signing_integration.rs`: full-pipeline tests for ed25519 round-trip + RSA round-trip + sha256 hashing | ~30min |
| **5** | Smoke test from `.fj` source: `let h = sha256("hello"); println(h)` etc. | ~10min |
| **6** | Update memory `pending_crypto_tasks.md` to "CQ1.4 CLOSED + ed25519 + sha256 exposed" | ~5min |
| **7** | Commit (single-step closure) | ~5min |
| **Optional Z** | CHANGELOG v35.2.3 entry + tag + push + GitHub Release | ~15min |
| **Total** | | **~2.5-3h** + optional ~15min ship |

## §7 — Decision gate (per CLAUDE.md §6.8 R6)

B0 closed → user picks A / B / C / pivot.

After CQ1.4 closure (whichever option), `pending_crypto_tasks.md`
fully closes. The remaining genuinely-open items (per yesterday's
audit sweep) reduce to:
- language fix #4 lexer perf (~2-4h, LOW)
- TQ12.3 web benchmark (~30min-1h, operational)
- TQ12.6 24h stability (needs Q6A hw)
- D-FULL cascade (deferred v36.x)

---

*CQ1_4_RSA_B0_FINDINGS — written 2026-05-09. Surfaces that CQ1.4
RSA itself IS genuinely pending (no crate dep, no fns, only enum/
struct scaffolding) — the pending memory was accurate. BUT the
surrounding context is much worse: ALL 30 crypto.rs pub fns are
unreachable from `.fj` source (zero analyzer entries, zero
interpreter dispatches). 5th stale-or-surface-finding today. Three
scope options surfaced (A narrow ~1-2h / B signing exposure ~2-3h
/ C full crypto exposure ~5-8h v35.3.0). Recommendation: Option B.*
