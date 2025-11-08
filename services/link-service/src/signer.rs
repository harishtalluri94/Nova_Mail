use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::hmac;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct LinkSigner {
    key: hmac::Key,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedToken {
    pub link_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

impl LinkSigner {
    pub fn new(secret: &str) -> Self {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        Self { key }
    }

    /// Sign a link ID and expiration time, returning a URL-safe token
    pub fn sign(&self, link_id: Uuid, expires_at: DateTime<Utc>) -> Result<String> {
        let token = SignedToken {
            link_id,
            expires_at,
        };

        let token_json = serde_json::to_string(&token)?;
        let token_bytes = token_json.as_bytes();

        // Compute HMAC
        let signature = hmac::sign(&self.key, token_bytes);
        let signature_bytes = signature.as_ref();

        // Combine token + signature
        let mut combined = Vec::with_capacity(token_bytes.len() + signature_bytes.len());
        combined.extend_from_slice(token_bytes);
        combined.extend_from_slice(signature_bytes);

        // Base64 encode
        Ok(base64::encode_config(combined, base64::URL_SAFE_NO_PAD))
    }

    /// Verify a token and return the decoded data
    pub fn verify(&self, token: &str) -> Result<SignedToken> {
        // Base64 decode
        let combined = base64::decode_config(token, base64::URL_SAFE_NO_PAD)
            .context("Invalid base64")?;

        if combined.len() < 32 {
            anyhow::bail!("Token too short");
        }

        // Split token and signature (HMAC-SHA256 is 32 bytes)
        let (token_bytes, signature_bytes) = combined.split_at(combined.len() - 32);

        // Verify signature
        hmac::verify(&self.key, token_bytes, signature_bytes)
            .map_err(|_| anyhow::anyhow!("Invalid signature"))?;

        // Deserialize token
        let signed_token: SignedToken = serde_json::from_slice(token_bytes)
            .context("Failed to deserialize token")?;

        Ok(signed_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let signer = LinkSigner::new("test-secret");
        let link_id = Uuid::new_v4();
        let expires_at = Utc::now() + chrono::Duration::hours(1);

        let token = signer.sign(link_id, expires_at).unwrap();
        let verified = signer.verify(&token).unwrap();

        assert_eq!(verified.link_id, link_id);
        assert_eq!(
            verified.expires_at.timestamp(),
            expires_at.timestamp()
        );
    }

    #[test]
    fn test_verify_invalid_signature() {
        let signer = LinkSigner::new("test-secret");
        let wrong_signer = LinkSigner::new("wrong-secret");

        let link_id = Uuid::new_v4();
        let expires_at = Utc::now() + chrono::Duration::hours(1);

        let token = signer.sign(link_id, expires_at).unwrap();
        let result = wrong_signer.verify(&token);

        assert!(result.is_err());
    }

    #[test]
    fn test_verify_tampered_token() {
        let signer = LinkSigner::new("test-secret");
        let link_id = Uuid::new_v4();
        let expires_at = Utc::now() + chrono::Duration::hours(1);

        let mut token = signer.sign(link_id, expires_at).unwrap();

        // Tamper with the token
        token.push('X');

        let result = signer.verify(&token);
        assert!(result.is_err());
    }
}
