//! Password hashing utilities using Argon2id.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;

/// Hash a password using Argon2id with secure defaults.
///
/// # Errors
///
/// Returns an error if hashing fails.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

/// Verify a password against a hash.
///
/// # Errors
///
/// Returns an error if the hash is invalid or verification fails.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed_hash = PasswordHash::new(hash)?;
    let argon2 = Argon2::default();
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let password = "secure_password_123";
        let hash = hash_password(password).expect("hashing should succeed");

        // Hash should not be plaintext
        assert_ne!(hash, password);

        // Hash should be in PHC format
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_verify_password() {
        let password = "correct_password";
        let hash = hash_password(password).expect("hashing should succeed");

        // Correct password should verify
        assert!(verify_password(password, &hash).expect("verification should succeed"));

        // Wrong password should not verify
        assert!(!verify_password("wrong_password", &hash).expect("verification should succeed"));
    }

    #[test]
    fn test_different_hashes() {
        let password = "same_password";
        let hash1 = hash_password(password).expect("hashing should succeed");
        let hash2 = hash_password(password).expect("hashing should succeed");

        // Different salts should produce different hashes
        assert_ne!(hash1, hash2);

        // But both should verify
        assert!(verify_password(password, &hash1).expect("verification should succeed"));
        assert!(verify_password(password, &hash2).expect("verification should succeed"));
    }
}
