//! Alpha-normalized structural identity of one accepted Semantic Model.
//!
//! Exact Model artifacts retain occurrence ULIDs and remain authoritative for
//! replay, provenance, and mutation.  This module instead constructs a closed,
//! versioned projection of the accepted kernel graph, canonically labels that
//! graph without consulting occurrence IDs, and hashes the resulting bytes.

use core::fmt;
use std::collections::BTreeMap;

use eqiora_core::{Diagnostic, DimExponents, DynQuantity, RawId, ValueShape};
use eqiora_graph::EdgeKind;
use eqiora_schema::kernel::{
    ActivationKind, BoundaryPairing, BoundarySide, ClockKind, ConnectionSemantics, DomainKind,
    EventDirection, ExprDag, ExprNode, KernelNode, PortPayload, RepresentationKind,
    SignalDirection, SymbolRef, UnaryMathFunction, ValueFrame,
};
use eqiora_sem::KernelProgram;
use sha2::{Digest, Sha256};

use crate::{ArtifactDigest, invalid_artifact};

const FINGERPRINT_DOMAIN: &[u8] = b"eqiora.structural-semantic-fingerprint/v1\0";
const PROJECTION_MAGIC: &[u8; 8] = b"EQIORASF";
const GENERATION_V1: u16 = 1;

/// Compatibility generation of the structural semantic projection.
///
/// Generations are intentionally independent of Model artifact wire versions.
/// Equality is defined only within one explicitly equal generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum SemanticFingerprintGeneration {
    /// Closed projection of the Semantic Kernel vocabulary through Model wire v6.
    V1,
}

impl SemanticFingerprintGeneration {
    /// Stable external spelling of this comparison generation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "eqiora.structural-semantic-fingerprint/v1",
        }
    }
}

/// Comparison/cache evidence for one alpha-normalized Semantic Model graph.
///
/// This value is deliberately not a Model artifact identity.  It cannot be
/// used as an execution input, replay key, provenance reference, or mutation
/// precondition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralSemanticFingerprint {
    generation: SemanticFingerprintGeneration,
    digest: ArtifactDigest,
}

impl StructuralSemanticFingerprint {
    /// Construct the current bounded structural projection.
    ///
    /// # Errors
    /// Returns `EQ0901` when the program contains vocabulary newer than this
    /// generation or exact canonical labeling exceeds the fixed generation-v1
    /// resource policy.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        ProjectionIdentity::from_program(program, SemanticFingerprintLimits::default())
            .map(|identity| identity.fingerprint)
    }

    /// Construct with an explicit bounded canonicalization policy.
    ///
    /// Limits affect admission only.  Every accepted construction produces
    /// exactly the same generation-v1 bytes and digest.
    ///
    /// # Errors
    /// Returns `EQ0901` for unsupported meaning or exhausted limits.
    #[cfg(test)]
    fn from_program_with_limits(
        program: &KernelProgram,
        limits: SemanticFingerprintLimits,
    ) -> Result<Self, Diagnostic> {
        ProjectionIdentity::from_program(program, limits).map(|identity| identity.fingerprint)
    }

    /// Exact structural comparison generation.
    #[must_use]
    pub const fn generation(&self) -> SemanticFingerprintGeneration {
        self.generation
    }

    /// Hexadecimal domain-separated SHA-256 of the closed canonical projection.
    ///
    /// The view deliberately does not expose [`ArtifactDigest`], which is an
    /// authority-bearing input to artifact and Run lineage constructors.
    #[must_use]
    pub fn digest(&self) -> &str {
        self.digest.as_str()
    }
}

impl fmt::Display for StructuralSemanticFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.generation.as_str(), self.digest)
    }
}

/// Resource policy for exact graph canonicalization.
///
/// The algorithm never falls back to occurrence ordering or a probabilistic
/// refinement.  A pathological symmetry that exceeds these limits is rejected
/// instead of producing a route-dependent fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SemanticFingerprintLimits {
    /// Maximum kernel vertices in one selected Model.
    max_nodes: usize,
    /// Maximum graph edges plus nominal and expression references.
    max_references: usize,
    /// Maximum expression nodes summed across Relations and Activations.
    max_expression_nodes: usize,
    /// Maximum bytes in one canonical projection or intermediate label set.
    max_canonical_bytes: usize,
    /// Maximum individualization/refinement search states.
    max_search_states: usize,
    /// Maximum recursive individualization depth.
    max_individualization_depth: usize,
    /// Maximum vertex/reference visits across refinement rounds.
    max_refinement_work: usize,
    /// Maximum canonical bytes produced across every discrete search leaf.
    max_serialization_work: usize,
}

impl Default for SemanticFingerprintLimits {
    fn default() -> Self {
        Self {
            max_nodes: 100_000,
            max_references: 1_000_000,
            max_expression_nodes: 1_000_000,
            max_canonical_bytes: 128 * 1_024 * 1_024,
            max_search_states: 100_000,
            max_individualization_depth: 256,
            max_refinement_work: 100_000_000,
            max_serialization_work: 512 * 1_024 * 1_024,
        }
    }
}

/// Compare two programs through the same closed canonical projection.
///
/// Unlike comparing only fingerprints, this bounded consumer also compares
/// canonical bytes after equal digests.  A cryptographic collision therefore
/// fails closed rather than being reported as semantic equality.
///
/// # Errors
/// Returns `EQ0901` for unsupported meaning, exhausted construction limits,
/// or a digest collision between unequal canonical projections.
pub fn structurally_equivalent(
    left: &KernelProgram,
    right: &KernelProgram,
) -> Result<bool, Diagnostic> {
    let limits = SemanticFingerprintLimits::default();
    let left = ProjectionIdentity::from_program(left, limits)?;
    let right = ProjectionIdentity::from_program(right, limits)?;
    if left.fingerprint != right.fingerprint {
        return Ok(false);
    }
    if left.canonical != right.canonical {
        return Err(fingerprint_error(
            "structural semantic fingerprint collision between unequal canonical projections",
        ));
    }
    Ok(true)
}

