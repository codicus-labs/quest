//! RS256 keys and ID tokens of the fake provider.

use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, rand_core::OsRng};
use serde_json::{Value, json};
use sha2::Sha256;

/// Key id of the published key. The "wrong key" fault signs with another key under the same id,
/// so only the signature check can catch it.
pub const KEY_ID: &str = "quest-fake-1";

/// The published key and an unpublished one, generated once per process (key generation is slow).
pub struct Keys {
    pub published: RsaPrivateKey,
    pub unpublished: RsaPrivateKey,
}

pub fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(|| Keys { published: generate(), unpublished: generate() })
}

fn generate() -> RsaPrivateKey {
    RsaPrivateKey::new(&mut OsRng, 2048).expect("generating an RSA key with the OS RNG succeeds")
}

/// The JWKS document with the published key.
pub fn jwks() -> Value {
    let public = keys().published.to_public_key();
    json!({
        "keys": [{
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": KEY_ID,
            "n": URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),
            "e": URL_SAFE_NO_PAD.encode(public.e().to_bytes_be()),
        }]
    })
}

/// A compact JWS (RS256) of `claims`.
pub fn sign(claims: &Value, key: &RsaPrivateKey) -> String {
    let header = json!({ "alg": "RS256", "typ": "JWT", "kid": KEY_ID });
    let encode = |value: &Value| URL_SAFE_NO_PAD.encode(serde_json::to_vec(value).expect("JSON values serialize"));
    let signing_input = format!("{}.{}", encode(&header), encode(claims));
    let signature = SigningKey::<Sha256>::new(key.clone()).sign(signing_input.as_bytes());
    format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
}

#[cfg(test)]
mod tests {
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::signature::Verifier;

    use super::*;

    #[test]
    fn signature_verifies_with_the_published_key_only() {
        let token = sign(&json!({ "sub": "anna" }), &keys().published);
        let (input, signature) = token.rsplit_once('.').unwrap();
        let signature = Signature::try_from(URL_SAFE_NO_PAD.decode(signature).unwrap().as_slice()).unwrap();
        let published = VerifyingKey::<Sha256>::new(keys().published.to_public_key());
        assert!(published.verify(input.as_bytes(), &signature).is_ok());
        let unpublished = VerifyingKey::<Sha256>::new(keys().unpublished.to_public_key());
        assert!(unpublished.verify(input.as_bytes(), &signature).is_err());
    }
}
