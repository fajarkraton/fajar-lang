---
phase: v35.3.0 — full crypto exposure (Option C from CQ1.4 B0) — CLOSED
status: CLOSED 2026-05-09 (all 4 batches landed; 31/31 crypto.rs pub fns now reachable from `.fj` source)
budget: 5-7h likely / ~9h ceiling (per V35_3_0_FULL_CRYPTO_B0_FINDINGS.md §3)
actual: ~3.5h Claude time across 6 commits (B0 + B1 + B2 + B3 + B4 + this Z)
variance: -50% vs likely / -61% vs ceiling
b0:   docs/V35_3_0_FULL_CRYPTO_B0_FINDINGS.md
prereq: v35.2.3 (CQ1.4 closed; 7 signing-primitive fns exposed) + 172G disk reclaimed in cleanup sweep
artifacts:
  - This findings doc
  - 24 fns exposed across analyzer + interpreter + name list
  - tests/stdlib_v3_crypto_signing_integration.rs — 16 new B1-B4 integration tests (total now 19)
---

# v35.3.0 — Full Crypto Exposure — Closure Findings

> CQ1.4 v35.2.3 closed the systemic gap for 7 signing-primitive fns;
> v35.3.0 closes the remaining 24 fns. **All of `src/stdlib_v3/crypto.rs`
> is now reachable from `.fj` source** — hashing (SHA-2 family), MAC
> (HMAC), AEAD encryption (AES-GCM), classic encryption (AES-CBC),
> KDFs (PBKDF2, HKDF, Argon2), digital signing (Ed25519, RSA), key
> exchange (X25519 keypair gen), encoding (hex, base64), CSPRNG
> (random_bytes, random_u64_range), and constant-time comparison.

## §1 — Headline numbers

| Probe | Number |
|---|---|
| Total `pub fn` in `src/stdlib_v3/crypto.rs` | **33** |
| Skipped: `hash(algorithm, data)` | redundant with sha256/384/512 |
| Skipped: `secure_zero(buf: &mut [u8])` | mutable-slice; not meaningful from `.fj` |
| Meaningful targets | **31** (33 − 2 skipped) |
| Already exposed via v35.2.3 (CQ1.4) | 7 |
| Newly exposed in v35.3.0 (this release) | **24** |
| **Total now reachable from `.fj` source** | **31 of 31** ✅ |

## §2 — Decision recap (D-LITE-style; no decision file)

CQ1.4 B0 surfaced 3 scope options: A narrow (just RSA, ~1-2h) / B
signing exposure (RSA + ed25519 + sha256, ~2-3h) / C full exposure
(~5-8h v35.3.0). User picked B for v35.2.3, then C for v35.3.0
in 2026-05-09 session. C executed in 4 phased batches with
checkpoint between each (per `feedback_lanjutkan_rekomendasi.md`).

## §3 — 24 fns delivered across 4 batches (B1 → B4)

| Batch | Commit | Fns | Effort |
|---|---|---|---|
| **B1** Trivial wrappers | `c124021d` | sha384, sha512, hex_encode_str, hex_decode_str, base64_encode_str, base64_decode_str, constant_time_eq, random_u64_range, argon2_hash, argon2_verify (10) | ~50min |
| **B2** MAC + KDF | `4c1f0b6b` | hmac_sha256, hmac_sha256_verify, pbkdf2_sha256, hkdf_sha256, random_bytes (5) | ~40min |
| **B3** AES variants | `cdf0b9f5` | aes128/256_gcm_encrypt/decrypt, aes128/256_cbc_encrypt/decrypt (8) | ~1h |
| **B4** X25519 | `6c9f065d` | x25519_generate (1) | ~15min |
| **Z** This commit | (this) | closure docs + CHANGELOG + CLAUDE.md + ship | ~45min |
| **B0** Pre-flight | `37857bdf` | scope verification + 4-batch plan | ~15min |
| **Total** | 6 commits | 24 fns + docs | **~3.5h** |

## §4 — API design (consistent across all 24)

### Byte-array I/O is hex-encoded as `str`

