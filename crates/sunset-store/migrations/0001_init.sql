-- Canonical tables from initial spec Part II §7.2. One SQLite file holds one library.
--
-- Conventions:
--   * ids are UUIDv7 text; timestamps are RFC 3339 UTC text;
--   * *_json columns hold serde JSON of sunset-domain types;
--   * knowledge rows are never deleted. A row made by a commit carries created_by_commit_id;
--     an undo commit hides it by setting retracted_by_commit_id. `commits` is append-only.
--   * user_id columns keep the schema reusable by a multi-user server. The desktop app has one
--     local user.

CREATE TABLE users (
    id          TEXT PRIMARY KEY,
    save_mode   TEXT NOT NULL DEFAULT 'approval' CHECK (save_mode IN ('approval', 'auto')),
    created_at  TEXT NOT NULL
);

CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users (id),
    created_at  TEXT NOT NULL,
    ended_at    TEXT
);

CREATE TABLE turns (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions (id),
    speaker     TEXT NOT NULL CHECK (speaker IN ('user', 'tutor')),
    text        TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
CREATE INDEX turns_by_session ON turns (session_id, created_at);

CREATE TABLE commits (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users (id),
    proposal_id TEXT NOT NULL,
    undo_of     TEXT REFERENCES commits (id),
    created_at  TEXT NOT NULL
);

CREATE TABLE proposals (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users (id),
    session_id     TEXT NOT NULL REFERENCES sessions (id),
    evidence_json  TEXT NOT NULL,  -- [TurnId]
    summary        TEXT NOT NULL,
    changes_json   TEXT NOT NULL,  -- [ProposedChange]
    status         TEXT NOT NULL
                   CHECK (status IN ('awaiting_approval', 'committed', 'rejected', 'undone')),
    commit_id      TEXT REFERENCES commits (id),
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE INDEX proposals_by_status ON proposals (status, created_at);

CREATE TABLE concepts (
    id                     TEXT PRIMARY KEY,
    user_id                TEXT NOT NULL REFERENCES users (id),
    slug                   TEXT NOT NULL,
    title                  TEXT NOT NULL,
    aliases_json           TEXT NOT NULL,  -- [String]
    created_at             TEXT NOT NULL,
    created_by_commit_id   TEXT NOT NULL REFERENCES commits (id),
    retracted_by_commit_id TEXT REFERENCES commits (id)
);
-- A slug is unique among live concepts; an undone concept frees its slug.
CREATE UNIQUE INDEX concepts_live_slug ON concepts (user_id, slug)
    WHERE retracted_by_commit_id IS NULL;

CREATE TABLE article_revisions (
    id                     TEXT PRIMARY KEY,
    concept_id             TEXT NOT NULL REFERENCES concepts (id),
    version                INTEGER NOT NULL,
    summary                TEXT NOT NULL,
    body_md                TEXT NOT NULL,
    commit_id              TEXT NOT NULL REFERENCES commits (id),
    created_at             TEXT NOT NULL,
    retracted_by_commit_id TEXT REFERENCES commits (id),
    UNIQUE (concept_id, version)
);

CREATE TABLE relations (
    id                     TEXT PRIMARY KEY,
    user_id                TEXT NOT NULL REFERENCES users (id),
    from_concept_id        TEXT NOT NULL REFERENCES concepts (id),
    kind                   TEXT NOT NULL,
    to_concept_id          TEXT NOT NULL REFERENCES concepts (id),
    label                  TEXT,
    evidence_json          TEXT NOT NULL,  -- [TurnId]
    created_by_commit_id   TEXT NOT NULL REFERENCES commits (id),
    retracted_by_commit_id TEXT REFERENCES commits (id)
);
CREATE INDEX relations_from ON relations (from_concept_id);
CREATE INDEX relations_to ON relations (to_concept_id);

-- Private learning overlay. Never exported with articles.
CREATE TABLE learning_observations (
    id                     TEXT PRIMARY KEY,
    user_id                TEXT NOT NULL REFERENCES users (id),
    concept_id             TEXT NOT NULL REFERENCES concepts (id),
    kind                   TEXT NOT NULL,
    source                 TEXT NOT NULL,
    note                   TEXT NOT NULL,
    evidence_json          TEXT NOT NULL,  -- [TurnId]
    confidence             REAL,
    recheck_after          TEXT,
    created_at             TEXT NOT NULL,
    created_by_commit_id   TEXT NOT NULL REFERENCES commits (id),
    retracted_by_commit_id TEXT REFERENCES commits (id)
);
CREATE INDEX observations_by_concept ON learning_observations (concept_id);

CREATE TABLE visual_artifacts (
    id                     TEXT PRIMARY KEY,
    concept_id             TEXT NOT NULL REFERENCES concepts (id),
    revision_id            TEXT REFERENCES article_revisions (id),
    title                  TEXT NOT NULL,
    kind                   TEXT NOT NULL,  -- VisualPayload tag, duplicated for filtering
    payload_json           TEXT NOT NULL,  -- VisualPayload
    renderer_version       INTEGER NOT NULL,
    created_at             TEXT NOT NULL,
    created_by_commit_id   TEXT NOT NULL REFERENCES commits (id),
    retracted_by_commit_id TEXT REFERENCES commits (id)
);
CREATE INDEX visuals_by_concept ON visual_artifacts (concept_id);

-- [P4] Sources and citations. Created now so the schema matches the spec from the start.
CREATE TABLE sources (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users (id),
    kind          TEXT NOT NULL,
    uri           TEXT,
    title         TEXT,
    content_hash  TEXT,
    license       TEXT,
    created_at    TEXT NOT NULL
);

CREATE TABLE source_spans (
    id            TEXT PRIMARY KEY,
    source_id     TEXT REFERENCES sources (id),
    turn_id       TEXT REFERENCES turns (id),
    locator_json  TEXT NOT NULL,  -- page/section/char offsets
    excerpt       TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    CHECK (source_id IS NOT NULL OR turn_id IS NOT NULL)
);

CREATE TABLE claims (
    id             TEXT PRIMARY KEY,
    concept_id     TEXT NOT NULL REFERENCES concepts (id),
    revision_id    TEXT REFERENCES article_revisions (id),
    text           TEXT NOT NULL,
    evidence_json  TEXT NOT NULL,  -- [source span id]
    verification   TEXT NOT NULL DEFAULT 'unverified',
    created_at     TEXT NOT NULL
);

-- Durable background work. Resumed on restart.
CREATE TABLE jobs (
    id               TEXT PRIMARY KEY,
    kind             TEXT NOT NULL,
    payload_json     TEXT NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('queued', 'running', 'done', 'failed')),
    attempts         INTEGER NOT NULL DEFAULT 0,
    last_error       TEXT,
    idempotency_key  TEXT NOT NULL UNIQUE,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE INDEX jobs_by_status ON jobs (status, created_at);
