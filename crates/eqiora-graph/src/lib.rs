//! **eqiora-graph** — Graph Federation store, typed transactions, and
//! semantic diffs (Layer L1).
//!
//! This crate deliberately knows nothing about Standard Ontology concepts
//! such as `Model` or `Coupling`. It stores their type-erased named subgraphs
//! in a registry beside the closed kernel/infra node maps, at the same
//! revision and through the same transaction. Schema semantics stay in
//! `eqiora-schema`; ontology views never become graph nodes.

mod edge;
mod store;
mod transaction;

pub use edge::{Edge, EdgeKind};
pub use store::{CommitRecord, GraphStore, InMemoryGraphStore, Node, Snapshot};
pub use transaction::{Committed, Op, Precondition, Revision, Transaction};
