//! One exact standalone prescribed dynamic-solid Realization artifact.

use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id, OntologyId, RawId, ValueShape};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{MeshEntity, MeshTopology, VertexId};
use eqiora_realization::{RealizationRevision, SemanticRevision};
use eqiora_schema::Model;
use eqiora_schema::kernel::{
    BoundarySide, CartesianCoordinateSource, ConnectionSemantics, DomainKind, ExprDag, ExprNode,
    KernelNode, RepresentationKind, SymbolRef, ValueFrame,
};
use eqiora_sem::KernelProgram;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, RealizationDecoderLimits,
    ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1, check_json_limits,
    invalid_artifact,
};

const SCHEMA: &str = "eqiora.prescribed-dynamic-solid-realization-envelope/v1";
const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
#[rustfmt::skip]
const REFERENCE_VERTICES: [[f64; 3]; 9] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [0.5, 0.5, 0.5]];
#[rustfmt::skip]
const REFERENCE_CELLS: [[usize; 4]; 12] = [[8, 0, 6, 2], [8, 0, 4, 6], [8, 1, 7, 5], [8, 1, 3, 7], [8, 0, 5, 4], [8, 0, 1, 5], [8, 2, 7, 3], [8, 2, 6, 7], [8, 0, 3, 1], [8, 0, 2, 3], [8, 4, 7, 6], [8, 4, 5, 7]];
const DRIVEN_VERTICES: [usize; 4] = [1, 3, 5, 7];
const DRIVEN_VALUE: [f64; 3] = [0.015, 0.0, 0.0];

/// Canonical standalone Realization of the exact prescribed dynamic-solid occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct PrescribedDynamicSolidRealizationEnvelopeV1 {
    wire: WireEnvelope,
    driven_total_displacement: Vec<(VertexId, [f64; 3])>,
}

impl PrescribedDynamicSolidRealizationEnvelopeV1 {
    /// Construct and resource-validate the exact standalone-solid Realization.
    ///
    /// # Errors
    /// Returns `EQ0901` for semantic-role, resource, mesh, policy, or candidate drift.
    pub fn new(
        model: &impl ReplayableCanonicalModelArtifact,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        realization_revision: RealizationRevision,
        driven_total_displacement: &[(VertexId, [f64; 3])],
    ) -> Result<Self, Diagnostic> {
        let replay = model.replay_model()?;
        let roles = derive_roles(replay.program())?;
        let reference = replay.artifact_reference();
        let value = Self {
            wire: WireEnvelope {
                schema: SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                model_sha256: reference.artifact().to_string(),
                model_ulid: reference.model().ulid().to_string(),
                semantic_revision: reference.semantic_revision().get(),
                source: WireSource {
                    kind: WireSourceKind::Explicit,
                    realization_revision: realization_revision.get(),
                },
                geometry_sha256: geometry.digest()?.to_string(),
                correspondence_sha256: correspondence.digest()?.to_string(),
                spatial: WireSpatial {
                    spatial_dimension: 3,
                    scalar: WireScalar::F64,
                    vector_layout: WireVectorLayout::Replicated,
                    solid_domain_ulid: roles.solid_domain.ulid().to_string(),
                    displacement_field_ulid: roles.displacement.ulid().to_string(),
                    velocity_field_ulid: roles.velocity.ulid().to_string(),
                    fixed_boundary_ulid: roles.fixed_boundary.ulid().to_string(),
                    driven_boundary_ulid: roles.driven_boundary.ulid().to_string(),
                    space: WireSpace {
                        kind: WireSpaceKind::ContinuousLagrange,
                        order: 1,
                    },
                    discretization: WireDiscretization {
                        method: WireDiscretizationMethod::ContinuousGalerkin,
                        mesh: WireMesh {
                            kind: WireMeshKind::ImportedSimplicial,
                            artifact_sha256: mesh.digest()?.to_string(),
                        },
                        quadrature: WireQuadrature::ExactAffineP1TetrahedronMassAndStiffness,
                    },
                },
                time: WireTime {
                    method: WireTimeMethod::BackwardEuler,
                    duration_s: 0.25,
                },
                driven_total_displacement: encode_driven(driven_total_displacement)?,
                solver: WireSolver {
                    operator_properties: WireOperatorProperties::SymmetricPositiveDefinite,
                    algorithm: WireAlgorithm::ConjugateGradient,
                    preconditioner: WirePreconditioner::Identity,
                    reduction: WireReduction::Reproducible,
                    relative_tolerance: 1.0e-13,
                    absolute_tolerance: 1.0e-15,
                    maximum_iterations: 500,
                },
                placement: WirePlacement {
                    target: WireTarget {
                        kind: WireTargetKind::HostCpu,
                        threads: 1,
                    },
                    schedule: WireSchedule {
                        kind: WireScheduleKind::Offline,
                    },
                    assembly_execution: WireExecution::HostSerial,
                    solve_execution: WireExecution::HostSerial,
                    verification_execution: WireExecution::HostSerial,
                    layout_artifacts: WireLayoutArtifacts {
                        kind: WireLayoutKind::Replicated,
                    },
                },
            },
            driven_total_displacement: driven_total_displacement.to_vec(),
        };
        value.validate_local(RealizationDecoderLimits::default())?;
        value.validate_against(model, geometry, correspondence, mesh)?;
        Ok(value)
    }

