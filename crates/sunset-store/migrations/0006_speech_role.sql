-- Speech is optional. Preserve existing chat assignments and do not silently select a provider.
CREATE TABLE role_assignments_with_speech (
    user_id      TEXT NOT NULL REFERENCES users (id),
    role         TEXT NOT NULL CHECK (role IN ('tutor', 'curator', 'stt')),
    provider_id  TEXT NOT NULL REFERENCES providers (id),
    model        TEXT NOT NULL,
    PRIMARY KEY (user_id, role)
);
INSERT INTO role_assignments_with_speech SELECT * FROM role_assignments;
DROP TABLE role_assignments;
ALTER TABLE role_assignments_with_speech RENAME TO role_assignments;
