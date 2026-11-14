-- Audit log (#13): who did what to which ticket, plus sign-ins and refused requests.
-- Append-only: the triggers below reject every UPDATE and DELETE.
-- Never holds secrets: no tokens, no passwords, no message bodies.

CREATE TABLE audit_log (
    id         TEXT PRIMARY KEY NOT NULL,   -- UUIDv7, so ordering by id is ordering by time
    at         TEXT NOT NULL,               -- RFC 3339, UTC, milliseconds
    actor      TEXT,                        -- worker username; NULL for customers and anonymous callers
    actor_kind TEXT NOT NULL,               -- worker | customer | admin | mcp | anonymous
    action     TEXT NOT NULL,               -- e.g. ticket.created, auth.refused
    outcome    TEXT NOT NULL,               -- ok | denied
    ticket_id  TEXT,
    ip         TEXT,                        -- client address from the reverse proxy
    method     TEXT,
    path       TEXT,                        -- path only, never the query string
    detail     TEXT NOT NULL DEFAULT '{}'   -- JSON object
);

CREATE INDEX audit_log_ticket ON audit_log (ticket_id, id);
CREATE INDEX audit_log_action ON audit_log (action, id);

CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only');
END;

CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only');
END;
