//! Model wire v2: scalar physical Domains, Ports, and symbols.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, EntityKind, OntologyId, RawId};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Revision, Transaction};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;
use serde::{Deserialize, Serialize};

use crate::model::{
    WireEdge, WireId, WireNode, WireQuantity, WireValue, checked_count_sum, parse_ulid,
    require_decoder_count,
};
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, DecoderLimits, check_wire_limits, invalid_artifact,
};

const MODEL_SCHEMA_V2: &str = "eqiora.model-envelope/v2";
const MODEL_SCHEMA_V3: &str = "eqiora.model-envelope/v3";
const MODEL_SCHEMA_V4: &str = "eqiora.model-envelope/v4";
const MODEL_SCHEMA_V5: &str = "eqiora.model-envelope/v5";
const MODEL_SCHEMA_V6: &str = "eqiora.model-envelope/v6";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelSchemaVersion {
    V2,
    V3,
    V4,
    V5,
    V6,
}

impl ModelSchemaVersion {
    const fn schema(self) -> &'static str {
        match self {
            Self::V2 => MODEL_SCHEMA_V2,
            Self::V3 => MODEL_SCHEMA_V3,
            Self::V4 => MODEL_SCHEMA_V4,
            Self::V5 => MODEL_SCHEMA_V5,
            Self::V6 => MODEL_SCHEMA_V6,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::V2 => "decode eqiora.model-envelope/v2",
            Self::V3 => "decode eqiora.model-envelope/v3",
            Self::V4 => "decode eqiora.model-envelope/v4",
            Self::V5 => "decode eqiora.model-envelope/v5",
            Self::V6 => "decode eqiora.model-envelope/v6",
        }
    }

    const fn model_label(self) -> &'static str {
        match self {
            Self::V2 => "model v2",
            Self::V3 => "model v3",
            Self::V4 => "model v4",
            Self::V5 => "model v5",
            Self::V6 => "model v6",
        }
    }

    const fn envelope_label(self) -> &'static str {
        match self {
            Self::V2 => "model v2 envelope",
            Self::V3 => "model v3 envelope",
            Self::V4 => "model v4 envelope",
            Self::V5 => "model v5 envelope",
            Self::V6 => "model v6 envelope",
        }
    }
}

/// Versioned canonical Semantic Model serialization with RFC 0024 semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEnvelopeV2 {
    wire: WireModelEnvelopeV2,
}

