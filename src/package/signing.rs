//! Sigstore-compatible package signing for Fajar Lang.
//!
//! Provides OIDC-based keyless signing via simulated Fulcio certificate
//! issuance and Rekor transparency log submission. All cryptographic
//! operations are simulation stubs — no external dependencies required.

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during package signing operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SigningError {
    /// OIDC authentication failed.
    #[error("OIDC authentication failed: {0}")]
    OidcAuthFailed(String),

    /// Certificate request to Fulcio failed.
    #[error("Fulcio certificate request failed: {0}")]
    CertificateRequestFailed(String),

    /// Signing operation failed.
    #[error("signing failed: {0}")]
    SignFailed(String),

    /// Rekor transparency log submission failed.
    #[error("Rekor submission failed: {0}")]
    RekorSubmitFailed(String),

    /// Bundle serialization/deserialization failed.
    #[error("bundle error: {0}")]
    BundleError(String),

    /// Invalid input parameter.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

// ═══════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for the Sigstore signing pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningConfig {
    /// OIDC identity provider URL (e.g., `https://accounts.google.com`).
    pub oidc_provider: String,
    /// Fulcio certificate authority URL.
    pub fulcio_url: String,
    /// Rekor transparency log URL.
    pub rekor_url: String,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            oidc_provider: "https://oauth2.sigstore.dev/auth".to_string(),
            fulcio_url: "https://fulcio.sigstore.dev".to_string(),
            rekor_url: "https://rekor.sigstore.dev".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OIDC Token
// ═══════════════════════════════════════════════════════════════════════

/// An OIDC identity token obtained from an identity provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcToken {
    /// Identity claim — email address or repository URI.
    pub identity: String,
    /// The raw JWT token string.
    pub token_string: String,
    /// Expiration timestamp (Unix epoch seconds).
    pub expires_at: u64,
}

