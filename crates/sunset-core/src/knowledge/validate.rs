//! Checks on curator output before it becomes a proposal. Invalid drafts are dropped.
//! Real models drift, so names are normalized first: "VAE" and "Variational Autoencoder"
//! resolve to the same existing concept (initial spec Part II §7.7).

use std::collections::{HashMap, HashSet};

use sunset_domain::{Concept, ProposedChange, VisualPayload, slugify};

use super::curator::ProposalDraft;

const MAX_CHANGES: usize = 50;
const MAX_TEXT: usize = 20_000;
const MAX_FLOW_NODES: usize = 40;

/// Maps every name of every live concept (slug, title, aliases; compared as slugs) to the
/// concept's slug.
pub struct NameIndex {
    names: HashMap<String, String>,
    slugs: HashSet<String>,
}

impl NameIndex {
    pub fn new(concepts: &[Concept]) -> Self {
        let mut names = HashMap::new();
        for c in concepts {
            for name in std::iter::once(&c.slug)
                .chain(std::iter::once(&c.title))
                .chain(&c.aliases)
            {
                names.entry(slugify(name)).or_insert_with(|| c.slug.clone());
            }
        }
        Self {
            names,
            slugs: concepts.iter().map(|c| c.slug.clone()).collect(),
        }
    }

    /// The existing concept a name refers to, if any.
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.names.get(&slugify(name)).map(String::as_str)
    }

    /// Rewrite slugs in place: tidy them, and point new names of existing concepts at the
    /// existing slug. An upsert of a known concept then adds aliases instead of a duplicate.
    pub fn normalize(&self, d: &mut ProposalDraft) {
        let mut renamed: HashMap<String, String> = HashMap::new();
        for change in &mut d.changes {
            if let ProposedChange::UpsertConcept {
                slug,
                title,
                aliases,
            } = change
            {
                let original = slug.clone();
                let existing = std::iter::once(slug.as_str())
                    .chain(std::iter::once(title.as_str()))
                    .chain(aliases.iter().map(String::as_str))
                    .find_map(|n| self.resolve(n))
                    .map(str::to_owned);
                *slug = existing.unwrap_or_else(|| slugify(slug));
                renamed.insert(original, slug.clone());
            }
        }
        let fix = |s: &mut String| {
            if let Some(new) = renamed.get(s.as_str()) {
                *s = new.clone();
            } else if let Some(existing) = self.resolve(s) {
                *s = existing.to_owned();
            } else {
                *s = slugify(s);
            }
        };
        for change in &mut d.changes {
            match change {
                ProposedChange::UpsertConcept { .. } => {}
                ProposedChange::ReviseArticle { concept, .. }
                | ProposedChange::AddObservation { concept, .. }
                | ProposedChange::AddVisual { concept, .. } => fix(concept),
                ProposedChange::AddRelation { from, to, .. } => {
                    fix(from);
                    fix(to);
                }
            }
        }
    }
}

/// Returns a reason when the draft is invalid. Run [`NameIndex::normalize`] first.
pub fn draft(d: &ProposalDraft, existing: &NameIndex) -> Result<(), String> {
    if d.summary.trim().is_empty() {
        return Err("summary is empty".into());
    }
    if d.changes.is_empty() || d.changes.len() > MAX_CHANGES {
        return Err(format!("needs 1 to {MAX_CHANGES} changes"));
    }
    let mut known: HashSet<&str> = existing.slugs.iter().map(String::as_str).collect();
    for change in &d.changes {
        if let ProposedChange::UpsertConcept { slug, .. } = change {
            known.insert(slug);
        }
    }
    let refers = |slug: &str| -> Result<(), String> {
        if known.contains(slug) {
            Ok(())
        } else {
            Err(format!("unknown concept {slug:?}"))
        }
    };
    for change in &d.changes {
        match change {
            ProposedChange::UpsertConcept { slug, title, .. } => {
                if slug.is_empty() || slugify(slug) != *slug {
                    return Err(format!("bad slug {slug:?}"));
                }
                text(title, "title")?;
            }
            ProposedChange::ReviseArticle {
                concept,
                summary,
                body_md,
            } => {
                refers(concept)?;
                text(summary, "article summary")?;
                if body_md.len() > MAX_TEXT {
                    return Err("article body is too long".into());
                }
            }
            ProposedChange::AddRelation { from, to, .. } => {
                refers(from)?;
                refers(to)?;
                if from == to {
                    return Err("relation points to itself".into());
                }
            }
            ProposedChange::AddObservation { concept, note, .. } => {
                refers(concept)?;
                text(note, "observation note")?;
            }
            ProposedChange::AddVisual {
                concept,
                title,
                payload,
            } => {
                refers(concept)?;
                text(title, "visual title")?;
                visual(payload)?;
            }
        }
    }
    Ok(())
}

