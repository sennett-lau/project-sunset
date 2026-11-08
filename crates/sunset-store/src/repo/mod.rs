//! One module per group of tables. Every function takes a `&mut SqliteConnection`, which can be
//! a pooled connection or a transaction.

pub mod commits;
pub mod jobs;
pub mod knowledge;
pub mod proposals;
pub mod sessions;
pub mod users;