/// Authenticates with an OIDC provider and obtains an identity token.
///
/// This is a simulation stub — no actual network request is made.
/// Returns a synthetic token for the given provider.
pub fn oidc_authenticate(provider: &str) -> Result<OidcToken, SigningError> {
    if provider.is_empty() {
        return Err(SigningError::OidcAuthFailed(
            "provider URL cannot be empty".to_string(),
        ));
    }

    let identity = format!("developer@{}", extract_domain(provider));
    let token_string = format!("eyJhbGciOiJSUzI1NiJ9.sim.{}", simple_hash(provider));

    Ok(OidcToken {
        identity,
        token_string,
        expires_at: current_timestamp() + 3600,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Signing Certificate
// ═══════════════════════════════════════════════════════════════════════

/// A short-lived signing certificate issued by Fulcio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningCertificate {
    /// PEM-encoded X.509 certificate.
    pub pem: String,
    /// Simulated private key bytes (NOT a real key).
    pub private_key: Vec<u8>,
    /// Certificate expiration timestamp (Unix epoch seconds).
    pub not_after: u64,
    /// Subject identity embedded in the certificate.
    pub subject: String,
}

/// Requests a short-lived signing certificate from Fulcio.
///
/// Simulation stub — generates a synthetic PEM certificate
/// bound to the OIDC token's identity.
pub fn request_certificate(oidc_token: &OidcToken) -> Result<SigningCertificate, SigningError> {
    if oidc_token.token_string.is_empty() {
        return Err(SigningError::CertificateRequestFailed(
            "OIDC token string is empty".to_string(),
        ));
    }
    if oidc_token.identity.is_empty() {
        return Err(SigningError::CertificateRequestFailed(
            "OIDC identity is empty".to_string(),
        ));
    }

    let pem = generate_simulated_pem(&oidc_token.identity);
    let private_key = generate_simulated_key(&oidc_token.token_string);

    Ok(SigningCertificate {
        pem,
        private_key,
        not_after: current_timestamp() + 600,
        subject: oidc_token.identity.clone(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Signature
// ═══════════════════════════════════════════════════════════════════════

/// A cryptographic signature over a package artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Signing algorithm identifier (e.g., `ECDSA-SHA256`).
    pub algorithm: String,
    /// Raw signature bytes (simulated).
    pub bytes: Vec<u8>,
    /// Identity of the signer from the certificate.
    pub signer_identity: String,
}

/// Signs a package hash using the provided signing certificate.
///
/// Simulation stub — produces a deterministic signature from the
/// package hash and certificate private key.
pub fn sign_package(
    package_hash: &str,
    cert: &SigningCertificate,
) -> Result<Signature, SigningError> {
    if package_hash.is_empty() {
        return Err(SigningError::SignFailed(
            "package hash cannot be empty".to_string(),
        ));
    }
    if cert.private_key.is_empty() {
        return Err(SigningError::SignFailed(
            "certificate private key is empty".to_string(),
        ));
    }

    let sig_input = format!("{}{}", package_hash, cert.subject);
    let sig_bytes = deterministic_sign(sig_input.as_bytes(), &cert.private_key);

    Ok(Signature {
        algorithm: "ECDSA-SHA256".to_string(),
        bytes: sig_bytes,
        signer_identity: cert.subject.clone(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Rekor Transparency Log
// ═══════════════════════════════════════════════════════════════════════

/// An entry in the Rekor transparency log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RekorEntry {
    /// Unique identifier for the log entry.
    pub uuid: String,
    /// Monotonic index in the transparency log.
    pub log_index: u64,
    /// Timestamp when the entry was integrated into the log.
    pub integrated_time: u64,
}

/// Submits a signing event to the Rekor transparency log.
///
/// Simulation stub — generates a deterministic log entry
/// from the signature, certificate, and artifact hash.
pub fn submit_to_rekor(
    signature: &Signature,
    cert: &SigningCertificate,
    artifact_hash: &str,
) -> Result<RekorEntry, SigningError> {
    if artifact_hash.is_empty() {
        return Err(SigningError::RekorSubmitFailed(
            "artifact hash cannot be empty".to_string(),
        ));
    }
    if signature.bytes.is_empty() {
        return Err(SigningError::RekorSubmitFailed(
            "signature bytes cannot be empty".to_string(),
        ));
    }

    let uuid_input = format!("{}{}{}", artifact_hash, cert.subject, signature.algorithm);
    let uuid = format!("{:016x}", simple_hash(&uuid_input));
    let log_index = simple_hash(artifact_hash) % 1_000_000;

    Ok(RekorEntry {
        uuid,
        log_index,
        integrated_time: current_timestamp(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Signature Bundle
// ═══════════════════════════════════════════════════════════════════════

/// A complete signature bundle containing all verification artifacts.
///
/// Bundles the signing certificate, signature, and Rekor log reference
/// into a single portable JSON-serializable structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FjSignatureBundle {
    /// PEM-encoded signing certificate.
    pub certificate_pem: String,
    /// Hex-encoded signature bytes.
    pub signature: String,
    /// Rekor transparency log entry UUID.
    pub rekor_log_id: String,
    /// Rekor transparency log index.
    pub rekor_log_index: u64,
}

impl FjSignatureBundle {
    /// Creates a new signature bundle from signing artifacts.
    pub fn new(cert: &SigningCertificate, sig: &Signature, rekor: &RekorEntry) -> Self {
        Self {
            certificate_pem: cert.pem.clone(),
            signature: hex_encode(&sig.bytes),
            rekor_log_id: rekor.uuid.clone(),
            rekor_log_index: rekor.log_index,
        }
    }

    /// Serializes the bundle to a JSON string.
    pub fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"certificate_pem\": {},\n",
                "  \"signature\": \"{}\",\n",
                "  \"rekor_log_id\": \"{}\",\n",
                "  \"rekor_log_index\": {}\n",
                "}}"
            ),
            json_escape_string(&self.certificate_pem),
            self.signature,
            self.rekor_log_id,
            self.rekor_log_index,
        )
    }

    /// Deserializes a bundle from a JSON string.
    pub fn from_json(s: &str) -> Result<Self, SigningError> {
        let fields = parse_json_fields(s)
            .map_err(|e| SigningError::BundleError(format!("JSON parse error: {e}")))?;

        let certificate_pem = fields
            .get("certificate_pem")
            .ok_or_else(|| SigningError::BundleError("missing certificate_pem".to_string()))?
            .clone();

        let signature = fields
            .get("signature")
            .ok_or_else(|| SigningError::BundleError("missing signature".to_string()))?
            .clone();

        let rekor_log_id = fields
            .get("rekor_log_id")
            .ok_or_else(|| SigningError::BundleError("missing rekor_log_id".to_string()))?
            .clone();

        let rekor_log_index_str = fields
            .get("rekor_log_index")
            .ok_or_else(|| SigningError::BundleError("missing rekor_log_index".to_string()))?;

        let rekor_log_index = rekor_log_index_str
            .parse::<u64>()
            .map_err(|_| SigningError::BundleError("invalid rekor_log_index".to_string()))?;

        Ok(Self {
            certificate_pem,
            signature,
            rekor_log_id,
            rekor_log_index,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 PR4.4-PR4.5: Simple Content-Based Signing
// ═══════════════════════════════════════════════════════════════════════

/// V14 PR4.4: Generate a package signature using a keyed hash.
///
/// Produces a deterministic `fjsig-` prefixed HMAC-SHA256 signature from
/// content bytes and a secret key. Uses cryptographic hashing for tamper
/// detection, suitable for registry package verification.
pub fn sign_package_content(content: &[u8], secret_key: &str) -> String {
    use sha2::{Digest, Sha256};

    // HMAC-SHA256: H(key || content || key) — simplified HMAC construction.
    let mut hasher = Sha256::new();
    hasher.update(secret_key.as_bytes());
    hasher.update(content);
    hasher.update(secret_key.as_bytes());
    let hash = hasher.finalize();
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    format!("fjsig-{hex}")
}

/// Verify a package signature against content and key using HMAC-SHA256.
///
/// Returns true if the signature matches the expected HMAC of the content.
pub fn verify_package_signature(content: &[u8], secret_key: &str, signature: &str) -> bool {
    let expected = sign_package_content(content, secret_key);
    // Constant-time comparison to prevent timing attacks.
    if expected.len() != signature.len() {
        return false;
    }
    expected
        .bytes()
        .zip(signature.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

// ═══════════════════════════════════════════════════════════════════════
// Internal Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Extracts a domain name from a URL for identity generation.
fn extract_domain(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped.split('/').next().unwrap_or("unknown").to_string()
}

/// Returns a simulated current timestamp (fixed for determinism in tests).
fn current_timestamp() -> u64 {
    // Use a fixed base for deterministic simulation
    1_700_000_000
}

/// Produces a simple deterministic hash from a string (FNV-1a inspired).
fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Generates a simulated PEM certificate block.
fn generate_simulated_pem(identity: &str) -> String {
    let body = hex_encode(identity.as_bytes());
    format!("-----BEGIN CERTIFICATE-----\n{body}\n-----END CERTIFICATE-----")
}

/// Generates simulated private key bytes from a token string.
fn generate_simulated_key(token: &str) -> Vec<u8> {
    let h = simple_hash(token);
    h.to_le_bytes().to_vec()
}

/// Produces a deterministic signature over data using a simulated key.
fn deterministic_sign(data: &[u8], key: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(32);
    for (i, &b) in data.iter().enumerate() {
        let k = key[i % key.len()];
        result.push(b ^ k);
    }
    // Pad to at least 32 bytes
    while result.len() < 32 {
        result.push(result.len() as u8);
    }
    result
}

/// Hex-encodes a byte slice.
fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

/// Escapes a string for JSON output.
fn json_escape_string(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

/// Parses a flat JSON object into key-value string pairs.
///
/// Supports string and numeric values only — sufficient for bundle format.
fn parse_json_fields(s: &str) -> Result<HashMap<String, String>, String> {
    let trimmed = s.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err("not a JSON object".to_string());
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    let mut fields = HashMap::new();

    for part in split_json_fields(inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = parse_json_kv(part)?;
        fields.insert(key, value);
    }

    Ok(fields)
}

/// Splits JSON fields respecting quoted strings.
fn split_json_fields(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape_next = false;

    for ch in s.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            current.push(ch);
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            current.push(ch);
            continue;
        }
        if ch == ',' && !in_string {
            parts.push(current.clone());
            current.clear();
            continue;
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Parses a single `"key": value` pair.
fn parse_json_kv(s: &str) -> Result<(String, String), String> {
    let colon_pos = find_kv_colon(s).ok_or_else(|| format!("missing ':' in field: {s}"))?;

    let key_part = s[..colon_pos].trim();
    let val_part = s[colon_pos + 1..].trim();

    let key = unquote_json_string(key_part)?;
    let value = if val_part.starts_with('"') {
        unescape_json_string(val_part)?
    } else {
        val_part.to_string()
    };

    Ok((key, value))
}

/// Finds the colon separating key from value, skipping colons inside strings.
fn find_kv_colon(s: &str) -> Option<usize> {
    let mut in_string = false;
    let mut escape_next = false;
    for (i, ch) in s.chars().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == ':' && !in_string {
            return Some(i);
        }
    }
    None
}

/// Removes quotes from a JSON string value.
fn unquote_json_string(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err(format!("expected quoted string: {s}"));
    }
    Ok(trimmed[1..trimmed.len() - 1].to_string())
}

/// Unescapes a JSON string value (handles \\n, \\t, \\", \\\\).
fn unescape_json_string(s: &str) -> Result<String, String> {
    let inner = unquote_json_string(s)?;
    let mut result = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s21_1_signing_config_defaults() {
        let config = SigningConfig::default();
        assert!(config.oidc_provider.contains("sigstore"));
        assert!(config.fulcio_url.contains("fulcio"));
        assert!(config.rekor_url.contains("rekor"));
    }

    #[test]
    fn s21_2_oidc_authenticate_success() {
        let token = oidc_authenticate("https://accounts.google.com").unwrap();
        assert!(token.identity.contains("accounts.google.com"));
        assert!(!token.token_string.is_empty());
        assert!(token.expires_at > 0);
    }

    #[test]
    fn s21_3_oidc_authenticate_empty_provider_fails() {
        let result = oidc_authenticate("");
        assert!(result.is_err());
        match result {
            Err(SigningError::OidcAuthFailed(msg)) => {
                assert!(msg.contains("empty"));
            }
            other => panic!("expected OidcAuthFailed, got {other:?}"),
        }
    }

    #[test]
    fn s21_4_request_certificate_success() {
        let token = oidc_authenticate("https://accounts.google.com").unwrap();
        let cert = request_certificate(&token).unwrap();
        assert!(cert.pem.contains("BEGIN CERTIFICATE"));
        assert!(cert.pem.contains("END CERTIFICATE"));
        assert!(!cert.private_key.is_empty());
        assert_eq!(cert.subject, token.identity);
        assert!(cert.not_after > 0);
    }

    #[test]
    fn s21_5_sign_package_success() {
        let token = oidc_authenticate("https://accounts.google.com").unwrap();
        let cert = request_certificate(&token).unwrap();
        let sig = sign_package("sha256:abcdef1234567890", &cert).unwrap();
        assert_eq!(sig.algorithm, "ECDSA-SHA256");
        assert!(!sig.bytes.is_empty());
        assert_eq!(sig.signer_identity, cert.subject);
    }

    #[test]
    fn s21_6_sign_package_empty_hash_fails() {
        let token = oidc_authenticate("https://example.com").unwrap();
        let cert = request_certificate(&token).unwrap();
        let result = sign_package("", &cert);
        assert!(result.is_err());
    }

    #[test]
    fn s21_7_submit_to_rekor_success() {
        let token = oidc_authenticate("https://example.com").unwrap();
        let cert = request_certificate(&token).unwrap();
        let sig = sign_package("sha256:abcdef", &cert).unwrap();
        let entry = submit_to_rekor(&sig, &cert, "sha256:abcdef").unwrap();
        assert!(!entry.uuid.is_empty());
        assert!(entry.integrated_time > 0);
    }

    #[test]
    fn s21_8_signature_bundle_roundtrip() {
        let token = oidc_authenticate("https://example.com").unwrap();
        let cert = request_certificate(&token).unwrap();
        let sig = sign_package("sha256:abc123", &cert).unwrap();
        let rekor = submit_to_rekor(&sig, &cert, "sha256:abc123").unwrap();

        let bundle = FjSignatureBundle::new(&cert, &sig, &rekor);
        let json = bundle.to_json();
        let restored = FjSignatureBundle::from_json(&json).unwrap();

        assert_eq!(bundle.certificate_pem, restored.certificate_pem);
        assert_eq!(bundle.signature, restored.signature);
        assert_eq!(bundle.rekor_log_id, restored.rekor_log_id);
        assert_eq!(bundle.rekor_log_index, restored.rekor_log_index);
    }

    #[test]
    fn s21_9_bundle_from_invalid_json_fails() {
        let result = FjSignatureBundle::from_json("not json");
        assert!(result.is_err());

        let result2 = FjSignatureBundle::from_json("{}");
        assert!(result2.is_err());
    }

    #[test]
    fn s21_10_full_signing_pipeline() {
        let config = SigningConfig::default();
        let token = oidc_authenticate(&config.oidc_provider).unwrap();
        let cert = request_certificate(&token).unwrap();
        let hash = "sha256:deadbeef01234567";
        let sig = sign_package(hash, &cert).unwrap();
        let rekor = submit_to_rekor(&sig, &cert, hash).unwrap();
        let bundle = FjSignatureBundle::new(&cert, &sig, &rekor);

        assert!(bundle.certificate_pem.contains("BEGIN CERTIFICATE"));
        assert!(!bundle.signature.is_empty());
        assert!(!bundle.rekor_log_id.is_empty());
    }

    // V14 PR4.4: Content-based signing
    #[test]
    fn v14_pr4_4_sign_package() {
        let sig = sign_package_content(b"hello world", "my-secret-key");
        assert!(sig.starts_with("fjsig-"));
        assert_eq!(sig.len(), 70); // "fjsig-" (6) + 64 hex chars (SHA-256)
        // Deterministic: same input → same signature
        let sig2 = sign_package_content(b"hello world", "my-secret-key");
        assert_eq!(sig, sig2);
        // Different key → different signature
        let sig3 = sign_package_content(b"hello world", "other-key");
        assert_ne!(sig, sig3);
    }

    // V14 PR4.5: Signature verification
    #[test]
    fn v14_pr4_5_verify_signature() {
        let content = b"package data";
        let key = "secret";
        let sig = sign_package_content(content, key);
        assert!(verify_package_signature(content, key, &sig));
        assert!(!verify_package_signature(content, "wrong-key", &sig));
        assert!(!verify_package_signature(b"other data", key, &sig));
    }
}
