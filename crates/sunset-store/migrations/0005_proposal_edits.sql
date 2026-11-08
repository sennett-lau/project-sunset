ALTER TABLE proposals ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;

-- Immutable snapshots preserve the original AI proposal as well as every learner edit.
CREATE TABLE proposal_versions (
    proposal_id TEXT NOT NULL REFERENCES proposals(id),
    revision INTEGER NOT NULL,
    summary TEXT NOT NULL,
    changes_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (proposal_id, revision)
);
INSERT INTO proposal_versions (proposal_id, revision, summary, changes_json, created_at)
SELECT id, 0, summary, changes_json, created_at FROM proposals;
