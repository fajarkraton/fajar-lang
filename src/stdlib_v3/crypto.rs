//! Crypto & Security — hashing, encryption, signing, key exchange, encoding.
//!
//! Phase S2 + V8 GC1: Real cryptographic implementations via RustCrypto.
//! Hashing (SHA-2), HMAC, AES-GCM encryption, Ed25519 signing, Argon2 password hashing.
//! Encoding: Base64, Hex. Utilities: constant-time comparison, secure zeroing.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S2.1: SHA-256/384/512
// ═══════════════════════════════════════════════════════════════════════

/// Hash algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    /// Returns the output length in bytes.
    pub fn output_len(self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    /// Returns the block size in bytes.
    pub fn block_size(self) -> usize {
        match self {
            Self::Sha256 => 64,
            Self::Sha384 | Self::Sha512 => 128,
        }
    }
}

/// A hash digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    /// Raw bytes.
    pub bytes: Vec<u8>,
    /// Algorithm used.
    pub algorithm: HashAlgorithm,
}

impl Digest {
    /// Returns the hex representation.
    pub fn hex(&self) -> String {
        self.bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hex())
    }
}

/// SHA-256 initial hash values (first 32 bits of fractional parts of sqrt(primes)).
pub const SHA256_H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// SHA-256 round constants.
pub const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.2-GC1.3: Real SHA-256/384/512 via sha2 crate
// ═══════════════════════════════════════════════════════════════════════

/// Compute SHA-256 hash of data. Returns 32-byte digest.
pub fn sha256(data: &[u8]) -> Digest {
    use sha2::Digest as Sha2Digest;
    let hash = sha2::Sha256::digest(data);
    Digest {
        bytes: hash.to_vec(),
        algorithm: HashAlgorithm::Sha256,
    }
}

/// Compute SHA-384 hash of data. Returns 48-byte digest.
pub fn sha384(data: &[u8]) -> Digest {
    use sha2::Digest as Sha2Digest;
    let hash = sha2::Sha384::digest(data);
    Digest {
        bytes: hash.to_vec(),
        algorithm: HashAlgorithm::Sha384,
    }
}

/// Compute SHA-512 hash of data. Returns 64-byte digest.
pub fn sha512(data: &[u8]) -> Digest {
    use sha2::Digest as Sha2Digest;
    let hash = sha2::Sha512::digest(data);
    Digest {
        bytes: hash.to_vec(),
        algorithm: HashAlgorithm::Sha512,
    }
}

/// Compute hash with specified algorithm.
pub fn hash(algorithm: HashAlgorithm, data: &[u8]) -> Digest {
    match algorithm {
        HashAlgorithm::Sha256 => sha256(data),
        HashAlgorithm::Sha384 => sha384(data),
        HashAlgorithm::Sha512 => sha512(data),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.4: Real HMAC via hmac crate
// ═══════════════════════════════════════════════════════════════════════

/// HMAC configuration.
#[derive(Debug, Clone)]
pub struct HmacConfig {
    /// Hash algorithm.
    pub algorithm: HashAlgorithm,
    /// Secret key.
    pub key: Vec<u8>,
}

/// Compute HMAC-SHA256 of data with given key.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Verify HMAC-SHA256 tag.
pub fn hmac_sha256_verify(key: &[u8], data: &[u8], tag: &[u8]) -> bool {
    constant_time_eq(&hmac_sha256(key, data), tag)
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.5: Real AES-256-GCM via aes-gcm crate
// ═══════════════════════════════════════════════════════════════════════

/// Encrypt data with AES-256-GCM.
///
/// `key` must be 32 bytes. `nonce` must be 12 bytes.
/// Returns (ciphertext, 16-byte authentication tag).
pub fn aes256_gcm_encrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<(Vec<u8>, [u8; 16]), String> {
    use aes_gcm::aead::Payload;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("AES key error: {e}"))?;
    let nonce = Nonce::from_slice(nonce);
    let payload = Payload {
        msg: plaintext,
        aad,
    };
    let ciphertext_with_tag = cipher
        .encrypt(nonce, payload)
        .map_err(|e| format!("AES encrypt error: {e}"))?;

    // aes-gcm appends 16-byte tag to ciphertext
    let tag_start = ciphertext_with_tag.len() - 16;
    let ciphertext = ciphertext_with_tag[..tag_start].to_vec();
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&ciphertext_with_tag[tag_start..]);
    Ok((ciphertext, tag))
}

/// Decrypt data with AES-256-GCM.
///
/// `key` must be 32 bytes. `nonce` must be 12 bytes.
pub fn aes256_gcm_decrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    ciphertext: &[u8],
    tag: &[u8; 16],
    aad: &[u8],
) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::Payload;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("AES key error: {e}"))?;
    let nonce = Nonce::from_slice(nonce);

    // Reconstruct ciphertext + tag as aes-gcm expects
    let mut ct_with_tag = ciphertext.to_vec();
    ct_with_tag.extend_from_slice(tag);

    let payload = Payload {
        msg: &ct_with_tag,
        aad,
    };
    cipher
        .decrypt(nonce, payload)
        .map_err(|e| format!("AES decrypt error: {e}"))
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.7: Real Ed25519 via ed25519-dalek crate
// ═══════════════════════════════════════════════════════════════════════

