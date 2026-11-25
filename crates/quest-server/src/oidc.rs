//! Sign-in at the OpenID Connect provider (any OIDC provider in production, Keycloak or the fake locally).
//!
//! The server is a confidential client using the authorization code flow with PKCE S256, `state`
//! and `nonce`, scopes `openid profile email`. [`IdentityProvider`] is the boundary; the rest of
//! the server sees only [`Identity`] and [`LoginError`], never `openidconnect` types.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use openidconnect::core::{
    CoreAuthDisplay, CoreAuthenticationFlow, CoreClaimName, CoreClaimType, CoreClient, CoreClientAuthMethod,
    CoreGrantType, CoreIdToken, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreJweKeyManagementAlgorithm,
    CoreResponseMode, CoreResponseType, CoreSubjectIdentifierType,
};
use openidconnect::{
    AdditionalProviderMetadata, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, ProviderMetadata, RedirectUrl, Scope,
    TokenResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use url::Url;

/// How long discovery metadata and keys are reused before they are fetched again.
const METADATA_TTL: Duration = Duration::from_secs(10 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// The identity provider the server signs users in with. Object-safe so tests can use any
/// provider; `Debug` must not print secrets.
pub trait IdentityProvider: fmt::Debug + Send + Sync {
    /// Where to send the browser, and what to keep until the callback.
    fn start_login<'a>(
        &'a self,
        redirect_uri: &'a str,
        force_login: bool,
    ) -> BoxFuture<'a, Result<StartedLogin, LoginError>>;

    /// Exchanges the callback's `code` and validates the ID token (signature over JWKS, `iss`,
    /// `aud`, `exp`, `nonce`). The caller has already matched `state` to `pending`.
    fn finish_login<'a>(
        &'a self,
        redirect_uri: &'a str,
        code: &'a str,
        pending: &'a PendingLogin,
    ) -> BoxFuture<'a, Result<Identity, LoginError>>;

    /// RP-initiated logout at the provider, if it offers it.
    fn logout_url<'a>(
        &'a self,
        id_token_hint: &'a str,
        post_logout_redirect_uri: &'a str,
        state: Option<&'a str>,
    ) -> BoxFuture<'a, Option<Url>>;
}

/// `Debug` hides the URL's query (it carries `state` and `nonce`) and the secrets.
pub struct StartedLogin {
    pub url: Url,
    pub pending: PendingLogin,
}

impl fmt::Debug for StartedLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut url = self.url.clone();
        url.set_query(None);
        f.debug_struct("StartedLogin").field("url", &url.as_str()).field("pending", &self.pending).finish()
    }
}

/// Secrets of a login in progress, bound to its `state`. `Debug` hides them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingLogin {
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

impl fmt::Debug for PendingLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingLogin")
            .field("state", &REDACTED)
            .field("nonce", &REDACTED)
            .field("pkce_verifier", &REDACTED)
            .finish()
    }
}

/// The signed-in user. `(issuer, subject)` is the identity; email and name are for display only.
/// `Debug` hides the ID token and the personal data.
#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
    /// Kept for `id_token_hint` at logout.
    pub id_token: String,
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("issuer", &self.issuer)
            .field("subject", &self.subject)
            .field("email", &self.email.as_ref().map(|_| REDACTED))
            .field("name", &self.name.as_ref().map(|_| REDACTED))
            .field("id_token", &REDACTED)
            .finish()
    }
}

const REDACTED: &str = "<redacted>";

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoginError {
    #[error("the identity provider cannot be reached or its discovery document is invalid: {0}")]
    Discovery(String),
    #[error("the identity provider refused the authorization code: {0}")]
    Exchange(String),
    #[error("the identity provider returned no ID token")]
    MissingIdToken,
    #[error("the ID token is invalid: {0}")]
    InvalidIdToken(String),
}

/// `QUEST_OIDC_*`: the provider's issuer and this server's client there.
#[derive(Clone)]
pub struct OidcSettings {
    pub issuer: Url,
    pub client_id: String,
    pub client_secret: String,
}

impl fmt::Debug for OidcSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcSettings")
            .field("issuer", &self.issuer.as_str())
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct LogoutMetadata {
    #[serde(default)]
    end_session_endpoint: Option<Url>,
}

impl AdditionalProviderMetadata for LogoutMetadata {}

