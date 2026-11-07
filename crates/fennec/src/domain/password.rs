//! The stored remote-access password hash. No I/O here.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::num::NonZeroU32;

pub const ITERATIONS: u32 = 600_000;

/// `pbkdf2-sha256$<iterations>$<salt base64>$<hash base64>`, 32-byte hash.
pub fn hash(password: &str, salt: &[u8], iterations: u32) -> String {
    let mut out = [0u8; 32];
    // Zero iterations is meaningless; one keeps the function total.
    let rounds = NonZeroU32::new(iterations).unwrap_or(NonZeroU32::MIN);
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        rounds,
        salt,
        password.as_bytes(),
        &mut out,
    );
    format!(
        "pbkdf2-sha256${iterations}${}${}",
        STANDARD.encode(salt),
        STANDARD.encode(out)
    )
}

/// Constant-time check of a candidate against a stored hash. Malformed → false.
pub fn verify(stored: &str, candidate: &str) -> bool {
    let Some((iterations, salt, expected)) = parse(stored) else {
        return false;
    };
    ring::pbkdf2::verify(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        &salt,
        candidate.as_bytes(),
        &expected,
    )
    .is_ok()
}

/// True when `stored` is a well-formed hash string.
pub fn is_valid(stored: &str) -> bool {
    parse(stored).is_some()
}

fn parse(stored: &str) -> Option<(NonZeroU32, Vec<u8>, Vec<u8>)> {
    let mut parts = stored.split('$');
    let scheme = parts.next()?;
    let iterations = parts.next()?;
    let salt = parts.next()?;
    let hash = parts.next()?;
    if scheme != "pbkdf2-sha256" || parts.next().is_some() {
        return None;
    }
    let iterations = iterations.parse().ok()?;
    let salt = STANDARD.decode(salt).ok()?;
    let hash = STANDARD.decode(hash).ok()?;
    (hash.len() == 32).then_some((iterations, salt, hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_right_password_verifies() {
        let salt = [7u8; 16];
        let stored = hash("correct horse battery staple", &salt, 1000);
        assert!(is_valid(&stored));
        assert!(verify(&stored, "correct horse battery staple"));
    }

    #[test]
    fn the_wrong_password_does_not_verify() {
        let stored = hash("correct horse battery staple", &[7; 16], 1000);
        assert!(!verify(&stored, "correct horse battery stapler"));
    }

    #[test]
    fn malformed_stored_values_are_invalid() {
        assert!(!is_valid(""));
        assert!(!is_valid("pbkdf2-sha256$abc$x$y"));
        assert!(!is_valid("pbkdf2-sha256$1000$"));
        assert!(!is_valid("pbkdf2-sha256$1000$AA=="));
        assert!(!verify("pbkdf2-sha256$1000$AA==", "x"));
    }
}
