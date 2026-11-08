//! Teaching visuals as typed data. The model never writes HTML or JavaScript; the UI renders
//! each `kind` with a trusted component. See docs/architecture.md §3.1.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ArtifactId, ConceptId, RevisionId, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VisualArtifact {
    pub id: ArtifactId,
    pub concept_id: ConceptId,
    /// The article revision this visual was made for, if any.
    pub revision_id: Option<RevisionId>,
    pub title: String,
    pub payload: VisualPayload,
    /// Bumped when a renderer changes how it reads a payload.
    pub renderer_version: u32,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum VisualPayload {
    /// Boxes and arrows, e.g. Input → Encoder → Latent → Decoder → Reconstruction.
    ConceptFlow {
        nodes: Vec<FlowNode>,
        edges: Vec<FlowEdge>,
    },
    /// Mermaid source, rendered with `securityLevel: "strict"`.
    Mermaid {
        diagram: String,
    },
    Plot {
        plot_type: PlotType,
        x_label: String,
        y_label: String,
        series: Vec<DataSeries>,
    },
    /// Sliders driving a built-in demo. `preset` names the trusted component.
    ParameterExplorer {
        preset: ExplorerPreset,
        params: Vec<SliderParam>,
        caption: String,
    },
    EquationSteps {
        steps: Vec<EquationStep>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FlowEdge {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PlotType {
    Line,
    Scatter,
    Bar,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct DataSeries {
    pub name: String,
    pub points: Vec<PlotPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct PlotPoint {
    pub x: f64,
    pub y: f64,
}

/// Built-in interactive demos. Add a variant together with its React component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExplorerPreset {
    /// 1-D Gaussian; params `mean` and `std_dev`.
    Gaussian1d,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct SliderParam {
    pub id: String,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub default: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct EquationStep {
    /// LaTeX, rendered with KaTeX.
    pub latex: String,
    pub explanation: String,
}
