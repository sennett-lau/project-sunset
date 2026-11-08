use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ConceptId, Timestamp};

/// A node of the wiki and graph. Its article lives in [`crate::ArticleRevision`]s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Concept {
    pub id: ConceptId,
    /// Unique within a library. Used for `[[WikiLinks]]` and vault file names.
    pub slug: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub created_at: Timestamp,
}

/// Turns a title into a slug: lowercase, words joined by '-'. Keeps non-Latin letters, so
/// Cantonese titles stay readable (`"變分自編碼器"` stays as is).
pub fn slugify(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut dash = false;
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.push(c);
        } else {
            dash = true;
        }
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugs() {
        assert_eq!(
            slugify("Variational Autoencoder"),
            "variational-autoencoder"
        );
        assert_eq!(slugify("  VAE (model)! "), "vae-model");
        assert_eq!(slugify("變分 自編碼器"), "變分-自編碼器");
        assert_eq!(slugify("!!!"), "");
    }
}