The analyzer `Type::*` enum doesn't have first-class byte-array
support that's ergonomic for `.fj` source. Adopting hex-encoded
`str` for all byte I/O across the entire crypto surface means:
- Uniform `.fj`-source API (no special types to learn)
- Trivial inspection (`println(hex_str)`)
- Composability (`hex_encode_str(string_input)` → bytes_hex for
  fns that take `_hex` params)
- Standard hex format matches OpenSSL/RustCrypto/etc. CLI tools

Trade-off accepted: 2× memory + 2× length over raw bytes. Not
relevant for keys/signatures (small fixed sizes); minor for
ciphertexts (fits typical `.fj` use cases).

### Tuple returns for keypair / encrypt-with-tag fns

GCM encrypt + all keypair generators return `Tuple(str, str)`.
Caller destructures via `.0` / `.1` field access. Same shape used
in `rsa_generate_2048` / `ed25519_generate` from CQ1.4.

### Failure semantics

| Function class | On failure |
|---|---|
| Decode/parse functions (`hex_decode_str`, `base64_decode_str`) | Empty `str` |
| Signature/MAC verify | `false` |
| AEAD/CBC decrypt (auth/padding error) | Empty `str` plaintext |
| Hash/encode/RNG (cannot fail in normal use) | n/a |
| Crypto core failures (e.g., RSA keygen, argon2_hash) | `RuntimeError` propagated |

The empty-str / false pattern is the simplest user-facing API —
callers check `len(plaintext) > 0` or `if ok { ... }`.

### Argument validation

All fns validate inputs at the wrapper level:
- Wrong-arity → `ArityMismatch`
- Wrong-type → `TypeError`
- Wrong byte length (e.g., AES-128 key must be 16 bytes) →
  `TypeError` with expected-vs-got diagnostic
- Negative / zero values where positive required (e.g., PBKDF2
  iterations) → `TypeError`

Helper fns `parse_hex_arg(args, idx, fn_name)` and
`check_len::<N>(bytes, what, fn_name)` factor out the verbose
boilerplate. Added in B3 commit; reused across all 8 AES fns.

## §5 — Tests

Total integration tests: **19** in
`tests/stdlib_v3_crypto_signing_integration.rs`. All exercise the
full `parse → analyze → eval` pipeline (catch the gap class that
existing `crypto.rs` lib unit tests miss — those bypass the
analyzer step).

| From | Tests | Coverage |
|---|---|---|
| CQ1.4 v35.2.3 | 3 | sha256 known vector + ed25519 round-trip + RSA round-trip |
| B1 v35.3.0 | 7 | sha384/512 known vectors + hex/base64 round-trip + constant_time_eq + RNG bounds + argon2 round-trip |
| B2 v35.3.0 | 4 | hmac round-trip + pbkdf2 length+determinism + hkdf length + random_bytes length variants |
| B3 v35.3.0 | 4 | AES128/256-GCM round-trip + AAD-tamper rejection + AES128/256-CBC round-trip |
| B4 v35.3.0 | 1 | x25519_generate keypair shape + uniqueness |
| **Total** | **19** | All 31 exposed fns have at least 1 integration test (some share via round-trip) |

Plus 19 lib unit tests in `src/stdlib_v3/crypto.rs` (test-the-Rust-API
direct, including 2 RSA tests added in CQ1.4 commit).

## §6 — Effort recap

| Sub-item | Plan §3 (likely) | Plan §3 (cap) | Actual | Variance vs cap |
|---|---|---|---|---|
| B0 audit + plan | n/a | n/a | ~15min | (meta-work) |
| B1 trivial wrappers | 45min-1h | 1.3h | 50min | -36% |
| B2 MAC + KDF | 45min-1h | 1.3h | 40min | -49% |
| B3 AES variants | 2-3h | 4h | 1h | **-75%** (helpers + B0 design carried) |
| B4 X25519 | 30min-1h | 1.3h | 15min | -81% (B0 redesign concern unfounded) |
| Z closure ship (this) | ~1h | 1.3h | ~45min | -42% |
| **Total** | **5-7h** | **~9h ceiling** | **~3.5h** | **-61% vs ceiling / -50% vs likely** |

