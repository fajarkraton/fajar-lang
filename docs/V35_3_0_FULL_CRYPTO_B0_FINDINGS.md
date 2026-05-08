---
phase: v35.3.0 — full crypto exposure (Option C from CQ1.4 B0) — B0 audit (2026-05-09)
status: B0 CLOSED — 26 fns remain unreachable from `.fj` source; 24 are meaningful targets (skip `hash` redundant + `secure_zero` mutable-slice). Phased plan: 4 batches × ~45min-3h each.
purpose: empirical scope verification + per-fn signature design before any code work. Continuation of CQ1.4 v35.2.3 closure pattern, scaled up to the full 24-fn surface.
---

# v35.3.0 Full Crypto Exposure — B0 Pre-Flight Audit Findings

> CQ1.4 v35.2.3 closed the systemic gap for 7 signing-primitive fns
> (sha256 + ed25519×3 + rsa×3). Per the CQ1.4 B0 doc §3 Option C,
> the remaining ~23 fns ship as v35.3.0 minor bump. This B0
> verifies the actual count + categorizes by signature complexity
> + proposes phased execution.

## §1 — Headline numbers

| Probe | Number |
|---|---|
| Total `pub fn` in `src/stdlib_v3/crypto.rs` | **33** |
| Already exposed via v35.2.3 (CQ1.4 closure) | **7** (sha256, ed25519_generate/sign/verify, rsa_generate_2048/sign/verify) |
| Remaining unreachable from `.fj` source | **26** |
| Meaningful targets (post-skip) | **24** |
| Skipped: `hash(algorithm, data)` | redundant with sha256/384/512; users call directly |
| Skipped: `secure_zero(buf: &mut [u8])` | mutates byte slice in place; not meaningful from `.fj` source (no mutable string semantics) |

## §2 — 24 fns categorized by signature complexity

### Batch 1 — Trivial wrappers (10 fns, ~45min-1h)

Single str → str / str → bool / no-arg → primitive. Same shape as CQ1.4's `sha256`. Hex-encoded byte I/O via existing `hex_encode` / `hex_decode` helpers.

| Fn | `.fj` signature |
|---|---|
| `sha384(data: str) -> str` | hex digest 96 chars |
| `sha512(data: str) -> str` | hex digest 128 chars |
| `hex_encode_str(data: str) -> str` | UTF-8 bytes → hex |
| `hex_decode_str(hex: str) -> str` | hex → UTF-8 string (errors on invalid) |
| `base64_encode_str(data: str) -> str` | UTF-8 bytes → base64 |
| `base64_decode_str(encoded: str) -> str` | base64 → UTF-8 string |
| `constant_time_eq(a_hex: str, b_hex: str) -> bool` | constant-time hex byte comparison |
| `random_u64_range(min: i64, max: i64) -> i64` | OS RNG bounded |
| `argon2_hash(password: str) -> str` | argon2id hash (already returns Result<String,String>) |
| `argon2_verify(password: str, hash_str: str) -> bool` | password verification |

### Batch 2 — MAC + KDF (5 fns, ~45min-1h)

Two-arg or four-arg with hex-bytes I/O. Same pattern as CQ1.4's `rsa_sign`.

| Fn | `.fj` signature |
|---|---|
| `hmac_sha256(key_hex: str, data: str) -> str` | returns hex tag (32 bytes) |
| `hmac_sha256_verify(key_hex: str, data: str, tag_hex: str) -> bool` | constant-time tag comparison |
| `pbkdf2_sha256(password: str, salt_hex: str, iterations: i64, output_len: i64) -> str` | KDF; returns hex bytes |
| `hkdf_sha256(ikm_hex: str, salt_hex: str, info: str, output_len: i64) -> str` | HKDF; returns hex bytes |
| `random_bytes(len: i64) -> str` | OS RNG; returns hex bytes |

### Batch 3 — AES variants (8 fns, ~2-3h)

Multi-arg with `Tuple(str, str)` or `Result`-shaped returns. The most design-heavy batch — needs careful encoding decisions for ciphertext+tag.