struct ProjectionIdentity {
    fingerprint: StructuralSemanticFingerprint,
    canonical: Vec<u8>,
}

impl ProjectionIdentity {
    fn from_program(
        program: &KernelProgram,
        limits: SemanticFingerprintLimits,
    ) -> Result<Self, Diagnostic> {
        validate_limits(limits)?;
        let graph = ProjectionGraph::from_program(program, limits)?;
        let canonical = Canonicalizer::new(&graph, limits).canonicalize()?;
        let mut hasher = Sha256::new();
        hasher.update(FINGERPRINT_DOMAIN);
        hasher.update(&canonical);
        let digest = ArtifactDigest::from_sha256(hasher.finalize().into());
        Ok(Self {
            fingerprint: StructuralSemanticFingerprint {
                generation: SemanticFingerprintGeneration::V1,
                digest,
            },
            canonical,
        })
    }
}

#[derive(Clone)]
struct Reference {
    label: Vec<u8>,
    target: usize,
}

struct Vertex {
    intrinsic: Vec<u8>,
    outgoing: Vec<Reference>,
    incoming: Vec<Reference>,
}

struct ProjectionGraph {
    vertices: Vec<Vertex>,
    reference_count: usize,
}

impl ProjectionGraph {
    fn from_program(
        program: &KernelProgram,
        limits: SemanticFingerprintLimits,
    ) -> Result<Self, Diagnostic> {
        let node_count = program.nodes().len();
        if node_count == 0 || node_count > limits.max_nodes {
            return Err(fingerprint_error(format!(
                "structural semantic projection requires 1..={} nodes, found {}",
                limits.max_nodes, node_count
            )));
        }
        if u32::try_from(node_count).is_err() {
            return Err(fingerprint_error(
                "structural semantic projection node count exceeds u32",
            ));
        }
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(node_count)
            .map_err(|_| fingerprint_error("cannot reserve semantic projection node view"))?;
        nodes.extend(program.nodes());

        let ids = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id(), index))
            .collect::<BTreeMap<_, _>>();
        let mut budget = ConstructionBudget::new(limits);
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(node_count)
            .map_err(|_| fingerprint_error("cannot reserve semantic projection vertices"))?;
        for node in nodes {
            let mut references = Vec::new();
            let intrinsic = encode_node(
                node,
                program.value(node.id()),
                program.boundary().contains(&node.id()),
                &ids,
                &mut references,
                &mut budget,
            )?;
            budget.account_bytes(intrinsic.len())?;
            vertices.push(Vertex {
                intrinsic,
                outgoing: references,
                incoming: Vec::new(),
            });
        }

        for edge in program.edges() {
            let source = lookup(&ids, edge.from(), "edge source")?;
            let target = lookup(&ids, edge.to(), "edge target")?;
            vertices[source].outgoing.push(Reference {
                label: edge_label(edge.kind())?,
                target,
            });
            budget.account_reference()?;
        }

        let reference_count = vertices.iter().map(|vertex| vertex.outgoing.len()).sum();
        if reference_count > limits.max_references {
            return Err(fingerprint_error(format!(
                "structural semantic projection has {reference_count} references, exceeding the {} reference limit",
                limits.max_references
            )));
        }
        for source in 0..vertices.len() {
            vertices[source].outgoing.sort_by(|left, right| {
                left.label
                    .cmp(&right.label)
                    .then(left.target.cmp(&right.target))
            });
            for reference in vertices[source].outgoing.clone() {
                vertices[reference.target].incoming.push(Reference {
                    label: reference.label,
                    target: source,
                });
            }
        }
        for vertex in &mut vertices {
            vertex.incoming.sort_by(|left, right| {
                left.label
                    .cmp(&right.label)
                    .then(left.target.cmp(&right.target))
            });
        }
        Ok(Self {
            vertices,
            reference_count,
        })
    }
}

struct ConstructionBudget {
    limits: SemanticFingerprintLimits,
    references: usize,
    expression_nodes: usize,
    bytes: usize,
}

impl ConstructionBudget {
    const fn new(limits: SemanticFingerprintLimits) -> Self {
        Self {
            limits,
            references: 0,
            expression_nodes: 0,
            bytes: 0,
        }
    }

    fn account_reference(&mut self) -> Result<(), Diagnostic> {
        self.references = self
            .references
            .checked_add(1)
            .ok_or_else(|| fingerprint_error("semantic reference count overflows usize"))?;
        if self.references > self.limits.max_references {
            return Err(fingerprint_error(format!(
                "semantic reference count exceeds the {} reference limit",
                self.limits.max_references
            )));
        }
        Ok(())
    }

    fn account_expression_nodes(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.expression_nodes = self
            .expression_nodes
            .checked_add(count)
            .ok_or_else(|| fingerprint_error("expression-node count overflows usize"))?;
        if self.expression_nodes > self.limits.max_expression_nodes {
            return Err(fingerprint_error(format!(
                "semantic projection expression-node count exceeds the {} node limit",
                self.limits.max_expression_nodes
            )));
        }
        Ok(())
    }

    fn account_bytes(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(count)
            .ok_or_else(|| fingerprint_error("semantic projection byte count overflows usize"))?;
        if self.bytes > self.limits.max_canonical_bytes {
            return Err(fingerprint_error(format!(
                "semantic projection exceeds the {} byte limit",
                self.limits.max_canonical_bytes
            )));
        }
        Ok(())
    }
}

