//! The slow path: turn an exchange into validated proposals, then apply, reject or undo them.

pub(crate) mod apply;
pub(crate) mod curator;
pub(crate) mod validate;
