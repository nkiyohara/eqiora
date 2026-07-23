//! Typed transaction contract.

use eqiora_core::{DynQuantity, EntityKind, Id, OntologyView, RawId, RawOntologyId, entity::kinds};
use eqiora_schema::kernel::KernelNode;

use crate::EdgeKind;

/// Monotonic revision of the federation, assigned at commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Revision(pub u64);

/// A precondition checked atomically at commit time.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Precondition {
    /// The target node still carries the expected value.
    ValueEquals {
        /// Node whose value is asserted.
        target: RawId,
        /// Expected current value.
        expected: DynQuantity,
    },
    /// The federation is still at the given revision.
    RevisionIs(Revision),
}

/// One primitive mutation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Op {
    /// Create a node. Attributes are supplied by later operations and the
    /// schema layer; the store verifies that `kind` agrees with `id`.
    AddNode {
        /// Kind of non-semantic infrastructure node to create. Semantic
        /// Kernel nodes use [`Self::DefineKernelNode`].
        kind: EntityKind,
        /// Pre-minted typed identifier, erased at the wire boundary.
        id: RawId,
    },
    /// Create one complete, locally validated Semantic Kernel node.
    DefineKernelNode {
        /// Typed definition; its enum variant determines the entity kind.
        node: KernelNode,
    },
    /// Set a quantitative parameter or field default.
    SetValue {
        /// Field or parameter to mutate.
        target: RawId,
        /// New SI value with runtime dimension at this storage boundary.
        value: DynQuantity,
    },
    /// Create a kernel-schema-approved edge.
    Connect {
        /// Source node.
        from: RawId,
        /// Destination node.
        to: RawId,
        /// Closed edge vocabulary; unchecked strings never enter the store.
        edge: EdgeKind,
    },
    /// Remove a node and its incident edges.
    RemoveNode {
        /// Node to remove.
        id: RawId,
    },
    /// Register a typed named subgraph beside the graph node maps.
    DefineOntologyView {
        /// Structurally and schema-validated, type-erased view.
        view: OntologyView,
    },
    /// Remove a named subgraph without removing any member kernel nodes.
    RemoveOntologyView {
        /// View identifier to remove.
        id: RawOntologyId,
    },
}

/// A typed transaction over all four graphs.
#[derive(Debug, Default)]
pub struct Transaction {
    label: String,
    ops: Vec<Op>,
    preconditions: Vec<Precondition>,
}

impl Transaction {
    /// Start a transaction with a human-readable intent label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ops: Vec::new(),
            preconditions: Vec::new(),
        }
    }

    /// Intent label recorded in the Action & Provenance Graph.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Append a mutation.
    pub fn push(&mut self, op: Op) -> &mut Self {
        self.ops.push(op);
        self
    }

    /// Append a commit-time precondition.
    pub fn require(&mut self, precondition: Precondition) -> &mut Self {
        self.preconditions.push(precondition);
        self
    }

    /// Queued mutations, in order.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// Queued preconditions.
    #[must_use]
    pub fn preconditions(&self) -> &[Precondition] {
        &self.preconditions
    }
}

/// Result of a successful atomic commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Committed {
    /// New federation revision.
    pub revision: Revision,
    /// Immutable provenance node created for the commit.
    pub transaction: Id<kinds::Transaction>,
}
