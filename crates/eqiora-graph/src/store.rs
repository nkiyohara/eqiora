//! Snapshot-isolated store contract and the Phase 0 in-memory backend.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{
    Diagnostic, DynQuantity, EntityKind, GraphClass, GraphPath, Id, OntologyView, RawId,
    RawOntologyId,
};
use eqiora_schema::kernel::KernelNode;

use crate::{Committed, Edge, EdgeKind, Op, Precondition, Revision, Transaction};

/// Immutable view of one node.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    id: RawId,
    value: Option<DynQuantity>,
    kernel_definition: Option<KernelNode>,
    label: Option<String>,
}

impl Node {
    fn new(id: RawId) -> Self {
        Self {
            id,
            value: None,
            kernel_definition: None,
            label: None,
        }
    }

    fn kernel(definition: KernelNode) -> Self {
        Self {
            id: definition.id(),
            value: definition.initial_value(),
            kernel_definition: Some(definition),
            label: None,
        }
    }

    fn provenance(id: RawId, label: String) -> Self {
        Self {
            id,
            value: None,
            kernel_definition: None,
            label: Some(label),
        }
    }

    /// Typed-erased ID.
    #[must_use]
    pub const fn id(&self) -> RawId {
        self.id
    }

    /// Quantitative value, if this field/parameter has one.
    #[must_use]
    pub const fn value(&self) -> Option<DynQuantity> {
        self.value
    }

    /// Complete Semantic Kernel definition, or `None` for infrastructure and
    /// provenance nodes.
    #[must_use]
    pub const fn kernel_definition(&self) -> Option<&KernelNode> {
        self.kernel_definition.as_ref()
    }

    /// Human-readable label, currently used by provenance records.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// Immutable semantic diff/provenance record for one commit.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitRecord {
    transaction: Id<kinds::Transaction>,
    revision: Revision,
    label: String,
    ops: Vec<Op>,
}

impl CommitRecord {
    /// Provenance node ID.
    #[must_use]
    pub const fn transaction(&self) -> Id<kinds::Transaction> {
        self.transaction
    }

    /// Revision produced by this commit.
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// Recorded intent.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Ordered primitive semantic diff.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }
}

#[derive(Debug, Clone, Default)]
struct State {
    revision: Revision,
    nodes: BTreeMap<RawId, Node>,
    edges: BTreeSet<Edge>,
    ontology_views: BTreeMap<RawOntologyId, OntologyView>,
    commits: Vec<CommitRecord>,
}

impl State {
    fn has_ulid(&self, id: RawId) -> bool {
        self.nodes
            .keys()
            .any(|existing| existing.ulid() == id.ulid())
    }
}

/// Immutable, cheap snapshot of all four graphs at one revision.
#[derive(Debug, Clone)]
pub struct Snapshot {
    state: Arc<State>,
}

impl Snapshot {
    /// Snapshot revision.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.state.revision
    }

    /// Look up an exactly typed erased ID.
    #[must_use]
    pub fn node(&self, id: RawId) -> Option<&Node> {
        self.state.nodes.get(&id)
    }

    /// Iterate all nodes in deterministic `(kind, ULID)` order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &Node> {
        self.state.nodes.values()
    }

    /// Iterate all edges in deterministic endpoint/kind order.
    pub fn edges(&self) -> impl ExactSizeIterator<Item = &Edge> {
        self.state.edges.iter()
    }

    /// Look up a Standard Ontology named subgraph by its erased ID.
    #[must_use]
    pub fn ontology_view(&self, id: &RawOntologyId) -> Option<&OntologyView> {
        self.state.ontology_views.get(id)
    }

    /// Iterate ontology views in deterministic `(schema key, ULID)` order.
    /// Views are not included in [`Self::nodes`].
    pub fn ontology_views(&self) -> impl ExactSizeIterator<Item = &OntologyView> {
        self.state.ontology_views.values()
    }

    /// Outgoing edges from a node.
    pub fn outgoing(&self, id: RawId) -> impl Iterator<Item = &Edge> {
        self.state
            .edges
            .iter()
            .filter(move |edge| edge.from() == id)
    }

    /// Immutable commit history in revision order.
    #[must_use]
    pub fn commits(&self) -> &[CommitRecord] {
        &self.state.commits
    }
}

