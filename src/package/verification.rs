//! Package verification for Fajar Lang.
//!
//! Verifies package signatures against Sigstore-compatible signing
//! bundles, including certificate chain validation, signature verification,
//! Rekor log verification, and signer identity checks.
//! All cryptographic operations are simulation stubs.

use std::collections::HashMap;

use super::signing::{FjSignatureBundle, SigningError};

// ═══════════════════════════════════════════════════════════════════════
// Verification Result
// ═══════════════════════════════════════════════════════════════════════

/// The outcome of verifying a package's signature bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationResult {
    /// Whether the package signature is valid.
    pub valid: bool,
    /// The identity of the signer (from the certificate).
    pub signer_identity: String,
    /// Timestamp when the signature was recorded.
    pub timestamp: u64,
    /// Non-fatal warnings encountered during verification.
    pub warnings: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// Verification Policy
// ═══════════════════════════════════════════════════════════════════════

/// Policy controlling which packages are accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPolicy {
    /// Whether signatures are required for all packages.
    pub require_signatures: bool,
    /// List of trusted publisher identities (email or repo URI).
    pub trusted_publishers: Vec<String>,
}

impl Default for VerificationPolicy {
    fn default() -> Self {
        Self {
            require_signatures: true,
            trusted_publishers: Vec::new(),
        }
    }
}

impl VerificationPolicy {
    /// Checks whether a signer identity is trusted under this policy.
    pub fn is_trusted(&self, identity: &str) -> bool {
        if self.trusted_publishers.is_empty() {
            return true;
        }
        self.trusted_publishers.iter().any(|t| t == identity)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Signature Cache
// ═══════════════════════════════════════════════════════════════════════

/// A cached verification result for a previously verified package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedVerification {
    /// The verification result.
    pub result: VerificationResult,
    /// Timestamp when the cache entry was created.
    pub cached_at: u64,
}

/// In-memory cache of package verification results.
#[derive(Debug, Clone)]
pub struct SignatureCache {
    /// Directory where cache files would be stored (simulation).
    pub cache_dir: String,
    /// Map of package ID to cached verification.
    pub verified: HashMap<String, CachedVerification>,
}

impl SignatureCache {
    /// Creates a new signature cache backed by the given directory.
    pub fn new(cache_dir: &str) -> Self {
        Self {
            cache_dir: cache_dir.to_string(),
            verified: HashMap::new(),
        }
    }

    /// Looks up a cached verification result for a package.
    ///
    /// Returns `None` if the package has not been verified or
    /// the cache entry has expired (older than 24 hours).
    pub fn check_cache(&self, package_id: &str) -> Option<VerificationResult> {
        self.verified.get(package_id).and_then(|entry| {
            let now = current_timestamp();
            let max_age = 86_400; // 24 hours
            if now.saturating_sub(entry.cached_at) < max_age {
                Some(entry.result.clone())
            } else {
                None
            }
        })
    }

