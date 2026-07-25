use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{
    Diagnostic, DimExponents, DynQuantity, Entity, EntityKind, Id, OntologyId, RawId, ValueShape,
};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Revision, Transaction};
use eqiora_schema::kernel::pure_operator::{
    CalculusBuilder, CalculusNode, CalculusNodeId, ExactRational, PureOperatorDefinition,
    PureValueClass, ResultAxis,
};
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, AxisBounds, BoundaryPairing, BoundaryPhysicalConnector,
    BoundarySide, ClockDomainDef, ClockKind, ConnectionDef, ConnectionSemantics, DomainDef,
    DomainKind, EventDirection, ExprDag, ExprDagBuilder, ExprId, ExprNode, FieldDef, KernelNode,
    ParameterDef, PortDef, PortPayload, RationalTime, RelationDef, RepresentationDef,
    RepresentationKind, SignalDirection, SymbolRef, UnaryMathFunction, ValueFrame,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{ArtifactDigest, JsonDecoderLimits, check_json_limits, invalid_artifact};

const MODEL_SCHEMA: &str = "eqiora.model-envelope/v1";
const CANONICAL_ENCODING: &str = "eqiora.canonical-json/v1";

/// Semantic work budgets shared by Model and Model-transaction generations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelDecoderLimits {
    /// Common JSON syntax admission.
    pub json: JsonDecoderLimits,
    /// Maximum Semantic Kernel nodes in one Model envelope.
    pub max_nodes: usize,
    /// Maximum graph edges in one Model envelope.
    pub max_edges: usize,
    /// Maximum expression nodes summed across one Model or transaction.
    pub max_expression_nodes: usize,
    /// Maximum expression roots summed across one Model or transaction.
    pub max_expression_roots: usize,
    /// Maximum pure-operator definitions summed across expressions.
    pub max_pure_operator_definitions: usize,
    /// Maximum pure-operator formals summed across definitions.
    pub max_pure_operator_formals: usize,
    /// Maximum exact component-calculus nodes summed across definitions.
    pub max_pure_operator_calculus_nodes: usize,
    /// Maximum ordered arguments summed across pure-operator applications.
    pub max_pure_operator_application_arguments: usize,
    /// Maximum Semantic Model members.
    pub max_model_view_members: usize,
    /// Maximum model-root boundary Ports.
    pub max_model_boundary: usize,
    /// Maximum rank of one exact Semantic Model value shape.
    pub max_value_shape_rank: usize,
    /// Maximum checked scalar components in one Semantic Model value shape.
    pub max_value_shape_components: usize,
    /// Maximum ordered operations in one Model transaction.
    pub max_transaction_ops: usize,
    /// Maximum atomic preconditions in one Model transaction.
    pub max_transaction_preconditions: usize,
}

impl Default for ModelDecoderLimits {
    fn default() -> Self {
        Self {
            json: JsonDecoderLimits::default(),
            max_nodes: 100_000,
            max_edges: 1_000_000,
            max_expression_nodes: 1_000_000,
            max_expression_roots: 1_000_000,
            max_pure_operator_definitions: 100_000,
            max_pure_operator_formals: 1_000_000,
            max_pure_operator_calculus_nodes: 4_000_000,
            max_pure_operator_application_arguments: 4_000_000,
            max_model_view_members: 100_000,
            max_model_boundary: 100_000,
            max_value_shape_rank: 8,
            max_value_shape_components: 4_096,
            max_transaction_ops: 1_000_000,
            max_transaction_preconditions: 100_000,
        }
    }
}

/// Versioned serialization of one validated canonical Semantic Model.
///
/// The envelope records its source federation revision for provenance. The
/// content digest excludes that revision so byte-identical model meaning at a
/// later graph revision retains the same content identity.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEnvelopeV1 {
    wire: WireModelEnvelopeV1,
}

impl ModelEnvelopeV1 {
    /// Encode one immutable, validated Semantic Kernel program.
    ///
    /// # Errors
    /// Returns `EQ0901` for a newer unsupported kernel variant or an envelope
    /// exceeding default decoder resource limits.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        let nodes = program
            .nodes()
            .map(WireNode::encode_v1)
            .collect::<Result<Vec<_>, _>>()?;
        let values = program
            .nodes()
            .filter_map(|node| {
                program.value(node.id()).map(|value| WireValue {
                    target: WireId::from_raw(node.id()),
                    value: WireQuantity::encode(value),
                })
            })
            .collect();
        let edges = program
            .edges()
            .iter()
            .map(|edge| {
                Ok(WireEdge {
                    from: WireId::from_raw(edge.from()),
                    to: WireId::from_raw(edge.to()),
                    kind: WireEdgeKind::encode(edge.kind())?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let boundary = program
            .boundary()
            .iter()
            .copied()
            .map(WireId::from_raw)
            .collect();
        let mut envelope = Self {
            wire: WireModelEnvelopeV1 {
                schema: MODEL_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                source_revision: program.revision().0,
                model_ulid: program.model().ulid().to_string(),
                nodes,
                values,
                edges,
                boundary,
            },
        };
        envelope.canonicalize_and_validate(ModelDecoderLimits::default())?;
        Ok(envelope)
    }

    /// Decode and structurally validate an envelope without mutating a store.
    ///
    /// # Errors
    /// Returns `EQ0901` for oversized, malformed, unknown-version, duplicate,
    /// dangling, or locally invalid model data.
    pub fn from_json(bytes: &[u8], limits: ModelDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits.json)?;
        let wire = serde_json::from_slice(bytes)
            .map_err(|error| invalid_artifact(format!("invalid model envelope JSON: {error}")))?;
        let mut envelope = Self { wire };
        envelope.canonicalize_and_validate(limits)?;
        Ok(envelope)
    }

    /// Deterministic canonical JSON, including source revision provenance.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire)
            .map_err(|error| invalid_artifact(format!("cannot serialize model envelope: {error}")))
    }

    /// Domain-separated SHA-256 identity of semantic content.
    ///
    /// Source revision is intentionally excluded; model identity, definitions,
    /// current values, edges, and boundary are included.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical content serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        let content = WireModelContentV1 {
            schema: &self.wire.schema,
            encoding: &self.wire.encoding,
            model_ulid: &self.wire.model_ulid,
            nodes: &self.wire.nodes,
            values: &self.wire.values,
            edges: &self.wire.edges,
            boundary: &self.wire.boundary,
        };
        let bytes = serde_json::to_vec(&content).map_err(|error| {
            invalid_artifact(format!("cannot serialize model content: {error}"))
        })?;
        Ok(ArtifactDigest::compute(MODEL_SCHEMA.as_bytes(), &bytes))
    }

    /// Decode the complete model into one typed graph transaction without
    /// mutating a store.
    ///
    /// # Errors
    /// Returns structured diagnostics when locally valid wire data cannot be
    /// reconstructed into the closed Semantic Model transaction vocabulary.
    pub fn to_transaction(&self) -> Result<(Transaction, OntologyId<Model>), Vec<Diagnostic>> {
        let model_ulid = parse_ulid(&self.wire.model_ulid).map_err(|error| vec![error])?;
        let model = OntologyId::<Model>::from_ulid(model_ulid);
        let mut ids = BTreeMap::new();
        let mut definitions = Vec::with_capacity(self.wire.nodes.len());
        for wire_node in &self.wire.nodes {
            let definition = wire_node.decode().map_err(|error| vec![error])?;
            if ids.insert(wire_node.id.clone(), definition.id()).is_some() {
                return Err(vec![invalid_artifact(
                    "model envelope contains a duplicate kernel node ID",
                )]);
            }
            definitions.push(definition);
        }

        let mut transaction = Transaction::new("decode eqiora.model-envelope/v1");
        for node in definitions {
            transaction.push(Op::DefineKernelNode { node });
        }
        for value in &self.wire.values {
            let target = lookup_id(&ids, &value.target).map_err(|error| vec![error])?;
            transaction.push(Op::SetValue {
                target,
                value: value.value.decode().map_err(|error| vec![error])?,
            });
        }
        for edge in &self.wire.edges {
            let from = lookup_id(&ids, &edge.from).map_err(|error| vec![error])?;
            let to = lookup_id(&ids, &edge.to).map_err(|error| vec![error])?;
            transaction.push(Op::Connect {
                from,
                to,
                edge: edge.kind.decode(),
            });
        }
        let boundary = self
            .wire
            .boundary
            .iter()
            .map(|id| lookup_id(&ids, id))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| vec![error])?;
        let view =
            ModelView::new(model, ids.values().copied(), boundary).map_err(|error| vec![error])?;
        transaction.push(Op::DefineOntologyView { view: view.into() });

        Ok((transaction, model))
    }

    /// Reconstruct through typed definitions, one atomic transaction, and
    /// whole-model `KernelProgram` validation.
    ///
    /// # Errors
    /// Returns structured diagnostics from wire conversion, graph commit, or
    /// whole-model validation. No partial graph state is observable.
    pub fn to_program(&self) -> Result<KernelProgram, Vec<Diagnostic>> {
        let (transaction, model) = self.to_transaction()?;
        let store =
            InMemoryGraphStore::restore_snapshot(transaction, Revision(self.source_revision()))?;
        KernelProgram::from_snapshot(&store.snapshot(), model)
    }

    /// Source Graph Federation revision recorded as provenance.
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.wire.source_revision
    }

    /// Typed Semantic Model identity carried by this envelope.
    ///
    /// # Errors
    /// Returns `EQ0901` only if internal validated state was corrupted.
    pub fn model(&self) -> Result<OntologyId<Model>, Diagnostic> {
        parse_ulid(&self.wire.model_ulid).map(OntologyId::from_ulid)
    }

    fn canonicalize_and_validate(&mut self, limits: ModelDecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != MODEL_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported model-envelope schema or canonical encoding",
            ));
        }
        if self.wire.source_revision == 0 {
            return Err(invalid_artifact(
                "model envelope source revision must be nonzero",
            ));
        }
        parse_ulid(&self.wire.model_ulid)?;
        if self.wire.nodes.is_empty() || self.wire.nodes.len() > limits.max_nodes {
            return Err(invalid_artifact(format!(
                "model envelope requires 1..={} nodes, found {}",
                limits.max_nodes,
                self.wire.nodes.len()
            )));
        }
        require_decoder_count(
            "model-view members",
            self.wire.nodes.len(),
            limits.max_model_view_members,
        )?;
        require_decoder_count(
            "model boundary Ports",
            self.wire.boundary.len(),
            limits.max_model_boundary,
        )?;
        if self.wire.edges.len() > limits.max_edges {
            return Err(invalid_artifact(format!(
                "model envelope has {} edges, exceeding the {} edge limit",
                self.wire.edges.len(),
                limits.max_edges
            )));
        }
        let expression_nodes = self.wire.nodes.iter().try_fold(0_usize, |count, node| {
            count
                .checked_add(node.expression_node_count())
                .ok_or_else(|| invalid_artifact("expression-node count overflows usize"))
        })?;
        if expression_nodes > limits.max_expression_nodes {
            return Err(invalid_artifact(format!(
                "model envelope has {expression_nodes} expression nodes, exceeding the {} node limit",
                limits.max_expression_nodes
            )));
        }
        let expression_roots = checked_count_sum(
            self.wire.nodes.iter().map(WireNode::expression_root_count),
            "expression-root count",
        )?;
        require_decoder_count(
            "model expression roots",
            expression_roots,
            limits.max_expression_roots,
        )?;

        self.wire
            .nodes
            .sort_by(|left, right| left.id.cmp(&right.id));
        reject_adjacent_duplicates(
            self.wire
                .nodes
                .windows(2)
                .map(|pair| pair[0].id == pair[1].id),
            "kernel node ID",
        )?;
        self.wire
            .values
            .sort_by(|left, right| left.target.cmp(&right.target));
        reject_adjacent_duplicates(
            self.wire
                .values
                .windows(2)
                .map(|pair| pair[0].target == pair[1].target),
            "current value target",
        )?;
        self.wire.edges.sort();
        reject_adjacent_duplicates(
            self.wire.edges.windows(2).map(|pair| pair[0] == pair[1]),
            "graph edge",
        )?;
        self.wire.boundary.sort();
        reject_adjacent_duplicates(
            self.wire.boundary.windows(2).map(|pair| pair[0] == pair[1]),
            "boundary ID",
        )?;

        // Decode locally now so malformed definitions fail before callers can
        // mistake a parsed envelope for a validated artifact.
        for node in &self.wire.nodes {
            node.ensure_v1()?;
            node.decode()?;
        }
        for value in &self.wire.values {
            value.value.decode()?;
        }
        let ids = self
            .wire
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<BTreeSet<_>>();
        for value in &self.wire.values {
            require_reference(&ids, &value.target, "current value")?;
        }
        for edge in &self.wire.edges {
            require_reference(&ids, &edge.from, "edge source")?;
            require_reference(&ids, &edge.to, "edge target")?;
            let from = edge.from.decode_raw()?;
            let to = edge.to.decode_raw()?;
            if !edge.kind.decode().permits(from.kind(), to.kind()) {
                return Err(invalid_artifact(
                    "wire edge endpoints violate the closed graph edge schema",
                ));
            }
        }
        for boundary in &self.wire.boundary {
            require_reference(&ids, boundary, "boundary")?;
            if boundary.kind != WireEntityKind::Port {
                return Err(invalid_artifact("model boundary may contain only Port IDs"));
            }
        }
        Ok(())
    }
}