/// Storage contract for the Graph Federation.
pub trait GraphStore: Send + Sync {
    /// Validate without committing. An empty result means the transaction
    /// would succeed against the current snapshot.
    fn validate(&self, transaction: &Transaction) -> Vec<Diagnostic>;

    /// Atomically apply a transaction and append its provenance record.
    ///
    /// # Errors
    /// Returns every violated invariant/precondition; no mutation is visible.
    fn commit(&mut self, transaction: Transaction) -> Result<Committed, Vec<Diagnostic>>;

    /// Obtain a snapshot-isolated immutable view.
    fn snapshot(&self) -> Snapshot;

    /// Current federation revision.
    fn revision(&self) -> Revision;
}

/// Phase 0 backend. Commits clone a compact state and swap one [`Arc`],
/// making atomicity and snapshot isolation obvious before optimization.
#[derive(Debug, Clone, Default)]
pub struct InMemoryGraphStore {
    state: Arc<State>,
}

impl InMemoryGraphStore {
    /// Empty store at revision zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restore one complete logical snapshot at its exact nonzero revision.
    ///
    /// This is snapshot hydration, not historical replay: `transaction` must
    /// describe the complete state and carry no optimistic preconditions. The
    /// returned store leaves commit history empty because the unavailable
    /// history cannot be reconstructed from one checkpoint. Subsequent
    /// ordinary commits advance from the restored revision and record only
    /// changes made after hydration.
    ///
    /// # Errors
    /// Returns structured diagnostics if the revision is zero, the
    /// transaction has preconditions, or its complete state is invalid.
    pub fn restore_snapshot(
        transaction: Transaction,
        revision: Revision,
    ) -> Result<Self, Vec<Diagnostic>> {
        if revision == Revision::default() {
            return Err(vec![Diagnostic::error(
                codes::INVALID_OPERATION,
                "a restored graph snapshot requires a nonzero revision",
            )]);
        }
        if !transaction.preconditions().is_empty() {
            return Err(vec![Diagnostic::error(
                codes::INVALID_OPERATION,
                "a complete snapshot restoration transaction cannot carry optimistic preconditions",
            )]);
        }
        let mut candidate = apply_transaction(State::default(), &transaction)?;
        candidate.revision = revision;
        Ok(Self {
            state: Arc::new(candidate),
        })
    }
}

impl GraphStore for InMemoryGraphStore {
    fn validate(&self, transaction: &Transaction) -> Vec<Diagnostic> {
        match apply_transaction((*self.state).clone(), transaction) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics,
        }
    }

    fn commit(&mut self, transaction: Transaction) -> Result<Committed, Vec<Diagnostic>> {
        let mut candidate = apply_transaction((*self.state).clone(), &transaction)?;
        let revision = Revision(candidate.revision.0.checked_add(1).ok_or_else(|| {
            vec![Diagnostic::error(
                codes::INVALID_OPERATION,
                "federation revision counter exhausted",
            )]
        })?);
        let transaction_id = loop {
            let candidate_id = Id::<kinds::Transaction>::new();
            if !candidate.has_ulid(candidate_id.erase()) {
                break candidate_id;
            }
        };
        let raw_transaction = transaction_id.erase();
        candidate.nodes.insert(
            raw_transaction,
            Node::provenance(raw_transaction, transaction.label().to_owned()),
        );
        candidate.revision = revision;
        candidate.commits.push(CommitRecord {
            transaction: transaction_id,
            revision,
            label: transaction.label().to_owned(),
            ops: transaction.ops().to_vec(),
        });
        self.state = Arc::new(candidate);

        Ok(Committed {
            revision,
            transaction: transaction_id,
        })
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: Arc::clone(&self.state),
        }
    }

    fn revision(&self) -> Revision {
        self.state.revision
    }
}

fn apply_transaction(
    mut state: State,
    transaction: &Transaction,
) -> Result<State, Vec<Diagnostic>> {
    let initially_present = state.nodes.keys().copied().collect::<BTreeSet<_>>();
    let mut diagnostics = validate_preconditions(&state, transaction.preconditions());
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for op in transaction.ops() {
        if let Err(diagnostic) = apply_op(&mut state, op) {
            diagnostics.push(diagnostic);
        }
    }

    if diagnostics.is_empty() {
        diagnostics.extend(validate_ontology_integrity(&state, &initially_present));
    }

    if diagnostics.is_empty() {
        Ok(state)
    } else {
        Err(diagnostics)
    }
}

