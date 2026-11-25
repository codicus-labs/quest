//! SQL of accounts (created just in time at the first OIDC sign-in), the server's browser sessions
//! and sign-ins in progress at the identity provider. Secrets arrive hashed; times are Unix ms.

use quest_protocol::{Epoch, UserId};
use sqlx::Row;
use uuid::Uuid;

use super::{Repository, decode_user_id};

/// An account as the account settings show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: UserId,
    /// `None` only for accounts from before OIDC.
    pub issuer: Option<String>,
    pub subject: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
    pub ai_enabled: bool,
}

pub struct NewBrowserSession<'a> {
    pub token_hash: &'a str,
    pub user: UserId,
    pub id_token: &'a str,
    pub expires_at: i64,
}

/// A valid browser session with its account (for "Continue as …").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserSession {
    pub user: UserId,
    pub email: Option<String>,
    pub name: Option<String>,
}

pub struct NewLogin<'a> {
    pub state_hash: &'a str,
    pub browser_hash: &'a str,
    pub nonce: &'a str,
    pub pkce_verifier: &'a str,
    pub authorize_query: &'a str,
    pub expires_at: i64,
}

/// A sign-in at the identity provider that was taken (and so ended) by its callback. `Debug`
/// hides nonce, verifier and the request (it carries the client's `state` and challenge).
#[derive(Clone, PartialEq, Eq)]
pub struct StartedLogin {
    pub browser_hash: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub authorize_query: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for StartedLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartedLogin")
            .field("browser_hash", &self.browser_hash)
            .field("nonce", &"<redacted>")
            .field("pkce_verifier", &"<redacted>")
            .field("authorize_query", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl Repository {
    /// The account of `(issuer, subject)`, created on first sign-in; email and name are updated on
    /// every sign-in (display only).
    pub async fn sign_in_account(
        &self,
        issuer: &str,
        subject: &str,
        email: Option<&str>,
        name: Option<&str>,
    ) -> sqlx::Result<UserId> {
        let row = sqlx::query(
            "INSERT INTO users (id, issuer, subject, email, name, epoch) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (issuer, subject) DO UPDATE SET email = excluded.email, name = excluded.name
             RETURNING id",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(issuer)
        .bind(subject)
        .bind(email)
        .bind(name)
        .bind(Epoch(Uuid::now_v7()).0.to_string())
        .fetch_one(&self.pool)
        .await?;
        decode_user_id(&row, "id")
    }

    pub async fn account(&self, user: UserId) -> sqlx::Result<Option<Account>> {
        let row = sqlx::query("SELECT id, issuer, subject, email, name, ai_enabled FROM users WHERE id = ?")
            .bind(user.0.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| {
            Ok(Account {
                id: decode_user_id(&row, "id")?,
                issuer: row.try_get("issuer")?,
                subject: row.try_get("subject")?,
                email: row.try_get("email")?,
                name: row.try_get("name")?,
                ai_enabled: row.try_get::<i64, _>("ai_enabled")? != 0,
            })
        })
        .transpose()
    }

    pub async fn ai_enabled(&self, user: UserId) -> sqlx::Result<bool> {
        let enabled: Option<i64> = sqlx::query_scalar("SELECT ai_enabled FROM users WHERE id = ?")
            .bind(user.0.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(enabled.is_some_and(|enabled| enabled != 0))
    }

    pub async fn set_ai_enabled(&self, user: UserId, enabled: bool) -> sqlx::Result<()> {
        sqlx::query("UPDATE users SET ai_enabled = ? WHERE id = ?")
            .bind(i64::from(enabled))
            .bind(user.0.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Stores a new browser session and forgets expired ones.
    pub async fn insert_browser_session(&self, session: &NewBrowserSession<'_>, now: i64) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM browser_sessions WHERE expires_at <= ?").bind(now).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO browser_sessions (token_hash, user_id, id_token, expires_at) VALUES (?, ?, ?, ?)")
            .bind(session.token_hash)
            .bind(session.user.0.to_string())
            .bind(session.id_token)
            .bind(session.expires_at)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    }

    pub async fn browser_session(&self, token_hash: &str, now: i64) -> sqlx::Result<Option<BrowserSession>> {
        let row = sqlx::query(
            "SELECT s.user_id, u.email, u.name FROM browser_sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = ? AND s.expires_at > ?",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(BrowserSession {
                user: decode_user_id(&row, "user_id")?,
                email: row.try_get("email")?,
                name: row.try_get("name")?,
            })
        })
        .transpose()
    }

    /// Ends a browser session; returns its ID token (the logout hint) if it existed.
    pub async fn delete_browser_session(&self, token_hash: &str) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar("DELETE FROM browser_sessions WHERE token_hash = ? RETURNING id_token")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
    }

    /// Stores a sign-in in progress and forgets expired ones.
    pub async fn insert_login(&self, login: &NewLogin<'_>, now: i64) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM oidc_logins WHERE expires_at <= ?").bind(now).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO oidc_logins (state_hash, browser_hash, nonce, pkce_verifier, authorize_query, expires_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(login.state_hash)
        .bind(login.browser_hash)
        .bind(login.nonce)
        .bind(login.pkce_verifier)
        .bind(login.authorize_query)
        .bind(login.expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    /// Removes and returns the sign-in of `state_hash`, so each callback is used once.
    pub async fn take_login(&self, state_hash: &str) -> sqlx::Result<Option<StartedLogin>> {
        let row = sqlx::query(
            "DELETE FROM oidc_logins WHERE state_hash = ?
             RETURNING browser_hash, nonce, pkce_verifier, authorize_query, expires_at",
        )
        .bind(state_hash)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(StartedLogin {
                browser_hash: row.try_get("browser_hash")?,
                nonce: row.try_get("nonce")?,
                pkce_verifier: row.try_get("pkce_verifier")?,
                authorize_query: row.try_get("authorize_query")?,
                expires_at: row.try_get("expires_at")?,
            })
        })
        .transpose()
    }
}
