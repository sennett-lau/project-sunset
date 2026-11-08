-- Derived index: refreshed transactionally with knowledge commits and undo.
CREATE VIRTUAL TABLE knowledge_fts USING fts5(concept_id UNINDEXED, user_id UNINDEXED, text, tokenize='trigram');
CREATE VIEW searchable_knowledge AS
SELECT c.id, c.user_id, c.title || ' ' || c.slug || ' ' || c.aliases_json || ' ' ||
 COALESCE((SELECT group_concat(alias, ' ') FROM concept_aliases a WHERE a.concept_id=c.id AND a.retracted_by_commit_id IS NULL), '') || ' ' ||
 COALESCE((SELECT summary || ' ' || body_md FROM article_revisions r WHERE r.concept_id=c.id AND r.retracted_by_commit_id IS NULL ORDER BY version DESC LIMIT 1), '') AS text
FROM concepts c WHERE c.retracted_by_commit_id IS NULL;
CREATE TABLE vault_files (path TEXT PRIMARY KEY, content TEXT NOT NULL);
