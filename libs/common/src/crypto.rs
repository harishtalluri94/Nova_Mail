use crate::error::{Error, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use ring::rand::{SecureRandom, SystemRandom};

/// Hash a password using Argon2
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| Error::Internal(format!("Failed to hash password: {}", e)))?;

    Ok(password_hash.to_string())
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| Error::Internal(format!("Invalid password hash: {}", e)))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Generate a random token of specified length
pub fn generate_token(length: usize) -> Result<String> {
    let rng = SystemRandom::new();
    let mut token = vec![0u8; length];

    rng.fill(&mut token)
        .map_err(|e| Error::Internal(format!("Failed to generate token: {:?}", e)))?;

    Ok(base64::encode_config(
        token,
        base64::URL_SAFE_NO_PAD,
    ))
}

/// Compute SHA-256 hash
pub fn sha256(data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Compute BLAKE3 hash (faster for large data)
pub fn blake3_hash(data: &[u8]) -> Vec<u8> {
    blake3::hash(data).as_bytes().to_vec()
}