    /// Decode locally canonical bytes without resolving referenced resources.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, noncanonical, unsupported, or over-budget bytes.
    pub fn from_json(bytes: &[u8], limits: RealizationDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits.json)?;
        let wire: WireEnvelope = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!(
                "invalid prescribed dynamic-solid Realization JSON: {error}"
            ))
        })?;
        let driven_total_displacement = decode_driven(&wire.driven_total_displacement)?;
        let value = Self {
            wire,
            driven_total_displacement,
        };
        value.validate_local(limits)?;
        if value.canonical_json()?.as_slice() != bytes {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization JSON is not the canonical encoding",
            ));
        }
        Ok(value)
    }

    /// Deterministic compact canonical JSON.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize prescribed dynamic-solid Realization: {error}"
            ))
        })
    }

    /// Domain-separated identity of the complete Realization.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Exact current Model artifact.
    #[must_use]
    pub fn model_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.model_sha256)
    }

    /// Typed current Model identity.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state was corrupted.
    pub fn model(&self) -> Result<OntologyId<Model>, Diagnostic> {
        parse_ulid(&self.wire.model_ulid, "Model").map(OntologyId::from_ulid)
    }

    /// Semantic revision selected by the Realization.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        SemanticRevision::new(self.wire.semantic_revision)
    }

    /// Explicit Realization revision.
    #[must_use]
    pub const fn realization_revision(&self) -> RealizationRevision {
        RealizationRevision::new(self.wire.source.realization_revision)
    }

    /// Exact Geometry identity artifact.
    #[must_use]
    pub fn geometry_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.geometry_sha256)
    }

    /// Exact Geometry-to-Mesh correspondence artifact.
    #[must_use]
    pub fn correspondence_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.correspondence_sha256)
    }

    /// Exact immutable imported mesh artifact.
    #[must_use]
    pub fn mesh_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.spatial.discretization.mesh.artifact_sha256)
    }

    /// Exact solid body Domain.
    #[must_use]
    pub fn solid_domain(&self) -> Id<kinds::Domain> {
        admitted_id(&self.wire.spatial.solid_domain_ulid, "solid Domain")
    }

    /// Exact displacement Field.
    #[must_use]
    pub fn displacement_field(&self) -> Id<kinds::Field> {
        admitted_id(
            &self.wire.spatial.displacement_field_ulid,
            "displacement Field",
        )
    }

    /// Exact velocity Field.
    #[must_use]
    pub fn velocity_field(&self) -> Id<kinds::Field> {
        admitted_id(&self.wire.spatial.velocity_field_ulid, "velocity Field")
    }

    /// Exact fixed x-lower boundary Domain.
    #[must_use]
    pub fn fixed_boundary(&self) -> Id<kinds::Domain> {
        admitted_id(&self.wire.spatial.fixed_boundary_ulid, "fixed boundary")
    }

    /// Exact driven x-upper boundary Domain.
    #[must_use]
    pub fn driven_boundary(&self) -> Id<kinds::Domain> {
        admitted_id(&self.wire.spatial.driven_boundary_ulid, "driven boundary")
    }

    /// Canonically ordered driven total displacement.
    #[must_use]
    pub fn driven_total_displacement(&self) -> &[(VertexId, [f64; 3])] {
        &self.driven_total_displacement
    }

    /// Replay semantic roles and every referenced external resource.
    ///
    /// This proves durable role and resource meaning only. It does not prove
    /// that an execution or accepted candidate exists.
    ///
    /// # Errors
    /// Returns `EQ0901` for Model, role, Geometry, mesh, correspondence, or policy drift.
    pub fn validate_against(
        &self,
        model: &impl ReplayableCanonicalModelArtifact,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        self.validate_local(RealizationDecoderLimits::default())?;
        let replay = model.replay_model()?;
        let reference = replay.artifact_reference();
        let roles = derive_roles(replay.program())?;
        if self.model_artifact() != *reference.artifact()
            || self.model()? != reference.model()
            || self.semantic_revision() != reference.semantic_revision()
            || self.solid_domain() != roles.solid_domain
            || self.displacement_field() != roles.displacement
            || self.velocity_field() != roles.velocity
            || self.fixed_boundary() != roles.fixed_boundary
            || self.driven_boundary() != roles.driven_boundary
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization Model identity, revision, or derived role binding differs",
            ));
        }

        geometry.validate_against(model)?;
        correspondence.validate_against(geometry, model, mesh)?;
        if self.geometry_artifact() != geometry.digest()?
            || self.correspondence_artifact() != correspondence.digest()?
            || self.mesh_artifact() != mesh.digest()?
            || geometry.tolerance_m().to_bits() != 1.0e-12_f64.to_bits()
            || mesh.mesh().quality_gate().minimum_mean_ratio().to_bits() != 0.1_f64.to_bits()
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization resource identity, Geometry tolerance, or mesh gate differs",
            ));
        }
        require_reference_mesh(mesh)?;
        require_geometry_and_correspondence(&roles, geometry, correspondence, mesh)?;
        let driven = boundary_vertices(mesh, correspondence, roles.driven_boundary)?;
        if driven != DRIVEN_VERTICES.map(VertexId::new)
            || self.driven_total_displacement
                != driven
                    .iter()
                    .copied()
                    .map(|vertex| (vertex, DRIVEN_VALUE))
                    .collect::<Vec<_>>()
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid driven values differ from the exact reconstructed boundary inventory",
            ));
        }
        Ok(())
    }

    fn validate_local(&self, limits: RealizationDecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != SCHEMA
            || self.wire.encoding != CANONICAL_ENCODING
            || limits.max_realization_fields < 2
        {
            return Err(invalid_artifact(
                "unsupported prescribed dynamic-solid schema, encoding, or Field decoder budget",
            ));
        }
        for digest in [
            &self.wire.model_sha256,
            &self.wire.geometry_sha256,
            &self.wire.correspondence_sha256,
            &self.wire.spatial.discretization.mesh.artifact_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        parse_ulid(&self.wire.model_ulid, "Model")?;
        for (value, label) in [
            (&self.wire.spatial.solid_domain_ulid, "solid Domain"),
            (
                &self.wire.spatial.displacement_field_ulid,
                "displacement Field",
            ),
            (&self.wire.spatial.velocity_field_ulid, "velocity Field"),
            (&self.wire.spatial.fixed_boundary_ulid, "fixed boundary"),
            (&self.wire.spatial.driven_boundary_ulid, "driven boundary"),
        ] {
            parse_ulid(value, label)?;
        }
        if self.wire.source.kind != WireSourceKind::Explicit
            || self.wire.source.realization_revision != 1
            || self.wire.spatial.spatial_dimension != 3
            || self.wire.spatial.scalar != WireScalar::F64
            || self.wire.spatial.vector_layout != WireVectorLayout::Replicated
            || self.wire.spatial.space.kind != WireSpaceKind::ContinuousLagrange
            || self.wire.spatial.space.order != 1
            || self.wire.spatial.discretization.method
                != WireDiscretizationMethod::ContinuousGalerkin
            || self.wire.spatial.discretization.mesh.kind != WireMeshKind::ImportedSimplicial
            || self.wire.spatial.discretization.quadrature
                != WireQuadrature::ExactAffineP1TetrahedronMassAndStiffness
            || self.wire.time.method != WireTimeMethod::BackwardEuler
            || self.wire.time.duration_s.to_bits() != 0.25_f64.to_bits()
            || !self.wire.solver.is_exact()
            || !self.wire.placement.is_exact()
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization contains unsupported fixed policy",
            ));
        }
        let decoded = decode_driven(&self.wire.driven_total_displacement)?;
        if decoded != self.driven_total_displacement
            || decoded
                != DRIVEN_VERTICES
                    .into_iter()
                    .map(|vertex| (VertexId::new(vertex), DRIVEN_VALUE))
                    .collect::<Vec<_>>()
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid driven displacement differs from the exact canonical inventory",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PrescribedDynamicSolidRoles {
    pub(crate) solid_domain: Id<kinds::Domain>,
    pub(crate) displacement: Id<kinds::Field>,
    pub(crate) velocity: Id<kinds::Field>,
    pub(crate) fixed_boundary: Id<kinds::Domain>,
    pub(crate) driven_boundary: Id<kinds::Domain>,
}