fn encode_node(
    node: &KernelNode,
    current_value: Option<DynQuantity>,
    boundary: bool,
    ids: &BTreeMap<RawId, usize>,
    references: &mut Vec<Reference>,
    budget: &mut ConstructionBudget,
) -> Result<Vec<u8>, Diagnostic> {
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    match node {
        KernelNode::Domain(domain) => {
            encoder.u8(1)?;
            encode_domain_kind(&mut encoder, domain.kind())?;
        }
        KernelNode::Representation(representation) => {
            encoder.u8(2)?;
            match representation.kind() {
                RepresentationKind::Abstract => encoder.u8(1)?,
                RepresentationKind::Continuum => encoder.u8(2)?,
                _ => return Err(newer_vocabulary("Representation kind")),
            }
        }
        KernelNode::Field(field) => {
            encoder.u8(3)?;
            encode_dimension(&mut encoder, field.dimension())?;
            encode_shape(&mut encoder, field.shape())?;
            encode_frame(&mut encoder, field.frame())?;
            encode_optional_quantity(&mut encoder, field.initial())?;
        }
        KernelNode::Parameter(parameter) => {
            encoder.u8(4)?;
            encode_quantity(&mut encoder, parameter.value())?;
        }
        KernelNode::Port(port) => {
            encoder.u8(5)?;
            match port.payload() {
                PortPayload::Signal {
                    direction,
                    dimension,
                } => {
                    encoder.u8(1)?;
                    encode_signal_direction(&mut encoder, direction)?;
                    encode_dimension(&mut encoder, dimension)?;
                }
                PortPayload::ConservingMarker { dimension } => {
                    encoder.u8(2)?;
                    encode_dimension(&mut encoder, dimension)?;
                }
                PortPayload::ScalarPhysical { domain } => {
                    encoder.u8(3)?;
                    push_reference(
                        references,
                        nominal_label(1),
                        lookup(ids, domain.erase(), "scalar physical Port Domain")?,
                        budget,
                    )?;
                }
                PortPayload::BoundaryPhysical {
                    connector,
                    boundary,
                } => {
                    encoder.u8(4)?;
                    push_reference(
                        references,
                        nominal_label(2),
                        lookup(ids, connector.erase(), "boundary Port connector")?,
                        budget,
                    )?;
                    push_reference(
                        references,
                        nominal_label(3),
                        lookup(ids, boundary.erase(), "boundary Port support")?,
                        budget,
                    )?;
                }
                _ => return Err(newer_vocabulary("Port payload")),
            }
        }
        KernelNode::Relation(relation) => {
            encoder.u8(6)?;
            encode_expression(
                &mut encoder,
                relation.residuals(),
                1,
                ids,
                references,
                budget,
            )?;
        }
        KernelNode::Activation(activation) => {
            encoder.u8(7)?;
            match activation.kind() {
                ActivationKind::Continuous => encoder.u8(1)?,
                ActivationKind::Periodic => encoder.u8(2)?,
                ActivationKind::Event { guard, direction } => {
                    encoder.u8(3)?;
                    encode_event_direction(&mut encoder, *direction)?;
                    encode_expression(&mut encoder, guard, 2, ids, references, budget)?;
                }
                ActivationKind::Guard { guard } => {
                    encoder.u8(4)?;
                    encode_expression(&mut encoder, guard, 3, ids, references, budget)?;
                }
                _ => return Err(newer_vocabulary("Activation kind")),
            }
        }
        KernelNode::Connection(connection) => {
            encoder.u8(8)?;
            match connection.semantics() {
                ConnectionSemantics::Signal => encoder.u8(1)?,
                ConnectionSemantics::Conserving => encoder.u8(2)?,
                ConnectionSemantics::SpatialPeriodic => encoder.u8(3)?,
                _ => return Err(newer_vocabulary("Connection semantics")),
            }
        }
        KernelNode::ClockDomain(clock) => {
            encoder.u8(9)?;
            match clock.kind() {
                ClockKind::Continuous => encoder.u8(1)?,
                ClockKind::Periodic { period, phase } => {
                    encoder.u8(2)?;
                    encoder.u64(period.numerator())?;
                    encoder.u64(period.denominator())?;
                    encoder.u64(phase.numerator())?;
                    encoder.u64(phase.denominator())?;
                }
                ClockKind::Aperiodic => encoder.u8(3)?,
                ClockKind::Inherited => encoder.u8(4)?,
                _ => return Err(newer_vocabulary("ClockDomain kind")),
            }
        }
        _ => return Err(newer_vocabulary("Semantic Kernel node")),
    }
    encode_optional_quantity(&mut encoder, current_value)?;
    encoder.bool(boundary)?;
    encoder.finish()
}

fn encode_domain_kind(encoder: &mut Encoder, kind: &DomainKind) -> Result<(), Diagnostic> {
    match kind {
        DomainKind::Abstract => encoder.u8(1),
        DomainKind::CartesianBox { bounds } => {
            encoder.u8(2)?;
            encoder.len(bounds.len())?;
            for axis in bounds {
                encode_quantity(encoder, axis.lower())?;
                encode_quantity(encoder, axis.upper())?;
            }
            Ok(())
        }
        DomainKind::CartesianBoundary { axis, side } => {
            encoder.u8(3)?;
            encoder.usize(*axis)?;
            encode_boundary_side(encoder, *side)
        }
        DomainKind::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => {
            encoder.u8(4)?;
            encode_dimension(encoder, *across_dimension)?;
            encode_dimension(encoder, *through_dimension)
        }
        DomainKind::BoundaryPhysical { connector } => {
            encoder.u8(5)?;
            encode_dimension(encoder, connector.trace_dimension())?;
            encode_dimension(encoder, connector.flux_dimension())?;
            encode_shape(encoder, connector.shape())?;
            encode_frame(encoder, connector.frame())?;
            match connector.pairing() {
                BoundaryPairing::EuclideanBoundaryDuality => encoder.u8(1),
            }
        }
        _ => Err(newer_vocabulary("Domain kind")),
    }
}

