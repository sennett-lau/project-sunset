use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{CommitId, ConceptId, RevisionId, Timestamp};

/// One version of a concept's general (shareable) article. Never edited in place: a change is a
/// new revision. The current article is the newest revision that has not been undone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArticleRevision {
    pub id: RevisionId,
    pub concept_id: ConceptId,
    /// 1, 2, 3, ... per concept.
    pub version: u32,
    /// Short definition shown first.
    pub summary: String,
    /// Article body as Markdown.
    pub body_md: String,
    pub commit_id: CommitId,
    pub created_at: Timestamp,
}