/// Generate an Ed25519 key pair.
pub fn ed25519_generate() -> Ed25519KeyPair {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let mut secret = [0u8; 64];
    secret[..32].copy_from_slice(&signing_key.to_bytes());
    secret[32..].copy_from_slice(verifying_key.as_bytes());

    Ed25519KeyPair {
        public_key: verifying_key.to_bytes(),
        secret_key: secret,
    }
}

/// Sign data with Ed25519.
pub fn ed25519_sign(secret_key: &[u8; 64], message: &[u8]) -> [u8; 64] {
    use ed25519_dalek::{Signer, SigningKey};

    let mut seed = [0u8; 32];
    seed.copy_from_slice(&secret_key[..32]);
    let signing_key = SigningKey::from_bytes(&seed);
    let signature = signing_key.sign(message);
    signature.to_bytes()
}

/// Verify Ed25519 signature.
pub fn ed25519_verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let Ok(verifying_key) = VerifyingKey::from_bytes(public_key) else {
        return false;
    };
    let sig = Signature::from_bytes(signature);
    verifying_key.verify(message, &sig).is_ok()
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.8: Real Argon2 password hashing
// ═══════════════════════════════════════════════════════════════════════

/// Hash a password with Argon2id. Returns the PHC-formatted hash string.
pub fn argon2_hash(password: &[u8]) -> Result<String, String> {
    use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
    use rand::rngs::OsRng;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password, &salt)
        .map_err(|e| format!("argon2 hash error: {e}"))?;
    Ok(hash.to_string())
}

/// Verify a password against an Argon2id hash string.
pub fn argon2_verify(password: &[u8], hash_str: &str) -> bool {
    use argon2::{Argon2, PasswordVerifier, password_hash::PasswordHash};

    let Ok(parsed_hash) = PasswordHash::new(hash_str) else {
        return false;
    };
    Argon2::default()
        .verify_password(password, &parsed_hash)
        .is_ok()
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.9: Real CSPRNG
// ═══════════════════════════════════════════════════════════════════════

/// Generate cryptographically secure random bytes.
pub fn random_bytes(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut buf = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf
}

// ═══════════════════════════════════════════════════════════════════════
// S2.3-S2.4: AES-128/256 (CBC/GCM)
// ═══════════════════════════════════════════════════════════════════════

/// AES mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AesMode {
    Cbc,
    Gcm,
}

/// AES key size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AesKeySize {
    Aes128,
    Aes256,
}

impl AesKeySize {
    /// Returns key length in bytes.
    pub fn len(self) -> usize {
        match self {
            Self::Aes128 => 16,
            Self::Aes256 => 32,
        }
    }

    /// Returns whether the key size is empty (always false for valid key sizes).
    pub fn is_empty(self) -> bool {
        false
    }
}

/// AES encryption parameters.
#[derive(Debug, Clone)]
pub struct AesParams {
    /// Key.
    pub key: Vec<u8>,
    /// Initialization vector (16 bytes for CBC, 12 bytes for GCM).
    pub iv: Vec<u8>,
    /// Mode.
    pub mode: AesMode,
    /// Additional authenticated data (GCM only).
    pub aad: Option<Vec<u8>>,
}

/// AES encryption result (for GCM includes auth tag).
#[derive(Debug, Clone)]
pub struct AesResult {
    /// Ciphertext.
    pub ciphertext: Vec<u8>,
    /// Authentication tag (GCM only, 16 bytes).
    pub tag: Option<Vec<u8>>,
}

// ═══════════════════════════════════════════════════════════════════════
// S2.5-S2.6: RSA + Ed25519
// ═══════════════════════════════════════════════════════════════════════

/// RSA key size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsaKeySize {
    Rsa2048,
    Rsa4096,
}

