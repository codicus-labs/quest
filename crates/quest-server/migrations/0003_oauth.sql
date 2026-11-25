-- OAuth 2.1 for MCP clients. Secrets (client secrets, codes, tokens) are stored as SHA-256 hashes.
-- Times are Unix milliseconds.

-- Clients registered with Dynamic Client Registration (RFC 7591). Clients that use a Client ID
-- Metadata Document are not stored: their URL is the client id and the document is fetched.
CREATE TABLE oauth_clients (
    client_id TEXT PRIMARY KEY NOT NULL,
    client_name TEXT NOT NULL,
    redirect_uris_json TEXT NOT NULL,
    -- NULL for public clients (token_endpoint_auth_method "none").
    secret_hash TEXT,
    created_at INTEGER NOT NULL
);

-- One consent of a user for a client: an "AI connection". Revoking it ends all its tokens.
CREATE TABLE oauth_grants (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    scope TEXT NOT NULL,
    resource TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    revoked_at INTEGER
);

CREATE INDEX oauth_grants_by_user ON oauth_grants (user_id, revoked_at);

CREATE TABLE oauth_codes (
    code_hash TEXT PRIMARY KEY NOT NULL,
    grant_id TEXT NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE,
    code_challenge TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    used INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE oauth_tokens (
    token_hash TEXT PRIMARY KEY NOT NULL,
    grant_id TEXT NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE,
    -- 'access' or 'refresh'
    kind TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    -- A refresh token is used once (rotation); using it again revokes the grant.
    used INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX oauth_tokens_by_grant ON oauth_tokens (grant_id);