fn encode_expression(
    encoder: &mut Encoder,
    expression: &ExprDag,
    scope: u8,
    ids: &BTreeMap<RawId, usize>,
    references: &mut Vec<Reference>,
    budget: &mut ConstructionBudget,
) -> Result<(), Diagnostic> {
    budget.account_expression_nodes(expression.nodes().len())?;
    let (order, canonical_index) = canonical_expression_order(expression)?;
    encoder.len(order.len())?;
    for original_index in order {
        let node = expression.nodes().get(original_index).ok_or_else(|| {
            fingerprint_error("canonical expression order references an absent node")
        })?;
        let index = canonical_index[original_index];
        match node {
            ExprNode::Constant(value) => {
                encoder.u8(1)?;
                encode_quantity(encoder, *value)?;
            }
            ExprNode::Symbol(symbol) => {
                encoder.u8(2)?;
                encode_symbol(encoder, *symbol, scope, index, ids, references, budget)?;
            }
            ExprNode::Neg(value) => unary_expr(encoder, 3, *value, &canonical_index)?,
            ExprNode::Add(left, right) => binary_expr(encoder, 4, *left, *right, &canonical_index)?,
            ExprNode::Sub(left, right) => binary_expr(encoder, 5, *left, *right, &canonical_index)?,
            ExprNode::Mul(left, right) => binary_expr(encoder, 6, *left, *right, &canonical_index)?,
            ExprNode::Div(left, right) => binary_expr(encoder, 7, *left, *right, &canonical_index)?,
            ExprNode::PowI(value, exponent) => {
                encoder.u8(8)?;
                encoder.u32(canonical_expr_id(*value, &canonical_index)?)?;
                encoder.i32(*exponent)?;
            }
            ExprNode::SpatialCoordinate(axis) => {
                encoder.u8(9)?;
                encoder.usize(*axis)?;
            }
            ExprNode::UnaryMath(function, value) => {
                encoder.u8(10)?;
                match function {
                    UnaryMathFunction::Sin => encoder.u8(1)?,
                    _ => return Err(newer_vocabulary("unary math function")),
                }
                encoder.u32(canonical_expr_id(*value, &canonical_index)?)?;
            }
            ExprNode::Gradient(value) => unary_expr(encoder, 11, *value, &canonical_index)?,
            ExprNode::Divergence(value) => unary_expr(encoder, 12, *value, &canonical_index)?,
            ExprNode::SymmetricPart(value) => unary_expr(encoder, 13, *value, &canonical_index)?,
            ExprNode::IsotropicLift(value) => unary_expr(encoder, 14, *value, &canonical_index)?,
            ExprNode::Trace(value) => unary_expr(encoder, 15, *value, &canonical_index)?,
            ExprNode::NormalComponent(value) => unary_expr(encoder, 16, *value, &canonical_index)?,
            ExprNode::PureOperatorApplication(application) => {
                encoder.u8(17)?;
                encoder.raw(&application.definition().bytes())?;
                encoder.len(application.arguments().len())?;
                for argument in application.arguments() {
                    encoder.u32(canonical_expr_id(*argument, &canonical_index)?)?;
                }
            }
            _ => return Err(newer_vocabulary("expression node")),
        }
    }
    encoder.len(expression.roots().len())?;
    for root in expression.roots() {
        encoder.u32(canonical_expr_id(*root, &canonical_index)?)?;
    }
    encoder.len(expression.definitions().len())?;
    for (digest, definition) in expression.definitions() {
        encoder.raw(&digest.bytes())?;
        let bytes = definition.canonical_bytes();
        budget.account_bytes(bytes.len())?;
        encoder.bytes(&bytes)?;
    }
    Ok(())
}

fn encode_symbol(
    encoder: &mut Encoder,
    symbol: SymbolRef,
    scope: u8,
    expression_index: u32,
    ids: &BTreeMap<RawId, usize>,
    references: &mut Vec<Reference>,
    budget: &mut ConstructionBudget,
) -> Result<(), Diagnostic> {
    let (tag, target) = match symbol {
        SymbolRef::Field(id) => (1, Some(id.erase())),
        SymbolRef::Derivative(id) => (2, Some(id.erase())),
        SymbolRef::Pre(id) => (3, Some(id.erase())),
        SymbolRef::Next(id) => (4, Some(id.erase())),
        SymbolRef::Parameter(id) => (5, Some(id.erase())),
        SymbolRef::Port(id) => (6, Some(id.erase())),
        SymbolRef::Across(id) => (7, Some(id.erase())),
        SymbolRef::Through(id) => (8, Some(id.erase())),
        SymbolRef::PortTrace(id) => (9, Some(id.erase())),
        SymbolRef::PortFlux(id) => (10, Some(id.erase())),
        SymbolRef::Time => (11, None),
        _ => return Err(newer_vocabulary("expression symbol")),
    };
    encoder.u8(tag)?;
    if let Some(target) = target {
        let mut label = Encoder::new(32);
        label.u8(3)?;
        label.u8(scope)?;
        label.u32(expression_index)?;
        label.u8(tag)?;
        push_reference(
            references,
            label.finish()?,
            lookup(ids, target, "expression symbol")?,
            budget,
        )?;
    }
    Ok(())
}

fn unary_expr(
    encoder: &mut Encoder,
    tag: u8,
    value: eqiora_schema::kernel::ExprId,
    canonical_index: &[u32],
) -> Result<(), Diagnostic> {
    encoder.u8(tag)?;
    encoder.u32(canonical_expr_id(value, canonical_index)?)
}

fn binary_expr(
    encoder: &mut Encoder,
    tag: u8,
    left: eqiora_schema::kernel::ExprId,
    right: eqiora_schema::kernel::ExprId,
    canonical_index: &[u32],
) -> Result<(), Diagnostic> {
    encoder.u8(tag)?;
    encoder.u32(canonical_expr_id(left, canonical_index)?)?;
    encoder.u32(canonical_expr_id(right, canonical_index)?)
}

