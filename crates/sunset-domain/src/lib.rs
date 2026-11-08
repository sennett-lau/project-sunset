//! Plain data types shared by every Sunset crate. No I/O and no business logic.
//!
//! TypeScript bindings for the UI are generated from these types.

mod identity;

pub use identity::{AppIdentity, IdentityError};
