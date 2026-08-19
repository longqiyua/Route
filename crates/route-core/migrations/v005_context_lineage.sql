-- Migration v005: Development-context lineage on every commit
--
-- Every commit edge now records which version of the Effective
-- Development Context was live when the edge was created. This lets
-- the lineage store answer questions like:
--
--   "When AI produced commit X, which version of the Constitution /
--    Protocol / Reference registry was it obeying?"
--
-- The full Constitution / Protocol / References are NOT copied into
-- each commit row. Instead we carry a stable semantic fingerprint
-- (plus a few one-line denormalised versions for display) that can
-- be re-expanded from `.route/context/history/<fingerprint>.json`.
--
-- All columns are nullable so pre-v005 commits remain valid and
-- simply report `None` when inspected.
--
--   context_hash              -- Stable semantic hash of the full
--                                Effective Development Context (the
--                                primary lookup key).
--   constitution_version      -- Denormalised constitution.version
--                                (uint32, stored as signed int).
--   protocol_revision         -- Denormalised protocol.revision
--                                (uint64, stored as TEXT because
--                                SQLite int is signed 64-bit and the
--                                revision is monotonically bumped).
--   reference_entries_hash    -- Hash of the sorted semantic view of
--                                the reference registry; lets us
--                                quickly compare two commits without
--                                re-reading the registry.

ALTER TABLE commits ADD COLUMN context_hash              TEXT;
ALTER TABLE commits ADD COLUMN constitution_version      INTEGER;
ALTER TABLE commits ADD COLUMN protocol_revision         TEXT;
ALTER TABLE commits ADD COLUMN reference_entries_hash    TEXT;

CREATE INDEX IF NOT EXISTS idx_commits_context_hash       ON commits(context_hash);
CREATE INDEX IF NOT EXISTS idx_commits_constitution_version ON commits(constitution_version);
CREATE INDEX IF NOT EXISTS idx_commits_protocol_revision  ON commits(protocol_revision);
CREATE INDEX IF NOT EXISTS idx_commits_reference_entries_hash ON commits(reference_entries_hash);
