//! Typed named subgraphs for the extensible Standard Ontology.
//!
//! An ontology view is first-class in APIs, transactions, and provenance,
//! but it is deliberately **not** a Graph Federation node. Its members are
//! real Semantic Kernel nodes, addressed by [`crate::RawId`]; the separate
//! [`OntologyId`] namespace prevents an ontology handle from being confused
//! with a graph-node [`crate::Id`] accepted by transactions and kernel
//! definitions.

use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use std::collections::BTreeSet;

use ulid::Ulid;

use crate::diagnostic::codes;
use crate::{Diagnostic, EntityKind, GraphClass, GraphPath, RawId};

/// Schema marker for one kind of Standard Ontology view.
///
/// Unlike the sealed [`crate::Entity`] trait, this trait is intentionally
/// open: third-party crates may define ontology schemas without changing the
/// Semantic Kernel. Keys are stable wire identifiers and should include a
/// version, for example `eqiora.model/v1`.
pub trait OntologySchema: 'static {
    /// Stable, globally unique schema key.
    const KEY: &'static str;

    /// Validate invariants specific to this schema after common structural
    /// checks have passed.
    ///
    /// The default accepts every structurally valid view, which is useful for
    /// third-party schemas whose constraints live in generated validators.
    fn validate(_members: &BTreeSet<RawId>, _boundary: &BTreeSet<RawId>) -> Result<(), Diagnostic> {
        Ok(())
    }
}

/// Strongly typed identifier for a Standard Ontology named subgraph.
///
/// This type has no conversion to [`crate::Id`]: ontology views are not
/// graph nodes.
pub struct OntologyId<S: OntologySchema> {
    ulid: Ulid,
    _marker: PhantomData<fn() -> S>,
}

impl<S: OntologySchema> OntologyId<S> {
    /// Mint a fresh, globally unique ontology-view identifier.
    #[must_use]
    pub fn new() -> Self {
        Self::from_ulid(Ulid::generate())
    }

    /// Rebuild a typed identifier from its serialized ULID.
    #[must_use]
    pub const fn from_ulid(ulid: Ulid) -> Self {
        Self {
            ulid,
            _marker: PhantomData,
        }
    }

    /// The underlying ULID.
    #[must_use]
    pub const fn ulid(&self) -> Ulid {
        self.ulid
    }

    /// Erase the schema type while retaining its stable key.
    #[must_use]
    pub fn erase(self) -> RawOntologyId {
        RawOntologyId {
            schema: S::KEY.to_owned(),
            ulid: self.ulid,
        }
    }
}

impl<S: OntologySchema> Default for OntologyId<S> {
    fn default() -> Self {
        Self::new()
    }
}

// Manual implementations avoid imposing irrelevant trait bounds on `S`.
impl<S: OntologySchema> Clone for OntologyId<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S: OntologySchema> Copy for OntologyId<S> {}
impl<S: OntologySchema> PartialEq for OntologyId<S> {
    fn eq(&self, other: &Self) -> bool {
        self.ulid == other.ulid
    }
}
impl<S: OntologySchema> Eq for OntologyId<S> {}
impl<S: OntologySchema> PartialOrd for OntologyId<S> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<S: OntologySchema> Ord for OntologyId<S> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.ulid.cmp(&other.ulid)
    }
}
impl<S: OntologySchema> Hash for OntologyId<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ulid.hash(state);
    }
}
impl<S: OntologySchema> fmt::Debug for OntologyId<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OntologyId<{}>({})", S::KEY, self.ulid)
    }
}
impl<S: OntologySchema> fmt::Display for OntologyId<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ulid)
    }
}

/// Type-erased ontology identifier for stores and wire formats.
///
/// The owned schema key is retained so dynamically loaded third-party
/// schemas survive type erasure and serialization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawOntologyId {
    schema: String,
    ulid: Ulid,
}

impl RawOntologyId {
    /// Stable ontology schema key.
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// The underlying ULID.
    #[must_use]
    pub const fn ulid(&self) -> Ulid {
        self.ulid
    }

    /// Recover a typed identifier only when the schema key matches.
    #[must_use]
    pub fn downcast<S: OntologySchema>(&self) -> Option<OntologyId<S>> {
        (self.schema == S::KEY).then(|| OntologyId::from_ulid(self.ulid))
    }
}

impl<S: OntologySchema> From<OntologyId<S>> for RawOntologyId {
    fn from(id: OntologyId<S>) -> Self {
        id.erase()
    }
}

impl fmt::Display for RawOntologyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.schema, self.ulid)
    }
}

/// A schema-typed, named subgraph over Semantic Kernel nodes.
///
/// This constructor validates only the schema-independent structural
/// invariants. Schema-specific rules belong in the schema crate; existence
/// of referenced nodes at a revision belongs in the graph store.
pub struct NamedSubgraph<S: OntologySchema> {
    id: OntologyId<S>,
    members: BTreeSet<RawId>,
    boundary: BTreeSet<RawId>,
}