fn derive_roles(program: &KernelProgram) -> Result<PrescribedDynamicSolidRoles, Diagnostic> {
    let body = unique_node(
        program.nodes().filter_map(|node| match node {
            KernelNode::Domain(domain) if is_reference_box(domain.kind()) => Some(domain.id()),
            _ => None,
        }),
        "exact unit-cube solid Domain",
    )?;
    let vector_shape = ValueShape::new([3]).map_err(|error| invalid_artifact(error.to_string()))?;
    let displacement = unique_field(program, body, LENGTH, &vector_shape)?;
    let velocity = unique_field(program, body, VELOCITY, &vector_shape)?;
    if displacement == velocity {
        return Err(invalid_artifact(
            "prescribed dynamic-solid displacement and velocity Fields are not distinct",
        ));
    }
    let mut boundaries = Vec::new();
    for axis in 0..3 {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            boundaries.push((axis, side, cartesian_boundary(program, body, axis, side)?));
        }
    }
    let fixed_boundary = boundaries[0].2;
    let driven_boundary = boundaries[1].2;
    require_terminal(program, fixed_boundary, TerminalKind::TraceZero)?;
    require_terminal(program, driven_boundary, TerminalKind::Live)?;
    for (_, _, boundary) in &boundaries[2..] {
        require_terminal(program, *boundary, TerminalKind::FluxZero)?;
    }
    Ok(PrescribedDynamicSolidRoles {
        solid_domain: body,
        displacement,
        velocity,
        fixed_boundary,
        driven_boundary,
    })
}

