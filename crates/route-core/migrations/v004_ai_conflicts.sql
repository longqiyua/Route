-- Migration v004: AI conflict resolution (智能取舍)
--
-- Adds a single table that records the user's verdict on every conflict
-- surfaced by an AI commit. The agent embeds `CONFLICTS:` lines in its
-- commit body (see AiPrompt::summary_prompt); the timeline view parses
-- them and asks the user to choose between the old logic, the AI's
-- replacement, or to keep both. The verdict is persisted here and
-- surfaced again on the history page so the user can revisit what
-- they decided months later.
--
-- Two files are saved for every "keep_both" verdict:
--   <project>/.route/conflicts/<commit_id>__<path>.old
--   <project>/.route/conflicts/<commit_id>__<path>.ai
-- so the user can recover either version at any time.

CREATE TABLE IF NOT EXISTS ai_conflict_verdicts (
    id          TEXT PRIMARY KEY,
    commit_id   TEXT NOT NULL,
    path        TEXT NOT NULL,
    verdict     TEXT NOT NULL,            -- "keep_old" | "keep_ai" | "keep_both"
    note        TEXT,
    created_at  INTEGER NOT NULL,
    UNIQUE(commit_id, path)
);

CREATE INDEX IF NOT EXISTS idx_ai_conflict_verdicts_commit
    ON ai_conflict_verdicts(commit_id);