impl RsaKeySize {
    /// Returns bit length.
    pub fn bits(self) -> u32 {
        match self {
            Self::Rsa2048 => 2048,
            Self::Rsa4096 => 4096,
        }
    }
}

/// RSA key pair.
#[derive(Debug, Clone)]
pub struct RsaKeyPair {
    /// Public key (DER encoded).
    pub public_key: Vec<u8>,
    /// Private key (DER encoded).
    pub private_key: Vec<u8>,
    /// Key size.
    pub key_size: RsaKeySize,
}

/// Ed25519 key pair (32-byte keys).
#[derive(Debug, Clone)]
pub struct Ed25519KeyPair {
    /// Public key (32 bytes).
    pub public_key: [u8; 32],
    /// Secret key (64 bytes: seed + public).
    pub secret_key: [u8; 64],
}

/// Digital signature.
#[derive(Debug, Clone)]
pub struct Signature {
    /// Raw signature bytes.
    pub bytes: Vec<u8>,
    /// Algorithm used.
    pub algorithm: SignatureAlgorithm,
}

/// Signature algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
    RsaPkcs1Sha256,
    RsaPssSha256,
    Ed25519,
    EcdsaP256,
}

// ═══════════════════════════════════════════════════════════════════════
// S2.7-S2.8: Key Exchange + Password Hashing
// ═══════════════════════════════════════════════════════════════════════

/// X25519 key exchange result.
#[derive(Debug, Clone)]
pub struct X25519Result {
    /// Our public key (32 bytes).
    pub public_key: [u8; 32],
    /// Shared secret (32 bytes).
    pub shared_secret: [u8; 32],
}

/// Password hashing algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordHashAlg {
    Pbkdf2Sha256,
    Argon2id,
}

/// Password hash parameters.
#[derive(Debug, Clone)]
pub struct PasswordHashParams {
    /// Algorithm.
    pub algorithm: PasswordHashAlg,
    /// Iterations (PBKDF2) or time cost (Argon2).
    pub iterations: u32,
    /// Memory cost in KB (Argon2 only).
    pub memory_kb: u32,
    /// Parallelism (Argon2 only).
    pub parallelism: u32,
    /// Salt length in bytes.
    pub salt_len: usize,
    /// Hash output length in bytes.
    pub hash_len: usize,
}

impl Default for PasswordHashParams {
    fn default() -> Self {
        Self {
            algorithm: PasswordHashAlg::Argon2id,
            iterations: 3,
            memory_kb: 65536, // 64MB
            parallelism: 4,
            salt_len: 16,
            hash_len: 32,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.9-S2.10: Base64 + Hex Encoding
// ═══════════════════════════════════════════════════════════════════════

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes to Base64.
pub fn base64_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Decodes Base64 to bytes.
pub fn base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let bytes: Vec<u8> = encoded
        .bytes()
        .filter(|b| *b != b'\n' && *b != b'\r')
        .collect();
    if !bytes.len().is_multiple_of(4) {
        return Err("invalid base64 length".to_string());
    }
    for chunk in bytes.chunks(4) {
        let vals: Result<Vec<u32>, String> = chunk
            .iter()
            .map(|&b| match b {
                b'A'..=b'Z' => Ok((b - b'A') as u32),
                b'a'..=b'z' => Ok((b - b'a' + 26) as u32),
                b'0'..=b'9' => Ok((b - b'0' + 52) as u32),
                b'+' => Ok(62),
                b'/' => Ok(63),
                b'=' => Ok(0),
                _ => Err(format!("invalid base64 character: {}", b as char)),
            })
            .collect();
        let vals = vals?;
        let triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        result.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            result.push((triple & 0xFF) as u8);
        }
    }
    Ok(result)
}

/// Encodes bytes to hex string.
pub fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decodes hex string to bytes.
pub fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("odd hex length".to_string());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| format!("{e}")))
        .collect()
}