fn is_reference_box(kind: &DomainKind) -> bool {
    let DomainKind::CartesianBox { coordinates } = kind else {
        return false;
    };
    coordinates.len() == 3
        && coordinates.iter().all(|axis| {
            matches!(
                (axis.lower(), axis.upper()),
                (CartesianCoordinateSource::Fixed(lower), CartesianCoordinateSource::Fixed(upper))
                    if lower.dim() == LENGTH
                        && upper.dim() == LENGTH
                        && lower.value().to_bits() == 0.0_f64.to_bits()
                        && upper.value().to_bits() == 1.0_f64.to_bits()
            )
        })
}

fn unique_field(
    program: &KernelProgram,
    body: Id<kinds::Domain>,
    dimension: DimExponents,
    shape: &ValueShape,
) -> Result<Id<kinds::Field>, Diagnostic> {
    unique_node(
        program.nodes().filter_map(|node| match node {
            KernelNode::Field(field)
                if field.dimension() == dimension
                    && field.shape() == shape
                    && field.frame() == ValueFrame::SpatialCartesian
                    && has_edge(
                        program,
                        field.id().erase(),
                        body.erase(),
                        EdgeKind::DefinedOn,
                    )
                    && continuum_representation(program, field.id().erase()) =>
            {
                Some(field.id())
            }
            _ => None,
        }),
        "exact prescribed dynamic-solid Field",
    )
}

