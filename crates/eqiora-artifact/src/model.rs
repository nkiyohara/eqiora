use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use eqiora_core::entity::kinds;
#[cfg(test)]
use eqiora_core::{DimExponents, DynQuantity, Id};
#[cfg(test)]
use eqiora_graph::EdgeKind;
#[cfg(test)]
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, EventDirection, ExprDagBuilder, FieldDef, KernelNode,
    ParameterDef, RelationDef, SymbolRef,
};

use eqiora_core::{Diagnostic, OntologyId, RawId};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Revision, Transaction};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;
use serde::{Deserialize, Serialize};

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

mod expression;
mod node;
mod primitive;
mod vocabulary;

pub(crate) use expression::PureOperatorWireCounts;
pub(crate) use expression::{WireQuantity, WireValue};
pub(crate) use node::WireNodeDefinition;
pub(crate) use primitive::{WireEdge, WireEdgeKind, WireEntityKind, WireId, parse_ulid};

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