fn canonical_expression_order(expression: &ExprDag) -> Result<(Vec<usize>, Vec<u32>), Diagnostic> {
    let nodes = expression.nodes();
    let mut state = vec![0_u8; nodes.len()];
    let mut order = Vec::new();
    order
        .try_reserve_exact(nodes.len())
        .map_err(|_| fingerprint_error("cannot reserve canonical expression order"))?;
    for root in expression.roots() {
        let root = expression_index(*root, nodes.len())?;
        let mut stack = vec![(root, false)];
        while let Some((index, exiting)) = stack.pop() {
            if exiting {
                if state[index] != 2 {
                    state[index] = 2;
                    order.push(index);
                }
                continue;
            }
            match state[index] {
                2 => continue,
                1 => {
                    return Err(fingerprint_error(
                        "structural semantic projection found a cyclic expression DAG",
                    ));
                }
                _ => state[index] = 1,
            }
            stack.push((index, true));
            let operands = expression_operands(&nodes[index]);
            for operand in operands.into_iter().rev() {
                let operand = expression_index(operand, nodes.len())?;
                if state[operand] != 2 {
                    stack.push((operand, false));
                }
            }
        }
    }
    if order.len() != nodes.len() {
        return Err(fingerprint_error(
            "structural semantic projection rejects unreachable expression nodes",
        ));
    }
    let mut canonical_index = vec![0_u32; nodes.len()];
    for (canonical, &original) in order.iter().enumerate() {
        canonical_index[original] = u32::try_from(canonical)
            .map_err(|_| fingerprint_error("canonical expression index exceeds u32"))?;
    }
    Ok((order, canonical_index))
}

fn expression_operands(node: &ExprNode) -> Vec<eqiora_schema::kernel::ExprId> {
    match node {
        ExprNode::Neg(value)
        | ExprNode::PowI(value, _)
        | ExprNode::UnaryMath(_, value)
        | ExprNode::Gradient(value)
        | ExprNode::Divergence(value)
        | ExprNode::SymmetricPart(value)
        | ExprNode::IsotropicLift(value)
        | ExprNode::Trace(value)
        | ExprNode::NormalComponent(value) => vec![*value],
        ExprNode::Add(left, right)
        | ExprNode::Sub(left, right)
        | ExprNode::Mul(left, right)
        | ExprNode::Div(left, right) => vec![*left, *right],
        ExprNode::PureOperatorApplication(application) => application.arguments().to_vec(),
        ExprNode::Constant(_) | ExprNode::Symbol(_) | ExprNode::SpatialCoordinate(_) => Vec::new(),
        _ => Vec::new(),
    }
}

fn canonical_expr_id(
    id: eqiora_schema::kernel::ExprId,
    canonical_index: &[u32],
) -> Result<u32, Diagnostic> {
    let index = expression_index(id, canonical_index.len())?;
    canonical_index
        .get(index)
        .copied()
        .ok_or_else(|| fingerprint_error("canonical expression index is absent"))
}

fn expression_index(id: eqiora_schema::kernel::ExprId, upper: usize) -> Result<usize, Diagnostic> {
    usize::try_from(id.index())
        .ok()
        .filter(|index| *index < upper)
        .ok_or_else(|| fingerprint_error("expression operand is outside its DAG"))
}

fn push_reference(
    references: &mut Vec<Reference>,
    label: Vec<u8>,
    target: usize,
    budget: &mut ConstructionBudget,
) -> Result<(), Diagnostic> {
    budget.account_reference()?;
    budget.account_bytes(label.len())?;
    references
        .try_reserve(1)
        .map_err(|_| fingerprint_error("cannot reserve semantic projection reference"))?;
    references.push(Reference { label, target });
    Ok(())
}

fn nominal_label(role: u8) -> Vec<u8> {
    vec![2, role]
}

fn edge_label(kind: EdgeKind) -> Result<Vec<u8>, Diagnostic> {
    let tag = match kind {
        EdgeKind::DefinedOn => 1,
        EdgeKind::AppliesOn => 2,
        EdgeKind::BoundaryOf => 3,
        EdgeKind::DependsOn => 4,
        EdgeKind::HasPort => 5,
        EdgeKind::Activates => 6,
        EdgeKind::Connects => 7,
        EdgeKind::ClockedBy => 8,
        _ => return Err(newer_vocabulary("Semantic Model edge")),
    };
    Ok(vec![1, tag])
}

fn lookup(ids: &BTreeMap<RawId, usize>, id: RawId, role: &str) -> Result<usize, Diagnostic> {
    ids.get(&id).copied().ok_or_else(|| {
        fingerprint_error(format!(
            "{role} {id} is outside the accepted Semantic Model projection"
        ))
    })
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Signature {
    intrinsic: Vec<u8>,
    outgoing: Vec<(Vec<u8>, usize)>,
    incoming: Vec<(Vec<u8>, usize)>,
}

struct Canonicalizer<'a> {
    graph: &'a ProjectionGraph,
    limits: SemanticFingerprintLimits,
    search_states: usize,
    refinement_work: usize,
    serialization_work: usize,
    best: Option<Vec<u8>>,
}

impl<'a> Canonicalizer<'a> {
    const fn new(graph: &'a ProjectionGraph, limits: SemanticFingerprintLimits) -> Self {
        Self {
            graph,
            limits,
            search_states: 0,
            refinement_work: 0,
            serialization_work: 0,
            best: None,
        }
    }

    fn canonicalize(mut self) -> Result<Vec<u8>, Diagnostic> {
        let mut groups = BTreeMap::<Vec<u8>, Vec<usize>>::new();
        for (index, vertex) in self.graph.vertices.iter().enumerate() {
            groups
                .entry(vertex.intrinsic.clone())
                .or_default()
                .push(index);
        }
        let partition = groups.into_values().collect::<Vec<_>>();
        let partition = self.refine(partition)?;
        self.search(partition, 0)?;
        self.best
            .take()
            .ok_or_else(|| fingerprint_error("canonical-label search produced no discrete leaf"))
    }

