//! SQL of the OAuth authorization server: registered clients, grants (consents), codes and tokens.
//! Secrets arrive here already hashed; times are Unix milliseconds.

use quest_protocol::UserId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use super::{Repository, corrupt, decode_user_id};

/// A client registered with Dynamic Client Registration.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredClient {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    /// `None` for public clients.
    pub secret_hash: Option<String>,
}

pub struct NewGrant<'a> {
    pub id: Uuid,
    pub user: UserId,
    pub client_id: &'a str,
    pub client_name: &'a str,
    pub redirect_uri: &'a str,
    pub scope: &'a str,
    pub resource: &'a str,
}

pub struct NewCode<'a> {
    pub code_hash: &'a str,
    pub code_challenge: &'a str,
    pub expires_at: i64,
}

/// A grant as a code or token points at it.
#[derive(Clone, Debug, PartialEq)]
pub struct GrantRef {
    pub id: Uuid,
    pub user: UserId,
    pub client_id: String,
    pub scope: String,
    pub resource: String,
    pub revoked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredCode {
    pub grant: GrantRef,
    pub code_challenge: String,
    pub redirect_uri: String,
    pub expires_at: i64,
    pub used: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Access,
    Refresh,
}

impl TokenKind {
    fn as_str(self) -> &'static str {
        match self {
            TokenKind::Access => "access",
            TokenKind::Refresh => "refresh",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredToken {
    pub grant: GrantRef,
    pub kind: TokenKind,
    pub expires_at: i64,
    pub used: bool,
}

pub struct NewTokens<'a> {
    pub access_hash: &'a str,
    pub access_expires_at: i64,
    pub refresh_hash: &'a str,
    pub refresh_expires_at: i64,
}

/// An active grant as the account settings list it.
#[derive(Clone, Debug, PartialEq)]
pub struct GrantSummary {
    pub id: Uuid,
    pub client_name: String,
    pub redirect_uri: String,
    pub scope: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

/// How often `last_used_at` is written at most (it is shown with day precision).
const TOUCH_INTERVAL_MS: i64 = 60_000;
/// Expired codes and tokens are kept this long (so a late reuse is still recognised), then purged.
const PURGE_AFTER_MS: i64 = 24 * 60 * 60 * 1000;

impl Repository {
    pub async fn insert_client(&self, client: &StoredClient, now: i64) -> sqlx::Result<()> {
        let redirect_uris = serde_json::to_string(&client.redirect_uris).expect("strings always serialize");
        sqlx::query(
            "INSERT INTO oauth_clients (client_id, client_name, redirect_uris_json, secret_hash, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&client.client_id)
        .bind(&client.client_name)
        .bind(redirect_uris)
        .bind(&client.secret_hash)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_client(&self, client_id: &str) -> sqlx::Result<Option<StoredClient>> {
        let row = sqlx::query(
            "SELECT client_id, client_name, redirect_uris_json, secret_hash FROM oauth_clients WHERE client_id = ?",
        )
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let redirect_uris: String = row.try_get("redirect_uris_json")?;
        Ok(Some(StoredClient {
            client_id: row.try_get("client_id")?,
            client_name: row.try_get("client_name")?,
            redirect_uris: serde_json::from_str(&redirect_uris)
                .map_err(|error| corrupt("redirect_uris_json", error))?,
            secret_hash: row.try_get("secret_hash")?,
        }))
    }

    /// Records the user's consent and the authorization code for it, atomically.
    pub async fn create_grant_with_code(&self, grant: &NewGrant<'_>, code: &NewCode<'_>, now: i64) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO oauth_grants (id, user_id, client_id, client_name, redirect_uri, scope, resource, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(grant.id.to_string())
        .bind(grant.user.0.to_string())
        .bind(grant.client_id)
        .bind(grant.client_name)
        .bind(grant.redirect_uri)
        .bind(grant.scope)
        .bind(grant.resource)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO oauth_codes (code_hash, grant_id, code_challenge, redirect_uri, expires_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(code.code_hash)
        .bind(grant.id.to_string())
        .bind(code.code_challenge)
        .bind(grant.redirect_uri)
        .bind(code.expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    pub async fn find_code(&self, code_hash: &str) -> sqlx::Result<Option<StoredCode>> {
        let row = sqlx::query(
            "SELECT g.id AS grant_id, g.user_id, g.client_id, g.scope, g.resource, g.revoked_at,
                    c.code_challenge, c.redirect_uri, c.expires_at, c.used
             FROM oauth_codes c JOIN oauth_grants g ON g.id = c.grant_id WHERE c.code_hash = ?",
        )
        .bind(code_hash)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(StoredCode {
                grant: decode_grant(&row)?,
                code_challenge: row.try_get("code_challenge")?,
                redirect_uri: row.try_get("redirect_uri")?,
                expires_at: row.try_get("expires_at")?,
                used: row.try_get::<i64, _>("used")? != 0,
            })
        })
        .transpose()
    }

    /// Marks the code used; false if it already was (a concurrent or repeated exchange).
    pub async fn use_code(&self, code_hash: &str) -> sqlx::Result<bool> {
        let result = sqlx::query("UPDATE oauth_codes SET used = 1 WHERE code_hash = ? AND used = 0")
            .bind(code_hash)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn insert_tokens(&self, grant: Uuid, tokens: &NewTokens<'_>, now: i64) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        for (hash, kind, expires_at) in [
            (tokens.access_hash, TokenKind::Access, tokens.access_expires_at),
            (tokens.refresh_hash, TokenKind::Refresh, tokens.refresh_expires_at),
        ] {
            sqlx::query("INSERT INTO oauth_tokens (token_hash, grant_id, kind, expires_at) VALUES (?, ?, ?, ?)")
                .bind(hash)
                .bind(grant.to_string())
                .bind(kind.as_str())
                .bind(expires_at)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE oauth_grants SET last_used_at = ? WHERE id = ?")
            .bind(now)
            .bind(grant.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM oauth_tokens WHERE expires_at < ?")
            .bind(now - PURGE_AFTER_MS)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM oauth_codes WHERE expires_at < ?")
            .bind(now - PURGE_AFTER_MS)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    }

    pub async fn find_token(&self, token_hash: &str) -> sqlx::Result<Option<StoredToken>> {
        let row = sqlx::query(
            "SELECT g.id AS grant_id, g.user_id, g.client_id, g.scope, g.resource, g.revoked_at,
                    t.kind, t.expires_at, t.used
             FROM oauth_tokens t JOIN oauth_grants g ON g.id = t.grant_id WHERE t.token_hash = ?",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            let kind: String = row.try_get("kind")?;
            Ok(StoredToken {
                grant: decode_grant(&row)?,
                kind: match kind.as_str() {
                    "access" => TokenKind::Access,
                    "refresh" => TokenKind::Refresh,
                    other => return Err(corrupt("kind", format!("unknown token kind {other:?}"))),
                },
                expires_at: row.try_get("expires_at")?,
                used: row.try_get::<i64, _>("used")? != 0,
            })
        })
        .transpose()
    }

    /// Marks a refresh token used (rotated); false if it already was.
    pub async fn use_refresh_token(&self, token_hash: &str) -> sqlx::Result<bool> {
        let result =
            sqlx::query("UPDATE oauth_tokens SET used = 1 WHERE token_hash = ? AND kind = 'refresh' AND used = 0")
                .bind(token_hash)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Whether the grant exists and is not revoked.
    pub async fn grant_active(&self, grant: Uuid) -> sqlx::Result<bool> {
        let active: Option<i64> = sqlx::query_scalar("SELECT 1 FROM oauth_grants WHERE id = ? AND revoked_at IS NULL")
            .bind(grant.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(active.is_some())
    }

    /// Revokes the grant and deletes its tokens. With `user`, only if the grant is theirs.
    /// Returns false if there was no such active grant.
    pub async fn revoke_grant(&self, grant: Uuid, user: Option<UserId>, now: i64) -> sqlx::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE oauth_grants SET revoked_at = ?
             WHERE id = ? AND revoked_at IS NULL AND (? IS NULL OR user_id = ?)",
        )
        .bind(now)
        .bind(grant.to_string())
        .bind(user.map(|user| user.0.to_string()))
        .bind(user.map(|user| user.0.to_string()))
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("DELETE FROM oauth_tokens WHERE grant_id = ?").bind(grant.to_string()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Notes that the grant was used, at most once a minute.
    pub async fn touch_grant(&self, grant: Uuid, now: i64) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE oauth_grants SET last_used_at = ? WHERE id = ? AND (last_used_at IS NULL OR last_used_at < ?)",
        )
        .bind(now)
        .bind(grant.to_string())
        .bind(now - TOUCH_INTERVAL_MS)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The user's active grants for `resource`, most recent first. A consent counts once its code
    /// got tokens (`insert_tokens` sets `last_used_at`); abandoned consents are not connections.
    pub async fn active_grants(&self, user: UserId, resource: &str) -> sqlx::Result<Vec<GrantSummary>> {
        let rows = sqlx::query(
            "SELECT id, client_name, redirect_uri, scope, created_at, last_used_at FROM oauth_grants
             WHERE user_id = ? AND resource = ? AND revoked_at IS NULL AND last_used_at IS NOT NULL
             ORDER BY created_at DESC, id DESC",
        )
        .bind(user.0.to_string())
        .bind(resource)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(GrantSummary {
                    id: decode_uuid(row, "id")?,
                    client_name: row.try_get("client_name")?,
                    redirect_uri: row.try_get("redirect_uri")?,
                    scope: row.try_get("scope")?,
                    created_at: row.try_get("created_at")?,
                    last_used_at: row.try_get("last_used_at")?,
                })
            })
            .collect()
    }
}

fn decode_uuid(row: &SqliteRow, column: &str) -> sqlx::Result<Uuid> {
    let raw: String = row.try_get(column)?;
    Uuid::parse_str(&raw).map_err(|error| corrupt(column, error))
}

fn decode_grant(row: &SqliteRow) -> sqlx::Result<GrantRef> {
    Ok(GrantRef {
        id: decode_uuid(row, "grant_id")?,
        user: decode_user_id(row, "user_id")?,
        client_id: row.try_get("client_id")?,
        scope: row.try_get("scope")?,
        resource: row.try_get("resource")?,
        revoked: row.try_get::<Option<i64>, _>("revoked_at")?.is_some(),
    })
}