impl<S: OntologySchema> NamedSubgraph<S> {
    /// Build and structurally validate a named subgraph.
    ///
    /// # Errors
    /// Returns `EQ0201` if the schema key is empty, the member set is empty,
    /// a member is not semantic, or the boundary is not a subset of members
    /// made entirely of ports.
    pub fn new(
        id: OntologyId<S>,
        members: impl IntoIterator<Item = RawId>,
        boundary: impl IntoIterator<Item = RawId>,
    ) -> Result<Self, Diagnostic> {
        let members = members.into_iter().collect::<BTreeSet<_>>();
        let boundary = boundary.into_iter().collect::<BTreeSet<_>>();
        let path = ontology_path(S::KEY, id.ulid);

        if S::KEY.trim().is_empty() {
            return Err(invalid_view("ontology schema key must not be empty", path));
        }
        if members.is_empty() {
            return Err(invalid_view(
                "ontology view must contain at least one kernel node",
                path,
            ));
        }
        if let Some(member) = members
            .iter()
            .find(|member| member.kind().graph() != GraphClass::Semantic)
        {
            return Err(invalid_view(
                format!(
                    "ontology member {member} belongs to {}, not the Semantic Model Graph",
                    member.kind().graph().name()
                ),
                path,
            ));
        }
        if let Some(boundary_id) = boundary
            .iter()
            .find(|boundary_id| !members.contains(boundary_id))
        {
            return Err(invalid_view(
                format!("boundary ID {boundary_id} is not a member of the view"),
                path,
            ));
        }
        if let Some(boundary_id) = boundary
            .iter()
            .find(|boundary_id| boundary_id.kind() != EntityKind::Port)
        {
            return Err(invalid_view(
                format!("boundary ID {boundary_id} is not a Port"),
                path,
            ));
        }

        S::validate(&members, &boundary)
            .map_err(|diagnostic| diagnostic.with_graph_path(path.clone()))?;

        Ok(Self {
            id,
            members,
            boundary,
        })
    }

    /// Typed view identifier.
    #[must_use]
    pub const fn id(&self) -> OntologyId<S> {
        self.id
    }

    /// Member Semantic Kernel IDs in deterministic order.
    #[must_use]
    pub fn members(&self) -> &BTreeSet<RawId> {
        &self.members
    }

    /// Boundary Port IDs in deterministic order.
    #[must_use]
    pub fn boundary(&self) -> &BTreeSet<RawId> {
        &self.boundary
    }

    /// Erase the schema type for storage without turning the view into a node.
    #[must_use]
    pub fn erase(self) -> OntologyView {
        OntologyView {
            id: self.id.erase(),
            members: self.members,
            boundary: self.boundary,
        }
    }
}

impl<S: OntologySchema> Clone for NamedSubgraph<S> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            members: self.members.clone(),
            boundary: self.boundary.clone(),
        }
    }
}
impl<S: OntologySchema> PartialEq for NamedSubgraph<S> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.members == other.members && self.boundary == other.boundary
    }
}
impl<S: OntologySchema> Eq for NamedSubgraph<S> {}
impl<S: OntologySchema> fmt::Debug for NamedSubgraph<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedSubgraph")
            .field("schema", &S::KEY)
            .field("id", &self.id)
            .field("members", &self.members)
            .field("boundary", &self.boundary)
            .finish()
    }
}

/// Type-erased ontology view stored beside, not inside, the node maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyView {
    id: RawOntologyId,
    members: BTreeSet<RawId>,
    boundary: BTreeSet<RawId>,
}

impl OntologyView {
    /// Type-erased view identifier.
    #[must_use]
    pub fn id(&self) -> &RawOntologyId {
        &self.id
    }

    /// Member Semantic Kernel IDs in deterministic order.
    #[must_use]
    pub fn members(&self) -> &BTreeSet<RawId> {
        &self.members
    }

    /// Boundary Port IDs in deterministic order.
    #[must_use]
    pub fn boundary(&self) -> &BTreeSet<RawId> {
        &self.boundary
    }

    /// Recover the schema-typed view only when the stable key matches.
    #[must_use]
    pub fn downcast<S: OntologySchema>(&self) -> Option<NamedSubgraph<S>> {
        self.id.downcast().map(|id| NamedSubgraph {
            id,
            members: self.members.clone(),
            boundary: self.boundary.clone(),
        })
    }
}

impl<S: OntologySchema> From<NamedSubgraph<S>> for OntologyView {
    fn from(view: NamedSubgraph<S>) -> Self {
        view.erase()
    }
}

fn invalid_view(message: impl Into<String>, path: GraphPath) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ONTOLOGY_VIEW, message).with_graph_path(path)
}

fn ontology_path(schema: &str, ulid: Ulid) -> GraphPath {
    GraphPath::new([
        "ontology-view".to_owned(),
        schema.to_owned(),
        ulid.to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Id;
    use crate::entity::kinds;

    struct Model;
    impl OntologySchema for Model {
        const KEY: &'static str = "test.model/v1";
    }

    struct Scale;
    impl OntologySchema for Scale {
        const KEY: &'static str = "test.scale/v1";
    }

    #[test]
    fn schema_identity_survives_type_erasure() {
        let id = OntologyId::<Model>::new();
        let raw = id.erase();

        assert_eq!(raw.downcast::<Model>(), Some(id));
        assert_eq!(raw.downcast::<Scale>(), None);
    }

    #[test]
    fn named_subgraph_enforces_structural_invariants() {
        let field = Id::<kinds::Field>::new().erase();
        let port = Id::<kinds::Port>::new().erase();
        let view = NamedSubgraph::<Model>::new(OntologyId::new(), [field, port], [port])
            .expect("semantic members and a port boundary are valid");

        assert_eq!(view.members().len(), 2);
        assert_eq!(view.boundary().len(), 1);
    }

    #[test]
    fn non_port_boundary_is_rejected() {
        let field = Id::<kinds::Field>::new().erase();
        let error = NamedSubgraph::<Model>::new(OntologyId::new(), [field], [field])
            .expect_err("a Field cannot be a boundary Port");

        assert_eq!(error.code(), codes::INVALID_ONTOLOGY_VIEW);
    }

    #[test]
    fn non_semantic_member_is_rejected() {
        let artifact = Id::<kinds::Artifact>::new().erase();
        let error = NamedSubgraph::<Model>::new(OntologyId::new(), [artifact], [])
            .expect_err("ontology views contain only semantic nodes");

        assert_eq!(error.code(), codes::INVALID_ONTOLOGY_VIEW);
    }
}
