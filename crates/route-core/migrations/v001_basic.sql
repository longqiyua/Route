-- Migration v001: Basic mode schema
-- Snapshot = pure state node; Commit = edge with metadata; Branch = subgraph label.

-- Snapshots: pure state, no user metadata
CREATE TABLE snapshots (
    id            TEXT PRIMARY KEY,        -- ULID
    manifest_hash TEXT NOT NULL,
    created_at    INTEGER NOT NULL,        -- unix millis
    FOREIGN KEY (manifest_hash) REFERENCES manifests(hash)
);
CREATE INDEX idx_snapshots_manifest ON snapshots(manifest_hash);

-- Manifests: path → blob hash map
CREATE TABLE manifests (
    hash    TEXT PRIMARY KEY,              -- SHA-256 of content JSON
    content TEXT NOT NULL                  -- JSON: {"/path/file": "<blob-hash>", ...}
);

-- Commits: edges between snapshots, metadata lives here (innovation point)
CREATE TABLE commits (
    id            TEXT PRIMARY KEY,        -- ULID
    from_snapshot TEXT NOT NULL,
    to_snapshot   TEXT NOT NULL,
    message       TEXT NOT NULL,
    author        TEXT,
    created_at    INTEGER NOT NULL,
    branch_id     TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK(kind IN (
        'incremental', 'full', 'merge', 'rollback'
    )),
    diff_summary  TEXT,                    -- JSON: {added:[], modified:[], removed:[]}
    FOREIGN KEY (from_snapshot) REFERENCES snapshots(id),
    FOREIGN KEY (to_snapshot) REFERENCES snapshots(id),
    FOREIGN KEY (branch_id) REFERENCES branches(id)
);
CREATE INDEX idx_commits_from    ON commits(from_snapshot);
CREATE INDEX idx_commits_to      ON commits(to_snapshot);
CREATE INDEX idx_commits_branch  ON commits(branch_id);
CREATE INDEX idx_commits_created ON commits(created_at);

-- Path annotations: N-N text attached to commit edges (innovation: edge has text, not node)
CREATE TABLE commit_path_annotations (
    id          TEXT PRIMARY KEY,
    commit_id   TEXT NOT NULL,
    text        TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (commit_id) REFERENCES commits(id) ON DELETE CASCADE
);
CREATE INDEX idx_annotations_commit ON commit_path_annotations(commit_id);

-- Branches: subgraph labels
CREATE TABLE branches (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL UNIQUE,
    kind              TEXT NOT NULL CHECK(kind IN ('main','inherited','sandbox')),
    parent_branch     TEXT,                -- inherited: parent; sandbox: source (informational)
    baseline_snapshot TEXT,                -- inherited: fork point
    head_snapshot     TEXT,                -- current HEAD
    created_at        INTEGER NOT NULL,
    FOREIGN KEY (parent_branch) REFERENCES branches(id),
    FOREIGN KEY (baseline_snapshot) REFERENCES snapshots(id),
    FOREIGN KEY (head_snapshot) REFERENCES snapshots(id)
);

-- Blob index (content lives on disk, DB only indexes)
CREATE TABLE blobs (
    hash    TEXT PRIMARY KEY,
    size    INTEGER NOT NULL,
    created INTEGER NOT NULL
);

-- Stats snapshots (maintained by route-stats plugin / built-in aggregator)
CREATE TABLE stats_snapshot (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
