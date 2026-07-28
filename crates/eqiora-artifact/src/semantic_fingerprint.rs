//! Alpha-normalized structural identity of one accepted Semantic Model.
//!
//! Exact Model artifacts retain occurrence ULIDs and remain authoritative for
//! replay, provenance, and mutation.  This module instead constructs a closed,
//! versioned projection of the accepted kernel graph, canonically labels that
//! graph without consulting occurrence IDs, and hashes the resulting bytes.

mod canonical;
mod projection;

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
use canonical::{Canonicalizer, Encoder};
use projection::{ConstructionBudget, ProjectionGraph, Reference};

const FINGERPRINT_DOMAIN: &[u8] = b"eqiora.structural-semantic-fingerprint/v2\0";
const PROJECTION_MAGIC: &[u8; 8] = b"EQIORASF";
const GENERATION_V2: u16 = 2;

/// Compatibility generation of the structural semantic projection.
///
/// Generations are intentionally independent of Model artifact wire versions.
/// Equality is defined only within one explicitly equal generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum SemanticFingerprintGeneration {
    /// Closed projection of the Semantic Kernel vocabulary through Model wire v6.
    V1,
    /// Closed projection through Model wire v7, including geometry-backed Domains.
    V2,
}

impl SemanticFingerprintGeneration {
    /// Stable external spelling of this comparison generation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "eqiora.structural-semantic-fingerprint/v1",
            Self::V2 => "eqiora.structural-semantic-fingerprint/v2",
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
    /// generation or exact canonical labeling exceeds the fixed generation-v2
    /// resource policy.
    pub fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        ProjectionIdentity::from_program(program, SemanticFingerprintLimits::default())
            .map(|identity| identity.fingerprint)
    }

    /// Construct with an explicit bounded canonicalization policy.
    ///
    /// Limits affect admission only.  Every accepted construction produces
    /// exactly the same generation-v2 bytes and digest.
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
                generation: SemanticFingerprintGeneration::V2,
                digest,
            },
            canonical,
        })
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
        DomainKind::GeometryRegion {
            geometry,
            entity_set,
        } => {
            encoder.u8(6)?;
            encoder.raw(&geometry.bytes())?;
            encoder.bytes(entity_set.as_bytes())
        }
        DomainKind::GeometryBoundary { entity_set } => {
            encoder.u8(7)?;
            encoder.bytes(entity_set.as_bytes())
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
        "{subject} is newer than structural semantic fingerprint generation v2"
    ))
}

fn fingerprint_error(message: impl Into<String>) -> Diagnostic {
    invalid_artifact(message)
}

#[cfg(test)]
mod tests;