impl ModelEnvelopeV2 {
    /// Encode one immutable validated Semantic Kernel program as explicit v2.
    ///
    /// # Errors
    /// Returns `EQ0901` for an unsupported kernel value or resource-limit
    /// violation.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        Self::from_program_version(program, ModelSchemaVersion::V2)
    }

    pub(crate) fn from_program_v3(program: &KernelProgram) -> Result<Self, Diagnostic> {
        Self::from_program_version(program, ModelSchemaVersion::V3)
    }

    pub(crate) fn from_program_v4(program: &KernelProgram) -> Result<Self, Diagnostic> {
        Self::from_program_version(program, ModelSchemaVersion::V4)
    }

    pub(crate) fn from_program_v5(program: &KernelProgram) -> Result<Self, Diagnostic> {
        Self::from_program_version(program, ModelSchemaVersion::V5)
    }

    pub(crate) fn from_program_v6(program: &KernelProgram) -> Result<Self, Diagnostic> {
        Self::from_program_version(program, ModelSchemaVersion::V6)
    }

    fn from_program_version(
        program: &KernelProgram,
        version: ModelSchemaVersion,
    ) -> Result<Self, Diagnostic> {
        let nodes = program
            .nodes()
            .map(|node| match version {
                ModelSchemaVersion::V2 => WireNode::encode_v2(node),
                ModelSchemaVersion::V3 => WireNode::encode_v3(node),
                ModelSchemaVersion::V4 => WireNode::encode_v4(node),
                ModelSchemaVersion::V5 => WireNode::encode_v5(node),
                ModelSchemaVersion::V6 => WireNode::encode_v6(node),
            })
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
                    kind: crate::model::WireEdgeKind::encode(edge.kind())?,
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
            wire: WireModelEnvelopeV2 {
                schema: version.schema().to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                source_revision: program.revision().0,
                model_ulid: program.model().ulid().to_string(),
                nodes,
                values,
                edges,
                boundary,
            },
        };
        envelope.canonicalize_and_validate(DecoderLimits::default(), version)?;
        Ok(envelope)
    }

    /// Decode and validate v2 bytes without mutating a graph store.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, dangling, duplicated, or
    /// wrong-version data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, ModelSchemaVersion::V2)
    }

    pub(crate) fn from_json_v3(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, ModelSchemaVersion::V3)
    }

    pub(crate) fn from_json_v4(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, ModelSchemaVersion::V4)
    }

    pub(crate) fn from_json_v5(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, ModelSchemaVersion::V5)
    }

    pub(crate) fn from_json_v6(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        Self::from_json_version(bytes, limits, ModelSchemaVersion::V6)
    }

    fn from_json_version(
        bytes: &[u8],
        limits: DecoderLimits,
        version: ModelSchemaVersion,
    ) -> Result<Self, Diagnostic> {
        check_wire_limits(bytes, limits)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid {} JSON: {error}", version.schema()))
        })?;
        let mut envelope = Self { wire };
        envelope.canonicalize_and_validate(limits, version)?;
        Ok(envelope)
    }

    /// Deterministic compact canonical JSON.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        let version = self.schema_version();
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize {}: {error}",
                version.envelope_label()
            ))
        })
    }

    /// Domain-separated SHA-256 identity of semantic v2 content.
    ///
    /// Source revision is provenance and is excluded from content identity.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(ModelSchemaVersion::V2)
    }

    pub(crate) fn digest_v3(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(ModelSchemaVersion::V3)
    }

    pub(crate) fn digest_v4(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(ModelSchemaVersion::V4)
    }

    pub(crate) fn digest_v5(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(ModelSchemaVersion::V5)
    }

    pub(crate) fn digest_v6(&self) -> Result<ArtifactDigest, Diagnostic> {
        self.digest_version(ModelSchemaVersion::V6)
    }

    fn digest_version(&self, version: ModelSchemaVersion) -> Result<ArtifactDigest, Diagnostic> {
        let content = WireModelContentV2 {
            schema: &self.wire.schema,
            encoding: &self.wire.encoding,
            model_ulid: &self.wire.model_ulid,
            nodes: &self.wire.nodes,
            values: &self.wire.values,
            edges: &self.wire.edges,
            boundary: &self.wire.boundary,
        };
        let bytes = serde_json::to_vec(&content).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize {} content: {error}",
                version.model_label()
            ))
        })?;
        Ok(ArtifactDigest::compute(version.schema().as_bytes(), &bytes))
    }

    /// Reconstruct one typed transaction without committing it.
    ///
    /// # Errors
    /// Returns structured diagnostics if validated wire data cannot be
    /// represented by the closed Semantic Model transaction vocabulary.
    pub fn to_transaction(&self) -> Result<(Transaction, OntologyId<Model>), Vec<Diagnostic>> {
        let version = self.schema_version();
        let model = OntologyId::<Model>::from_ulid(
            parse_ulid(&self.wire.model_ulid).map_err(|error| vec![error])?,
        );
        let mut ids = BTreeMap::new();
        let mut definitions = Vec::with_capacity(self.wire.nodes.len());
        for wire_node in &self.wire.nodes {
            let definition = wire_node.decode().map_err(|error| vec![error])?;
            if ids.insert(wire_node.id.clone(), definition.id()).is_some() {
                return Err(vec![invalid_artifact(format!(
                    "{} contains a duplicate kernel node ID",
                    version.envelope_label()
                ))]);
            }
            definitions.push(definition);
        }

        let mut transaction = Transaction::new(version.label());
        for node in definitions {
            transaction.push(Op::DefineKernelNode { node });
        }
        for value in &self.wire.values {
            transaction.push(Op::SetValue {
                target: lookup_id(&ids, &value.target).map_err(|error| vec![error])?,
                value: value.value.decode().map_err(|error| vec![error])?,
            });
        }
        for edge in &self.wire.edges {
            transaction.push(Op::Connect {
                from: lookup_id(&ids, &edge.from).map_err(|error| vec![error])?,
                to: lookup_id(&ids, &edge.to).map_err(|error| vec![error])?,
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

    /// Reconstruct through typed definitions, an atomic commit, and complete
    /// [`KernelProgram`] validation.
    ///
    /// # Errors
    /// Returns diagnostics from reconstruction, commit, or whole-model
    /// validation; no partial state is exposed.
    pub fn to_program(&self) -> Result<KernelProgram, Vec<Diagnostic>> {
        let (transaction, model) = self.to_transaction()?;
        let store =
            InMemoryGraphStore::restore_snapshot(transaction, Revision(self.source_revision()))?;
        KernelProgram::from_snapshot(&store.snapshot(), model)
    }

    /// Source graph revision retained as provenance.
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.wire.source_revision
    }

    /// Typed Semantic Model identity.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state were corrupted.
    pub fn model(&self) -> Result<OntologyId<Model>, Diagnostic> {
        parse_ulid(&self.wire.model_ulid).map(OntologyId::from_ulid)
    }

    fn schema_version(&self) -> ModelSchemaVersion {
        match self.wire.schema.as_str() {
            MODEL_SCHEMA_V3 => ModelSchemaVersion::V3,
            MODEL_SCHEMA_V4 => ModelSchemaVersion::V4,
            MODEL_SCHEMA_V5 => ModelSchemaVersion::V5,
            MODEL_SCHEMA_V6 => ModelSchemaVersion::V6,
            _ => ModelSchemaVersion::V2,
        }
    }

    fn canonicalize_and_validate(
        &mut self,
        limits: DecoderLimits,
        version: ModelSchemaVersion,
    ) -> Result<(), Diagnostic> {
        if self.wire.schema != version.schema() || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(format!(
                "unsupported {} schema or canonical encoding",
                version.schema()
            )));
        }
        if self.wire.source_revision == 0 {
            return Err(invalid_artifact(format!(
                "{} source revision must be nonzero",
                version.envelope_label()
            )));
        }
        parse_ulid(&self.wire.model_ulid)?;
        if self.wire.nodes.is_empty() || self.wire.nodes.len() > limits.max_nodes {
            return Err(invalid_artifact(format!(
                "{} requires 1..={} nodes, found {}",
                version.envelope_label(),
                limits.max_nodes,
                self.wire.nodes.len()
            )));
        }
        require_decoder_count(
            &format!("{} view members", version.model_label()),
            self.wire.nodes.len(),
            limits.max_model_view_members,
        )?;
        require_decoder_count(
            &format!("{} boundary Ports", version.model_label()),
            self.wire.boundary.len(),
            limits.max_model_boundary,
        )?;
        if self.wire.edges.len() > limits.max_edges {
            return Err(invalid_artifact(format!(
                "{} has {} edges, exceeding the {} edge limit",
                version.envelope_label(),
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
                "{} has {expression_nodes} expression nodes, exceeding the {} node limit",
                version.envelope_label(),
                limits.max_expression_nodes
            )));
        }
        let expression_roots = checked_count_sum(
            self.wire.nodes.iter().map(WireNode::expression_root_count),
            &format!("{} expression-root count", version.model_label()),
        )?;
        require_decoder_count(
            &format!("{} expression roots", version.model_label()),
            expression_roots,
            limits.max_expression_roots,
        )?;
        let pure_operator_counts = self.wire.nodes.iter().try_fold(
            crate::model::PureOperatorWireCounts::default(),
            |counts, node| counts.checked_add(node.pure_operator_counts()?),
        )?;
        pure_operator_counts.ensure_limits(limits, version.model_label())?;

        self.wire
            .nodes
            .sort_by(|left, right| left.id.cmp(&right.id));
        reject_duplicates(
            self.wire
                .nodes
                .windows(2)
                .any(|pair| pair[0].id == pair[1].id),
            "kernel node ID",
            version,
        )?;
        self.wire
            .values
            .sort_by(|left, right| left.target.cmp(&right.target));
        reject_duplicates(
            self.wire
                .values
                .windows(2)
                .any(|pair| pair[0].target == pair[1].target),
            "current value target",
            version,
        )?;
        self.wire.edges.sort();
        reject_duplicates(
            self.wire.edges.windows(2).any(|pair| pair[0] == pair[1]),
            "graph edge",
            version,
        )?;
        self.wire.boundary.sort();
        reject_duplicates(
            self.wire.boundary.windows(2).any(|pair| pair[0] == pair[1]),
            "boundary ID",
            version,
        )?;

        if matches!(version, ModelSchemaVersion::V5 | ModelSchemaVersion::V6) {
            for node in &self.wire.nodes {
                node.validate_v5_features()?;
            }
            for node in &mut self.wire.nodes {
                node.canonicalize_v5_definitions()?;
            }
        }

        for node in &self.wire.nodes {
            node.ensure_value_shape_limits(limits)?;
            match version {
                ModelSchemaVersion::V2 => node.ensure_v2()?,
                ModelSchemaVersion::V3 => node.ensure_v3()?,
                ModelSchemaVersion::V4 => node.ensure_v4()?,
                ModelSchemaVersion::V5 => node.ensure_v5()?,
                ModelSchemaVersion::V6 => node.ensure_v6()?,
            }
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
        for node in &self.wire.nodes {
            for reference in node.semantic_references() {
                require_reference(&ids, reference, "definition reference", version)?;
            }
        }
        for value in &self.wire.values {
            require_reference(&ids, &value.target, "current value", version)?;
        }
        for edge in &self.wire.edges {
            require_reference(&ids, &edge.from, "edge source", version)?;
            require_reference(&ids, &edge.to, "edge target", version)?;
            let from = edge.from.decode_raw()?;
            let to = edge.to.decode_raw()?;
            if !edge.kind.decode().permits(from.kind(), to.kind()) {
                return Err(invalid_artifact(format!(
                    "{} edge endpoints violate the closed graph edge schema",
                    version.model_label()
                )));
            }
        }
        for boundary in &self.wire.boundary {
            require_reference(&ids, boundary, "boundary", version)?;
            if boundary.decode_raw()?.kind() != EntityKind::Port {
                return Err(invalid_artifact("model boundary may contain only Port IDs"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireModelEnvelopeV2 {
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
struct WireModelContentV2<'a> {
    schema: &'a str,
    encoding: &'a str,
    model_ulid: &'a str,
    nodes: &'a [WireNode],
    values: &'a [WireValue],
    edges: &'a [WireEdge],
    boundary: &'a [WireId],
}

fn lookup_id(ids: &BTreeMap<WireId, RawId>, id: &WireId) -> Result<RawId, Diagnostic> {
    ids.get(id)
        .copied()
        .ok_or_else(|| invalid_artifact("model v2 reference is not a model node"))
}

fn require_reference(
    ids: &BTreeSet<WireId>,
    id: &WireId,
    label: &str,
    version: ModelSchemaVersion,
) -> Result<(), Diagnostic> {
    if ids.contains(id) {
        Ok(())
    } else {
        Err(invalid_artifact(format!(
            "{} {label} is not a model node",
            version.model_label()
        )))
    }
}

fn reject_duplicates(
    duplicate: bool,
    label: &str,
    version: ModelSchemaVersion,
) -> Result<(), Diagnostic> {
    if duplicate {
        Err(invalid_artifact(format!(
            "{} contains a duplicate {label}",
            version.envelope_label()
        )))
    } else {
        Ok(())
    }
}
