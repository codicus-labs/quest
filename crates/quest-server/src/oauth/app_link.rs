//! The Android App Link of the sign-in: `{origin}/auth/android` as the redirect URI, verified by
//! the Digital Asset Links file at `/.well-known/assetlinks.json`.

use std::fmt;
use std::str::FromStr;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use quest_protocol::ANDROID_PACKAGE_NAME;
use serde_json::{Value, json};

use super::page;
use crate::AppState;

/// SHA-256 fingerprint of an Android signing certificate, e.g. `4B:A6:…:69` (32 bytes as
/// colon-separated hex). Parsing accepts lower case and stores upper case, as `assetlinks.json`
/// expects.
#[derive(Clone, PartialEq, Eq)]
pub struct CertFingerprint(String);

impl FromStr for CertFingerprint {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim();
        let pairs: Vec<&str> = raw.split(':').collect();
        let well_formed = pairs.len() == 32
            && pairs.iter().all(|pair| pair.len() == 2 && pair.bytes().all(|b| b.is_ascii_hexdigit()));
        if !well_formed {
            return Err(format!(
                "\"{raw}\" is not a SHA-256 certificate fingerprint; expected 32 colon-separated hex bytes like \
                 AB:CD:… (keytool -list -v or apksigner verify --print-certs prints it)"
            ));
        }
        Ok(CertFingerprint(raw.to_ascii_uppercase()))
    }
}

impl fmt::Display for CertFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for CertFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CertFingerprint({})", self.0)
    }
}

/// The statement list that lets the Android app with these certificates handle this origin's links.
pub fn asset_links(fingerprints: &[CertFingerprint]) -> Value {
    let fingerprints: Vec<&str> = fingerprints.iter().map(|fingerprint| fingerprint.0.as_str()).collect();
    json!([{
        "relation": ["delegate_permission/common.handle_all_urls"],
        "target": {
            "namespace": "android_app",
            "package_name": ANDROID_PACKAGE_NAME,
            "sha256_cert_fingerprints": fingerprints,
        },
    }])
}

/// `GET /.well-known/assetlinks.json`; 404 without configured fingerprints.
pub async fn asset_links_file(State(state): State<AppState>) -> Response {
    let fingerprints = &state.oauth.config.android_cert_fingerprints;
    if fingerprints.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }
    Json(asset_links(fingerprints)).into_response()
}

/// `GET /auth/android`: shown only if Android did not hand the App Link to the app (e.g. the link
/// is not verified, or the browser did not switch apps without a tap). The query holds the
/// authorization code: never log or echo it. The page sends no referrer and is not cached.
pub async fn fallback_page() -> Response {
    page::app_link_fallback()
}

#[cfg(test)]
#[path = "app_link_tests.rs"]
mod tests;