fn continuum_representation(program: &KernelProgram, field: RawId) -> bool {
    let values = program
        .edges()
        .iter()
        .filter(|edge| edge.from() == field && edge.kind() == EdgeKind::DefinedOn)
        .filter(|edge| {
            matches!(
                program.node(edge.to()),
                Some(KernelNode::Representation(value))
                    if value.kind() == RepresentationKind::Continuum
            )
        })
        .count();
    values == 1
}

fn cartesian_boundary(
    program: &KernelProgram,
    body: Id<kinds::Domain>,
    axis: usize,
    side: BoundarySide,
) -> Result<Id<kinds::Domain>, Diagnostic> {
    unique_node(
        program.nodes().filter_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(
                    domain.kind(),
                    DomainKind::CartesianBoundary {
                        axis: candidate_axis,
                        side: candidate_side,
                    } if *candidate_axis == axis && *candidate_side == side
                ) && has_edge(
                    program,
                    domain.id().erase(),
                    body.erase(),
                    EdgeKind::BoundaryOf,
                ) =>
            {
                Some(domain.id())
            }
            _ => None,
        }),
        "exact Cartesian boundary Domain",
    )
}

#[derive(Clone, Copy)]
enum TerminalKind {
    TraceZero,
    FluxZero,
    Live,
}

fn require_terminal(
    program: &KernelProgram,
    boundary: Id<kinds::Domain>,
    kind: TerminalKind,
) -> Result<(), Diagnostic> {
    let relations = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::AppliesOn && edge.to() == boundary.erase())
        .filter_map(|edge| edge.from().downcast::<kinds::Relation>())
        .collect::<Vec<_>>();
    if relations.len() != 2 {
        return Err(invalid_artifact(
            "prescribed dynamic-solid boundary does not have one interface and one terminal Relation",
        ));
    }
    let matching = relations
        .into_iter()
        .filter(|relation| terminal_matches(program, *relation, boundary, kind))
        .count();
    if matching != 1 {
        return Err(invalid_artifact(
            "prescribed dynamic-solid boundary terminal semantics differ from the exact trace/flux role",
        ));
    }
    Ok(())
}

fn terminal_matches(
    program: &KernelProgram,
    relation: Id<kinds::Relation>,
    boundary: Id<kinds::Domain>,
    kind: TerminalKind,
) -> bool {
    let Some(KernelNode::Relation(definition)) = program.node(relation.erase()) else {
        return false;
    };
    let residuals = definition.residuals();
    let port = match kind {
        TerminalKind::TraceZero => single_port_symbol(residuals, true),
        TerminalKind::FluxZero => single_port_symbol(residuals, false),
        TerminalKind::Live => live_self_cancellation_port(residuals),
    };
    let Some(port) = port else {
        return false;
    };
    if !has_edge(program, relation.erase(), port.erase(), EdgeKind::HasPort)
        || !has_edge(program, relation.erase(), port.erase(), EdgeKind::DependsOn)
        || !matches!(
            program.node(port.erase()),
            Some(KernelNode::Port(definition))
                if definition
                    .boundary_physical_contract()
                    .is_some_and(|(_, support)| support == boundary)
        )
    {
        return false;
    }
    let connections = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Connects && edge.to() == port.erase())
        .filter_map(|edge| match program.node(edge.from()) {
            Some(KernelNode::Connection(connection))
                if connection.semantics() == ConnectionSemantics::Conserving =>
            {
                Some(edge.from())
            }
            _ => None,
        })
        .count();
    connections == 1
}