fn lookup_id(ids: &BTreeMap<WireId, RawId>, id: &WireId) -> Result<RawId, Diagnostic> {
    ids.get(id)
        .copied()
        .ok_or_else(|| invalid_artifact(format!("wire reference {} is not a model node", id.ulid)))
}

fn require_reference(ids: &BTreeSet<WireId>, id: &WireId, label: &str) -> Result<(), Diagnostic> {
    if ids.contains(id) {
        Ok(())
    } else {
        Err(invalid_artifact(format!(
            "{label} {} is not a model node",
            id.ulid
        )))
    }
}

fn reject_adjacent_duplicates(
    duplicates: impl IntoIterator<Item = bool>,
    label: &str,
) -> Result<(), Diagnostic> {
    if duplicates.into_iter().any(|duplicate| duplicate) {
        Err(invalid_artifact(format!(
            "model envelope contains a duplicate {label}"
        )))
    } else {
        Ok(())
    }
}

pub(crate) fn checked_count_sum(
    counts: impl IntoIterator<Item = usize>,
    label: &str,
) -> Result<usize, Diagnostic> {
    counts.into_iter().try_fold(0_usize, |total, count| {
        total
            .checked_add(count)
            .ok_or_else(|| invalid_artifact(format!("{label} overflows usize")))
    })
}

