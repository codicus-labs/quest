//! Migration 0004 (OIDC) on a database from the password era: data stays, passwords and app
//! sessions go, grants of scope `quest` become `quest:write`.

mod common;

use std::borrow::Cow;

use common::{Setup, TestServer};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::{Row, SqlitePool as Pool};

async fn pool(database_url: &str) -> Pool {
    pool_with(database_url, true).await
}

async fn pool_with(database_url: &str, foreign_keys: bool) -> Pool {
    let options: SqliteConnectOptions = database_url.parse().unwrap();
    SqlitePool::connect_with(options.create_if_missing(true).foreign_keys(foreign_keys)).await.unwrap()
}

/// Applies the migrations before 0004 to a new database at `database_url`.
async fn password_era_database(database_url: &str, foreign_keys: bool) -> Pool {
    let old = pool_with(database_url, foreign_keys).await;
    let mut before_oidc = sqlx::migrate!();
    before_oidc.migrations = Cow::Owned(before_oidc.migrations.iter().filter(|m| m.version < 4).cloned().collect());
    before_oidc.run(&old).await.unwrap();
    old
}

#[tokio::test]
async fn password_era_databases_keep_their_data() {
    let dir = tempfile::tempdir().unwrap();
    let database_url = common::database_url(&dir);
    let old = password_era_database(&database_url, true).await;
    for statement in [
        "INSERT INTO users (id, email, password_hash, last_seq, epoch) VALUES ('u1', 'ada@example.com', 'hash', 1, 'e1')",
        "INSERT INTO sessions (token_hash, user_id) VALUES ('s1', 'u1')",
        "INSERT INTO fields (user_id, kind, entity_id, field, value_json, hlc, seq)
         VALUES ('u1', 'task', 't1', 'title', '\"kept\"', 'h', 1)",
        "INSERT INTO oauth_grants (id, user_id, client_id, client_name, redirect_uri, scope, resource, created_at)
         VALUES ('g1', 'u1', 'c', 'Client', 'http://127.0.0.1/cb', 'quest', 'http://127.0.0.1/mcp', 0)",
    ] {
        sqlx::query(statement).execute(&old).await.unwrap();
    }
    old.close().await;

    let server = TestServer::start_custom(Setup { database_url: Some(database_url.clone()), ..Setup::default() }).await;
    let migrated = pool(&database_url).await;
    let user = sqlx::query("SELECT email, issuer, subject, ai_enabled, last_seq, epoch FROM users WHERE id = 'u1'")
        .fetch_one(&migrated)
        .await
        .unwrap();
    assert_eq!(user.get::<String, _>("email"), "ada@example.com");
    assert_eq!(user.get::<Option<String>, _>("issuer"), None, "not linked to an identity yet");
    assert_eq!(user.get::<Option<String>, _>("subject"), None);
    assert_eq!((user.get::<i64, _>("ai_enabled"), user.get::<i64, _>("last_seq")), (0, 1));
    assert_eq!(user.get::<String, _>("epoch"), "e1");
    let fields: i64 =
        sqlx::query_scalar("SELECT count(*) FROM fields WHERE user_id = 'u1'").fetch_one(&migrated).await.unwrap();
    assert_eq!(fields, 1, "data survives the rebuild of users");
    let scope: String =
        sqlx::query_scalar("SELECT scope FROM oauth_grants WHERE id = 'g1'").fetch_one(&migrated).await.unwrap();
    assert_eq!(scope, "quest:write");
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE name = 'sessions'")
        .fetch_one(&migrated)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
    let broken = sqlx::query("PRAGMA foreign_key_check").fetch_all(&migrated).await.unwrap();
    assert!(broken.is_empty(), "foreign keys still point at users");

    // New sign-ins are new accounts; the old one stays until it is linked.
    let session = server.sign_in("ada").await;
    assert_ne!(server.account(&session.token).await.unwrap().user_id.0.to_string(), "u1");
}

#[tokio::test]
async fn broken_references_abort_the_migration() {
    let dir = tempfile::tempdir().unwrap();
    let database_url = common::database_url(&dir);
    let old = password_era_database(&database_url, false).await;
    sqlx::query(
        "INSERT INTO fields (user_id, kind, entity_id, field, value_json, hlc, seq)
         VALUES ('nobody', 'task', 't1', 'title', '\"orphan\"', 'h', 1)",
    )
    .execute(&old)
    .await
    .unwrap();
    old.close().await;

    let fake = common::start_fake().await;
    let config = quest_server::Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        database_url: database_url.clone(),
        ai: None,
        auth: common::auth_config(&fake),
        mcp: None,
    };
    assert!(quest_server::start(config).await.is_err(), "0004 refuses to commit dangling references");
    let migrated = pool(&database_url).await;
    let users_columns: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_table_info('users') WHERE name = 'password_hash'")
            .fetch_one(&migrated)
            .await
            .unwrap();
    assert_eq!(users_columns, 1, "rolled back: the old users table is still there");
}
