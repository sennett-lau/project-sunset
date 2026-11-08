-- Phase 1: configured model endpoints, which model each role uses, probe results, and aliases
-- added to existing concepts. API keys are NOT here; they live in the macOS Keychain.

CREATE TABLE providers (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users (id),
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL CHECK (kind IN ('demo', 'openai_compatible')),
    base_url     TEXT NOT NULL DEFAULT '',
    has_api_key  INTEGER NOT NULL DEFAULT 0,  -- whether the Keychain holds a key; never the key
    created_at   TEXT NOT NULL
);

-- Last "Test connection" result per provider and model.
CREATE TABLE model_capabilities (
    provider_id        TEXT NOT NULL REFERENCES providers (id) ON DELETE CASCADE,
    model              TEXT NOT NULL,
    capabilities_json  TEXT NOT NULL,  -- ModelCapabilities
    PRIMARY KEY (provider_id, model)
);

CREATE TABLE role_assignments (
    user_id      TEXT NOT NULL REFERENCES users (id),
    role         TEXT NOT NULL CHECK (role IN ('tutor', 'curator')),
    provider_id  TEXT NOT NULL REFERENCES providers (id),
    model        TEXT NOT NULL,
    PRIMARY KEY (user_id, role)
);

-- Aliases learned after a concept was created. Undoable like other knowledge rows.
CREATE TABLE concept_aliases (
    id                     TEXT PRIMARY KEY,
    concept_id             TEXT NOT NULL REFERENCES concepts (id),
    alias                  TEXT NOT NULL,
    created_by_commit_id   TEXT NOT NULL REFERENCES commits (id),
    retracted_by_commit_id TEXT REFERENCES commits (id)
);
CREATE INDEX concept_aliases_by_concept ON concept_aliases (concept_id);