pub(crate) fn require_decoder_count(
    label: &str,
    actual: usize,
    limit: usize,
) -> Result<(), Diagnostic> {
    if actual > limit {
        Err(invalid_artifact(format!(
            "{label} count {actual} exceeds decoder limit {limit}",
        )))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireModelEnvelopeV1 {
    schema: String,
    encoding: String,
    source_revision: u64,
    model_ulid: String,
    nodes: Vec<WireNode>,
    values: Vec<WireValue>,
    edges: Vec<WireEdge>,
    boundary: Vec<WireId>,
}

#[derive(Serialize)]
struct WireModelContentV1<'a> {
    schema: &'a str,
    encoding: &'a str,
    model_ulid: &'a str,
    nodes: &'a [WireNode],
    values: &'a [WireValue],
    edges: &'a [WireEdge],
    boundary: &'a [WireId],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireNode {
    pub(crate) id: WireId,
    definition: WireNodeDefinition,
}

impl WireNode {
    pub(crate) fn encode_v1(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V1)
    }

    pub(crate) fn encode_v2(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V2)
    }

    pub(crate) fn encode_v3(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V3)
    }

    pub(crate) fn encode_v4(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V4)
    }

    pub(crate) fn encode_v5(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V5)
    }

    pub(crate) fn encode_v6(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V6)
    }

    fn encode(node: &KernelNode, version: WireVersion) -> Result<Self, Diagnostic> {
        let definition = match node {
            KernelNode::Domain(value) => WireNodeDefinition::Domain {
                domain: WireDomainKind::encode(value.kind(), version)?,
            },
            KernelNode::Representation(value) => WireNodeDefinition::Representation {
                representation: WireRepresentationKind::encode(value.kind())?,
            },
            KernelNode::Field(value) if version.supports_shaped_fields() => {
                WireNodeDefinition::ShapedField {
                    dimension: WireDimension::encode(value.dimension()),
                    shape: WireValueShape::encode(value.shape()),
                    frame: WireValueFrame::encode(value.frame()),
                    initial: value.initial().map(WireQuantity::encode),
                }
            }
            KernelNode::Field(value)
                if value.shape().is_scalar() && value.frame() == ValueFrame::Invariant =>
            {
                WireNodeDefinition::Field {
                    dimension: WireDimension::encode(value.dimension()),
                    initial: value.initial().map(WireQuantity::encode),
                }
            }
            KernelNode::Field(_) => {
                return Err(invalid_artifact("shaped Field requires model wire v3"));
            }
            KernelNode::Parameter(value) => WireNodeDefinition::Parameter {
                value: WireQuantity::encode(value.value()),
            },
            KernelNode::Port(value) => match value.payload() {
                PortPayload::ScalarPhysical { domain } if version.supports_scalar_physical() => {
                    WireNodeDefinition::ScalarPhysicalPort {
                        domain: WireId::from_raw(domain.erase()),
                    }
                }
                PortPayload::ScalarPhysical { .. } => {
                    return Err(invalid_artifact(
                        "scalar physical Port requires model wire v2",
                    ));
                }
                PortPayload::BoundaryPhysical {
                    connector,
                    boundary,
                } if version.supports_boundary_physical() => {
                    WireNodeDefinition::BoundaryPhysicalPort {
                        connector: WireId::from_raw(connector.erase()),
                        boundary: WireId::from_raw(boundary.erase()),
                    }
                }
                PortPayload::BoundaryPhysical { .. } => {
                    return Err(invalid_artifact(
                        "field-valued boundary physical Port requires model wire v3",
                    ));
                }
                payload => WireNodeDefinition::Port {
                    port: WirePortKind::encode(payload)?,
                    dimension: WireDimension::encode(
                        value
                            .signal_contract()
                            .map(|(_, dimension)| dimension)
                            .or_else(|| value.marker_dimension())
                            .ok_or_else(|| invalid_artifact("Port payload has no v1 dimension"))?,
                    ),
                },
            },
            KernelNode::Relation(value) => WireNodeDefinition::Relation {
                residuals: WireExpression::encode(value.residuals(), version)?,
            },
            KernelNode::Activation(value) => WireNodeDefinition::Activation {
                activation: WireActivationKind::encode(value.kind(), version)?,
            },
            KernelNode::Connection(value) => WireNodeDefinition::Connection {
                connection: WireConnectionKind::encode(value.semantics(), version)?,
            },
            KernelNode::ClockDomain(value) => WireNodeDefinition::ClockDomain {
                clock: WireClockKind::encode(value.kind())?,
            },
            _ => {
                return Err(invalid_artifact(
                    "kernel node variant is newer than wire v1",
                ));
            }
        };
        Ok(Self {
            id: WireId::from_raw(node.id()),
            definition,
        })
    }

    pub(crate) fn decode(&self) -> Result<KernelNode, Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Domain { domain } => {
                let id = self.id.typed::<kinds::Domain>()?;
                Ok(domain.decode(id)?.into())
            }
            WireNodeDefinition::Representation { representation } => {
                let id = self.id.typed::<kinds::Representation>()?;
                Ok(match representation {
                    WireRepresentationKind::Abstract => RepresentationDef::new(id),
                    WireRepresentationKind::Continuum => RepresentationDef::continuum(id),
                }
                .into())
            }
            WireNodeDefinition::Field { dimension, initial } => {
                let id = self.id.typed::<kinds::Field>()?;
                let mut definition = FieldDef::new(id, dimension.decode());
                if let Some(initial) = initial {
                    definition = definition
                        .with_initial(initial.decode()?)
                        .map_err(|error| invalid_artifact(error.message()))?;
                }
                Ok(definition.into())
            }
            WireNodeDefinition::ShapedField {
                dimension,
                shape,
                frame,
                initial,
            } => {
                let id = self.id.typed::<kinds::Field>()?;
                let mut definition =
                    FieldDef::shaped(id, dimension.decode(), shape.decode()?, frame.decode())
                        .map_err(|error| invalid_artifact(error.message()))?;
                if let Some(initial) = initial {
                    definition = definition
                        .with_initial(initial.decode()?)
                        .map_err(|error| invalid_artifact(error.message()))?;
                }
                Ok(definition.into())
            }
            WireNodeDefinition::Parameter { value } => {
                Ok(ParameterDef::new(self.id.typed::<kinds::Parameter>()?, value.decode()?).into())
            }
            WireNodeDefinition::Port { port, dimension } => Ok(port
                .decode(self.id.typed::<kinds::Port>()?, dimension.decode())
                .into()),
            WireNodeDefinition::ScalarPhysicalPort { domain } => Ok(PortDef::scalar_physical(
                self.id.typed::<kinds::Port>()?,
                domain.typed::<kinds::Domain>()?,
            )
            .into()),
            WireNodeDefinition::BoundaryPhysicalPort {
                connector,
                boundary,
            } => Ok(PortDef::boundary_physical(
                self.id.typed::<kinds::Port>()?,
                connector.typed::<kinds::Domain>()?,
                boundary.typed::<kinds::Domain>()?,
            )
            .into()),
            WireNodeDefinition::Relation { residuals } => Ok(RelationDef::new(
                self.id.typed::<kinds::Relation>()?,
                residuals.decode()?,
            )
            .into()),
            WireNodeDefinition::Activation { activation } => Ok(ActivationDef::new(
                self.id.typed::<kinds::Activation>()?,
                activation.decode()?,
            )
            .map_err(|error| invalid_artifact(error.message()))?
            .into()),
            WireNodeDefinition::Connection { connection } => Ok(ConnectionDef::new(
                self.id.typed::<kinds::Connection>()?,
                connection.decode(),
            )
            .into()),
            WireNodeDefinition::ClockDomain { clock } => {
                Ok(clock.decode(self.id.typed::<kinds::ClockDomain>()?)?.into())
            }
        }
    }

    pub(crate) fn expression_node_count(&self) -> usize {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.nodes.len(),
            WireNodeDefinition::Activation { activation } => activation.expression_node_count(),
            _ => 0,
        }
    }

    pub(crate) fn expression_root_count(&self) -> usize {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.roots.len(),
            WireNodeDefinition::Activation { activation } => activation.expression_root_count(),
            _ => 0,
        }
    }

    pub(crate) fn pure_operator_counts(&self) -> Result<PureOperatorWireCounts, Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.pure_operator_counts(),
            WireNodeDefinition::Activation { activation } => activation.pure_operator_counts(),
            _ => Ok(PureOperatorWireCounts::default()),
        }
    }

    pub(crate) fn validate_v5_features(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.validate_v5_features(),
            WireNodeDefinition::Activation { activation } => activation.validate_v5_features(),
            _ => Ok(()),
        }
    }

    pub(crate) fn canonicalize_v5_definitions(&mut self) -> Result<(), Diagnostic> {
        match &mut self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.canonicalize_v5_definitions(),
            WireNodeDefinition::Activation { activation } => {
                activation.canonicalize_v5_definitions()
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v1(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Domain {
                domain: WireDomainKind::ScalarPhysical { .. },
            }
            | WireNodeDefinition::ScalarPhysicalPort { .. }
            | WireNodeDefinition::Domain {
                domain: WireDomainKind::BoundaryPhysical { .. },
            }
            | WireNodeDefinition::ShapedField { .. }
            | WireNodeDefinition::BoundaryPhysicalPort { .. }
            | WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "model wire v1 cannot contain physical interface semantics or shaped Fields",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v1(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v1(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v2(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Domain {
                domain: WireDomainKind::BoundaryPhysical { .. },
            }
            | WireNodeDefinition::ShapedField { .. }
            | WireNodeDefinition::BoundaryPhysicalPort { .. }
            | WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "model wire v2 cannot contain boundary physical semantics or shaped Fields",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v2(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v2(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v3(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v3 requires the single shaped Field representation",
            )),
            WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v3(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v3(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v4(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v4 requires the single shaped Field representation",
            )),
            WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v4(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v4(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v5(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v5 requires the single shaped Field representation",
            )),
            WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v6(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v6 requires the single shaped Field representation",
            )),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_value_shape_limits(
        &self,
        limits: ModelDecoderLimits,
    ) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::ShapedField { shape, .. }
            | WireNodeDefinition::Domain {
                domain: WireDomainKind::BoundaryPhysical { shape, .. },
            } => shape.ensure_limits(limits),
            _ => Ok(()),
        }
    }

    pub(crate) fn semantic_references(&self) -> Vec<&WireId> {
        match &self.definition {
            WireNodeDefinition::ScalarPhysicalPort { domain } => vec![domain],
            WireNodeDefinition::BoundaryPhysicalPort {
                connector,
                boundary,
            } => vec![connector, boundary],
            WireNodeDefinition::Relation { residuals } => residuals.semantic_references(),
            WireNodeDefinition::Activation { activation } => activation.semantic_references(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WireVersion {
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
}

impl WireVersion {
    const fn supports_scalar_physical(self) -> bool {
        matches!(self, Self::V2 | Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }

    const fn supports_boundary_physical(self) -> bool {
        matches!(self, Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }

    const fn supports_shaped_fields(self) -> bool {
        matches!(self, Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireNodeDefinition {
    Domain {
        domain: WireDomainKind,
    },
    Representation {
        representation: WireRepresentationKind,
    },
    Field {
        dimension: WireDimension,
        initial: Option<WireQuantity>,
    },
    ShapedField {
        dimension: WireDimension,
        shape: WireValueShape,
        frame: WireValueFrame,
        initial: Option<WireQuantity>,
    },
    Parameter {
        value: WireQuantity,
    },
    Port {
        port: WirePortKind,
        dimension: WireDimension,
    },
    ScalarPhysicalPort {
        domain: WireId,
    },
    BoundaryPhysicalPort {
        connector: WireId,
        boundary: WireId,
    },
    Relation {
        residuals: WireExpression,
    },
    Activation {
        activation: WireActivationKind,
    },
    Connection {
        connection: WireConnectionKind,
    },
    ClockDomain {
        clock: WireClockKind,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireDomainKind {
    Abstract,
    CartesianBox {
        bounds: Vec<WireAxisBounds>,
    },
    CartesianBoundary {
        axis: usize,
        side: WireBoundarySide,
    },
    ScalarPhysical {
        across_dimension: WireDimension,
        through_dimension: WireDimension,
    },
    BoundaryPhysical {
        trace_dimension: WireDimension,
        flux_dimension: WireDimension,
        shape: WireValueShape,
        frame: WireValueFrame,
        pairing: WireBoundaryPairing,
    },
}

impl WireDomainKind {
    fn encode(value: &DomainKind, version: WireVersion) -> Result<Self, Diagnostic> {
        Ok(match value {
            DomainKind::Abstract => Self::Abstract,
            DomainKind::CartesianBox { bounds } => Self::CartesianBox {
                bounds: bounds.iter().copied().map(WireAxisBounds::encode).collect(),
            },
            DomainKind::CartesianBoundary { axis, side } => Self::CartesianBoundary {
                axis: *axis,
                side: WireBoundarySide::encode(*side),
            },
            DomainKind::ScalarPhysical {
                across_dimension,
                through_dimension,
            } if version.supports_scalar_physical() => Self::ScalarPhysical {
                across_dimension: WireDimension::encode(*across_dimension),
                through_dimension: WireDimension::encode(*through_dimension),
            },
            DomainKind::BoundaryPhysical { connector } if version.supports_boundary_physical() => {
                Self::BoundaryPhysical {
                    trace_dimension: WireDimension::encode(connector.trace_dimension()),
                    flux_dimension: WireDimension::encode(connector.flux_dimension()),
                    shape: WireValueShape::encode(connector.shape()),
                    frame: WireValueFrame::encode(connector.frame()),
                    pairing: WireBoundaryPairing::encode(connector.pairing()),
                }
            }
            _ => {
                return Err(invalid_artifact(
                    "the model contains a Domain kind unsupported by this model wire",
                ));
            }
        })
    }

    fn decode(&self, id: Id<kinds::Domain>) -> Result<DomainDef, Diagnostic> {
        match self {
            Self::Abstract => Ok(DomainDef::new(id)),
            Self::CartesianBox { bounds } => DomainDef::cartesian_box(
                id,
                bounds
                    .iter()
                    .map(WireAxisBounds::decode)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .map_err(|error| invalid_artifact(error.message())),
            Self::CartesianBoundary { axis, side } => {
                Ok(DomainDef::cartesian_boundary(id, *axis, side.decode()))
            }
            Self::ScalarPhysical {
                across_dimension,
                through_dimension,
            } => Ok(DomainDef::scalar_physical(
                id,
                across_dimension.decode(),
                through_dimension.decode(),
            )),
            Self::BoundaryPhysical {
                trace_dimension,
                flux_dimension,
                shape,
                frame,
                pairing,
            } => Ok(DomainDef::boundary_physical(
                id,
                BoundaryPhysicalConnector::new(
                    trace_dimension.decode(),
                    flux_dimension.decode(),
                    shape.decode()?,
                    frame.decode(),
                    pairing.decode(),
                )
                .map_err(|_| invalid_artifact("invalid boundary physical connector contract"))?,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct WireValueShape(Vec<u32>);

impl WireValueShape {
    fn encode(value: &ValueShape) -> Self {
        Self(value.extents().iter().map(|extent| extent.get()).collect())
    }

    fn decode(&self) -> Result<ValueShape, Diagnostic> {
        ValueShape::new(self.0.iter().copied()).map_err(|error| {
            invalid_artifact(format!(
                "wire value shape has an invalid extent at axis {}",
                error.axis()
            ))
        })
    }

    fn ensure_limits(&self, limits: ModelDecoderLimits) -> Result<(), Diagnostic> {
        require_decoder_count(
            "value-shape rank",
            self.0.len(),
            limits.max_value_shape_rank,
        )?;
        let shape = self.decode()?;
        let components = shape.component_count().ok_or_else(|| {
            invalid_artifact("wire value-shape component product exceeds local usize")
        })?;
        require_decoder_count(
            "value-shape scalar components",
            components,
            limits.max_value_shape_components,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireValueFrame {
    Invariant,
    SpatialCartesian,
}

impl WireValueFrame {
    const fn encode(value: ValueFrame) -> Self {
        match value {
            ValueFrame::Invariant => Self::Invariant,
            ValueFrame::SpatialCartesian => Self::SpatialCartesian,
        }
    }

    const fn decode(self) -> ValueFrame {
        match self {
            Self::Invariant => ValueFrame::Invariant,
            Self::SpatialCartesian => ValueFrame::SpatialCartesian,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireBoundaryPairing {
    EuclideanBoundaryDuality,
}

impl WireBoundaryPairing {
    const fn encode(value: BoundaryPairing) -> Self {
        match value {
            BoundaryPairing::EuclideanBoundaryDuality => Self::EuclideanBoundaryDuality,
        }
    }

    const fn decode(self) -> BoundaryPairing {
        match self {
            Self::EuclideanBoundaryDuality => BoundaryPairing::EuclideanBoundaryDuality,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAxisBounds {
    lower: WireQuantity,
    upper: WireQuantity,
}

impl WireAxisBounds {
    fn encode(value: AxisBounds) -> Self {
        Self {
            lower: WireQuantity::encode(value.lower()),
            upper: WireQuantity::encode(value.upper()),
        }
    }

    fn decode(&self) -> Result<AxisBounds, Diagnostic> {
        AxisBounds::new(self.lower.decode()?, self.upper.decode()?)
            .map_err(|error| invalid_artifact(error.message()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireBoundarySide {
    Lower,
    Upper,
}

impl WireBoundarySide {
    const fn encode(value: BoundarySide) -> Self {
        match value {
            BoundarySide::Lower => Self::Lower,
            BoundarySide::Upper => Self::Upper,
        }
    }

    const fn decode(self) -> BoundarySide {
        match self {
            Self::Lower => BoundarySide::Lower,
            Self::Upper => BoundarySide::Upper,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireRepresentationKind {
    Abstract,
    Continuum,
}

impl WireRepresentationKind {
    fn encode(value: RepresentationKind) -> Result<Self, Diagnostic> {
        match value {
            RepresentationKind::Abstract => Ok(Self::Abstract),
            RepresentationKind::Continuum => Ok(Self::Continuum),
            _ => Err(invalid_artifact(
                "representation kind is newer than model wire v1",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WirePortKind {
    Signal { direction: WireSignalDirection },
    Conserving,
}

impl WirePortKind {
    fn encode(value: PortPayload) -> Result<Self, Diagnostic> {
        match value {
            PortPayload::Signal { direction, .. } => Ok(Self::Signal {
                direction: WireSignalDirection::encode(direction),
            }),
            PortPayload::ConservingMarker { .. } => Ok(Self::Conserving),
            _ => Err(invalid_artifact(
                "Port payload is newer than the supported model wire vocabulary",
            )),
        }
    }

    const fn decode(self, id: Id<kinds::Port>, dimension: DimExponents) -> PortDef {
        match self {
            Self::Signal { direction } => PortDef::signal(id, direction.decode(), dimension),
            Self::Conserving => PortDef::conserving_marker(id, dimension),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSignalDirection {
    Input,
    Output,
}

impl WireSignalDirection {
    const fn encode(value: SignalDirection) -> Self {
        match value {
            SignalDirection::Input => Self::Input,
            SignalDirection::Output => Self::Output,
        }
    }

    const fn decode(self) -> SignalDirection {
        match self {
            Self::Input => SignalDirection::Input,
            Self::Output => SignalDirection::Output,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireActivationKind {
    Continuous,
    Periodic,
    Event {
        guard: WireExpression,
        direction: WireEventDirection,
    },
    Guard {
        guard: WireExpression,
    },
}

impl WireActivationKind {
    fn encode(value: &ActivationKind, version: WireVersion) -> Result<Self, Diagnostic> {
        match value {
            ActivationKind::Continuous => Ok(Self::Continuous),
            ActivationKind::Periodic => Ok(Self::Periodic),
            ActivationKind::Event { guard, direction } => Ok(Self::Event {
                guard: WireExpression::encode(guard, version)?,
                direction: WireEventDirection::encode(*direction),
            }),
            ActivationKind::Guard { guard } => Ok(Self::Guard {
                guard: WireExpression::encode(guard, version)?,
            }),
            _ => Err(invalid_artifact(
                "Activation kind is newer than the supported model wire vocabulary",
            )),
        }
    }

    fn decode(&self) -> Result<ActivationKind, Diagnostic> {
        Ok(match self {
            Self::Continuous => ActivationKind::Continuous,
            Self::Periodic => ActivationKind::Periodic,
            Self::Event { guard, direction } => ActivationKind::Event {
                guard: guard.decode()?,
                direction: direction.decode(),
            },
            Self::Guard { guard } => ActivationKind::Guard {
                guard: guard.decode()?,
            },
        })
    }

    fn expression_node_count(&self) -> usize {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.nodes.len(),
            Self::Continuous | Self::Periodic => 0,
        }
    }

    fn expression_root_count(&self) -> usize {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.roots.len(),
            Self::Continuous | Self::Periodic => 0,
        }
    }

    fn pure_operator_counts(&self) -> Result<PureOperatorWireCounts, Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.pure_operator_counts(),
            Self::Continuous | Self::Periodic => Ok(PureOperatorWireCounts::default()),
        }
    }

    fn validate_v5_features(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.validate_v5_features(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    fn canonicalize_v5_definitions(&mut self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => {
                guard.canonicalize_v5_definitions()
            }
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    fn ensure_v1(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v1(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    fn ensure_v2(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v2(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    fn ensure_v3(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v3(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    fn ensure_v4(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v4(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    fn semantic_references(&self) -> Vec<&WireId> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.semantic_references(),
            Self::Continuous | Self::Periodic => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireEventDirection {
    Any,
    Rising,
    Falling,
}

impl WireEventDirection {
    const fn encode(value: EventDirection) -> Self {
        match value {
            EventDirection::Any => Self::Any,
            EventDirection::Rising => Self::Rising,
            EventDirection::Falling => Self::Falling,
        }
    }

    const fn decode(self) -> EventDirection {
        match self {
            Self::Any => EventDirection::Any,
            Self::Rising => EventDirection::Rising,
            Self::Falling => EventDirection::Falling,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireConnectionKind {
    Signal,
    Conserving,
    SpatialPeriodic,
}

impl WireConnectionKind {
    fn encode(value: ConnectionSemantics, version: WireVersion) -> Result<Self, Diagnostic> {
        match value {
            ConnectionSemantics::Signal => Ok(Self::Signal),
            ConnectionSemantics::Conserving => Ok(Self::Conserving),
            ConnectionSemantics::SpatialPeriodic if version == WireVersion::V6 => {
                Ok(Self::SpatialPeriodic)
            }
            ConnectionSemantics::SpatialPeriodic => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            _ => Err(invalid_artifact(
                "connection semantics are newer than model wire v1",
            )),
        }
    }

    const fn decode(self) -> ConnectionSemantics {
        match self {
            Self::Signal => ConnectionSemantics::Signal,
            Self::Conserving => ConnectionSemantics::Conserving,
            Self::SpatialPeriodic => ConnectionSemantics::SpatialPeriodic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireClockKind {
    Continuous,
    Periodic {
        period: WireRationalTime,
        phase: WireRationalTime,
    },
    Aperiodic,
    Inherited,
}

impl WireClockKind {
    fn encode(value: ClockKind) -> Result<Self, Diagnostic> {
        match value {
            ClockKind::Continuous => Ok(Self::Continuous),
            ClockKind::Periodic { period, phase } => Ok(Self::Periodic {
                period: WireRationalTime::encode(period),
                phase: WireRationalTime::encode(phase),
            }),
            ClockKind::Aperiodic => Ok(Self::Aperiodic),
            ClockKind::Inherited => Ok(Self::Inherited),
            _ => Err(invalid_artifact("clock kind is newer than model wire v1")),
        }
    }

    fn decode(&self, id: Id<kinds::ClockDomain>) -> Result<ClockDomainDef, Diagnostic> {
        match self {
            Self::Continuous => Ok(ClockDomainDef::continuous(id)),
            Self::Periodic { period, phase } => {
                ClockDomainDef::periodic(id, period.decode()?, phase.decode()?)
                    .map_err(|error| invalid_artifact(error.message()))
            }
            Self::Aperiodic => Ok(ClockDomainDef::aperiodic(id)),
            Self::Inherited => Ok(ClockDomainDef::inherited(id)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRationalTime {
    numerator: u64,
    denominator: u64,
}

impl WireRationalTime {
    const fn encode(value: RationalTime) -> Self {
        Self {
            numerator: value.numerator(),
            denominator: value.denominator(),
        }
    }

    fn decode(self) -> Result<RationalTime, Diagnostic> {
        let value = RationalTime::new(self.numerator, self.denominator)
            .map_err(|error| invalid_artifact(error.message()))?;
        if value.numerator() != self.numerator || value.denominator() != self.denominator {
            return Err(invalid_artifact(
                "wire rational time must already be in canonical reduced form",
            ));
        }
        Ok(value)
    }
}

const PURE_COMPONENT_CALCULUS_V1: &str = "eqiora.pure-component-calculus/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePureOperatorDefinition {
    digest: String,
    required_features: Vec<String>,
    formals: Vec<WirePureValueClass>,
    result: WirePureValueClass,
    nodes: Vec<WirePureCalculusNode>,
    root: u32,
}

impl WirePureOperatorDefinition {
    fn encode(definition: &PureOperatorDefinition) -> Self {
        Self {
            digest: definition.digest().to_string(),
            required_features: vec![PURE_COMPONENT_CALCULUS_V1.to_owned()],
            formals: definition
                .formals()
                .iter()
                .copied()
                .map(WirePureValueClass::encode)
                .collect(),
            result: WirePureValueClass::encode(definition.result_rule()),
            nodes: definition
                .nodes()
                .iter()
                .map(WirePureCalculusNode::encode)
                .collect(),
            root: definition.root().index(),
        }
    }

    fn validate_features(&self) -> Result<(), Diagnostic> {
        if self.required_features.is_empty() {
            return Err(invalid_artifact(
                "pure-operator definition is missing its required component-calculus feature",
            ));
        }
        if let Some(feature) = self
            .required_features
            .iter()
            .find(|feature| feature.as_str() != PURE_COMPONENT_CALCULUS_V1)
        {
            return Err(invalid_artifact(format!(
                "pure-operator definition requires unknown feature `{feature}`"
            )));
        }
        if self
            .required_features
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid_artifact(
                "pure-operator required features must be sorted and duplicate-free",
            ));
        }
        Ok(())
    }

    fn rebuild_and_validate_digest(&self) -> Result<PureOperatorDefinition, Diagnostic> {
        let formals = self
            .formals
            .iter()
            .copied()
            .map(WirePureValueClass::decode)
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.result.decode()?;
        let mut builder = CalculusBuilder::new(formals, result).map_err(|error| {
            invalid_artifact(format!("invalid pure-operator definition: {error}"))
        })?;
        let mut ids = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let id = node.decode(&mut builder, &ids)?;
            ids.push(id);
        }
        let root = calculus_operand(&ids, self.root)?;
        let definition = builder.finish(root).map_err(|error| {
            invalid_artifact(format!("invalid pure-operator definition: {error}"))
        })?;
        validate_operator_digest(&self.digest)?;
        let actual = definition.digest().to_string();
        if self.digest != actual {
            return Err(invalid_artifact(format!(
                "pure-operator definition digest mismatch: claimed {}, rebuilt {actual}",
                self.digest
            )));
        }
        Ok(definition)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WirePureValueClass {
    InvariantScalar,
    SpatialTensor { rank: u16 },
}

impl WirePureValueClass {
    const fn encode(value: PureValueClass) -> Self {
        match value.spatial_rank() {
            None => Self::InvariantScalar,
            Some(rank) => Self::SpatialTensor { rank },
        }
    }

    fn decode(self) -> Result<PureValueClass, Diagnostic> {
        match self {
            Self::InvariantScalar => Ok(PureValueClass::invariant_scalar()),
            Self::SpatialTensor { rank } => PureValueClass::spatial_tensor(rank)
                .map_err(|error| invalid_artifact(format!("invalid pure value class: {error}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum WirePureCalculusNode {
    Rational { numerator: i64, denominator: u64 },
    FormalComponent { formal: u16, axes: Vec<u16> },
    KroneckerDelta { left_axis: u16, right_axis: u16 },
    Neg { value: u32 },
    Add { left: u32, right: u32 },
    Mul { left: u32, right: u32 },
}

impl WirePureCalculusNode {
    fn encode(node: &CalculusNode) -> Self {
        match node {
            CalculusNode::Rational(value) => Self::Rational {
                numerator: value.numerator(),
                denominator: value.denominator(),
            },
            CalculusNode::FormalComponent { formal, axes } => Self::FormalComponent {
                formal: *formal,
                axes: axes.iter().map(|axis| axis.index()).collect(),
            },
            CalculusNode::KroneckerDelta(left, right) => Self::KroneckerDelta {
                left_axis: left.index(),
                right_axis: right.index(),
            },
            CalculusNode::Neg(value) => Self::Neg {
                value: value.index(),
            },
            CalculusNode::Add(left, right) => Self::Add {
                left: left.index(),
                right: right.index(),
            },
            CalculusNode::Mul(left, right) => Self::Mul {
                left: left.index(),
                right: right.index(),
            },
        }
    }

    fn decode(
        &self,
        builder: &mut CalculusBuilder,
        ids: &[CalculusNodeId],
    ) -> Result<CalculusNodeId, Diagnostic> {
        let node = match self {
            Self::Rational {
                numerator,
                denominator,
            } => CalculusNode::Rational(
                ExactRational::from_canonical_parts(*numerator, *denominator).map_err(|error| {
                    invalid_artifact(format!("invalid canonical pure rational: {error}"))
                })?,
            ),
            Self::FormalComponent { formal, axes } => CalculusNode::FormalComponent {
                formal: *formal,
                axes: axes
                    .iter()
                    .copied()
                    .map(ResultAxis::new)
                    .collect::<Box<[_]>>(),
            },
            Self::KroneckerDelta {
                left_axis,
                right_axis,
            } => CalculusNode::KroneckerDelta(
                ResultAxis::new(*left_axis),
                ResultAxis::new(*right_axis),
            ),
            Self::Neg { value } => CalculusNode::Neg(calculus_operand(ids, *value)?),
            Self::Add { left, right } => CalculusNode::Add(
                calculus_operand(ids, *left)?,
                calculus_operand(ids, *right)?,
            ),
            Self::Mul { left, right } => CalculusNode::Mul(
                calculus_operand(ids, *left)?,
                calculus_operand(ids, *right)?,
            ),
        };
        builder
            .push(node)
            .map_err(|error| invalid_artifact(format!("invalid pure calculus node: {error}")))
    }
}

fn calculus_operand(ids: &[CalculusNodeId], index: u32) -> Result<CalculusNodeId, Diagnostic> {
    usize::try_from(index)
        .ok()
        .and_then(|index| ids.get(index))
        .copied()
        .ok_or_else(|| {
            invalid_artifact(format!(
                "pure calculus operand {index} is not topologically prior"
            ))
        })
}

fn validate_operator_digest(digest: &str) -> Result<(), Diagnostic> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_artifact(
            "pure-operator digest must be exactly 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireExpression {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    definitions: Vec<WirePureOperatorDefinition>,
    nodes: Vec<WireExpressionNode>,
    roots: Vec<u32>,
}

impl WireExpression {
    fn encode(expression: &ExprDag, version: WireVersion) -> Result<Self, Diagnostic> {
        Ok(Self {
            definitions: if matches!(version, WireVersion::V5 | WireVersion::V6) {
                expression
                    .definitions()
                    .values()
                    .map(WirePureOperatorDefinition::encode)
                    .collect()
            } else {
                Vec::new()
            },
            nodes: expression
                .nodes()
                .iter()
                .map(|node| WireExpressionNode::encode(node, version))
                .collect::<Result<Vec<_>, _>>()?,
            roots: expression.roots().iter().map(|root| root.index()).collect(),
        })
    }

    fn decode(&self) -> Result<ExprDag, Diagnostic> {
        if self.nodes.is_empty() || self.roots.is_empty() {
            return Err(invalid_artifact(
                "wire expression requires non-empty nodes and roots",
            ));
        }
        let definitions = self.decode_definitions()?;
        let mut builder = ExprDagBuilder::new();
        let mut ids = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let id = node.decode(&mut builder, &ids, &definitions)?;
            ids.push(id);
        }
        let referenced_definitions = self
            .nodes
            .iter()
            .filter_map(|node| match node {
                WireExpressionNode::PureOperatorApplication { definition, .. } => {
                    Some(definition.as_str())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let supplied_definitions = definitions
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if referenced_definitions != supplied_definitions {
            return Err(invalid_artifact(
                "wire expression must supply exactly its referenced pure-operator definitions",
            ));
        }
        let roots = self
            .roots
            .iter()
            .map(|&root| operand(&ids, root))
            .collect::<Result<Vec<_>, _>>()?;
        builder
            .finish(roots)
            .map_err(|error| invalid_artifact(error.message()))
    }

    fn pure_operator_counts(&self) -> Result<PureOperatorWireCounts, Diagnostic> {
        let formals = checked_count_sum(
            self.definitions
                .iter()
                .map(|definition| definition.formals.len()),
            "pure-operator formal count",
        )?;
        let calculus_nodes = checked_count_sum(
            self.definitions
                .iter()
                .map(|definition| definition.nodes.len()),
            "pure-operator calculus-node count",
        )?;
        let application_arguments = checked_count_sum(
            self.nodes.iter().map(WireExpressionNode::application_arity),
            "pure-operator application-argument count",
        )?;
        Ok(PureOperatorWireCounts {
            definitions: self.definitions.len(),
            formals,
            calculus_nodes,
            application_arguments,
        })
    }

    fn validate_v5_features(&self) -> Result<(), Diagnostic> {
        for definition in &self.definitions {
            definition.validate_features()?;
        }
        Ok(())
    }

    fn canonicalize_v5_definitions(&mut self) -> Result<(), Diagnostic> {
        for definition in &self.definitions {
            definition.rebuild_and_validate_digest()?;
        }
        self.definitions
            .sort_by(|left, right| left.digest.cmp(&right.digest));
        if self
            .definitions
            .windows(2)
            .any(|pair| pair[0].digest == pair[1].digest)
        {
            return Err(invalid_artifact(
                "wire expression contains a duplicate pure-operator definition digest",
            ));
        }
        Ok(())
    }

    fn decode_definitions(&self) -> Result<BTreeMap<String, PureOperatorDefinition>, Diagnostic> {
        let mut definitions = BTreeMap::new();
        for definition in &self.definitions {
            let rebuilt = definition.rebuild_and_validate_digest()?;
            if definitions
                .insert(definition.digest.clone(), rebuilt)
                .is_some()
            {
                return Err(invalid_artifact(
                    "wire expression contains a duplicate pure-operator definition digest",
                ));
            }
        }
        Ok(definitions)
    }

    fn ensure_v1(&self) -> Result<(), Diagnostic> {
        if self.nodes.iter().any(|node| {
            matches!(
                node,
                WireExpressionNode::Symbol {
                    symbol: WireSymbol::Across { .. }
                        | WireSymbol::Through { .. }
                        | WireSymbol::PortTrace { .. }
                        | WireSymbol::PortFlux { .. }
                } | WireExpressionNode::SymmetricPart { .. }
                    | WireExpressionNode::IsotropicLift { .. }
                    | WireExpressionNode::PureOperatorApplication { .. }
            )
        }) || !self.definitions.is_empty()
        {
            Err(invalid_artifact(
                "model wire v1 cannot contain physical interface symbols or tensor operators",
            ))
        } else {
            Ok(())
        }
    }

    fn ensure_v2(&self) -> Result<(), Diagnostic> {
        if self.nodes.iter().any(|node| {
            matches!(
                node,
                WireExpressionNode::Symbol {
                    symbol: WireSymbol::PortTrace { .. } | WireSymbol::PortFlux { .. }
                } | WireExpressionNode::SymmetricPart { .. }
                    | WireExpressionNode::IsotropicLift { .. }
                    | WireExpressionNode::PureOperatorApplication { .. }
            )
        }) || !self.definitions.is_empty()
        {
            Err(invalid_artifact(
                "model wire v2 cannot contain boundary symbols or tensor operators",
            ))
        } else {
            Ok(())
        }
    }

    fn ensure_v3(&self) -> Result<(), Diagnostic> {
        if self.nodes.iter().any(|node| {
            matches!(
                node,
                WireExpressionNode::SymmetricPart { .. }
                    | WireExpressionNode::IsotropicLift { .. }
                    | WireExpressionNode::PureOperatorApplication { .. }
            )
        }) || !self.definitions.is_empty()
        {
            Err(invalid_artifact(
                "model wire v3 cannot contain tensor operators introduced by model wire v4",
            ))
        } else {
            Ok(())
        }
    }

    fn ensure_v4(&self) -> Result<(), Diagnostic> {
        if !self.definitions.is_empty()
            || self
                .nodes
                .iter()
                .any(|node| matches!(node, WireExpressionNode::PureOperatorApplication { .. }))
        {
            Err(invalid_artifact(
                "model wire v4 cannot contain pure-operator definitions or generic applications",
            ))
        } else {
            Ok(())
        }
    }

    fn semantic_references(&self) -> Vec<&WireId> {
        self.nodes
            .iter()
            .filter_map(|node| match node {
                WireExpressionNode::Symbol { symbol } => symbol.id(),
                _ => None,
            })
            .collect()
    }
}

/// Aggregate resource counters for the v5 pure-operator wire extension.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PureOperatorWireCounts {
    pub(crate) definitions: usize,
    pub(crate) formals: usize,
    pub(crate) calculus_nodes: usize,
    pub(crate) application_arguments: usize,
}

impl PureOperatorWireCounts {
    pub(crate) fn checked_add(self, other: Self) -> Result<Self, Diagnostic> {
        Ok(Self {
            definitions: self
                .definitions
                .checked_add(other.definitions)
                .ok_or_else(|| {
                    invalid_artifact("pure-operator definition count overflows usize")
                })?,
            formals: self
                .formals
                .checked_add(other.formals)
                .ok_or_else(|| invalid_artifact("pure-operator formal count overflows usize"))?,
            calculus_nodes: self
                .calculus_nodes
                .checked_add(other.calculus_nodes)
                .ok_or_else(|| {
                    invalid_artifact("pure-operator calculus-node count overflows usize")
                })?,
            application_arguments: self
                .application_arguments
                .checked_add(other.application_arguments)
                .ok_or_else(|| {
                    invalid_artifact("pure-operator application-argument count overflows usize")
                })?,
        })
    }

    pub(crate) fn ensure_limits(
        self,
        limits: ModelDecoderLimits,
        label: &str,
    ) -> Result<(), Diagnostic> {
        require_decoder_count(
            &format!("{label} pure-operator definitions"),
            self.definitions,
            limits.max_pure_operator_definitions,
        )?;
        require_decoder_count(
            &format!("{label} pure-operator formals"),
            self.formals,
            limits.max_pure_operator_formals,
        )?;
        require_decoder_count(
            &format!("{label} pure-operator calculus nodes"),
            self.calculus_nodes,
            limits.max_pure_operator_calculus_nodes,
        )?;
        require_decoder_count(
            &format!("{label} pure-operator application arguments"),
            self.application_arguments,
            limits.max_pure_operator_application_arguments,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum WireExpressionNode {
    Constant {
        value: WireQuantity,
    },
    Symbol {
        symbol: WireSymbol,
    },
    Neg {
        value: u32,
    },
    Add {
        left: u32,
        right: u32,
    },
    Sub {
        left: u32,
        right: u32,
    },
    Mul {
        left: u32,
        right: u32,
    },
    Div {
        left: u32,
        right: u32,
    },
    PowI {
        base: u32,
        exponent: i32,
    },
    SpatialCoordinate {
        axis: usize,
    },
    UnaryMath {
        function: WireUnaryMath,
        value: u32,
    },
    Gradient {
        value: u32,
    },
    Divergence {
        value: u32,
    },
    Trace {
        value: u32,
    },
    NormalComponent {
        value: u32,
    },
    SymmetricPart {
        value: u32,
    },
    IsotropicLift {
        value: u32,
    },
    PureOperatorApplication {
        definition: String,
        arguments: Vec<u32>,
    },
}

impl WireExpressionNode {
    fn encode(node: &ExprNode, version: WireVersion) -> Result<Self, Diagnostic> {
        Ok(match node {
            ExprNode::Constant(value) => Self::Constant {
                value: WireQuantity::encode(*value),
            },
            ExprNode::Symbol(symbol) => Self::Symbol {
                symbol: WireSymbol::encode(*symbol, version)?,
            },
            ExprNode::Neg(value) => Self::Neg {
                value: value.index(),
            },
            ExprNode::Add(left, right) => Self::Add {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::Sub(left, right) => Self::Sub {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::Mul(left, right) => Self::Mul {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::Div(left, right) => Self::Div {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::PowI(base, exponent) => Self::PowI {
                base: base.index(),
                exponent: *exponent,
            },
            ExprNode::SpatialCoordinate(axis) => Self::SpatialCoordinate { axis: *axis },
            ExprNode::UnaryMath(function, value) => Self::UnaryMath {
                function: WireUnaryMath::encode(*function)?,
                value: value.index(),
            },
            ExprNode::Gradient(value) => Self::Gradient {
                value: value.index(),
            },
            ExprNode::Divergence(value) => Self::Divergence {
                value: value.index(),
            },
            ExprNode::Trace(value) => Self::Trace {
                value: value.index(),
            },
            ExprNode::NormalComponent(value) => Self::NormalComponent {
                value: value.index(),
            },
            ExprNode::SymmetricPart(value)
                if matches!(version, WireVersion::V4 | WireVersion::V5 | WireVersion::V6) =>
            {
                Self::SymmetricPart {
                    value: value.index(),
                }
            }
            ExprNode::IsotropicLift(value)
                if matches!(version, WireVersion::V4 | WireVersion::V5 | WireVersion::V6) =>
            {
                Self::IsotropicLift {
                    value: value.index(),
                }
            }
            ExprNode::PureOperatorApplication(application)
                if matches!(version, WireVersion::V5 | WireVersion::V6) =>
            {
                Self::PureOperatorApplication {
                    definition: application.definition().to_string(),
                    arguments: application
                        .arguments()
                        .iter()
                        .map(|argument| argument.index())
                        .collect(),
                }
            }
            _ => {
                return Err(invalid_artifact(
                    "expression node is newer than the supported model wire vocabulary",
                ));
            }
        })
    }

    fn decode(
        &self,
        builder: &mut ExprDagBuilder,
        ids: &[ExprId],
        definitions: &BTreeMap<String, PureOperatorDefinition>,
    ) -> Result<ExprId, Diagnostic> {
        let result = match self {
            Self::Constant { value } => builder.constant(value.decode()?),
            Self::Symbol { symbol } => builder.symbol(symbol.decode()?),
            Self::Neg { value } => builder.neg(operand(ids, *value)?),
            Self::Add { left, right } => builder.add(operand(ids, *left)?, operand(ids, *right)?),
            Self::Sub { left, right } => builder.sub(operand(ids, *left)?, operand(ids, *right)?),
            Self::Mul { left, right } => builder.mul(operand(ids, *left)?, operand(ids, *right)?),
            Self::Div { left, right } => builder.div(operand(ids, *left)?, operand(ids, *right)?),
            Self::PowI { base, exponent } => builder.powi(operand(ids, *base)?, *exponent),
            Self::SpatialCoordinate { axis } => builder.spatial_coordinate(*axis),
            Self::UnaryMath { function, value } => {
                builder.unary_math(function.decode(), operand(ids, *value)?)
            }
            Self::Gradient { value } => builder.gradient(operand(ids, *value)?),
            Self::Divergence { value } => builder.divergence(operand(ids, *value)?),
            Self::Trace { value } => builder.trace(operand(ids, *value)?),
            Self::NormalComponent { value } => builder.normal_component(operand(ids, *value)?),
            Self::SymmetricPart { value } => builder.symmetric_part(operand(ids, *value)?),
            Self::IsotropicLift { value } => builder.isotropic_lift(operand(ids, *value)?),
            Self::PureOperatorApplication {
                definition,
                arguments,
            } => {
                validate_operator_digest(definition)?;
                let definition = definitions.get(definition).ok_or_else(|| {
                    invalid_artifact(
                        "pure-operator application references a missing local definition",
                    )
                })?;
                let arguments = arguments
                    .iter()
                    .map(|argument| operand(ids, *argument))
                    .collect::<Result<Vec<_>, _>>()?;
                builder.pure_operator(definition, arguments)
            }
        };
        result.map_err(|error| invalid_artifact(error.message()))
    }

    fn application_arity(&self) -> usize {
        match self {
            Self::PureOperatorApplication { arguments, .. } => arguments.len(),
            _ => 0,
        }
    }
}

fn operand(ids: &[ExprId], index: u32) -> Result<ExprId, Diagnostic> {
    usize::try_from(index)
        .ok()
        .and_then(|index| ids.get(index))
        .copied()
        .ok_or_else(|| {
            invalid_artifact(format!(
                "wire expression operand {index} is not topologically prior"
            ))
        })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireSymbol {
    Field { id: WireId },
    Derivative { id: WireId },
    Pre { id: WireId },
    Next { id: WireId },
    Parameter { id: WireId },
    Port { id: WireId },
    Across { id: WireId },
    Through { id: WireId },
    PortTrace { id: WireId },
    PortFlux { id: WireId },
    Time,
}

impl WireSymbol {
    fn encode(value: SymbolRef, version: WireVersion) -> Result<Self, Diagnostic> {
        match value {
            SymbolRef::Field(id) => Ok(Self::Field {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Derivative(id) => Ok(Self::Derivative {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Pre(id) => Ok(Self::Pre {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Next(id) => Ok(Self::Next {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Parameter(id) => Ok(Self::Parameter {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Port(id) => Ok(Self::Port {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Across(id) if version.supports_scalar_physical() => Ok(Self::Across {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Through(id) if version.supports_scalar_physical() => Ok(Self::Through {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::PortTrace(id) if version.supports_boundary_physical() => {
                Ok(Self::PortTrace {
                    id: WireId::from_raw(id.erase()),
                })
            }
            SymbolRef::PortFlux(id) if version.supports_boundary_physical() => Ok(Self::PortFlux {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Time => Ok(Self::Time),
            _ => Err(invalid_artifact(
                "symbol kind is newer than the supported model wire vocabulary",
            )),
        }
    }

    fn decode(&self) -> Result<SymbolRef, Diagnostic> {
        Ok(match self {
            Self::Field { id } => SymbolRef::Field(id.typed::<kinds::Field>()?),
            Self::Derivative { id } => SymbolRef::Derivative(id.typed::<kinds::Field>()?),
            Self::Pre { id } => SymbolRef::Pre(id.typed::<kinds::Field>()?),
            Self::Next { id } => SymbolRef::Next(id.typed::<kinds::Field>()?),
            Self::Parameter { id } => SymbolRef::Parameter(id.typed::<kinds::Parameter>()?),
            Self::Port { id } => SymbolRef::Port(id.typed::<kinds::Port>()?),
            Self::Across { id } => SymbolRef::Across(id.typed::<kinds::Port>()?),
            Self::Through { id } => SymbolRef::Through(id.typed::<kinds::Port>()?),
            Self::PortTrace { id } => SymbolRef::PortTrace(id.typed::<kinds::Port>()?),
            Self::PortFlux { id } => SymbolRef::PortFlux(id.typed::<kinds::Port>()?),
            Self::Time => SymbolRef::Time,
        })
    }

    fn id(&self) -> Option<&WireId> {
        match self {
            Self::Field { id }
            | Self::Derivative { id }
            | Self::Pre { id }
            | Self::Next { id }
            | Self::Parameter { id }
            | Self::Port { id }
            | Self::Across { id }
            | Self::Through { id }
            | Self::PortTrace { id }
            | Self::PortFlux { id } => Some(id),
            Self::Time => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireUnaryMath {
    Sin,
}

impl WireUnaryMath {
    fn encode(value: UnaryMathFunction) -> Result<Self, Diagnostic> {
        match value {
            UnaryMathFunction::Sin => Ok(Self::Sin),
            _ => Err(invalid_artifact(
                "unary math function is newer than model wire v1",
            )),
        }
    }

    const fn decode(self) -> UnaryMathFunction {
        match self {
            Self::Sin => UnaryMathFunction::Sin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireValue {
    pub(crate) target: WireId,
    pub(crate) value: WireQuantity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireQuantity {
    value: f64,
    dimension: WireDimension,
}

impl WireQuantity {
    pub(crate) fn encode(value: DynQuantity) -> Self {
        Self {
            value: value.value(),
            dimension: WireDimension::encode(value.dim()),
        }
    }

    pub(crate) fn decode(&self) -> Result<DynQuantity, Diagnostic> {
        if !self.value.is_finite() {
            return Err(invalid_artifact("wire quantity value must be finite"));
        }
        Ok(DynQuantity::new(self.value, self.dimension.decode()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDimension {
    mass: i8,
    length: i8,
    time: i8,
    current: i8,
    temperature: i8,
    amount: i8,
    luminous_intensity: i8,
}

impl WireDimension {
    const fn encode(value: DimExponents) -> Self {
        Self {
            mass: value.mass,
            length: value.length,
            time: value.time,
            current: value.current,
            temperature: value.temperature,
            amount: value.amount,
            luminous_intensity: value.luminous_intensity,
        }
    }

    const fn decode(self) -> DimExponents {
        DimExponents {
            mass: self.mass,
            length: self.length,
            time: self.time,
            current: self.current,
            temperature: self.temperature,
            amount: self.amount,
            luminous_intensity: self.luminous_intensity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireId {
    kind: WireEntityKind,
    ulid: String,
}

impl WireId {
    pub(crate) fn from_raw(value: RawId) -> Self {
        Self {
            kind: WireEntityKind::encode(value.kind()),
            ulid: value.ulid().to_string(),
        }
    }

    fn typed<E: Entity>(&self) -> Result<Id<E>, Diagnostic> {
        if self.kind != WireEntityKind::encode(E::KIND) {
            return Err(invalid_artifact(format!(
                "wire ID kind {:?} does not match expected {:?}",
                self.kind,
                E::KIND
            )));
        }
        Ok(Id::from_ulid(parse_ulid(&self.ulid)?))
    }

    pub(crate) fn decode_raw(&self) -> Result<RawId, Diagnostic> {
        macro_rules! typed {
            ($kind:ty) => {
                self.typed::<$kind>()?.erase()
            };
        }
        Ok(match self.kind {
            WireEntityKind::Domain => typed!(kinds::Domain),
            WireEntityKind::Representation => typed!(kinds::Representation),
            WireEntityKind::Field => typed!(kinds::Field),
            WireEntityKind::Parameter => typed!(kinds::Parameter),
            WireEntityKind::Port => typed!(kinds::Port),
            WireEntityKind::Relation => typed!(kinds::Relation),
            WireEntityKind::Activation => typed!(kinds::Activation),
            WireEntityKind::Connection => typed!(kinds::Connection),
            WireEntityKind::ClockDomain => typed!(kinds::ClockDomain),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireEntityKind {
    Domain,
    Representation,
    Field,
    Parameter,
    Port,
    Relation,
    Activation,
    Connection,
    ClockDomain,
}

impl WireEntityKind {
    fn encode(value: EntityKind) -> Self {
        match value {
            EntityKind::Domain => Self::Domain,
            EntityKind::Representation => Self::Representation,
            EntityKind::Field => Self::Field,
            EntityKind::Parameter => Self::Parameter,
            EntityKind::Port => Self::Port,
            EntityKind::Relation => Self::Relation,
            EntityKind::Activation => Self::Activation,
            EntityKind::Connection => Self::Connection,
            EntityKind::ClockDomain => Self::ClockDomain,
            _ => unreachable!("ModelEnvelopeV1 contains Semantic Kernel nodes only"),
        }
    }
}

pub(crate) fn parse_ulid(value: &str) -> Result<Ulid, Diagnostic> {
    Ulid::from_str(value)
        .map_err(|error| invalid_artifact(format!("invalid canonical ULID `{value}`: {error}")))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireEdge {
    pub(crate) from: WireId,
    pub(crate) to: WireId,
    pub(crate) kind: WireEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireEdgeKind {
    DefinedOn,
    AppliesOn,
    BoundaryOf,
    DependsOn,
    HasPort,
    Activates,
    Connects,
    ClockedBy,
}

impl WireEdgeKind {
    pub(crate) fn encode(value: EdgeKind) -> Result<Self, Diagnostic> {
        match value {
            EdgeKind::DefinedOn => Ok(Self::DefinedOn),
            EdgeKind::AppliesOn => Ok(Self::AppliesOn),
            EdgeKind::BoundaryOf => Ok(Self::BoundaryOf),
            EdgeKind::DependsOn => Ok(Self::DependsOn),
            EdgeKind::HasPort => Ok(Self::HasPort),
            EdgeKind::Activates => Ok(Self::Activates),
            EdgeKind::Connects => Ok(Self::Connects),
            EdgeKind::ClockedBy => Ok(Self::ClockedBy),
            _ => Err(invalid_artifact(
                "non-semantic edge cannot enter a Semantic Model envelope",
            )),
        }
    }

    pub(crate) const fn decode(self) -> EdgeKind {
        match self {
            Self::DefinedOn => EdgeKind::DefinedOn,
            Self::AppliesOn => EdgeKind::AppliesOn,
            Self::BoundaryOf => EdgeKind::BoundaryOf,
            Self::DependsOn => EdgeKind::DependsOn,
            Self::HasPort => EdgeKind::HasPort,
            Self::Activates => EdgeKind::Activates,
            Self::Connects => EdgeKind::Connects,
            Self::ClockedBy => EdgeKind::ClockedBy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_compiler::compile;

    const POISSON: &str =
        include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");

    const SAMPLED: &str = r#"
model sampled {
  field temperature: K = 293;
  field command: K / s = 0;
  clock control = periodic(period = 1 / 10, phase = 0 / 1);
  relation plant continuous {
    derivative(temperature) - command = 0;
  }
  relation controller periodic(control) {
    next(command) - pre(command) = 0;
  }
}
"#;

    #[test]
    fn spatial_and_sampled_models_round_trip_to_identical_canonical_bytes() {
        for (file, source) in [("poisson.eqi", POISSON), ("sampled.eqi", SAMPLED)] {
            let program = compile_program(file, source);
            assert_round_trip(&program);
        }
        assert_round_trip(&event_program());
    }

    #[test]
    fn array_order_is_canonicalized_and_duplicates_are_rejected() {
        let program = compile_program("sampled.eqi", SAMPLED);
        let envelope = ModelEnvelopeV1::from_program(&program).unwrap();
        let canonical = envelope.canonical_json().unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["nodes"].as_array_mut().unwrap().reverse();
        value["edges"].as_array_mut().unwrap().reverse();
        let reordered = serde_json::to_vec(&value).unwrap();
        let decoded =
            ModelEnvelopeV1::from_json(&reordered, ModelDecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), canonical);

        let mut duplicate: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        let duplicate_node = duplicate["nodes"][0].clone();
        duplicate["nodes"]
            .as_array_mut()
            .unwrap()
            .push(duplicate_node);
        assert_eq!(
            ModelEnvelopeV1::from_json(
                &serde_json::to_vec(&duplicate).unwrap(),
                ModelDecoderLimits::default(),
            )
            .unwrap_err()
            .code(),
            eqiora_core::diagnostic::codes::INVALID_ARTIFACT
        );
    }

    #[test]
    fn byte_and_expression_limits_precede_graph_mutation() {
        let program = compile_program("sampled.eqi", SAMPLED);
        let bytes = ModelEnvelopeV1::from_program(&program)
            .unwrap()
            .canonical_json()
            .unwrap();
        assert_eq!(
            ModelEnvelopeV1::from_json(
                &bytes,
                ModelDecoderLimits {
                    json: JsonDecoderLimits {
                        max_bytes: bytes.len() - 1,
                        ..Default::default()
                    },
                    ..ModelDecoderLimits::default()
                },
            )
            .unwrap_err()
            .code(),
            eqiora_core::diagnostic::codes::INVALID_ARTIFACT
        );
        assert_eq!(
            ModelEnvelopeV1::from_json(
                &bytes,
                ModelDecoderLimits {
                    max_expression_nodes: 1,
                    ..ModelDecoderLimits::default()
                },
            )
            .unwrap_err()
            .code(),
            eqiora_core::diagnostic::codes::INVALID_ARTIFACT
        );
    }

    fn compile_program(file: &str, source: &str) -> KernelProgram {
        let mut compiled = compile(file, source).unwrap();
        let (transaction, model, _) = compiled.remove(0).into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
    }

    fn assert_round_trip(program: &KernelProgram) {
        let envelope = ModelEnvelopeV1::from_program(program).unwrap();
        let bytes = envelope.canonical_json().unwrap();
        let digest = envelope.digest().unwrap();
        let decoded = ModelEnvelopeV1::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
        let round_trip_program = decoded.to_program().unwrap();
        let round_trip = ModelEnvelopeV1::from_program(&round_trip_program).unwrap();
        assert_eq!(round_trip.canonical_json().unwrap(), bytes);
        assert_eq!(round_trip.digest().unwrap(), digest);
    }

    fn event_program() -> KernelProgram {
        let inverse_time = DimExponents {
            time: -1,
            ..DimExponents::DIMENSIONLESS
        };
        let state = Id::<kinds::Field>::new();
        let rate = Id::<kinds::Parameter>::new();
        let flow = Id::<kinds::Relation>::new();
        let reset = Id::<kinds::Relation>::new();
        let continuous = Id::<kinds::Activation>::new();
        let event = Id::<kinds::Activation>::new();
        let model = OntologyId::<Model>::new();

        let mut flow_expression = ExprDagBuilder::new();
        let derivative = flow_expression
            .symbol(SymbolRef::Derivative(state))
            .unwrap();
        let rate_value = flow_expression.symbol(SymbolRef::Parameter(rate)).unwrap();
        let flow_residual = flow_expression.add(derivative, rate_value).unwrap();

        let mut reset_expression = ExprDagBuilder::new();
        let next = reset_expression.symbol(SymbolRef::Next(state)).unwrap();
        let zero = reset_expression
            .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let reset_residual = reset_expression.sub(next, zero).unwrap();

        let mut guard = ExprDagBuilder::new();
        let guard_state = guard.symbol(SymbolRef::Field(state)).unwrap();
        let event_definition = ActivationDef::new(
            event,
            ActivationKind::Event {
                guard: guard.finish([guard_state]).unwrap(),
                direction: EventDirection::Falling,
            },
        )
        .unwrap();
        let nodes = [
            KernelNode::from(
                FieldDef::new(state, DimExponents::DIMENSIONLESS)
                    .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                    .unwrap(),
            ),
            KernelNode::from(ParameterDef::new(rate, DynQuantity::new(1.0, inverse_time))),
            KernelNode::from(RelationDef::new(
                flow,
                flow_expression.finish([flow_residual]).unwrap(),
            )),
            KernelNode::from(RelationDef::new(
                reset,
                reset_expression.finish([reset_residual]).unwrap(),
            )),
            KernelNode::from(ActivationDef::continuous(continuous)),
            KernelNode::from(event_definition),
        ];
        let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
        let mut transaction = Transaction::new("event artifact round trip");
        for node in nodes {
            transaction.push(Op::DefineKernelNode { node });
        }
        for (relation, dependency) in [
            (flow, state.erase()),
            (flow, rate.erase()),
            (reset, state.erase()),
        ] {
            transaction.push(Op::Connect {
                from: relation.erase(),
                to: dependency,
                edge: EdgeKind::DependsOn,
            });
        }
        for (activation, relation) in [(continuous, flow), (event, reset)] {
            transaction.push(Op::Connect {
                from: activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            });
        }
        transaction.push(Op::DefineOntologyView {
            view: ModelView::new(model, members, []).unwrap().into(),
        });
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
    }
}