    fn search(&mut self, partition: Vec<Vec<usize>>, depth: usize) -> Result<(), Diagnostic> {
        self.search_states = self
            .search_states
            .checked_add(1)
            .ok_or_else(|| fingerprint_error("canonical-label search-state count overflows"))?;
        if self.search_states > self.limits.max_search_states {
            return Err(fingerprint_error(format!(
                "exact semantic graph canonicalization exceeds the {} search-state limit",
                self.limits.max_search_states
            )));
        }
        let Some(cell_index) = partition.iter().position(|cell| cell.len() > 1) else {
            let remaining = self
                .limits
                .max_serialization_work
                .checked_sub(self.serialization_work)
                .ok_or_else(|| fingerprint_error("canonical serialization work overflows"))?;
            if remaining == 0 {
                return Err(fingerprint_error(format!(
                    "exact semantic graph canonicalization exceeds the {} serialization-byte-work limit",
                    self.limits.max_serialization_work
                )));
            }
            let canonical = self.serialize(&partition, remaining)?;
            self.serialization_work = self
                .serialization_work
                .checked_add(canonical.len())
                .ok_or_else(|| fingerprint_error("canonical serialization work overflows"))?;
            if self
                .best
                .as_ref()
                .is_none_or(|current| canonical < *current)
            {
                self.best = Some(canonical);
            }
            return Ok(());
        };
        if depth >= self.limits.max_individualization_depth {
            return Err(fingerprint_error(format!(
                "exact semantic graph canonicalization exceeds the {} individualization-depth limit",
                self.limits.max_individualization_depth
            )));
        }

        let candidates = partition[cell_index].clone();
        for candidate in candidates {
            let mut branch = partition.clone();
            let remainder = branch[cell_index]
                .iter()
                .copied()
                .filter(|vertex| *vertex != candidate)
                .collect::<Vec<_>>();
            branch.splice(cell_index..=cell_index, [vec![candidate], remainder]);
            let branch = self.refine(branch)?;
            self.search(branch, depth + 1)?;
        }
        Ok(())
    }

    fn refine(&mut self, mut partition: Vec<Vec<usize>>) -> Result<Vec<Vec<usize>>, Diagnostic> {
        loop {
            self.account_refinement_work()?;
            let mut cell_of = vec![0_usize; self.graph.vertices.len()];
            for (cell, vertices) in partition.iter().enumerate() {
                for &vertex in vertices {
                    cell_of[vertex] = cell;
                }
            }
            let mut groups = BTreeMap::<(usize, Signature), Vec<usize>>::new();
            for (cell, vertices) in partition.iter().enumerate() {
                for &vertex in vertices {
                    let value = &self.graph.vertices[vertex];
                    let mut outgoing = value
                        .outgoing
                        .iter()
                        .map(|reference| (reference.label.clone(), cell_of[reference.target]))
                        .collect::<Vec<_>>();
                    outgoing.sort_unstable();
                    let mut incoming = value
                        .incoming
                        .iter()
                        .map(|reference| (reference.label.clone(), cell_of[reference.target]))
                        .collect::<Vec<_>>();
                    incoming.sort_unstable();
                    let signature = Signature {
                        intrinsic: value.intrinsic.clone(),
                        outgoing,
                        incoming,
                    };
                    groups.entry((cell, signature)).or_default().push(vertex);
                }
            }
            let refined = groups.into_values().collect::<Vec<_>>();
            if refined.len() == partition.len() {
                return Ok(refined);
            }
            partition = refined;
        }
    }

    fn account_refinement_work(&mut self) -> Result<(), Diagnostic> {
        let round = self
            .graph
            .vertices
            .len()
            .checked_add(self.graph.reference_count.saturating_mul(2))
            .ok_or_else(|| fingerprint_error("canonical refinement work overflows usize"))?;
        self.refinement_work = self
            .refinement_work
            .checked_add(round)
            .ok_or_else(|| fingerprint_error("canonical refinement work overflows usize"))?;
        if self.refinement_work > self.limits.max_refinement_work {
            return Err(fingerprint_error(format!(
                "exact semantic graph canonicalization exceeds the {} refinement-work limit",
                self.limits.max_refinement_work
            )));
        }
        Ok(())
    }

