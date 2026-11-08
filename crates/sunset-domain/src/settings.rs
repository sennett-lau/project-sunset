use serde::{Deserialize, Serialize};
use specta::Type;

/// Whether valid proposals are committed at once or wait for the user.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SaveMode {
    #[default]
    Approval,
    Auto,
}
