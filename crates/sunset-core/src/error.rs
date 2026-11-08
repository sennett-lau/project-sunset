use sunset_providers::ProviderError;
use sunset_store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("{0} not found")]
    NotFound(&'static str),
    /// The request does not fit the current state, e.g. approving a rejected proposal.
    #[error("{0}")]
    Conflict(String),
    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