fn single_port_symbol(residuals: &ExprDag, trace: bool) -> Option<Id<kinds::Port>> {
    if residuals.roots().len() != 1
        || residuals.roots()[0].index() != 0
        || residuals.nodes().len() != 1
    {
        return None;
    }
    match residuals.nodes()[0] {
        ExprNode::Symbol(SymbolRef::PortTrace(port)) if trace => Some(port),
        ExprNode::Symbol(SymbolRef::PortFlux(port)) if !trace => Some(port),
        _ => None,
    }
}

fn live_self_cancellation_port(residuals: &ExprDag) -> Option<Id<kinds::Port>> {
    let nodes = residuals.nodes();
    if residuals.roots().len() != 2
        || residuals.roots()[0].index() != 2
        || residuals.roots()[1].index() != 5
        || nodes.len() != 6
    {
        return None;
    }
    let ExprNode::Symbol(SymbolRef::PortTrace(port)) = nodes[0] else {
        return None;
    };
    if nodes[1] != nodes[0]
        || !matches!(nodes[2], ExprNode::Sub(left, right) if left.index() == 0 && right.index() == 1)
        || !matches!(nodes[3], ExprNode::Symbol(SymbolRef::PortFlux(candidate)) if candidate == port)
        || nodes[4] != nodes[3]
        || !matches!(nodes[5], ExprNode::Sub(left, right) if left.index() == 3 && right.index() == 4)
    {
        return None;
    }
    Some(port)
}

fn require_reference_mesh(mesh: &SimplicialMeshEnvelopeV1) -> Result<(), Diagnostic> {
    let expected_vertices = REFERENCE_VERTICES
        .iter()
        .map(|value| value.to_vec())
        .collect::<Vec<_>>();
    let expected_cells = REFERENCE_CELLS
        .iter()
        .map(|value| value.to_vec())
        .collect::<Vec<_>>();
    if mesh.dimension() != 3
        || mesh.mesh().vertices() != expected_vertices
        || mesh.mesh().cells() != expected_cells
    {
        return Err(invalid_artifact(
            "prescribed dynamic-solid Realization requires the exact ordered unit-cube mesh",
        ));
    }
    Ok(())
}

fn require_geometry_and_correspondence(
    roles: &PrescribedDynamicSolidRoles,
    geometry: &GeometryIdentityEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
) -> Result<(), Diagnostic> {
    if geometry.bodies().len() != 1
        || geometry.bodies()[0].domain() != roles.solid_domain
        || geometry.bodies()[0].bounds_m() != [(0.0, 1.0); 3]
        || correspondence.body_cells(roles.solid_domain)
            != Some((0..REFERENCE_CELLS.len()).collect())
    {
        return Err(invalid_artifact(
            "prescribed dynamic-solid Geometry or body-cell correspondence differs",
        ));
    }
    for (axis, side, role) in [
        (0, BoundarySide::Lower, Some(roles.fixed_boundary)),
        (0, BoundarySide::Upper, Some(roles.driven_boundary)),
        (1, BoundarySide::Lower, None),
        (1, BoundarySide::Upper, None),
        (2, BoundarySide::Lower, None),
        (2, BoundarySide::Upper, None),
    ] {
        let candidates = geometry
            .boundaries()
            .iter()
            .filter(|entry| entry.axis() == axis && entry.side() == side)
            .collect::<Vec<_>>();
        if candidates.len() != 1 || role.is_some_and(|role| candidates[0].domain() != role) {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Geometry boundary inventory differs",
            ));
        }
        if correspondence
            .boundary_facets(candidates[0].domain())
            .is_none_or(|facets| facets.is_empty())
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid boundary has no exact mesh facets",
            ));
        }
    }
    let cells = correspondence
        .body_cells(roles.solid_domain)
        .ok_or_else(|| invalid_artifact("prescribed dynamic-solid body has no mesh cells"))?;
    if cells.len() != mesh.mesh().cells().len() {
        return Err(invalid_artifact(
            "prescribed dynamic-solid body support is not the complete mesh",
        ));
    }
    Ok(())
}

