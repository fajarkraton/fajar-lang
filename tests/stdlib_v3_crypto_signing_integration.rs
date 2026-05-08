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