fn validate_preconditions(state: &State, preconditions: &[Precondition]) -> Vec<Diagnostic> {
    preconditions
        .iter()
        .filter_map(|precondition| match precondition {
            Precondition::RevisionIs(expected) if *expected != state.revision => {
                Some(Diagnostic::error(
                    codes::PRECONDITION_FAILED,
                    format!(
                        "revision precondition failed: expected {}, found {}",
                        expected.0, state.revision.0
                    ),
                ))
            }
            Precondition::ValueEquals { target, expected } => match state.nodes.get(target) {
                Some(node) if node.value == Some(*expected) => None,
                Some(node) => Some(
                    Diagnostic::error(
                        codes::PRECONDITION_FAILED,
                        format!(
                            "value precondition failed: expected {expected}, found {}",
                            node.value
                                .map_or_else(|| "<unset>".to_owned(), |value| value.to_string())
                        ),
                    )
                    .with_graph_path(path_for(*target)),
                ),
                None => Some(
                    Diagnostic::error(
                        codes::PRECONDITION_FAILED,
                        "value precondition failed: target does not exist",
                    )
                    .with_graph_path(path_for(*target)),
                ),
            },
            Precondition::RevisionIs(_) => None,
        })
        .collect()
}

fn apply_op(state: &mut State, op: &Op) -> Result<(), Diagnostic> {
    match op {
        Op::AddNode { kind, id } => add_node(state, *kind, *id),
        Op::DefineKernelNode { node } => define_kernel_node(state, node.clone()),
        Op::SetValue { target, value } => set_value(state, *target, *value),
        Op::Connect { from, to, edge } => connect(state, *from, *to, *edge),
        Op::RemoveNode { id } => remove_node(state, *id),
        Op::DefineOntologyView { view } => define_ontology_view(state, view.clone()),
        Op::RemoveOntologyView { id } => remove_ontology_view(state, id),
    }
}

fn add_node(state: &mut State, kind: EntityKind, id: RawId) -> Result<(), Diagnostic> {
    if kind != id.kind() {
        return Err(Diagnostic::error(
            codes::ID_KIND_MISMATCH,
            format!("AddNode declares {kind:?}, but ID carries {:?}", id.kind()),
        )
        .with_graph_path(path_for(id)));
    }
    if kind == EntityKind::Transaction {
        return Err(Diagnostic::error(
            codes::IMMUTABLE_PROVENANCE,
            "transaction provenance IDs are minted only by commit",
        )
        .with_graph_path(path_for(id)));
    }
    if kind.graph() == GraphClass::Semantic {
        return Err(Diagnostic::error(
            codes::INVALID_OPERATION,
            "Semantic Kernel nodes require a complete DefineKernelNode operation",
        )
        .with_graph_path(path_for(id)));
    }
    if state.has_ulid(id) {
        return Err(Diagnostic::error(
            codes::NODE_ALREADY_EXISTS,
            "identifier is already used in the federation",
        )
        .with_graph_path(path_for(id)));
    }
    state.nodes.insert(id, Node::new(id));
    Ok(())
}

fn define_kernel_node(state: &mut State, definition: KernelNode) -> Result<(), Diagnostic> {
    let id = definition.id();
    if state.has_ulid(id) {
        return Err(Diagnostic::error(
            codes::NODE_ALREADY_EXISTS,
            "identifier is already used in the federation",
        )
        .with_graph_path(path_for(id)));
    }
    state.nodes.insert(id, Node::kernel(definition));
    Ok(())
}

fn set_value(state: &mut State, target: RawId, value: DynQuantity) -> Result<(), Diagnostic> {
    let Some(node) = state.nodes.get_mut(&target) else {
        return Err(not_found(target));
    };
    if !matches!(target.kind(), EntityKind::Field | EntityKind::Parameter) {
        return Err(Diagnostic::error(
            codes::INVALID_OPERATION,
            format!("SetValue is not valid for {:?}", target.kind()),
        )
        .with_graph_path(path_for(target)));
    }
    if matches!(
        node.kernel_definition.as_ref(),
        Some(KernelNode::Field(field)) if !field.shape().is_scalar()
    ) {
        return Err(Diagnostic::error(
            codes::INVALID_OPERATION,
            "SetValue does not admit a scalar value for a shaped Field",
        )
        .with_graph_path(path_for(target)));
    }
    if node
        .kernel_definition
        .as_ref()
        .and_then(KernelNode::value_dimension)
        .is_some_and(|declared| declared != value.dim())
    {
        return Err(Diagnostic::error(
            codes::DIMENSION_MISMATCH,
            format!(
                "value dimension [{}] differs from the node definition",
                value.dim()
            ),
        )
        .with_graph_path(path_for(target)));
    }
    if node
        .value
        .is_some_and(|current| current.dim() != value.dim())
    {
        return Err(Diagnostic::error(
            codes::DIMENSION_MISMATCH,
            format!(
                "cannot change stored dimension from [{}] to [{}]",
                node.value.expect("checked as Some").dim(),
                value.dim()
            ),
        )
        .with_graph_path(path_for(target)));
    }
    node.value = Some(value);
    Ok(())
}

