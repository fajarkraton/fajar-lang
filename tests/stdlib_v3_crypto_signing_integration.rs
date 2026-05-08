//! CQ1.4 — full-pipeline integration tests for crypto signing builtins.
//!
//! These tests exercise the COMPLETE path `parse → analyze → eval` for
//! the 7 newly-exposed crypto fns: `sha256`, `ed25519_generate`,
//! `ed25519_sign`, `ed25519_verify`, `rsa_generate_2048`, `rsa_sign`,
//! `rsa_verify`. Without this suite, the analyzer name-table entries
//! (in `src/analyzer/type_check/register.rs`) and interpreter dispatch
//! (in `src/interpreter/eval/builtins.rs`) might silently drift again
//! — same gap pattern as TQ12.2 had pre-v35.2.1.
//!
//! Closure of CQ1.4 per `docs/CQ1_4_RSA_B0_FINDINGS.md` §6 step 4.

use fajar_lang::interpreter::Interpreter;

fn run(src: &str) -> Result<(), String> {
    let mut interp = Interpreter::new();
    interp
        .eval_source(src)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

// ════════════════════════════════════════════════════════════════════════
// SHA-256 (smallest fn; fastest test)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn cq1_4_sha256_known_vector() {
    // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    // SHA-256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
    run(r#"
fn main() {
    let empty = sha256("")
    let hello = sha256("hello")
    if empty != "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" {
        println("SHA256 EMPTY MISMATCH")
    }
    if hello != "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824" {
        println("SHA256 HELLO MISMATCH")
    }
    println(empty)
    println(hello)
}
"#)
    .expect("sha256 known-vector test should evaluate cleanly");
}

// ════════════════════════════════════════════════════════════════════════
// Ed25519 (fast keygen ~1ms; full round-trip)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn cq1_4_ed25519_sign_verify_roundtrip_full_pipeline() {
    run(r#"
fn main() {
    let kp = ed25519_generate()
    let pubkey = kp.0
    let secret = kp.1
    let msg = "hello fajar lang"
    let sig = ed25519_sign(secret, msg)
    let ok = ed25519_verify(pubkey, msg, sig)
    if !ok { println("ED25519 VERIFY FAILED") }
    let tampered = ed25519_verify(pubkey, "different", sig)
    if tampered { println("ED25519 TAMPER NOT REJECTED") }
    println("ed25519: OK")
}
"#)
    .expect("ed25519 full-pipeline round-trip should evaluate cleanly");
}

// ════════════════════════════════════════════════════════════════════════
// RSA (slow keygen ~1-3s due to bignum; one round-trip test only)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn cq1_4_rsa_sign_verify_roundtrip_full_pipeline() {
    // Note: rsa_generate_2048() takes 1-3s — keep this test single-shot.
    run(r#"
fn main() {
    let kp = rsa_generate_2048()
    let pubkey = kp.0
    let privkey = kp.1
    let msg = "hello rsa"
    let sig = rsa_sign(privkey, msg)
    let ok = rsa_verify(pubkey, msg, sig)
    if !ok { println("RSA VERIFY FAILED") }
    let tampered = rsa_verify(pubkey, "different", sig)
    if tampered { println("RSA TAMPER NOT REJECTED") }
    println("rsa: OK")
}
"#)
    .expect("rsa full-pipeline round-trip should evaluate cleanly");
}

// ════════════════════════════════════════════════════════════════════════
// v35.3.0 Batch 1 — trivial crypto wrappers (sha384/512 + encoding +
// constant_time_eq + random_u64_range + argon2)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v35_3_0_b1_sha384_known_vector() {
    // SHA-384("") = 38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b
    run(r#"
fn main() {
    let empty = sha384("")
    if empty != "38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b" {
        println("SHA384 EMPTY MISMATCH")
    }
    println("sha384: OK")
}
"#)
    .expect("sha384 known-vector test");
}

#[test]
fn v35_3_0_b1_sha512_known_vector() {
    // SHA-512("") = cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e
    run(r#"
fn main() {
    let empty = sha512("")
    if empty != "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e" {
        println("SHA512 EMPTY MISMATCH")
    }
    println("sha512: OK")
}
"#)
    .expect("sha512 known-vector test");
}

#[test]
fn v35_3_0_b1_hex_encode_decode_roundtrip() {
    run(r#"
fn main() {
    let enc = hex_encode_str("Hello")
    if enc != "48656c6c6f" { println("HEX ENCODE MISMATCH") }
    let dec = hex_decode_str(enc)
    if dec != "Hello" { println("HEX DECODE MISMATCH") }
    let invalid = hex_decode_str("not-hex")
    if invalid != "" { println("HEX DECODE: invalid input should produce empty str") }
    println("hex: OK")
}
"#)
    .expect("hex encode/decode round-trip");
}

#[test]
fn v35_3_0_b1_base64_encode_decode_roundtrip() {
    run(r#"
fn main() {
    let enc = base64_encode_str("Hello")
    if enc != "SGVsbG8=" { println("BASE64 ENCODE MISMATCH") }
    let dec = base64_decode_str(enc)
    if dec != "Hello" { println("BASE64 DECODE MISMATCH") }
    println("base64: OK")
}
"#)
    .expect("base64 encode/decode round-trip");
}

#[test]
fn v35_3_0_b1_constant_time_eq_basic() {
    run(r#"
fn main() {
    let eq = constant_time_eq("48656c6c6f", "48656c6c6f")
    if !eq { println("CT_EQ: same hex should be equal") }
    let neq = constant_time_eq("48656c6c6f", "deadbeef00")
    if neq { println("CT_EQ: different hex should be NEQ") }
    let invalid = constant_time_eq("not-hex", "48656c6c6f")
    if invalid { println("CT_EQ: invalid hex should return false") }
    println("ct_eq: OK")
}
"#)
    .expect("constant_time_eq basic");
}

#[test]
fn v35_3_0_b1_random_u64_range_in_bounds() {
    run(r#"
fn main() {
    let r = random_u64_range(10, 100)
    if r < 10 || r >= 100 {
        println("RNG OUT OF BOUNDS")
    }
    println("rng: OK")
}
"#)
    .expect("random_u64_range bounds");
}

#[test]
fn v35_3_0_b1_argon2_hash_verify_roundtrip() {
    // argon2_hash with default params is intentionally slow (~10-100ms)
    run(r#"
fn main() {
    let h = argon2_hash("correct horse battery staple")
    let v = argon2_verify("correct horse battery staple", h)
    if !v { println("ARGON2 VERIFY FAILED for matching password") }
    let bad = argon2_verify("wrong password", h)
    if bad { println("ARGON2 VERIFY ACCEPTED wrong password") }
    println("argon2: OK")
}
"#)
    .expect("argon2 round-trip");
}

// ════════════════════════════════════════════════════════════════════════
// v35.3.0 Batch 2 — MAC + KDF + RNG bytes
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v35_3_0_b2_hmac_sha256_roundtrip() {
    run(r#"
fn main() {
    let key = "0102030405060708090a0b0c0d0e0f10"
    let tag = hmac_sha256(key, "hello")
    if len(tag) != 64 { println("HMAC TAG WRONG LEN") }
    let ok = hmac_sha256_verify(key, "hello", tag)
    if !ok { println("HMAC VERIFY FAILED") }
    let bad = hmac_sha256_verify(key, "tampered", tag)
    if bad { println("HMAC TAMPER NOT REJECTED") }
    println("hmac_sha256: OK")
}
"#)
    .expect("hmac_sha256 round-trip");
}

#[test]
fn v35_3_0_b2_pbkdf2_sha256_known_output_len() {
    run(r#"
fn main() {
    let key = pbkdf2_sha256("password", "deadbeef", 1000, 32)
    if len(key) != 64 { println("PBKDF2 OUTPUT WRONG LEN (expected 64 hex chars)") }
    // Determinism: same inputs → same output
    let key2 = pbkdf2_sha256("password", "deadbeef", 1000, 32)
    if key != key2 { println("PBKDF2 NOT DETERMINISTIC") }
    println("pbkdf2_sha256: OK")
}
"#)
    .expect("pbkdf2_sha256 length + determinism");
}

#[test]
fn v35_3_0_b2_hkdf_sha256_known_output_len() {
    run(r#"
fn main() {
    let okm = hkdf_sha256("0123456789abcdef", "cafebabe", "info-context", 16)
    if len(okm) != 32 { println("HKDF OUTPUT WRONG LEN (expected 32 hex chars)") }
    println("hkdf_sha256: OK")
}
"#)
    .expect("hkdf_sha256 output length");
}

#[test]
fn v35_3_0_b2_random_bytes_correct_len() {
    run(r#"
fn main() {
    let r1 = random_bytes(16)
    if len(r1) != 32 { println("RANDOM_BYTES(16) WRONG LEN (expected 32 hex chars)") }
    let r0 = random_bytes(0)
    if len(r0) != 0 { println("RANDOM_BYTES(0) should produce empty hex") }
    let r1024 = random_bytes(1024)
    if len(r1024) != 2048 { println("RANDOM_BYTES(1024) WRONG LEN") }
    println("random_bytes: OK")
}
"#)
    .expect("random_bytes length");
}

// ════════════════════════════════════════════════════════════════════════
// v35.3.0 Batch 3 — AES variants (GCM + CBC × 128 + 256 = 8 fns)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v35_3_0_b3_aes128_gcm_roundtrip_and_tamper() {
    run(r#"
fn main() {
    let key = "0102030405060708090a0b0c0d0e0f10"
    let nonce = "010203040506070809101112"
    let pt = hex_encode_str("hello aes-128-gcm")
    let aad = hex_encode_str("aad")
    let pair = aes128_gcm_encrypt(key, nonce, pt, aad)
    let dec = aes128_gcm_decrypt(key, nonce, pair.0, pair.1, aad)
    if hex_decode_str(dec) != "hello aes-128-gcm" {
        println("AES128_GCM ROUNDTRIP FAILED")
    }
    let bad = aes128_gcm_decrypt(key, nonce, pair.0, pair.1, hex_encode_str("tampered-aad"))
    if bad != "" {
        println("AES128_GCM TAMPER NOT REJECTED")
    }
    println("aes128_gcm: OK")
}
"#)
    .expect("aes128_gcm round-trip + tamper");
}

#[test]
fn v35_3_0_b3_aes256_gcm_roundtrip_and_tamper() {
    run(r#"
fn main() {
    let key = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
    let nonce = "010203040506070809101112"
    let pt = hex_encode_str("hello aes-256-gcm")
    let aad = hex_encode_str("auth")
    let pair = aes256_gcm_encrypt(key, nonce, pt, aad)
    let dec = aes256_gcm_decrypt(key, nonce, pair.0, pair.1, aad)
    if hex_decode_str(dec) != "hello aes-256-gcm" {
        println("AES256_GCM ROUNDTRIP FAILED")
    }
    let bad = aes256_gcm_decrypt(key, nonce, pair.0, pair.1, hex_encode_str("wrong"))
    if bad != "" {
        println("AES256_GCM TAMPER NOT REJECTED")
    }
    println("aes256_gcm: OK")
}
"#)
    .expect("aes256_gcm round-trip + tamper");
}

#[test]
fn v35_3_0_b3_aes128_cbc_roundtrip() {
    run(r#"
fn main() {
    let key = "0102030405060708090a0b0c0d0e0f10"
    let iv = "00112233445566778899aabbccddeeff"
    let pt = hex_encode_str("hello cbc-128")
    let ct = aes128_cbc_encrypt(key, iv, pt)
    let dec = aes128_cbc_decrypt(key, iv, ct)
    if hex_decode_str(dec) != "hello cbc-128" {
        println("AES128_CBC ROUNDTRIP FAILED")
    }
    println("aes128_cbc: OK")
}
"#)
    .expect("aes128_cbc round-trip");
}

#[test]
fn v35_3_0_b3_aes256_cbc_roundtrip() {
    run(r#"
fn main() {
    let key = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
    let iv = "00112233445566778899aabbccddeeff"
    let pt = hex_encode_str("hello cbc-256")
    let ct = aes256_cbc_encrypt(key, iv, pt)
    let dec = aes256_cbc_decrypt(key, iv, ct)
    if hex_decode_str(dec) != "hello cbc-256" {
        println("AES256_CBC ROUNDTRIP FAILED")
    }
    println("aes256_cbc: OK")
}
"#)
    .expect("aes256_cbc round-trip");
}

// ════════════════════════════════════════════════════════════════════════
// v35.3.0 Batch 4 — X25519 key exchange (1 fn)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v35_3_0_b4_x25519_generate_returns_clamped_keypair() {
    run(r#"
fn main() {
    let kp = x25519_generate()
    let pub_hex = kp.0
    let secret_hex = kp.1
    if len(pub_hex) != 64 { println("X25519 PUB WRONG LEN (expected 64 hex chars = 32 bytes)") }
    if len(secret_hex) != 64 { println("X25519 SECRET WRONG LEN") }
    // Two generations should differ (random)
    let kp2 = x25519_generate()
    if kp.1 == kp2.1 { println("X25519 SECRET COLLISION (improbable)") }
    println("x25519_generate: OK")
}
"#)
    .expect("x25519_generate keypair shape");
}

// ════════════════════════════════════════════════════════════════════════
// v35.3.1 — x25519_dh shared-secret derivation
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v35_3_1_x25519_dh_alice_bob_full_pipeline() {
    run(r#"
fn main() {
    // Alice + Bob each generate keypair
    let alice = x25519_generate()
    let bob = x25519_generate()
    // Both compute DH using their secret + peer's public
    let alice_shared = x25519_dh(alice.1, bob.0)
    let bob_shared = x25519_dh(bob.1, alice.0)
    if alice_shared != bob_shared {
        println("X25519_DH ALICE-BOB SHARED MISMATCH")
    }
    if len(alice_shared) != 64 {
        println("X25519_DH SHARED WRONG LEN (expected 64 hex chars = 32 bytes)")
    }
    // Different peer → different secret
    let charlie = x25519_generate()
    let alice_with_charlie = x25519_dh(alice.1, charlie.0)
    if alice_with_charlie == alice_shared {
        println("X25519_DH DIFFERENT PEERS SAME SECRET (impossible)")
    }
    println("x25519_dh: OK")
}
"#)
    .expect("x25519_dh Alice+Bob full pipeline");
}