/// Constant-time comparison (prevents timing attacks).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Securely zeros memory.
pub fn secure_zero(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        // SAFETY: write_volatile ensures the zero write is not optimized away,
        // preventing sensitive data from lingering in memory.
        unsafe {
            std::ptr::write_volatile(byte, 0);
        }
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

// ═══════════════════════════════════════════════════════════════════════
// S2.11-S2.12: JWT + Certificates
// ═══════════════════════════════════════════════════════════════════════

/// JWT header.
#[derive(Debug, Clone)]
pub struct JwtHeader {
    /// Algorithm (HS256, RS256, ES256, EdDSA).
    pub alg: String,
    /// Token type (always "JWT").
    pub typ: String,
}

/// JWT claims.
#[derive(Debug, Clone)]
pub struct JwtClaims {
    /// Issuer.
    pub iss: Option<String>,
    /// Subject.
    pub sub: Option<String>,
    /// Audience.
    pub aud: Option<String>,
    /// Expiration time (UNIX timestamp).
    pub exp: Option<u64>,
    /// Not before (UNIX timestamp).
    pub nbf: Option<u64>,
    /// Issued at (UNIX timestamp).
    pub iat: Option<u64>,
    /// JWT ID.
    pub jti: Option<String>,
    /// Custom claims.
    pub custom: std::collections::HashMap<String, String>,
}

impl JwtClaims {
    /// Checks if the token is expired.
    pub fn is_expired(&self, now: u64) -> bool {
        self.exp.is_some_and(|exp| now > exp)
    }

    /// Checks if the token is not yet valid.
    pub fn is_before_nbf(&self, now: u64) -> bool {
        self.nbf.is_some_and(|nbf| now < nbf)
    }
}

/// X.509 certificate info.
#[derive(Debug, Clone)]
pub struct X509Info {
    /// Subject common name.
    pub subject_cn: String,
    /// Issuer common name.
    pub issuer_cn: String,
    /// Not before (UNIX timestamp).
    pub not_before: u64,
    /// Not after (UNIX timestamp).
    pub not_after: u64,
    /// Serial number (hex).
    pub serial: String,
    /// Subject alternative names.
    pub san: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s2_1_hash_algorithm_sizes() {
        assert_eq!(HashAlgorithm::Sha256.output_len(), 32);
        assert_eq!(HashAlgorithm::Sha384.output_len(), 48);
        assert_eq!(HashAlgorithm::Sha512.output_len(), 64);
        assert_eq!(HashAlgorithm::Sha256.block_size(), 64);
    }

    #[test]
    fn s2_1_digest_hex() {
        let digest = Digest {
            bytes: vec![0xab, 0xcd, 0xef],
            algorithm: HashAlgorithm::Sha256,
        };
        assert_eq!(digest.hex(), "abcdef");
        assert_eq!(format!("{digest}"), "abcdef");
    }

    #[test]
    fn s2_3_aes_key_sizes() {
        assert_eq!(AesKeySize::Aes128.len(), 16);
        assert_eq!(AesKeySize::Aes256.len(), 32);
    }

    #[test]
    fn s2_5_rsa_key_sizes() {
        assert_eq!(RsaKeySize::Rsa2048.bits(), 2048);
        assert_eq!(RsaKeySize::Rsa4096.bits(), 4096);
    }

    #[test]
    fn s2_7_password_hash_defaults() {
        let params = PasswordHashParams::default();
        assert_eq!(params.algorithm, PasswordHashAlg::Argon2id);
        assert_eq!(params.memory_kb, 65536);
        assert_eq!(params.hash_len, 32);
    }

    #[test]
    fn s2_9_base64_roundtrip() {
        let data = b"Hello, Fajar Lang!";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn s2_9_base64_known() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
    }

    #[test]
    fn s2_10_hex_roundtrip() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let encoded = hex_encode(&data);
        assert_eq!(encoded, "deadbeef");
        let decoded = hex_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn s2_10_hex_decode_error() {
        assert!(hex_decode("abc").is_err()); // odd length
        assert!(hex_decode("zzzz").is_err()); // invalid hex
    }

    #[test]
    fn s2_14_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hi", b"hello"));
    }

    #[test]
    fn s2_15_secure_zero() {
        let mut buf = vec![0xAA; 32];
        secure_zero(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn s2_11_jwt_expiry() {
        let claims = JwtClaims {
            iss: Some("fajarlang.dev".to_string()),
            sub: Some("user123".to_string()),
            aud: None,
            exp: Some(1000),
            nbf: Some(500),
            iat: Some(500),
            jti: None,
            custom: std::collections::HashMap::new(),
        };
        assert!(!claims.is_expired(900));
        assert!(claims.is_expired(1001));
        assert!(claims.is_before_nbf(400));
        assert!(!claims.is_before_nbf(600));
    }

    #[test]
    fn s2_1_sha256_constants() {
        assert_eq!(SHA256_H.len(), 8);
        assert_eq!(SHA256_K.len(), 64);
        assert_eq!(SHA256_H[0], 0x6a09e667);
    }

    // ═══════════════════════════════════════════════════════════════════
    // V8 GC1.10: Real crypto integration tests (NIST test vectors)
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn gc1_sha256_empty() {
        let d = sha256(b"");
        assert_eq!(
            d.hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn gc1_sha256_abc() {
        let d = sha256(b"abc");
        assert_eq!(
            d.hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn gc1_sha384_abc() {
        let d = sha384(b"abc");
        assert_eq!(d.bytes.len(), 48);
        assert_eq!(d.algorithm, HashAlgorithm::Sha384);
    }

    #[test]
    fn gc1_sha512_abc() {
        let d = sha512(b"abc");
        assert_eq!(d.bytes.len(), 64);
        assert_eq!(d.algorithm, HashAlgorithm::Sha512);
    }

    #[test]
    fn gc1_hash_dispatch() {
        let d1 = hash(HashAlgorithm::Sha256, b"test");
        let d2 = sha256(b"test");
        assert_eq!(d1.bytes, d2.bytes);
    }

    #[test]
    fn gc1_hmac_sha256_rfc4231() {
        // RFC 4231 Test Case 2
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let mac = hmac_sha256(key, data);
        assert_eq!(
            hex_encode(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn gc1_hmac_verify() {
        let key = b"secret";
        let data = b"message";
        let mac = hmac_sha256(key, data);
        assert!(hmac_sha256_verify(key, data, &mac));
        assert!(!hmac_sha256_verify(key, b"wrong", &mac));
    }

    #[test]
    fn gc1_aes256_gcm_roundtrip() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 12];
        let plaintext = b"Hello, Fajar Lang crypto!";
        let aad = b"additional data";

        let (ct, tag) = aes256_gcm_encrypt(&key, &nonce, plaintext, aad).unwrap();
        assert_ne!(ct, plaintext.to_vec()); // ciphertext differs

        let decrypted = aes256_gcm_decrypt(&key, &nonce, &ct, &tag, aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn gc1_aes256_gcm_tamper_detection() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 12];
        let (ct, mut tag) = aes256_gcm_encrypt(&key, &nonce, b"data", b"").unwrap();
        tag[0] ^= 1; // tamper with tag
        assert!(aes256_gcm_decrypt(&key, &nonce, &ct, &tag, b"").is_err());
    }

    #[test]
    fn gc1_ed25519_sign_verify() {
        let kp = ed25519_generate();
        let msg = b"Fajar Lang is making history in IT";
        let sig = ed25519_sign(&kp.secret_key, msg);
        assert!(ed25519_verify(&kp.public_key, msg, &sig));
        assert!(!ed25519_verify(&kp.public_key, b"wrong message", &sig));
    }

    #[test]
    fn gc1_ed25519_different_keys() {
        let kp1 = ed25519_generate();
        let kp2 = ed25519_generate();
        let msg = b"test";
        let sig = ed25519_sign(&kp1.secret_key, msg);
        assert!(!ed25519_verify(&kp2.public_key, msg, &sig)); // wrong key
    }

    #[test]
    fn gc1_argon2_hash_verify() {
        let password = b"fajar-lang-2026";
        let hash_str = argon2_hash(password).unwrap();
        assert!(hash_str.starts_with("$argon2"));
        assert!(argon2_verify(password, &hash_str));
        assert!(!argon2_verify(b"wrong-password", &hash_str));
    }

    #[test]
    fn gc1_random_bytes_length() {
        assert_eq!(random_bytes(0).len(), 0);
        assert_eq!(random_bytes(32).len(), 32);
        assert_eq!(random_bytes(64).len(), 64);
    }

    #[test]
    fn gc1_random_bytes_different() {
        let a = random_bytes(32);
        let b = random_bytes(32);
        assert_ne!(a, b); // astronomically unlikely to be equal
    }

    #[test]
    fn gc1_full_crypto_pipeline() {
        // Generate keypair
        let kp = ed25519_generate();

        // Hash some data
        let data = b"important document";
        let digest = sha256(data);

        // Sign the hash
        let sig = ed25519_sign(&kp.secret_key, &digest.bytes);

        // Verify
        assert!(ed25519_verify(&kp.public_key, &digest.bytes, &sig));

        // Encrypt with AES
        let key = sha256(b"encryption-key");
        let mut aes_key = [0u8; 32];
        aes_key.copy_from_slice(&key.bytes);
        let nonce = [0u8; 12];
        let (ct, tag) = aes256_gcm_encrypt(&aes_key, &nonce, data, b"").unwrap();
        let pt = aes256_gcm_decrypt(&aes_key, &nonce, &ct, &tag, b"").unwrap();
        assert_eq!(pt, data);
    }
}
