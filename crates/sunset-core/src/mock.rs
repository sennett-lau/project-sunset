//! The built-in "Demo" provider: a scripted tutor and curator that know one topic,
//! "What is a VAE?". The app uses it until a real model is configured, and tests use it.

use std::sync::Arc;
use std::time::Duration;

use sunset_domain::{
    FlowEdge, FlowNode, ObservationKind, ObservationSource, ProposedChange, RelationKind,
    VisualPayload,
};
use sunset_providers::ChatRequest;
use sunset_providers::adapters::mock::MockChatModel;

use sunset_domain::Role;
use sunset_providers::ChatModel;

use crate::knowledge::curator::{ANSWER_LABEL, CuratorOutput, ProposalDraft, QUESTION_LABEL};

/// The only model name the Demo provider offers.
pub const DEMO_MODEL: &str = "scripted";

/// The scripted model for `role`. `chunk_delay` paces the streamed answer; zero in tests.
pub fn model(role: Role, chunk_delay: Duration) -> Arc<dyn ChatModel> {
    match role {
        Role::Tutor => {
            Arc::new(MockChatModel::new("demo/tutor", tutor).with_chunk_delay(chunk_delay))
        }
        Role::Curator => Arc::new(MockChatModel::new("demo/curator", curator)),
    }
}

fn is_about_vae(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("vae") || t.contains("variational autoencoder")
}

fn tutor(req: &ChatRequest) -> String {
    if is_about_vae(req.last_user_message().unwrap_or_default()) {
        VAE_ANSWER.to_owned()
    } else {
        "(Demo model) I am a scripted stand-in that knows one topic. Try asking \
         \"What is a VAE?\", or connect a real model under Models."
            .to_owned()
    }
}

const VAE_ANSWER: &str = "A variational autoencoder (VAE) is a neural network that learns to \
compress data into a small set of numbers and then rebuild it.\n\n\
It has two halves. The encoder turns an input, such as an image, into a latent representation: \
not a single point, but a small Gaussian distribution described by a mean and a spread. The \
decoder takes a sample from that distribution and tries to reconstruct the original input.\n\n\
Training balances two goals: reconstructions should look like the inputs, and the latent \
distributions should stay close to a standard Gaussian. That second goal keeps the latent space \
smooth, so you can sample new points from it and decode them into new, plausible data.\n\n\
Related topics you might explore next: latent diffusion models, or the reparameterization trick.";

fn curator(req: &ChatRequest) -> String {
    let exchange = req.last_user_message().unwrap_or_default();
    let question = exchange
        .split_once(QUESTION_LABEL)
        .and_then(|(_, rest)| rest.split_once(ANSWER_LABEL))
        .map(|(q, _)| q.trim())
        .unwrap_or_default();
    let output = if is_about_vae(question) {
        CuratorOutput {
            proposals: vec![vae_proposal(question)],
        }
    } else {
        CuratorOutput { proposals: vec![] }
    };
    serde_json::to_string(&output).unwrap_or_default()
}

fn vae_proposal(question: &str) -> ProposalDraft {
    const VAE: &str = "variational-autoencoder";
    const AE: &str = "autoencoder";
    const LATENT: &str = "latent-representation";
    let node = |id: &str, label: &str| FlowNode {
        id: id.into(),
        label: label.into(),
    };
    let edge = |s: &str, t: &str| FlowEdge {
        source: s.into(),
        target: t.into(),
        label: None,
    };
    ProposalDraft {
        summary: "Add Variational Autoencoder, linked to Autoencoder and Latent Representation"
            .into(),
        changes: vec![
            ProposedChange::UpsertConcept {
                slug: VAE.into(),
                title: "Variational Autoencoder".into(),
                aliases: vec!["VAE".into()],
            },
            ProposedChange::ReviseArticle {
                concept: VAE.into(),
                summary: "A generative model that encodes data as a probability distribution \
                          in a latent space and decodes samples back into data."
                    .into(),
                body_md: "## Intuition\n\nAn **encoder** maps an input to a Gaussian \
                          distribution over a [[latent-representation]]. A **decoder** maps a \
                          sample from it back to data.\n\n## Training\n\nThe loss combines \
                          reconstruction error with a KL-divergence term that keeps each latent \
                          distribution close to a standard Gaussian. This keeps the latent space \
                          smooth enough to sample from.\n\n## Related\n\nA VAE is a kind of \
                          [[autoencoder]]."
                    .into(),
            },
            ProposedChange::UpsertConcept {
                slug: AE.into(),
                title: "Autoencoder".into(),
                aliases: vec![],
            },
            ProposedChange::ReviseArticle {
                concept: AE.into(),
                summary: "A neural network trained to reconstruct its input through a smaller \
                          intermediate representation."
                    .into(),
                body_md: String::new(),
            },
            ProposedChange::UpsertConcept {
                slug: LATENT.into(),
                title: "Latent Representation".into(),
                aliases: vec!["latent space".into()],
            },
            ProposedChange::ReviseArticle {
                concept: LATENT.into(),
                summary: "A compact set of learned features that describes data in fewer \
                          dimensions."
                    .into(),
                body_md: String::new(),
            },
            ProposedChange::AddRelation {
                from: VAE.into(),
                to: AE.into(),
                kind: RelationKind::IsA,
                label: None,
            },
            ProposedChange::AddRelation {
                from: VAE.into(),
                to: LATENT.into(),
                kind: RelationKind::Uses,
                label: Some("learns a distribution over".into()),
            },
            ProposedChange::AddObservation {
                concept: VAE.into(),
                kind: ObservationKind::Question,
                source: ObservationSource::Observed,
                note: format!("Asked: \"{question}\""),
            },
            ProposedChange::AddVisual {
                concept: VAE.into(),
                title: "What a VAE encoder and decoder do".into(),
                payload: VisualPayload::ConceptFlow {
                    nodes: vec![
                        node("x", "Input"),
                        node("enc", "Encoder"),
                        node("z", "Latent representation"),
                        node("dec", "Decoder"),
                        node("out", "Reconstruction"),
                    ],
                    edges: vec![
                        edge("x", "enc"),
                        edge("enc", "z"),
                        edge("z", "dec"),
                        edge("dec", "out"),
                    ],
                },
            },
        ],
    }
}