The B3 surprise (-75%) was the biggest contributor to the underrun.
Helper fns (parse_hex_arg + check_len) collapsed per-method
boilerplate from ~60 LOC to ~25 LOC, and the B0 design decisions
(empty-on-failure, Tuple returns) were settled before code.

## §7 — Honest scope at v35.3.0 close

What works:
- ✅ All 31 meaningful crypto.rs pub fns now reachable from `.fj` source
- ✅ Byte-array I/O via hex strings (uniform across all 31 fns)
- ✅ 19 integration tests exercise full parse→analyze→eval pipeline
- ✅ All Stage 2 byte-equality + stage1_full + lib gates GREEN
- ✅ Stage 2 byte-equality preserved (no codegen/stdlib touched)
- ✅ Wrong-length / wrong-type / arity errors gracefully reported
- ✅ Deterministic where expected (PBKDF2, HKDF); random where expected (keygen, RNG)

What does NOT work yet (legitimate scope-boundary):
- ⚠️ **`x25519_dh(secret_hex, peer_pub_hex) -> shared_secret_hex`** —
  the actual Diffie-Hellman shared-secret derivation. Current Rust
  impl in `crypto.rs` doesn't expose this directly; would need
  `x25519-dalek` crate dep. Future patch candidate.
- ⚠️ **`hash(algorithm, data)`** dispatch wrapper — skipped as
  redundant with sha256/384/512 direct calls.
- ⚠️ **`secure_zero(buf: &mut [u8])`** — skipped; mutates byte slice
  in place; not meaningful from `.fj` source (no mutable string
  semantics). Only useful inside Rust impl boundaries.
- ⚠️ **Streaming hash / stream cipher APIs** — current crypto.rs is
  one-shot only. Not in scope.

What's available as raw building blocks for future extension:
- Adding `x25519_dh` requires `x25519-dalek = "2"` in Cargo.toml +
  ~30 LOC wrapper. Estimated ~1h ship as v35.3.1 patch.
- Streaming hash APIs would require redesign of crypto.rs internals
  (currently uses one-shot RustCrypto APIs). Not on near-term roadmap.

## §8 — Cumulative state at v35.3.0 close

| Aggregate | At v35.2.3 | At v35.3.0-pre |
|---|---|---|
| `.fj`-callable crypto fns | 7 | **31** (+24) |
| Integration tests in crypto signing suite | 3 | **19** (+16) |
| Lib unit tests in crypto.rs | 19 | 19 (unchanged) |
| Heap-leak classes closed | _FjArr realloc + R15 string-arena | unchanged |
| Stage 2 byte-equality | preserved | preserved (no stdlib/codegen touched) |
| Open code-pending items in `pending_*.md` | CQ1.4 + #4 lexer perf | only #4 lexer perf (CQ1.4 fully closed including Option C) |

## §9 — Decision gate (per CLAUDE.md §6.8 R6)

v35.3.0 closed → ready for ship sequence (push 6 commits + tag +
GitHub Release). After ship, the verified-actionable open list
across all `pending_*.md` reduces to:

- **Code work**: language fix #4 lexer perf (~2-4h, LOW priority)
- **Operational**: TQ12.3 web bench (~30min-1h) · TQ12.6 24h
  stability (needs Q6A hw)
- **Strategic**: D-FULL cascade for Compass §4.4 default-on
  (deferred v36.x or @kernel mode landing)
- **Future patch candidate**: `x25519_dh` shared-secret derivation
  (~1h, v35.3.1)

---

*V35_3_0_FULL_CRYPTO_FINDINGS — written 2026-05-09. v35.3.0 closes
in ~3.5h actual / ~9h ceiling (-61%). Full crypto exposure
complete: 31 of 31 meaningful crypto.rs fns now reachable from
`.fj` source. 16 new B1-B4 integration tests + 1 closure findings
doc. Stage 2 byte-equality preserved. All 4 batches landed in
single 2026-05-09 session per phased-checkpoint discipline.*