    fn serialize(
        &self,
        partition: &[Vec<usize>],
        remaining_work: usize,
    ) -> Result<Vec<u8>, Diagnostic> {
        let order = partition
            .iter()
            .map(|cell| {
                if cell.len() == 1 {
                    Ok(cell[0])
                } else {
                    Err(fingerprint_error(
                        "canonical semantic partition is unexpectedly non-discrete",
                    ))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut ordinal = vec![0_usize; order.len()];
        for (position, &vertex) in order.iter().enumerate() {
            ordinal[vertex] = position;
        }
        let mut encoder = Encoder::new(self.limits.max_canonical_bytes.min(remaining_work));
        encoder.raw(PROJECTION_MAGIC)?;
        encoder.u16(GENERATION_V1)?;
        encoder.len(order.len())?;
        for vertex in order {
            let value = &self.graph.vertices[vertex];
            encoder.bytes(&value.intrinsic)?;
            let mut references = value
                .outgoing
                .iter()
                .map(|reference| (reference.label.as_slice(), ordinal[reference.target]))
                .collect::<Vec<_>>();
            references.sort_unstable();
            encoder.len(references.len())?;
            for (label, target) in references {
                encoder.bytes(label)?;
                encoder.usize(target)?;
            }
        }
        encoder.finish()
    }
}

struct Encoder {
    bytes: Vec<u8>,
    limit: usize,
}

impl Encoder {
    const fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn reserve(&mut self, count: usize) -> Result<(), Diagnostic> {
        let required = self
            .bytes
            .len()
            .checked_add(count)
            .ok_or_else(|| fingerprint_error("canonical projection length overflows usize"))?;
        if required > self.limit {
            return Err(fingerprint_error(format!(
                "canonical semantic projection exceeds the {} byte limit",
                self.limit
            )));
        }
        self.bytes
            .try_reserve_exact(count)
            .map_err(|_| fingerprint_error("cannot reserve canonical semantic projection bytes"))
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), Diagnostic> {
        self.reserve(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), Diagnostic> {
        self.len(value.len())?;
        self.raw(value)
    }

    fn bool(&mut self, value: bool) -> Result<(), Diagnostic> {
        self.u8(u8::from(value))
    }

    fn u8(&mut self, value: u8) -> Result<(), Diagnostic> {
        self.raw(&[value])
    }

    fn i8(&mut self, value: i8) -> Result<(), Diagnostic> {
        self.u8(value as u8)
    }

    fn u16(&mut self, value: u16) -> Result<(), Diagnostic> {
        self.raw(&value.to_be_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), Diagnostic> {
        self.raw(&value.to_be_bytes())
    }

    fn i32(&mut self, value: i32) -> Result<(), Diagnostic> {
        self.raw(&value.to_be_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), Diagnostic> {
        self.raw(&value.to_be_bytes())
    }

    fn usize(&mut self, value: usize) -> Result<(), Diagnostic> {
        let value = u64::try_from(value)
            .map_err(|_| fingerprint_error("canonical usize value exceeds u64"))?;
        self.u64(value)
    }

    fn len(&mut self, value: usize) -> Result<(), Diagnostic> {
        let value = u32::try_from(value)
            .map_err(|_| fingerprint_error("canonical collection length exceeds u32"))?;
        self.u32(value)
    }

    fn finish(self) -> Result<Vec<u8>, Diagnostic> {
        if self.bytes.len() > self.limit {
            Err(fingerprint_error(
                "canonical semantic projection exceeds its byte limit",
            ))
        } else {
            Ok(self.bytes)
        }
    }
}

fn encode_optional_quantity(
    encoder: &mut Encoder,
    value: Option<DynQuantity>,
) -> Result<(), Diagnostic> {
    match value {
        Some(value) => {
            encoder.u8(1)?;
            encode_quantity(encoder, value)
        }
        None => encoder.u8(0),
    }
}

fn encode_quantity(encoder: &mut Encoder, value: DynQuantity) -> Result<(), Diagnostic> {
    if !value.value().is_finite() {
        return Err(fingerprint_error(
            "structural semantic projection requires finite quantities",
        ));
    }
    let scalar = if value.value() == 0.0 {
        0.0
    } else {
        value.value()
    };
    encoder.u64(scalar.to_bits())?;
    encode_dimension(encoder, value.dim())
}

fn encode_dimension(encoder: &mut Encoder, value: DimExponents) -> Result<(), Diagnostic> {
    for exponent in [
        value.mass,
        value.length,
        value.time,
        value.current,
        value.temperature,
        value.amount,
        value.luminous_intensity,
    ] {
        encoder.i8(exponent)?;
    }
    Ok(())
}

fn encode_shape(encoder: &mut Encoder, shape: &ValueShape) -> Result<(), Diagnostic> {
    encoder.len(shape.extents().len())?;
    for extent in shape.extents() {
        encoder.u32(extent.get())?;
    }
    Ok(())
}

fn encode_frame(encoder: &mut Encoder, frame: ValueFrame) -> Result<(), Diagnostic> {
    match frame {
        ValueFrame::Invariant => encoder.u8(1),
        ValueFrame::SpatialCartesian => encoder.u8(2),
    }
}

fn encode_boundary_side(encoder: &mut Encoder, side: BoundarySide) -> Result<(), Diagnostic> {
    match side {
        BoundarySide::Lower => encoder.u8(1),
        BoundarySide::Upper => encoder.u8(2),
    }
}

fn encode_signal_direction(
    encoder: &mut Encoder,
    direction: SignalDirection,
) -> Result<(), Diagnostic> {
    match direction {
        SignalDirection::Input => encoder.u8(1),
        SignalDirection::Output => encoder.u8(2),
    }
}

fn encode_event_direction(
    encoder: &mut Encoder,
    direction: EventDirection,
) -> Result<(), Diagnostic> {
    match direction {
        EventDirection::Any => encoder.u8(1),
        EventDirection::Rising => encoder.u8(2),
        EventDirection::Falling => encoder.u8(3),
    }
}

fn validate_limits(limits: SemanticFingerprintLimits) -> Result<(), Diagnostic> {
    if limits.max_nodes == 0
        || limits.max_references == 0
        || limits.max_expression_nodes == 0
        || limits.max_canonical_bytes < PROJECTION_MAGIC.len() + 16
        || limits.max_search_states == 0
        || limits.max_individualization_depth == 0
        || limits.max_refinement_work == 0
        || limits.max_serialization_work == 0
    {
        return Err(fingerprint_error(
            "structural semantic fingerprint limits must all admit non-empty bounded work",
        ));
    }
    Ok(())
}

fn newer_vocabulary(subject: &str) -> Diagnostic {
    fingerprint_error(format!(
        "{subject} is newer than structural semantic fingerprint generation v1"
    ))
}

fn fingerprint_error(message: impl Into<String>) -> Diagnostic {
    invalid_artifact(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_compiler::compile;
    use eqiora_core::entity::kinds;
    use eqiora_core::{Id, OntologyId};
    use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
    use eqiora_schema::kernel::{
        ActivationDef, ExprDagBuilder, FieldDef, PortDef, RelationDef, SignalDirection, SymbolRef,
    };
    use eqiora_schema::{Model, ModelView};

    const DECAY: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous { derivative(x) + rate * x = 0; }
}
"#;

    fn program(source: &str) -> KernelProgram {
        let compiled = compile("fingerprint.eqi", source)
            .expect("valid source")
            .remove(0);
        let (transaction, model, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).expect("valid graph transaction");
        KernelProgram::from_snapshot(&store.snapshot(), model).expect("valid kernel program")
    }

    #[test]
    fn independent_occurrence_ids_and_formatting_do_not_change_the_projection() {
        let first = program(DECAY);
        let second = program(
            "model renamed { parameter r: 1/s = 1; field state: 1=1;\nrelation balance continuous { derivative(state)+r*state=0; } }",
        );

        assert_ne!(first.model(), second.model());
        assert_eq!(
            StructuralSemanticFingerprint::from_program(&first).unwrap(),
            StructuralSemanticFingerprint::from_program(&second).unwrap()
        );
        assert!(structurally_equivalent(&first, &second).unwrap());
    }

    #[test]
    fn symmetric_graphs_choose_the_same_exact_label_across_fresh_ids_and_order() {
        let first = program(
            "model first { parameter a: 1 = 1; parameter b: 1 = 1; relation r continuous { 0 = 0; } }",
        );
        let second = program(
            "model second { relation balance continuous { 0 = 0; } parameter y: 1 = 1; parameter x: 1 = 1; }",
        );
        assert!(structurally_equivalent(&first, &second).unwrap());
        assert_eq!(
            StructuralSemanticFingerprint::from_program(&first).unwrap(),
            StructuralSemanticFingerprint::from_program(&second).unwrap()
        );
    }

    #[test]
    fn expression_arena_allocation_is_alpha_normalized_too() {
        let first = manually_allocated_expression(false, false);
        let reversed = manually_allocated_expression(true, false);
        assert!(structurally_equivalent(&first, &reversed).unwrap());
    }

    #[test]
    fn model_boundary_membership_is_not_alpha_normalized_away() {
        let internal = manually_allocated_expression(false, false);
        let exposed = manually_allocated_expression(false, true);
        assert!(!structurally_equivalent(&internal, &exposed).unwrap());
    }

    #[test]
    fn value_operator_and_rewiring_changes_are_not_alpha_normalized_away() {
        let baseline = program(DECAY);
        let changed_value = program(&DECAY.replace("= 1;\n  relation", "= 2;\n  relation"));
        let changed_operator = program(&DECAY.replace("rate * x", "rate / x"));
        assert!(!structurally_equivalent(&baseline, &changed_value).unwrap());
        assert!(!structurally_equivalent(&baseline, &changed_operator).unwrap());

        let separate = program(
            "model p { parameter a: 1 = 2; parameter b: 1 = 2; relation r continuous { a-b=0; } }",
        );
        let aliased = program(
            "model p { parameter a: 1 = 2; parameter b: 1 = 2; relation r continuous { a-a=0; } }",
        );
        assert!(!structurally_equivalent(&separate, &aliased).unwrap());
    }

    #[test]
    fn nominally_distinct_equal_domains_remain_distinct_vertices() {
        let distinct = program(
            r#"
model network {
  domain a = scalar_physical(across = 1, through = 1);
  domain b = scalar_physical(across = 1, through = 1);
  port a1: conserving on a;
  port a2: conserving on a;
  port b1: conserving on b;
  port b2: conserving on b;
  relation ra continuous { across(a1) - across(a2) = 0; through(a1) + through(a2) = 0; }
  relation rb continuous { across(b1) - across(b2) = 0; through(b1) + through(b2) = 0; }
  connect conserving a1, a2;
  connect conserving b1, b2;
}
"#,
        );
        let shared = program(
            r#"
model network {
  domain a = scalar_physical(across = 1, through = 1);
  domain b = scalar_physical(across = 1, through = 1);
  port a1: conserving on a;
  port a2: conserving on a;
  port b1: conserving on a;
  port b2: conserving on a;
  relation ra continuous { across(a1) - across(a2) = 0; through(a1) + through(a2) = 0; }
  relation rb continuous { across(b1) - across(b2) = 0; through(b1) + through(b2) = 0; }
  connect conserving a1, a2;
  connect conserving b1, b2;
}
"#,
        );
        assert!(!structurally_equivalent(&distinct, &shared).unwrap());
    }

    #[test]
    fn exact_canonicalization_fails_instead_of_using_occurrence_order() {
        let value = program(DECAY);
        let limits = SemanticFingerprintLimits {
            max_search_states: 1,
            ..SemanticFingerprintLimits::default()
        };
        let result = StructuralSemanticFingerprint::from_program_with_limits(&value, limits);
        assert!(
            result.is_ok(),
            "ordinary asymmetric models need no search branch"
        );

        let symmetric = program(
            "model symmetric { parameter a: 1 = 1; parameter b: 1 = 1; relation r continuous { 0 = 0; } }",
        );
        let error = StructuralSemanticFingerprint::from_program_with_limits(&symmetric, limits)
            .expect_err("ambiguous exact labeling must respect the state limit");
        assert!(error.message().contains("search-state limit"));
    }

    fn manually_allocated_expression(reverse: bool, expose_port: bool) -> KernelProgram {
        let left = Id::<kinds::Field>::new();
        let right = Id::<kinds::Field>::new();
        let relation = Id::<kinds::Relation>::new();
        let activation = Id::<kinds::Activation>::new();
        let port = Id::<kinds::Port>::new();
        let model = OntologyId::<Model>::new();
        let mut expression = ExprDagBuilder::new();
        let (left_value, right_value) = if reverse {
            let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
            let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
            (left_value, right_value)
        } else {
            let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
            let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
            (left_value, right_value)
        };
        let root = expression.add(left_value, right_value).unwrap();
        let expression = expression.finish([root]).unwrap();
        let members = [
            left.erase(),
            right.erase(),
            relation.erase(),
            activation.erase(),
            port.erase(),
        ];
        let boundary = expose_port.then_some(port.erase());
        let view = ModelView::new(model, members, boundary).unwrap();
        let mut transaction = Transaction::new("manual expression allocation");
        transaction
            .push(Op::DefineKernelNode {
                node: FieldDef::new(left, DimExponents::DIMENSIONLESS)
                    .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                    .unwrap()
                    .into(),
            })
            .push(Op::DefineKernelNode {
                node: FieldDef::new(right, DimExponents::DIMENSIONLESS)
                    .with_initial(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
                    .unwrap()
                    .into(),
            })
            .push(Op::DefineKernelNode {
                node: RelationDef::new(relation, expression).into(),
            })
            .push(Op::DefineKernelNode {
                node: ActivationDef::continuous(activation).into(),
            })
            .push(Op::DefineKernelNode {
                node: PortDef::signal(port, SignalDirection::Input, DimExponents::DIMENSIONLESS)
                    .into(),
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: left.erase(),
                edge: EdgeKind::DependsOn,
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: right.erase(),
                edge: EdgeKind::DependsOn,
            })
            .push(Op::Connect {
                from: activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            })
            .push(Op::DefineOntologyView { view: view.into() });
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
    }
}
