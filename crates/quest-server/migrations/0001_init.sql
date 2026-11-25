CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    last_seq INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE fields (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    field TEXT NOT NULL,
    value_json TEXT NOT NULL,
    hlc TEXT NOT NULL,
    seq INTEGER NOT NULL,
    PRIMARY KEY (user_id, kind, entity_id, field)
);

CREATE UNIQUE INDEX fields_by_seq ON fields (user_id, seq);
