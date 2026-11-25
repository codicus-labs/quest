//! Bearer-token authentication of the app routes, and the random secrets of the server.
//!
//! The apps send an OAuth access token for the app resource (`{origin}/v1`); tokens for `/mcp`
//! are refused here, and app tokens are refused at `/mcp`.

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::WWW_AUTHENTICATE;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use quest_protocol::{ErrorCode, UserId};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AppState;
use crate::error::HttpFailure;
use crate::oauth::policy::Resource;
use crate::oauth::{TokenRejection, ValidToken, validate_bearer};

/// The user a request is authenticated as, from `Authorization: Bearer <access token>`.
pub struct AuthenticatedUser(pub UserId);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        app_token(parts, state).await.map(|token| AuthenticatedUser(token.user)).map_err(|response| *response)
    }
}

/// The app access token of a request that outlives it (a sync connection): whose, of which
/// grant, and until when it is valid.
pub struct AppToken {
    pub user: UserId,
    pub grant: Uuid,
    /// Unix ms.
    pub expires_at: i64,
}

impl FromRequestParts<AppState> for AppToken {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        app_token(parts, state)
            .await
            .map(|token| AppToken { user: token.user, grant: token.grant, expires_at: token.expires_at })
            .map_err(|response| *response)
    }
}

async fn app_token(parts: &Parts, state: &AppState) -> Result<ValidToken, Box<Response>> {
    let unauthorized = |message: &str, challenge: &'static str| {
        let mut response = HttpFailure::new(StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized, message).into_response();
        response.headers_mut().insert(WWW_AUTHENTICATE, axum::http::HeaderValue::from_static(challenge));
        Box::new(response)
    };
    match validate_bearer(state, &parts.headers, Resource::App).await {
        Ok(token) => Ok(token),
        Err(TokenRejection::Missing) => {
            Err(unauthorized("Sign in first: send Authorization: Bearer <access token>", "Bearer"))
        }
        Err(TokenRejection::Invalid(reason)) => Err(unauthorized(reason, "Bearer error=\"invalid_token\"")),
        Err(TokenRejection::Internal) => Err(Box::new(
            HttpFailure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::Internal,
                "Internal server error; try again later",
            )
            .into_response(),
        )),
    }
}

/// 32 random bytes, hex: tokens, codes, cookies.
pub(crate) fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    hex::encode(bytes)
}

/// Secrets are stored hashed so a leaked database does not leak sessions or tokens.
pub(crate) fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