fn text(s: &str, what: &str) -> Result<(), String> {
    if s.trim().is_empty() {
        Err(format!("{what} is empty"))
    } else if s.len() > MAX_TEXT {
        Err(format!("{what} is too long"))
    } else {
        Ok(())
    }
}

/// Per-kind checks so renderers never see malformed data.
pub fn visual(payload: &VisualPayload) -> Result<(), String> {
    match payload {
        VisualPayload::ConceptFlow { nodes, edges } => {
            if nodes.is_empty() || nodes.len() > MAX_FLOW_NODES {
                return Err(format!("concept flow needs 1 to {MAX_FLOW_NODES} nodes"));
            }
            let mut ids = HashSet::new();
            for n in nodes {
                text(&n.label, "node label")?;
                if !ids.insert(n.id.as_str()) {
                    return Err(format!("duplicate node id {:?}", n.id));
                }
            }
            for e in edges {
                if !ids.contains(e.source.as_str()) || !ids.contains(e.target.as_str()) {
                    return Err(format!(
                        "edge {} -> {} names a missing node",
                        e.source, e.target
                    ));
                }
            }
        }
        VisualPayload::Mermaid { diagram } => text(diagram, "diagram")?,
        VisualPayload::Plot { series, .. } => {
            if series.is_empty() {
                return Err("plot has no series".into());
            }
            let finite = series
                .iter()
                .flat_map(|s| &s.points)
                .all(|p| p.x.is_finite() && p.y.is_finite());
            if !finite {
                return Err("plot has a non-finite point".into());
            }
        }
        VisualPayload::ParameterExplorer { params, .. } => {
            if params.is_empty() {
                return Err("explorer has no parameters".into());
            }
            for p in params {
                if !(p.min < p.max && p.step > 0.0 && (p.min..=p.max).contains(&p.default)) {
                    return Err(format!("parameter {:?} has an invalid range", p.id));
                }
            }
        }
        VisualPayload::EquationSteps { steps } => {
            if steps.is_empty() {
                return Err("no equation steps".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunset_domain::{FlowEdge, FlowNode};

    fn flow(edges: &[(&str, &str)]) -> VisualPayload {
        VisualPayload::ConceptFlow {
            nodes: ["a", "b"]
                .iter()
                .map(|id| FlowNode {
                    id: (*id).into(),
                    label: id.to_uppercase(),
                })
                .collect(),
            edges: edges
                .iter()
                .map(|(s, t)| FlowEdge {
                    source: (*s).into(),
                    target: (*t).into(),
                    label: None,
                })
                .collect(),
        }
    }

    #[test]
    fn concept_flow_edges_must_name_nodes() {
        assert!(visual(&flow(&[("a", "b")])).is_ok());
        assert!(visual(&flow(&[("a", "missing")])).is_err());
    }

    #[test]
    fn references_must_exist() {
        let d = ProposalDraft {
            summary: "s".into(),
            changes: vec![ProposedChange::ReviseArticle {
                concept: "nope".into(),
                summary: "x".into(),
                body_md: String::new(),
            }],
        };
        assert!(draft(&d, &NameIndex::new(&[])).is_err());
        assert!(draft(&d, &NameIndex::new(&[concept("nope", "Nope", &[])])).is_ok());
    }

    fn concept(slug: &str, title: &str, aliases: &[&str]) -> Concept {
        Concept {
            id: sunset_domain::ConceptId::new(),
            slug: slug.into(),
            title: title.into(),
            aliases: aliases.iter().map(|a| (*a).into()).collect(),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn normalize_maps_synonyms_to_the_existing_concept() {
        let index = NameIndex::new(&[concept(
            "variational-autoencoder",
            "Variational Autoencoder",
            &["VAE"],
        )]);
        let mut d = ProposalDraft {
            summary: "s".into(),
            changes: vec![
                ProposedChange::UpsertConcept {
                    slug: "VAE".into(),
                    title: "VAE".into(),
                    aliases: vec!["variational auto-encoder".into()],
                },
                ProposedChange::ReviseArticle {
                    concept: "VAE".into(),
                    summary: "x".into(),
                    body_md: String::new(),
                },
                ProposedChange::AddRelation {
                    from: "Variational Autoencoder".into(),
                    to: "Latent Space".into(),
                    kind: sunset_domain::RelationKind::Uses,
                    label: None,
                },
            ],
        };
        index.normalize(&mut d);
        let ProposedChange::UpsertConcept { slug, .. } = &d.changes[0] else {
            panic!()
        };
        assert_eq!(slug, "variational-autoencoder");
        let ProposedChange::ReviseArticle { concept, .. } = &d.changes[1] else {
            panic!()
        };
        assert_eq!(concept, "variational-autoencoder");
        let ProposedChange::AddRelation { from, to, .. } = &d.changes[2] else {
            panic!()
        };
        assert_eq!(
            (from.as_str(), to.as_str()),
            ("variational-autoencoder", "latent-space")
        );
    }
}