fn connect(
    state: &mut State,
    from: RawId,
    to: RawId,
    edge_kind: EdgeKind,
) -> Result<(), Diagnostic> {
    if !state.nodes.contains_key(&from) {
        return Err(not_found(from));
    }
    if !state.nodes.contains_key(&to) {
        return Err(not_found(to));
    }
    if !edge_kind.permits(from.kind(), to.kind()) {
        return Err(Diagnostic::error(
            codes::INVALID_EDGE,
            format!(
                "edge {edge_kind:?} does not permit {:?} -> {:?}",
                from.kind(),
                to.kind()
            ),
        )
        .with_graph_path(path_for(from)));
    }
    state.edges.insert(Edge::new(from, to, edge_kind));
    Ok(())
}

fn remove_node(state: &mut State, id: RawId) -> Result<(), Diagnostic> {
    if id.kind() == EntityKind::Transaction {
        return Err(Diagnostic::error(
            codes::IMMUTABLE_PROVENANCE,
            "committed transaction provenance cannot be removed",
        )
        .with_graph_path(path_for(id)));
    }
    if state.nodes.remove(&id).is_none() {
        return Err(not_found(id));
    }
    state
        .edges
        .retain(|edge| edge.from() != id && edge.to() != id);
    Ok(())
}

fn define_ontology_view(state: &mut State, view: OntologyView) -> Result<(), Diagnostic> {
    if state.ontology_views.contains_key(view.id()) {
        return Err(Diagnostic::error(
            codes::ONTOLOGY_VIEW_ALREADY_EXISTS,
            "ontology-view identifier is already registered",
        )
        .with_graph_path(ontology_path(view.id())));
    }
    state.ontology_views.insert(view.id().clone(), view);
    Ok(())
}

fn remove_ontology_view(state: &mut State, id: &RawOntologyId) -> Result<(), Diagnostic> {
    if state.ontology_views.remove(id).is_none() {
        return Err(Diagnostic::error(
            codes::ONTOLOGY_VIEW_NOT_FOUND,
            "ontology view does not exist",
        )
        .with_graph_path(ontology_path(id)));
    }
    Ok(())
}

fn validate_ontology_integrity(
    state: &State,
    initially_present: &BTreeSet<RawId>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for view in state.ontology_views.values() {
        for missing in view
            .members()
            .iter()
            .filter(|member| !state.nodes.contains_key(member))
        {
            let (code, message) = if initially_present.contains(missing) {
                (
                    codes::NODE_REFERENCED_BY_ONTOLOGY_VIEW,
                    format!(
                        "transaction would leave ontology view {} with a removed member",
                        view.id()
                    ),
                )
            } else {
                (
                    codes::NODE_NOT_FOUND,
                    format!(
                        "ontology view {} references a missing kernel node",
                        view.id()
                    ),
                )
            };
            diagnostics.push(Diagnostic::error(code, message).with_graph_path(path_for(*missing)));
        }
    }
    diagnostics
}

fn not_found(id: RawId) -> Diagnostic {
    Diagnostic::error(codes::NODE_NOT_FOUND, "node does not exist").with_graph_path(path_for(id))
}

fn path_for(id: RawId) -> GraphPath {
    GraphPath::new([
        id.kind().graph().name().to_owned(),
        format!("{:?}", id.kind()),
        id.to_string(),
    ])
}

fn ontology_path(id: &RawOntologyId) -> GraphPath {
    GraphPath::new([
        "ontology-view".to_owned(),
        id.schema().to_owned(),
        id.ulid().to_string(),
    ])
}