type Metadata = ProviderMetadata<
    LogoutMetadata,
    CoreAuthDisplay,
    CoreClientAuthMethod,
    CoreClaimName,
    CoreClaimType,
    CoreGrantType,
    CoreJweContentEncryptionAlgorithm,
    CoreJweKeyManagementAlgorithm,
    CoreJsonWebKey,
    CoreResponseMode,
    CoreResponseType,
    CoreSubjectIdentifierType,
>;

/// [`IdentityProvider`] over the `openidconnect` crate. Discovery is lazy and cached, so the
/// server starts while the provider is down.
pub struct OpenIdConnect {
    settings: OidcSettings,
    http: http::HttpClient,
    metadata: Mutex<Option<(Instant, Arc<Metadata>)>>,
}

impl fmt::Debug for OpenIdConnect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenIdConnect").field("settings", &self.settings).finish_non_exhaustive()
    }
}

impl OpenIdConnect {
    /// Checks the settings: https issuer (plain http only on loopback), client id and secret set.
    pub fn new(settings: OidcSettings) -> Result<Self, String> {
        let issuer = &settings.issuer;
        match issuer.scheme() {
            "https" => {}
            "http" if crate::oauth::policy::is_loopback(issuer) => {}
            _ => return Err(format!("QUEST_OIDC_ISSUER {issuer} must use https (http only on localhost)")),
        }
        if settings.client_id.trim().is_empty() || settings.client_secret.is_empty() {
            return Err("QUEST_OIDC_CLIENT_ID and QUEST_OIDC_CLIENT_SECRET must not be empty".to_owned());
        }
        let http = reqwest::Client::builder()
            .tls_backend_preconfigured(crate::ai::tls_config().map_err(|error| error.to_string())?)
            // Following redirects would let a provider point the server at other hosts (SSRF).
            .redirect(reqwest::redirect::Policy::none())
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(OpenIdConnect { settings, http: http::HttpClient(http), metadata: Mutex::new(None) })
    }

    async fn metadata(&self, refresh: bool) -> Result<Arc<Metadata>, LoginError> {
        let mut cached = self.metadata.lock().await;
        if let Some((fetched, metadata)) = cached.as_ref()
            && !refresh
            && fetched.elapsed() < METADATA_TTL
        {
            return Ok(metadata.clone());
        }
        // `IssuerUrl` keeps the configured spelling; a trailing slash would not match `iss`.
        let issuer = IssuerUrl::new(self.settings.issuer.as_str().trim_end_matches('/').to_owned())
            .map_err(|error| LoginError::Discovery(error.to_string()))?;
        let metadata = Metadata::discover_async(issuer, &self.http)
            .await
            .map_err(|error| LoginError::Discovery(error_chain(&error)))?;
        let metadata = Arc::new(metadata);
        *cached = Some((Instant::now(), metadata.clone()));
        Ok(metadata)
    }

    fn client(
        &self,
        metadata: &Metadata,
    ) -> CoreClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet, EndpointMaybeSet>
    {
        CoreClient::from_provider_metadata(
            metadata.clone(),
            ClientId::new(self.settings.client_id.clone()),
            Some(ClientSecret::new(self.settings.client_secret.clone())),
        )
    }

    async fn finish(&self, redirect_uri: &str, code: &str, pending: &PendingLogin) -> Result<Identity, LoginError> {
        let metadata = self.metadata(false).await?;
        let redirect =
            RedirectUrl::new(redirect_uri.to_owned()).map_err(|error| LoginError::Exchange(error.to_string()))?;
        let client = self.client(&metadata).set_redirect_uri(redirect);
        let response = client
            .exchange_code(AuthorizationCode::new(code.to_owned()))
            .map_err(|error| LoginError::Exchange(error.to_string()))?
            .set_pkce_verifier(PkceCodeVerifier::new(pending.pkce_verifier.clone()))
            .request_async(&self.http)
            .await
            .map_err(|error| LoginError::Exchange(error_chain(&error)))?;
        let id_token = response.id_token().ok_or(LoginError::MissingIdToken)?;
        let nonce = Nonce::new(pending.nonce.clone());
        match self.verify(&metadata, id_token, &nonce) {
            // The provider may have rotated its keys since discovery: fetch them once more.
            Err(_) if metadata_age(&self.metadata).await > Duration::from_secs(5) => {
                let fresh = self.metadata(true).await?;
                self.verify(&fresh, id_token, &nonce)
            }
            verified => verified,
        }
    }