fn boundary_vertices(
    mesh: &SimplicialMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    boundary: Id<kinds::Domain>,
) -> Result<Vec<VertexId>, Diagnostic> {
    let facets = correspondence
        .boundary_facets(boundary)
        .filter(|facets| !facets.is_empty())
        .ok_or_else(|| {
            invalid_artifact("prescribed dynamic-solid driven boundary has no facets")
        })?;
    let mut vertices = Vec::new();
    for facet in facets {
        let values = mesh
            .mesh()
            .entity_vertices(MeshEntity::new(2, facet))
            .ok_or_else(|| invalid_artifact("driven correspondence names a foreign facet"))?;
        vertices.extend(values.into_iter().map(|value| VertexId::new(value.index())));
    }
    vertices.sort_by_key(|value| value.index());
    vertices.dedup();
    Ok(vertices)
}

fn encode_driven(values: &[(VertexId, [f64; 3])]) -> Result<Vec<WireDriven>, Diagnostic> {
    values
        .iter()
        .map(|(vertex, value)| {
            Ok(WireDriven {
                vertex_index: u64::try_from(vertex.index()).map_err(|_| {
                    invalid_artifact("driven vertex index exceeds portable wire u64")
                })?,
                value_m: *value,
            })
        })
        .collect()
}

fn decode_driven(values: &[WireDriven]) -> Result<Vec<(VertexId, [f64; 3])>, Diagnostic> {
    values
        .iter()
        .map(|value| {
            let index = usize::try_from(value.vertex_index).map_err(|_| {
                invalid_artifact("driven vertex index exceeds the local usize range")
            })?;
            if value
                .value_m
                .iter()
                .any(|entry| !entry.is_finite() || (*entry == 0.0 && entry.is_sign_negative()))
            {
                return Err(invalid_artifact(
                    "driven displacement must be finite and use canonical positive zero",
                ));
            }
            Ok((VertexId::new(index), value.value_m))
        })
        .collect()
}

fn unique_node<T>(values: impl IntoIterator<Item = T>, label: &str) -> Result<T, Diagnostic> {
    let mut values = values.into_iter();
    let value = values
        .next()
        .ok_or_else(|| invalid_artifact(format!("prescribed dynamic-solid Model omits {label}")))?;
    if values.next().is_some() {
        return Err(invalid_artifact(format!(
            "prescribed dynamic-solid Model has more than one {label}"
        )));
    }
    Ok(value)
}

fn has_edge(program: &KernelProgram, from: RawId, to: RawId, kind: EdgeKind) -> bool {
    program
        .edges()
        .iter()
        .any(|edge| edge.from() == from && edge.to() == to && edge.kind() == kind)
}

fn parse_ulid(value: &str, label: &str) -> Result<Ulid, Diagnostic> {
    let parsed = Ulid::from_str(value)
        .map_err(|_| invalid_artifact(format!("{label} ULID is malformed")))?;
    if parsed.to_string() != value {
        return Err(invalid_artifact(format!(
            "{label} ULID is not in canonical spelling"
        )));
    }
    Ok(parsed)
}

