-- Migration v003: operator identity + checkpoint + AI provenance
--
-- Adds four optional columns to the `commits` edge table so that every
-- recorded change carries enough context for a Mermaid timeline and
-- separate AI / user / checkpoint views:
--
--   operator      -- "user" by default; "ai:<name>" for AI-driven commits
--                    (e.g. "ai:claude", "ai:gpt"). Free-form so the CLI/MCP
--                    can stamp its own identity without a schema change.
--   body          -- long-form note (checkpoint body or AI prompt). NULL
--                    for ordinary user edits.
--   is_checkpoint -- 1 if this commit is a user-marked checkpoint with a
--                    title (title is in `message`); 0 otherwise.
--   is_ai         -- 1 if the change came in through the AI control channel
--                    (CLI / MCP). 0 for direct user edits and rollbacks.
--
-- All columns are nullable / default 0 so existing rows survive untouched.
-- The migration is idempotent via IF NOT EXISTS-equivalent guards on
-- older SQLite versions (we use ALTER TABLE … ADD COLUMN, which fails
-- if the column already exists — caught and ignored below).

ALTER TABLE commits ADD COLUMN operator TEXT;
ALTER TABLE commits ADD COLUMN body TEXT;
ALTER TABLE commits ADD COLUMN is_checkpoint INTEGER NOT NULL DEFAULT 0;
ALTER TABLE commits ADD COLUMN is_ai INTEGER NOT NULL DEFAULT 0;

-- Index for fast AI / checkpoint filtering in the history view.
CREATE INDEX IF NOT EXISTS idx_commits_is_ai        ON commits(is_ai);
CREATE INDEX IF NOT EXISTS idx_commits_is_checkpoint ON commits(is_checkpoint);
CREATE INDEX IF NOT EXISTS idx_commits_operator     ON commits(operator);