    /// Stores a verification result in the cache.
    pub fn store_cache(&mut self, package_id: &str, result: VerificationResult) {
        self.verified.insert(
            package_id.to_string(),
            CachedVerification {
                result,
                cached_at: current_timestamp(),
            },
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Core Verification Functions
// ═══════════════════════════════════════════════════════════════════════

/// Verifies a package against its signature bundle.
///
/// Runs the full verification pipeline: certificate chain, signature,
/// Rekor log, and assembles a `VerificationResult`.
pub fn verify_package(
    package_hash: &str,
    bundle: &FjSignatureBundle,
) -> Result<VerificationResult, SigningError> {
    if package_hash.is_empty() {
        return Err(SigningError::InvalidInput(
            "package hash cannot be empty".to_string(),
        ));
    }

    let mut warnings = Vec::new();

    let cert_valid = verify_certificate_chain(&bundle.certificate_pem, "sigstore-root-ca")?;
    if !cert_valid {
        return Ok(failed_result("certificate chain validation failed"));
    }

    let sig_valid = verify_signature(package_hash, &bundle.signature, &bundle.certificate_pem)?;
    if !sig_valid {
        return Ok(failed_result("signature verification failed"));
    }

    let rekor_valid = verify_rekor_entry(&bundle.rekor_log_id)?;
    if !rekor_valid {
        warnings.push("Rekor log entry could not be verified".to_string());
    }

    let identity = extract_identity_from_pem(&bundle.certificate_pem);

    Ok(VerificationResult {
        valid: true,
        signer_identity: identity,
        timestamp: current_timestamp(),
        warnings,
    })
}

/// Constructs a failed verification result with a single warning.
fn failed_result(reason: &str) -> VerificationResult {
    VerificationResult {
        valid: false,
        signer_identity: String::new(),
        timestamp: 0,
        warnings: vec![reason.to_string()],
    }
}

/// Verifies the certificate chain from a leaf certificate to a root CA.
///
/// Simulation stub — checks that the PEM is well-formed and the
/// root CA identifier is non-empty.
pub fn verify_certificate_chain(cert_pem: &str, root_ca: &str) -> Result<bool, SigningError> {
    if cert_pem.is_empty() {
        return Err(SigningError::InvalidInput(
            "certificate PEM cannot be empty".to_string(),
        ));
    }
    if root_ca.is_empty() {
        return Err(SigningError::InvalidInput(
            "root CA identifier cannot be empty".to_string(),
        ));
    }

    let has_begin = cert_pem.contains("BEGIN CERTIFICATE");
    let has_end = cert_pem.contains("END CERTIFICATE");

    Ok(has_begin && has_end)
}

/// Verifies a signature against a package hash and certificate.
///
/// Simulation stub — checks that the signature hex string is
/// non-empty and at least 16 hex characters.
pub fn verify_signature(
    package_hash: &str,
    signature_hex: &str,
    cert_pem: &str,
) -> Result<bool, SigningError> {
    if package_hash.is_empty() || signature_hex.is_empty() || cert_pem.is_empty() {
        return Err(SigningError::InvalidInput(
            "all verification inputs must be non-empty".to_string(),
        ));
    }

    // Simulated verification: check signature length and hex validity
    let is_valid_hex = signature_hex.chars().all(|c| c.is_ascii_hexdigit());
    let sufficient_length = signature_hex.len() >= 16;

    Ok(is_valid_hex && sufficient_length)
}

/// Verifies that a Rekor transparency log entry exists.
///
/// Simulation stub — checks that the entry UUID is a valid hex string.
pub fn verify_rekor_entry(entry_uuid: &str) -> Result<bool, SigningError> {
    if entry_uuid.is_empty() {
        return Err(SigningError::InvalidInput(
            "Rekor entry UUID cannot be empty".to_string(),
        ));
    }

    let is_valid = entry_uuid.chars().all(|c| c.is_ascii_hexdigit());
    Ok(is_valid)
}

/// Verifies that a certificate's subject matches an expected identity.
///
/// Simulation stub — extracts the subject from PEM and compares.
pub fn verify_signer_identity(
    cert_pem: &str,
    expected_identity: &str,
) -> Result<bool, SigningError> {
    if cert_pem.is_empty() {
        return Err(SigningError::InvalidInput(
            "certificate PEM cannot be empty".to_string(),
        ));
    }
    if expected_identity.is_empty() {
        return Err(SigningError::InvalidInput(
            "expected identity cannot be empty".to_string(),
        ));
    }

    let actual = extract_identity_from_pem(cert_pem);
    Ok(actual == expected_identity)
}

// ═══════════════════════════════════════════════════════════════════════
// Internal Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Returns a simulated current timestamp.
fn current_timestamp() -> u64 {
    1_700_000_000
}

/// Extracts a signer identity from a simulated PEM certificate.
///
/// Our simulated PEM encodes the identity as hex between the markers.
fn extract_identity_from_pem(pem: &str) -> String {
    let start_marker = "-----BEGIN CERTIFICATE-----\n";
    let end_marker = "\n-----END CERTIFICATE-----";

    let start = match pem.find(start_marker) {
        Some(pos) => pos + start_marker.len(),
        None => return "unknown".to_string(),
    };
    let end = match pem.find(end_marker) {
        Some(pos) => pos,
        None => return "unknown".to_string(),
    };

    if start >= end {
        return "unknown".to_string();
    }

    let hex_body = &pem[start..end];
    hex_decode_to_string(hex_body)
}

/// Decodes a hex string to a UTF-8 string, returning "unknown" on failure.
fn hex_decode_to_string(hex: &str) -> String {
    if !hex.len().is_multiple_of(2) {
        return "unknown".to_string();
    }

    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let mut i = 0;
    while i + 2 <= hex.len() {
        match u8::from_str_radix(&hex[i..i + 2], 16) {
            Ok(b) => bytes.push(b),
            Err(_) => return "unknown".to_string(),
        }
        i += 2;
    }

    String::from_utf8(bytes).unwrap_or_else(|_| "unknown".to_string())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::super::signing;
    use super::*;

    /// Helper: creates a full signing bundle for testing.
    fn make_test_bundle(hash: &str) -> FjSignatureBundle {
        let token = signing::oidc_authenticate("https://example.com").unwrap();
        let cert = signing::request_certificate(&token).unwrap();
        let sig = signing::sign_package(hash, &cert).unwrap();
        let rekor = signing::submit_to_rekor(&sig, &cert, hash).unwrap();
        FjSignatureBundle::new(&cert, &sig, &rekor)
    }

    #[test]
    fn s22_1_verify_package_success() {
        let hash = "sha256:abcdef1234567890";
        let bundle = make_test_bundle(hash);
        let result = verify_package(hash, &bundle).unwrap();
        assert!(result.valid);
        assert!(!result.signer_identity.is_empty());
    }

    #[test]
    fn s22_2_verify_package_empty_hash_fails() {
        let bundle = make_test_bundle("sha256:abc");
        let result = verify_package("", &bundle);
        assert!(result.is_err());
    }

    #[test]
    fn s22_3_verify_certificate_chain_valid() {
        let pem = "-----BEGIN CERTIFICATE-----\nabcdef\n-----END CERTIFICATE-----";
        assert!(verify_certificate_chain(pem, "root-ca").unwrap());
    }

    #[test]
    fn s22_4_verify_certificate_chain_invalid_pem() {
        assert!(!verify_certificate_chain("not a certificate", "root-ca").unwrap());
    }

    #[test]
    fn s22_5_verify_signature_valid() {
        let hex_sig = "abcdef0123456789abcdef0123456789";
        let pem = "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----";
        assert!(verify_signature("sha256:abc", hex_sig, pem).unwrap());
    }

    #[test]
    fn s22_6_verify_signature_too_short() {
        let pem = "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----";
        assert!(!verify_signature("sha256:abc", "abcd", pem).unwrap());
    }

    #[test]
    fn s22_7_verify_rekor_entry_valid() {
        assert!(verify_rekor_entry("0123456789abcdef").unwrap());
    }

    #[test]
    fn s22_8_verify_signer_identity_match() {
        let token = signing::oidc_authenticate("https://example.com").unwrap();
        let cert = signing::request_certificate(&token).unwrap();
        let identity = &token.identity;
        assert!(verify_signer_identity(&cert.pem, identity).unwrap());
    }

    #[test]
    fn s22_9_verification_policy_trusted() {
        let policy = VerificationPolicy {
            require_signatures: true,
            trusted_publishers: vec!["dev@example.com".to_string()],
        };
        assert!(policy.is_trusted("dev@example.com"));
        assert!(!policy.is_trusted("attacker@evil.com"));

        // Empty trusted list trusts everyone
        let open_policy = VerificationPolicy::default();
        assert!(open_policy.is_trusted("anyone@anywhere.com"));
    }

    #[test]
    fn s22_10_signature_cache_store_and_retrieve() {
        let mut cache = SignatureCache::new("/tmp/fj-cache");
        let result = VerificationResult {
            valid: true,
            signer_identity: "dev@example.com".to_string(),
            timestamp: 1_700_000_000,
            warnings: vec![],
        };

        assert!(cache.check_cache("fj-math@1.0.0").is_none());
        cache.store_cache("fj-math@1.0.0", result.clone());
        let cached = cache.check_cache("fj-math@1.0.0");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().signer_identity, "dev@example.com");
    }
}
