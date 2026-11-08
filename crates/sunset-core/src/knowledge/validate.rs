//! Checks on curator output before it becomes a proposal. Invalid drafts are dropped.

use std::collections::HashSet;

use sunset_domain::{ProposedChange, VisualPayload, slugify};

use super::curator::ProposalDraft;

const MAX_CHANGES: usize = 50;
const MAX_TEXT: usize = 20_000;
const MAX_FLOW_NODES: usize = 40;

/// `existing` holds the slugs of live concepts. Returns a reason when the draft is invalid.
pub fn draft(d: &ProposalDraft, existing: &HashSet<String>) -> Result<(), String> {
    if d.summary.trim().is_empty() {
        return Err("summary is empty".into());
    }
    if d.changes.is_empty() || d.changes.len() > MAX_CHANGES {
        return Err(format!("needs 1 to {MAX_CHANGES} changes"));
    }
    let mut known: HashSet<&str> = existing.iter().map(String::as_str).collect();
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
        assert!(draft(&d, &HashSet::new()).is_err());
        assert!(draft(&d, &HashSet::from(["nope".to_owned()])).is_ok());
    }
}