| Fn | `.fj` signature |
|---|---|
| `aes128_gcm_encrypt(key_hex: str, nonce_hex: str, plaintext_hex: str, aad_hex: str) -> (str, str)` | (ciphertext_hex, tag_hex) — both hex-encoded |
| `aes128_gcm_decrypt(key_hex: str, nonce_hex: str, ciphertext_hex: str, tag_hex: str, aad_hex: str) -> str` | plaintext_hex on success, empty str on auth failure |
| `aes256_gcm_encrypt(key_hex, nonce_hex, plaintext_hex, aad_hex) -> (str, str)` | same as 128 variant |
| `aes256_gcm_decrypt(...) -> str` | same as 128 variant |
| `aes128_cbc_encrypt(key_hex: str, iv_hex: str, plaintext_hex: str) -> str` | ciphertext_hex (PKCS#7 padded) |
| `aes128_cbc_decrypt(key_hex: str, iv_hex: str, ciphertext_hex: str) -> str` | plaintext_hex on success, empty on padding failure |
| `aes256_cbc_encrypt(...) -> str` | same |
| `aes256_cbc_decrypt(...) -> str` | same |

**Design decision pending in Batch 3:** how to signal decrypt failure?
- **Option A (chosen for B0):** Return empty `str` on failure. Simplest. User checks `len(plaintext) > 0`.
- Option B: Return `(str, bool)` tuple — bool is success indicator. More explicit but breaks single-value pattern.
- Option C: Add `aes_*_decrypt_safe` variants returning Tuple. Doubles surface area.

### Batch 4 — X25519 key exchange (1 fn, ~30min)

| Fn | `.fj` signature |
|---|---|
| `x25519_generate() -> (str, str)` | (pub_hex, shared_secret_hex) — note the existing Rust impl returns X25519KeyExchange struct; for `.fj` exposure, return as Tuple |

**Note:** the existing `x25519_generate()` returns `X25519Result { public_key: [u8; 32], shared_secret: [u8; 32] }` — but key exchange typically needs TWO parties' public keys to derive a shared secret. The current Rust impl's signature is unusual — `shared_secret` is computed from a fixed peer key (need to verify). May need to restructure or expose a 2-arg version. Flag for review.

## §3 — Phased commit plan

| Batch | Fns | Effort | Ship checkpoint |
|---|---|---|---|
| **B1** Trivial (sha384/512 + encoding + RNG + argon2) | 10 | ~45min-1h | Single commit; lib + integration tests |
| **B2** MAC + KDF | 5 | ~45min-1h | Single commit; integration tests for HMAC + PBKDF2 |
| **B3** AES variants (4 GCM + 4 CBC) | 8 | ~2-3h | Single commit; integration tests for each algorithm + decrypt-failure path |
| **B4** X25519 + design-review | 1 | ~30min-1h (incl. review) | Single commit; may need to redesign x25519_generate signature |
| **Z** Closure docs + CHANGELOG v35.3.0 + CLAUDE.md §3 + push + tag + Release | n/a | ~1h | Atomic ship-commit |
| **Total** | **24 fns** | **~5-7h** | + ~15min ship overhead |

## §4 — Risks (per CLAUDE.md §6.8)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Stage 2 byte-equality breaks | NONE | n/a | No `stdlib/*.fj` nor codegen touched; analyzer + interpreter Rust-only changes |
| stage1_full chain breaks | NONE | n/a | Same — additive only |
| Lib regressions | LOW | LOW | Each batch is additive; integration tests cover happy path + failure where applicable |
| Type design surprises (Batch 3 AES) | MEDIUM | LOW | Empty-str-on-failure pattern is simple but slightly unusual; user-facing docs should call this out |
| X25519 signature redesign needed (Batch 4) | MEDIUM | LOW | Documented in §2 Batch 4 note; if redesign needed, B4 takes 1-2h instead of 30min; flag for user during B4 execution |
| Compile time | LOW | LOW | No new crates needed (everything already in Cargo.toml); only Rust additions |
| Disk usage during build | NONE | n/a | Just freed 172G in cleanup; plenty of headroom |

## §5 — Execution recommendation

Per `feedback_lanjutkan_rekomendasi.md` first-step-only: execute
**Batch 1 only** in next turn (10 trivial fns + 1 commit), then
checkpoint with user. Each subsequent batch is its own first-step
authorization to keep the checkpoint discipline that's worked all
session.

If user prefers all-batches-in-one-session: proceed sequentially
B1 → B2 → B3 → B4 → Z, with brief status reports between batches
but no AskUserQuestion until end. Estimated total session: ~6-8h
focused work.

## §6 — Decision gate (per CLAUDE.md §6.8 R6)

B0 closed → user picks execution mode:
1. **Phased with checkpoints (recommended)**: B1 first-step now, ASK before each subsequent batch. Conservative; matches established session pattern.
2. **All batches in one session**: ~6-8h focused continuous execution; report between batches but no question gates until Z ship.
3. **Just Batch 1 then close**: ~45min-1h then close session; v35.3.0 closure deferred to future session.

After v35.3.0 ship, ALL of crypto.rs is reachable from `.fj` source.
The genuinely-actionable open list reduces to:
- language fix #4 lexer perf (~2-4h)
- TQ12.3 web bench (~30min-1h, op)
- TQ12.6 24h stability (needs Q6A hw)
- D-FULL cascade (deferred v36.x)
- @kernel mode (substantial)

---

*V35_3_0_FULL_CRYPTO_B0_FINDINGS — written 2026-05-09. 24 meaningful
crypto fns to expose (skip `hash` redundant + `secure_zero` mutable-
slice). 4-batch phased plan: B1 trivial (10) → B2 MAC+KDF (5) → B3
AES (8) → B4 X25519 (1, needs design review). Total ~5-7h + ~1h
ship. Risks all LOW; no stdlib/codegen touched. User picks execution
mode (phased with checkpoints / all-in-one / just-B1).*
