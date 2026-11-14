-- Login SSO (#14): links an external identity (e.g. a GitHub user id) to a worker account.

CREATE TABLE oauth_identities (
    provider   TEXT NOT NULL,
    subject    TEXT NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (provider, subject)
);
