-- Migration v002: Tags (named snapshot pointers) + working dir status support
-- Tags are lightweight immutable pointers to snapshots, like git tags.

CREATE TABLE tags (
    id          TEXT PRIMARY KEY,        -- ULID
    name        TEXT NOT NULL UNIQUE,    -- human-readable name, e.g. "v1.0", "demo"
    snapshot_id TEXT NOT NULL,
    message     TEXT,                    -- optional annotation
    created_at  INTEGER NOT NULL,        -- unix millis
    FOREIGN KEY (snapshot_id) REFERENCES snapshots(id) ON DELETE CASCADE
);
CREATE INDEX idx_tags_snapshot ON tags(snapshot_id);
CREATE INDEX idx_tags_created  ON tags(created_at);
