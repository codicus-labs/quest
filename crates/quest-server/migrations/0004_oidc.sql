-- no-transaction
-- Phase 11: sign-in only through OpenID Connect. Accounts are identified by (issuer, subject); email
-- and name are display only. Password hashes and app sessions go away; app tokens are OAuth grants.
--
-- `users` is rebuilt (SQLite cannot drop the UNIQUE email column in place). Foreign keys are off
-- meanwhile so dropping the old table does not cascade into fields and grants.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE users_new (
    id TEXT PRIMARY KEY NOT NULL,
    -- NULL only for accounts from before OIDC: they cannot sign in until linked.
    issuer TEXT,
    subject TEXT,
    email TEXT,
    name TEXT,
    -- AI features opted in (default off).
    ai_enabled INTEGER NOT NULL DEFAULT 0,
    last_seq INTEGER NOT NULL DEFAULT 0,
    epoch TEXT NOT NULL DEFAULT '',
    UNIQUE (issuer, subject)
);

INSERT INTO users_new (id, email, last_seq, epoch) SELECT id, email, last_seq, epoch FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- App sessions of the password sign-in.
DROP TABLE sessions;

-- The scope `quest` is now `quest:write` (read and write).
UPDATE oauth_grants SET scope = 'quest:write' WHERE scope = 'quest';

-- The server's own browser session after signing in at the identity provider (cookie
-- `quest_session`, stored hashed). `id_token` is the hint for RP-initiated logout.
CREATE TABLE browser_sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    id_token TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);

-- Sign-ins at the identity provider in progress, by hashed `state`; used once.
CREATE TABLE oidc_logins (
    state_hash TEXT PRIMARY KEY NOT NULL,
    -- Hash of the cookie that binds the sign-in to the browser that started it.
    browser_hash TEXT NOT NULL,
    nonce TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    -- Query string of the /authorize request to continue with.
    authorize_query TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);

-- Every reference to users must still resolve before committing (foreign keys were off). A
-- violation fails the CHECK, so the migration aborts and the transaction is rolled back.
CREATE TEMP TABLE foreign_keys_intact (violations INTEGER NOT NULL CHECK (violations = 0));
INSERT INTO foreign_keys_intact SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE foreign_keys_intact;

COMMIT;

PRAGMA foreign_keys = ON;
