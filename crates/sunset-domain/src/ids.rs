//! Typed UUIDv7 ids. Separate types stop a `ConceptId` being passed where a `TurnId` is expected.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

macro_rules! id_types {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type,
        )]
        #[serde(transparent)]
        #[specta(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// A new, time-ordered id (UUIDv7).
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }
    )*};
}

id_types! {
    /// The owner of a library. The desktop app has exactly one.
    UserId,
    SessionId,
    TurnId,
    ConceptId,
    /// Id of one article revision.
    RevisionId,
    RelationId,
    ObservationId,
    /// Id of a visual artifact.
    ArtifactId,
    ProposalId,
    CommitId,
    JobId,
    /// A configured model endpoint (docs/architecture.md §4.3).
    ProviderId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_v7_and_round_trip() {
        let id = ConceptId::new();
        assert_eq!(id.as_uuid().get_version_num(), 7);
        assert_eq!(id.to_string().parse::<ConceptId>().unwrap(), id);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
    }
}
