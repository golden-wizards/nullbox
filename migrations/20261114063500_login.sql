-- Login (#14): worker accounts and server-side sessions.
-- Passwords are argon2id PHC strings; session tokens are stored as SHA-256 hashes only.

CREATE TABLE users (
    id            TEXT PRIMARY KEY NOT NULL,
    username      TEXT NOT NULL UNIQUE COLLATE nocase,
    password_hash TEXT,          -- NULL for accounts that only sign in via SSO
    created_at    TEXT NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX sessions_user ON sessions (user_id);