    fn verify(&self, metadata: &Metadata, id_token: &CoreIdToken, nonce: &Nonce) -> Result<Identity, LoginError> {
        let client = self.client(metadata);
        let claims = id_token
            .claims(&client.id_token_verifier(), nonce)
            .map_err(|error| LoginError::InvalidIdToken(error_chain(&error)))?;
        let email = claims.email().map(|email| email.to_string());
        let name = claims
            .name()
            .and_then(|name| name.get(None))
            .map(|name| name.to_string())
            .or_else(|| claims.preferred_username().map(|name| name.to_string()));
        Ok(Identity {
            issuer: claims.issuer().to_string(),
            subject: claims.subject().to_string(),
            email,
            name,
            id_token: id_token.to_string(),
        })
    }
}

async fn metadata_age(cache: &Mutex<Option<(Instant, Arc<Metadata>)>>) -> Duration {
    cache.lock().await.as_ref().map_or(Duration::MAX, |(fetched, _)| fetched.elapsed())
}

impl IdentityProvider for OpenIdConnect {
    fn start_login<'a>(
        &'a self,
        redirect_uri: &'a str,
        force_login: bool,
    ) -> BoxFuture<'a, Result<StartedLogin, LoginError>> {
        Box::pin(async move {
            let metadata = self.metadata(false).await?;
            let redirect =
                RedirectUrl::new(redirect_uri.to_owned()).map_err(|error| LoginError::Discovery(error.to_string()))?;
            let client = self.client(&metadata).set_redirect_uri(redirect);
            let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
            let mut request = client
                .authorize_url(CoreAuthenticationFlow::AuthorizationCode, CsrfToken::new_random, Nonce::new_random)
                .add_scope(Scope::new("profile".to_owned()))
                .add_scope(Scope::new("email".to_owned()))
                .set_pkce_challenge(challenge);
            if force_login {
                request = request.add_prompt(openidconnect::core::CoreAuthPrompt::Login);
            }
            let (url, state, nonce) = request.url();
            let pending = PendingLogin {
                state: state.secret().clone(),
                nonce: nonce.secret().clone(),
                pkce_verifier: verifier.secret().clone(),
            };
            Ok(StartedLogin { url, pending })
        })
    }

    fn finish_login<'a>(
        &'a self,
        redirect_uri: &'a str,
        code: &'a str,
        pending: &'a PendingLogin,
    ) -> BoxFuture<'a, Result<Identity, LoginError>> {
        Box::pin(self.finish(redirect_uri, code, pending))
    }

    fn logout_url<'a>(
        &'a self,
        id_token_hint: &'a str,
        post_logout_redirect_uri: &'a str,
        state: Option<&'a str>,
    ) -> BoxFuture<'a, Option<Url>> {
        Box::pin(async move {
            let metadata = self.metadata(false).await.ok()?;
            let mut url = metadata.additional_metadata().end_session_endpoint.clone()?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("id_token_hint", id_token_hint);
                query.append_pair("client_id", &self.settings.client_id);
                query.append_pair("post_logout_redirect_uri", post_logout_redirect_uri);
                if let Some(state) = state {
                    query.append_pair("state", state);
                }
            }
            Some(url)
        })
    }
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// `openidconnect`'s HTTP calls over our reqwest client.
mod http {
    use std::future::Future;
    use std::pin::Pin;

    use openidconnect::{AsyncHttpClient, HttpRequest, HttpResponse};

    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    pub struct HttpError(String);

    pub struct HttpClient(pub reqwest::Client);

    impl<'c> AsyncHttpClient<'c> for HttpClient {
        type Error = HttpError;
        type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send + 'c>>;

        fn call(&'c self, request: HttpRequest) -> Self::Future {
            Box::pin(send(&self.0, request))
        }
    }

    async fn send(client: &reqwest::Client, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        let (parts, body) = request.into_parts();
        let url = parts.uri.to_string();
        let response = client
            .request(parts.method, url)
            .headers(parts.headers)
            .body(body)
            .send()
            .await
            .map_err(|error| HttpError(format!("request failed: {error}")))?;
        let mut builder = ::http::Response::builder().status(response.status());
        for (name, value) in response.headers() {
            builder = builder.header(name, value);
        }
        let body = response.bytes().await.map_err(|error| HttpError(format!("reading body failed: {error}")))?;
        builder.body(body.to_vec()).map_err(|error| HttpError(error.to_string()))
    }
}