fn admitted_id<E: eqiora_core::Entity>(value: &str, label: &str) -> Id<E> {
    Id::from_ulid(parse_ulid(value, label).expect("locally validated Realization ULID"))
}

fn admitted_digest(value: &str) -> ArtifactDigest {
    ArtifactDigest::from_hex(value.to_owned())
        .expect("locally validated prescribed dynamic-solid digest")
}

impl WireSolver {
    fn is_exact(&self) -> bool {
        self.operator_properties == WireOperatorProperties::SymmetricPositiveDefinite
            && self.algorithm == WireAlgorithm::ConjugateGradient
            && self.preconditioner == WirePreconditioner::Identity
            && self.reduction == WireReduction::Reproducible
            && self.relative_tolerance.to_bits() == 1.0e-13_f64.to_bits()
            && self.absolute_tolerance.to_bits() == 1.0e-15_f64.to_bits()
            && self.maximum_iterations == 500
            && usize::try_from(self.maximum_iterations).is_ok()
    }
}

impl WirePlacement {
    const fn is_exact(&self) -> bool {
        matches!(self.target.kind, WireTargetKind::HostCpu)
            && self.target.threads == 1
            && matches!(self.schedule.kind, WireScheduleKind::Offline)
            && matches!(self.assembly_execution, WireExecution::HostSerial)
            && matches!(self.solve_execution, WireExecution::HostSerial)
            && matches!(self.verification_execution, WireExecution::HostSerial)
            && matches!(self.layout_artifacts.kind, WireLayoutKind::Replicated)
    }
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEnvelope { schema: String, encoding: String, model_sha256: String, model_ulid: String, semantic_revision: u64, source: WireSource, geometry_sha256: String, correspondence_sha256: String, spatial: WireSpatial, time: WireTime, driven_total_displacement: Vec<WireDriven>, solver: WireSolver, placement: WirePlacement }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSource { kind: WireSourceKind, realization_revision: u64 }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSourceKind { Explicit }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpatial { spatial_dimension: u64, scalar: WireScalar, vector_layout: WireVectorLayout, solid_domain_ulid: String, displacement_field_ulid: String, velocity_field_ulid: String, fixed_boundary_ulid: String, driven_boundary_ulid: String, space: WireSpace, discretization: WireDiscretization }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireScalar { F64 }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireVectorLayout { Replicated }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpace { kind: WireSpaceKind, order: u64 }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSpaceKind { ContinuousLagrange }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDiscretization { method: WireDiscretizationMethod, mesh: WireMesh, quadrature: WireQuadrature }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireDiscretizationMethod { ContinuousGalerkin }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMesh { kind: WireMeshKind, artifact_sha256: String }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireMeshKind { ImportedSimplicial }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireQuadrature { ExactAffineP1TetrahedronMassAndStiffness }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTime { method: WireTimeMethod, duration_s: f64 }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireTimeMethod { BackwardEuler }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDriven { vertex_index: u64, value_m: [f64; 3] }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSolver { operator_properties: WireOperatorProperties, algorithm: WireAlgorithm, preconditioner: WirePreconditioner, reduction: WireReduction, relative_tolerance: f64, absolute_tolerance: f64, maximum_iterations: u64 }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireOperatorProperties { SymmetricPositiveDefinite }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireAlgorithm { ConjugateGradient }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WirePreconditioner { Identity }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireReduction { Reproducible }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePlacement { target: WireTarget, schedule: WireSchedule, assembly_execution: WireExecution, solve_execution: WireExecution, verification_execution: WireExecution, layout_artifacts: WireLayoutArtifacts }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTarget { kind: WireTargetKind, threads: u64 }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireTargetKind { HostCpu }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSchedule { kind: WireScheduleKind }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireScheduleKind { Offline }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireExecution { HostSerial }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLayoutArtifacts { kind: WireLayoutKind }

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireLayoutKind { Replicated }
