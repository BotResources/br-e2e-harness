//! RSA key-pool generation and JWK encoding.
//!
//! The entire pool is generated once at startup (in parallel — pure-Rust RSA
//! keygen is slow) so that key rotation during a test is instantaneous.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};

const RSA_BITS: usize = 2048;

/// One signing key of the pool: the private half for minting, the public
/// half pre-encoded as the JWK the `/jwks` endpoint serves when published.
pub struct KeyEntry {
    pub kid: String,
    pub encoding_key: jsonwebtoken::EncodingKey,
    pub jwk: serde_json::Value,
}

/// Generate `size` RSA keys in parallel, kids `e2e-key-0` … `e2e-key-{size-1}`.
pub fn generate_pool(size: usize) -> Vec<KeyEntry> {
    let handles: Vec<_> = (0..size)
        .map(|i| std::thread::spawn(move || generate_one(i)))
        .collect();
    handles
        .into_iter()
        .map(|h| h.join().expect("RSA keygen thread panicked"))
        .collect()
}

fn generate_one(index: usize) -> KeyEntry {
    let mut rng = rand::thread_rng();
    let private = RsaPrivateKey::new(&mut rng, RSA_BITS).expect("RSA key generation failed");
    let public = RsaPublicKey::from(&private);
    let kid = format!("e2e-key-{index}");

    let der = private
        .to_pkcs1_der()
        .expect("PKCS#1 DER export of a freshly generated key cannot fail");
    let encoding_key = jsonwebtoken::EncodingKey::from_rsa_der(der.as_bytes());

    let jwk = serde_json::json!({
        "kty": "RSA",
        "use": "sig",
        "alg": "RS256",
        "kid": kid,
        "n": URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),
        "e": URL_SAFE_NO_PAD.encode(public.e().to_bytes_be()),
    });

    KeyEntry {
        kid,
        encoding_key,
        jwk,
    }
}
