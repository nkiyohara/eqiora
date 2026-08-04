//! Closed Realization artifact for one prescribed dynamic-solid occurrence.

use std::num::NonZeroUsize;
use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id, OntologyId, RawId};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{MeshEntity, MeshTopology, VertexId};
use eqiora_realization::{RealizationRevision, SemanticRevision};
use eqiora_schema::Model;
use eqiora_schema::kernel::{
    BoundarySide, DomainKind, ExprDag, ExprNode, KernelNode, SymbolRef, ValueFrame,
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

const REALIZATION_SCHEMA: &str = "eqiora.prescribed-dynamic-solid-realization-envelope/v1";
const REFERENCE_VERTICES: [[f64; 3]; 9] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
    [0.5, 0.5, 0.5],
];
const REFERENCE_CELLS: [[usize; 4]; 12] = [
    [8, 0, 6, 2],
    [8, 0, 4, 6],
    [8, 1, 7, 5],
    [8, 1, 3, 7],
    [8, 0, 5, 4],
    [8, 0, 1, 5],
    [8, 2, 7, 3],
    [8, 2, 6, 7],
    [8, 0, 3, 1],
    [8, 0, 2, 3],
    [8, 4, 7, 6],
    [8, 4, 5, 7],
];
const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};

/// One exact standalone prescribed-solid Realization artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct PrescribedDynamicSolidRealizationEnvelopeV1 {
    wire: WirePrescribedDynamicSolidRealizationV1,
    driven_total_displacement: Vec<(VertexId, [f64; 3])>,
}

impl PrescribedDynamicSolidRealizationEnvelopeV1 {
    /// Construct the closed Realization from its exact durable resources.
    ///
    /// Every semantic role and policy is derived or fixed by this family;
    /// callers supply only the accepted driven-boundary values.
    ///
    /// # Errors
    /// Returns `EQ0901` for foreign meaning, resources, revision, mesh, or
    /// driven values.
    #[allow(clippy::too_many_arguments)]
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
        validate_resources(
            model,
            geometry,
            correspondence,
            mesh,
            &roles,
            driven_total_displacement,
        )?;
        if realization_revision != RealizationRevision::new(1) {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization requires explicit revision 1",
            ));
        }
        let model_reference = replay.artifact_reference();
        let wire = WirePrescribedDynamicSolidRealizationV1 {
            schema: REALIZATION_SCHEMA.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            model_sha256: model_reference.artifact().to_string(),
            model_ulid: model_reference.model().ulid().to_string(),
            semantic_revision: model_reference.semantic_revision().get(),
            source: WireSource {
                kind: "explicit".to_owned(),
                realization_revision: 1,
            },
            geometry_sha256: geometry.digest()?.to_string(),
            correspondence_sha256: correspondence.digest()?.to_string(),
            spatial: WireSpatial {
                spatial_dimension: 3,
                scalar: "f64".to_owned(),
                vector_layout: "replicated".to_owned(),
                solid_domain_ulid: roles.solid_domain.ulid().to_string(),
                displacement_field_ulid: roles.displacement_field.ulid().to_string(),
                velocity_field_ulid: roles.velocity_field.ulid().to_string(),
                fixed_boundary_ulid: roles.fixed_boundary.ulid().to_string(),
                driven_boundary_ulid: roles.driven_boundary.ulid().to_string(),
                space: WireSpace {
                    kind: "continuous-lagrange".to_owned(),
                    order: 1,
                },
                discretization: WireDiscretization {
                    method: "continuous-galerkin".to_owned(),
                    mesh: WireMesh {
                        kind: "imported-simplicial".to_owned(),
                        artifact_sha256: mesh.digest()?.to_string(),
                    },
                    quadrature: "exact-affine-p1-tetrahedron-mass-and-stiffness".to_owned(),
                },
            },
            time: WireTime {
                method: "backward-euler".to_owned(),
                duration_s: 0.25,
            },
            driven_total_displacement: driven_total_displacement
                .iter()
                .map(|(vertex, value)| WireDrivenValue {
                    vertex_index: u64::try_from(vertex.index())
                        .expect("validated reference vertex fits u64"),
                    value_m: *value,
                })
                .collect(),
            solver: WireSolver {
                operator_properties: "symmetric-positive-definite".to_owned(),
                algorithm: "conjugate-gradient".to_owned(),
                preconditioner: "identity".to_owned(),
                reduction: "reproducible".to_owned(),
                relative_tolerance: 1.0e-13,
                absolute_tolerance: 1.0e-15,
                maximum_iterations: 500,
            },
            placement: WirePlacement {
                target: WireTarget {
                    kind: "host-cpu".to_owned(),
                    threads: 1,
                },
                schedule: WireSchedule {
                    kind: "offline".to_owned(),
                },
                assembly_execution: "host-serial".to_owned(),
                solve_execution: "host-serial".to_owned(),
                verification_execution: "host-serial".to_owned(),
                layout_artifacts: WireLayoutArtifacts {
                    kind: "replicated".to_owned(),
                },
            },
        };
        let value = Self {
            wire,
            driven_total_displacement: driven_total_displacement.to_vec(),
        };
        value.validate_local(RealizationDecoderLimits::default())?;
        Ok(value)
    }

    /// Decode the closed logical wire without resolving external resources.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, noncanonical, or unsupported
    /// wire data.
    pub fn from_json(bytes: &[u8], limits: RealizationDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits.json)?;
        let wire: WirePrescribedDynamicSolidRealizationV1 =
            serde_json::from_slice(bytes).map_err(|error| {
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
                "prescribed dynamic-solid Realization JSON is not canonical",
            ));
        }
        Ok(value)
    }

    /// Deterministic canonical JSON bytes.
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

    /// Domain-separated identity of the complete canonical wire.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            REALIZATION_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Exact current Model artifact.
    #[must_use]
    pub fn model_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.model_sha256.clone())
    }

    /// Typed current Model identity.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state was corrupted.
    pub fn model(&self) -> Result<OntologyId<Model>, Diagnostic> {
        parse_ulid(&self.wire.model_ulid, "Model").map(OntologyId::from_ulid)
    }

    /// Exact semantic revision.
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
        ArtifactDigest(self.wire.geometry_sha256.clone())
    }

    /// Exact Geometry-to-Mesh correspondence artifact.
    #[must_use]
    pub fn correspondence_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.correspondence_sha256.clone())
    }

    /// Exact imported mesh artifact.
    #[must_use]
    pub fn mesh_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(
            self.wire
                .spatial
                .discretization
                .mesh
                .artifact_sha256
                .clone(),
        )
    }

    /// Derived solid body Domain.
    #[must_use]
    pub fn solid_domain(&self) -> Id<kinds::Domain> {
        parse_id(&self.wire.spatial.solid_domain_ulid, "solid Domain")
            .expect("validated solid Domain ULID")
    }

    /// Derived displacement Field.
    #[must_use]
    pub fn displacement_field(&self) -> Id<kinds::Field> {
        parse_id(
            &self.wire.spatial.displacement_field_ulid,
            "displacement Field",
        )
        .expect("validated displacement Field ULID")
    }

    /// Derived velocity Field.
    #[must_use]
    pub fn velocity_field(&self) -> Id<kinds::Field> {
        parse_id(&self.wire.spatial.velocity_field_ulid, "velocity Field")
            .expect("validated velocity Field ULID")
    }

    /// Derived fixed x-lower boundary Domain.
    #[must_use]
    pub fn fixed_boundary(&self) -> Id<kinds::Domain> {
        parse_id(&self.wire.spatial.fixed_boundary_ulid, "fixed boundary")
            .expect("validated fixed boundary ULID")
    }

    /// Derived live driven x-upper boundary Domain.
    #[must_use]
    pub fn driven_boundary(&self) -> Id<kinds::Domain> {
        parse_id(&self.wire.spatial.driven_boundary_ulid, "driven boundary")
            .expect("validated driven boundary ULID")
    }

    /// Canonically ordered total displacement on the driven surface.
    #[must_use]
    pub fn driven_total_displacement(&self) -> &[(VertexId, [f64; 3])] {
        &self.driven_total_displacement
    }

    /// Re-derive every semantic role and resource edge from exact dependencies.
    ///
    /// Detached local validity alone does not establish execution.
    ///
    /// # Errors
    /// Returns `EQ0901` for stale lineage, changed role meaning, resource drift,
    /// or departure from the one admitted occurrence.
    pub fn validate_against(
        &self,
        model: &impl ReplayableCanonicalModelArtifact,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        let replay = model.replay_model()?;
        let reference = replay.artifact_reference();
        if self.model_artifact() != *reference.artifact()
            || self.model()? != reference.model()
            || self.semantic_revision() != reference.semantic_revision()
            || self.geometry_artifact() != geometry.digest()?
            || self.correspondence_artifact() != correspondence.digest()?
            || self.mesh_artifact() != mesh.digest()?
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization resource lineage differs",
            ));
        }
        let roles = derive_roles(replay.program())?;
        if self.solid_domain() != roles.solid_domain
            || self.displacement_field() != roles.displacement_field
            || self.velocity_field() != roles.velocity_field
            || self.fixed_boundary() != roles.fixed_boundary
            || self.driven_boundary() != roles.driven_boundary
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization role identities differ from bound Model meaning",
            ));
        }
        validate_resources(
            model,
            geometry,
            correspondence,
            mesh,
            &roles,
            &self.driven_total_displacement,
        )
    }

    fn validate_local(&self, limits: RealizationDecoderLimits) -> Result<(), Diagnostic> {
        let w = &self.wire;
        if w.schema != REALIZATION_SCHEMA
            || w.encoding != CANONICAL_ENCODING
            || w.semantic_revision != 1
            || w.source.kind != "explicit"
            || w.source.realization_revision != 1
            || w.spatial.spatial_dimension != 3
            || w.spatial.scalar != "f64"
            || w.spatial.vector_layout != "replicated"
            || w.spatial.space.kind != "continuous-lagrange"
            || w.spatial.space.order != 1
            || w.spatial.discretization.method != "continuous-galerkin"
            || w.spatial.discretization.mesh.kind != "imported-simplicial"
            || w.spatial.discretization.quadrature
                != "exact-affine-p1-tetrahedron-mass-and-stiffness"
            || w.time.method != "backward-euler"
            || w.time.duration_s.to_bits() != 0.25f64.to_bits()
            || w.solver.operator_properties != "symmetric-positive-definite"
            || w.solver.algorithm != "conjugate-gradient"
            || w.solver.preconditioner != "identity"
            || w.solver.reduction != "reproducible"
            || w.solver.relative_tolerance.to_bits() != 1.0e-13f64.to_bits()
            || w.solver.absolute_tolerance.to_bits() != 1.0e-15f64.to_bits()
            || w.solver.maximum_iterations != 500
            || w.placement.target.kind != "host-cpu"
            || w.placement.target.threads != 1
            || w.placement.schedule.kind != "offline"
            || w.placement.assembly_execution != "host-serial"
            || w.placement.solve_execution != "host-serial"
            || w.placement.verification_execution != "host-serial"
            || w.placement.layout_artifacts.kind != "replicated"
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization differs from its closed grammar",
            ));
        }
        if limits.max_realization_fields < 2 {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization exceeds the configured Field limit",
            ));
        }
        for digest in [
            &w.model_sha256,
            &w.geometry_sha256,
            &w.correspondence_sha256,
            &w.spatial.discretization.mesh.artifact_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        parse_ulid(&w.model_ulid, "Model")?;
        parse_id::<kinds::Domain>(&w.spatial.solid_domain_ulid, "solid Domain")?;
        parse_id::<kinds::Field>(&w.spatial.displacement_field_ulid, "displacement Field")?;
        parse_id::<kinds::Field>(&w.spatial.velocity_field_ulid, "velocity Field")?;
        parse_id::<kinds::Domain>(&w.spatial.fixed_boundary_ulid, "fixed boundary")?;
        parse_id::<kinds::Domain>(&w.spatial.driven_boundary_ulid, "driven boundary")?;
        if self.solid_domain() == self.fixed_boundary()
            || self.solid_domain() == self.driven_boundary()
            || self.fixed_boundary() == self.driven_boundary()
            || self.displacement_field() == self.velocity_field()
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Realization role identities are not distinct",
            ));
        }
        let decoded = decode_driven(&w.driven_total_displacement)?;
        if decoded != self.driven_total_displacement || !exact_driven_values(&decoded) {
            return Err(invalid_artifact(
                "prescribed dynamic-solid driven values differ from the closed occurrence",
            ));
        }
        usize::try_from(w.solver.maximum_iterations).map_err(|_| {
            invalid_artifact("prescribed dynamic-solid solver iteration count exceeds usize")
        })?;
        NonZeroUsize::new(usize::try_from(w.placement.target.threads).map_err(|_| {
            invalid_artifact("prescribed dynamic-solid worker count exceeds usize")
        })?)
        .ok_or_else(|| invalid_artifact("prescribed dynamic-solid worker count is zero"))?;
        Ok(())
    }
}

#[derive(Debug)]
struct SolidRoles {
    solid_domain: Id<kinds::Domain>,
    displacement_field: Id<kinds::Field>,
    velocity_field: Id<kinds::Field>,
    fixed_boundary: Id<kinds::Domain>,
    driven_boundary: Id<kinds::Domain>,
}

fn derive_roles(program: &KernelProgram) -> Result<SolidRoles, Diagnostic> {
    let bodies = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain) => {
                let bounds = program.resolved_cartesian_bounds(domain.id()).ok()?;
                (bounds.len() == 3
                    && bounds.iter().all(|axis| {
                        axis.lower().value().to_bits() == 0.0f64.to_bits()
                            && axis.upper().value().to_bits() == 1.0f64.to_bits()
                    }))
                .then_some(domain.id())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let solid_domain = exactly_one(bodies, "unit-cube solid Domain")?;
    let displacement_field = exact_vector_field(program, solid_domain, LENGTH, "displacement")?;
    let velocity_field = exact_vector_field(program, solid_domain, VELOCITY, "velocity")?;
    let boundaries = (0..3)
        .flat_map(|axis| {
            [BoundarySide::Lower, BoundarySide::Upper]
                .into_iter()
                .map(move |side| (axis, side))
        })
        .map(|(axis, side)| {
            exact_boundary(program, solid_domain, axis, side).map(|id| ((axis, side), id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for ((axis, side), boundary) in &boundaries {
        let expected = match (*axis, *side) {
            (0, BoundarySide::Lower) => Disposition::TraceZero,
            (0, BoundarySide::Upper) => Disposition::Live,
            _ => Disposition::FluxZero,
        };
        require_disposition(program, *boundary, expected)?;
    }
    let fixed_boundary = boundaries
        .iter()
        .find(|((axis, side), _)| *axis == 0 && *side == BoundarySide::Lower)
        .expect("complete boundary inventory")
        .1;
    let driven_boundary = boundaries
        .iter()
        .find(|((axis, side), _)| *axis == 0 && *side == BoundarySide::Upper)
        .expect("complete boundary inventory")
        .1;
    Ok(SolidRoles {
        solid_domain,
        displacement_field,
        velocity_field,
        fixed_boundary,
        driven_boundary,
    })
}

fn exact_vector_field(
    program: &KernelProgram,
    domain: Id<kinds::Domain>,
    dimension: DimExponents,
    label: &'static str,
) -> Result<Id<kinds::Field>, Diagnostic> {
    let fields = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Field(field)
                if field.dimension() == dimension
                    && field.frame() == ValueFrame::SpatialCartesian
                    && field
                        .shape()
                        .extents()
                        .iter()
                        .map(|value| value.get())
                        .eq([3])
                    && program.edges().iter().any(|edge| {
                        edge.kind() == EdgeKind::DefinedOn
                            && edge.from() == field.id().erase()
                            && edge.to() == domain.erase()
                    }) =>
            {
                Some(field.id())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    exactly_one(fields, label)
}

fn exact_boundary(
    program: &KernelProgram,
    body: Id<kinds::Domain>,
    axis: usize,
    side: BoundarySide,
) -> Result<Id<kinds::Domain>, Diagnostic> {
    let boundaries = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(
                    domain.kind(),
                    DomainKind::CartesianBoundary { axis: candidate_axis, side: candidate_side }
                        if *candidate_axis == axis && *candidate_side == side
                ) && program.edges().iter().any(|edge| {
                    edge.kind() == EdgeKind::BoundaryOf
                        && edge.from() == domain.id().erase()
                        && edge.to() == body.erase()
                }) =>
            {
                Some(domain.id())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    exactly_one(boundaries, "Cartesian boundary")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Disposition {
    TraceZero,
    FluxZero,
    Live,
}

fn require_disposition(
    program: &KernelProgram,
    boundary: Id<kinds::Domain>,
    expected: Disposition,
) -> Result<(), Diagnostic> {
    let relations = program
        .edges()
        .iter()
        .filter(|edge| {
            edge.kind() == EdgeKind::AppliesOn
                && edge.to() == boundary.erase()
                && matches!(program.node(edge.from()), Some(KernelNode::Relation(_)))
        })
        .filter_map(|edge| match program.node(edge.from()) {
            Some(KernelNode::Relation(relation)) => {
                disposition_port(relation.residuals(), expected).map(|port| (edge.from(), port))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let (relation, port) = exactly_one(relations, "boundary disposition Relation")?;
    let has_ports = edge_targets(program, relation, EdgeKind::HasPort);
    let dependencies = edge_targets(program, relation, EdgeKind::DependsOn);
    if has_ports != [port.erase()] || dependencies != [port.erase()] {
        return Err(invalid_artifact(
            "boundary disposition Relation does not own exactly its derived Port",
        ));
    }
    let Some(KernelNode::Port(definition)) = program.node(port.erase()) else {
        return Err(invalid_artifact("boundary disposition Port is absent"));
    };
    if definition
        .boundary_physical_contract()
        .is_none_or(|(_, support)| support != boundary)
    {
        return Err(invalid_artifact(
            "boundary disposition Port uses another exact boundary",
        ));
    }
    let connections = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Connects && edge.to() == port.erase())
        .map(|edge| edge.from())
        .collect::<Vec<_>>();
    let connection = exactly_one(connections, "boundary disposition Connection")?;
    if !matches!(program.node(connection), Some(KernelNode::Connection(_))) {
        return Err(invalid_artifact(
            "boundary disposition connection identity is not a Connection",
        ));
    }
    Ok(())
}

fn disposition_port(dag: &ExprDag, expected: Disposition) -> Option<Id<kinds::Port>> {
    if !dag.definitions().is_empty() {
        return None;
    }
    match expected {
        Disposition::TraceZero | Disposition::FluxZero => {
            let [node] = dag.nodes() else { return None };
            let [root] = dag.roots() else { return None };
            if root.index() != 0 {
                return None;
            }
            match (expected, node) {
                (Disposition::TraceZero, ExprNode::Symbol(SymbolRef::PortTrace(port)))
                | (Disposition::FluxZero, ExprNode::Symbol(SymbolRef::PortFlux(port))) => {
                    Some(*port)
                }
                _ => None,
            }
        }
        Disposition::Live => {
            let [
                ExprNode::Symbol(SymbolRef::PortTrace(trace_a)),
                ExprNode::Symbol(SymbolRef::PortTrace(trace_b)),
                ExprNode::Sub(trace_left, trace_right),
                ExprNode::Symbol(SymbolRef::PortFlux(flux_a)),
                ExprNode::Symbol(SymbolRef::PortFlux(flux_b)),
                ExprNode::Sub(flux_left, flux_right),
            ] = dag.nodes()
            else {
                return None;
            };
            let [trace_root, flux_root] = dag.roots() else {
                return None;
            };
            (*trace_a == *trace_b
                && *trace_a == *flux_a
                && *flux_a == *flux_b
                && trace_left.index() == 0
                && trace_right.index() == 1
                && flux_left.index() == 3
                && flux_right.index() == 4
                && trace_root.index() == 2
                && flux_root.index() == 5)
                .then_some(*trace_a)
        }
    }
}

fn validate_resources(
    model: &impl ReplayableCanonicalModelArtifact,
    geometry: &GeometryIdentityEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
    roles: &SolidRoles,
    driven: &[(VertexId, [f64; 3])],
) -> Result<(), Diagnostic> {
    geometry.validate_against(model)?;
    correspondence.validate_against(geometry, model, mesh)?;
    if geometry.tolerance_m().to_bits() != 1.0e-12f64.to_bits()
        || mesh.mesh().quality_gate().minimum_mean_ratio().to_bits() != 0.1f64.to_bits()
        || mesh.mesh().vertices()
            != REFERENCE_VERTICES
                .iter()
                .map(|value| value.to_vec())
                .collect::<Vec<_>>()
        || mesh.mesh().cells()
            != REFERENCE_CELLS
                .iter()
                .map(|value| value.to_vec())
                .collect::<Vec<_>>()
    {
        return Err(invalid_artifact(
            "prescribed dynamic-solid resources differ from the exact Geometry or mesh gates",
        ));
    }
    let bodies = geometry.bodies();
    if bodies.len() != 1
        || bodies[0].domain() != roles.solid_domain
        || bodies[0].bounds_m() != [(0.0, 1.0); 3]
        || correspondence.body_cells(roles.solid_domain)
            != Some((0..REFERENCE_CELLS.len()).collect())
    {
        return Err(invalid_artifact(
            "prescribed dynamic-solid Geometry does not realize the exact body",
        ));
    }
    for (boundary, axis, side) in [
        (roles.fixed_boundary, 0, BoundarySide::Lower),
        (roles.driven_boundary, 0, BoundarySide::Upper),
    ] {
        if !geometry
            .boundaries()
            .iter()
            .any(|entry| entry.domain() == boundary && entry.axis() == axis && entry.side() == side)
        {
            return Err(invalid_artifact(
                "prescribed dynamic-solid Geometry boundary role differs",
            ));
        }
    }
    if boundary_vertices(mesh, correspondence, roles.fixed_boundary)? != [0, 2, 4, 6]
        || boundary_vertices(mesh, correspondence, roles.driven_boundary)? != [1, 3, 5, 7]
        || !exact_driven_values(driven)
    {
        return Err(invalid_artifact(
            "prescribed dynamic-solid boundary coefficients differ from exact support",
        ));
    }
    Ok(())
}

fn boundary_vertices(
    mesh: &SimplicialMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    boundary: Id<kinds::Domain>,
) -> Result<Vec<usize>, Diagnostic> {
    let facets = correspondence
        .boundary_facets(boundary)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| invalid_artifact("prescribed dynamic-solid boundary has no facets"))?;
    let mut vertices = Vec::new();
    for facet in facets {
        let incidence = mesh
            .mesh()
            .incidence(MeshEntity::new(2, facet), 0)
            .ok_or_else(|| {
                invalid_artifact("prescribed dynamic-solid boundary facet is foreign")
            })?;
        vertices.extend(incidence.iter().map(|entry| entry.entity.index()));
    }
    vertices.sort_unstable();
    vertices.dedup();
    Ok(vertices)
}

fn exact_driven_values(values: &[(VertexId, [f64; 3])]) -> bool {
    values.len() == 4
        && values
            .iter()
            .zip([1, 3, 5, 7])
            .all(|((vertex, value), expected)| {
                vertex.index() == expected
                    && value.map(f64::to_bits)
                        == [0.015f64.to_bits(), 0.0f64.to_bits(), 0.0f64.to_bits()]
            })
}

fn decode_driven(values: &[WireDrivenValue]) -> Result<Vec<(VertexId, [f64; 3])>, Diagnostic> {
    values
        .iter()
        .map(|entry| {
            let index = usize::try_from(entry.vertex_index).map_err(|_| {
                invalid_artifact("prescribed dynamic-solid vertex index exceeds usize")
            })?;
            if entry
                .value_m
                .iter()
                .any(|value| !value.is_finite() || (*value == 0.0 && value.is_sign_negative()))
            {
                return Err(invalid_artifact(
                    "prescribed dynamic-solid driven value is non-finite or negative zero",
                ));
            }
            Ok((VertexId::new(index), entry.value_m))
        })
        .collect()
}

fn edge_targets(program: &KernelProgram, from: RawId, kind: EdgeKind) -> Vec<RawId> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.from() == from && edge.kind() == kind)
        .map(|edge| edge.to())
        .collect()
}

fn exactly_one<T>(values: Vec<T>, label: &str) -> Result<T, Diagnostic> {
    let mut values = values.into_iter();
    let value = values
        .next()
        .ok_or_else(|| invalid_artifact(format!("prescribed dynamic-solid {label} is absent")))?;
    if values.next().is_some() {
        return Err(invalid_artifact(format!(
            "prescribed dynamic-solid {label} is not structurally unique"
        )));
    }
    Ok(value)
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

fn parse_id<E: eqiora_core::Entity>(value: &str, label: &str) -> Result<Id<E>, Diagnostic> {
    parse_ulid(value, label).map(Id::from_ulid)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePrescribedDynamicSolidRealizationV1 {
    schema: String,
    encoding: String,
    model_sha256: String,
    model_ulid: String,
    semantic_revision: u64,
    source: WireSource,
    geometry_sha256: String,
    correspondence_sha256: String,
    spatial: WireSpatial,
    time: WireTime,
    driven_total_displacement: Vec<WireDrivenValue>,
    solver: WireSolver,
    placement: WirePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSource {
    kind: String,
    realization_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpatial {
    spatial_dimension: u64,
    scalar: String,
    vector_layout: String,
    solid_domain_ulid: String,
    displacement_field_ulid: String,
    velocity_field_ulid: String,
    fixed_boundary_ulid: String,
    driven_boundary_ulid: String,
    space: WireSpace,
    discretization: WireDiscretization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpace {
    kind: String,
    order: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDiscretization {
    method: String,
    mesh: WireMesh,
    quadrature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMesh {
    kind: String,
    artifact_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTime {
    method: String,
    duration_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDrivenValue {
    vertex_index: u64,
    value_m: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSolver {
    operator_properties: String,
    algorithm: String,
    preconditioner: String,
    reduction: String,
    relative_tolerance: f64,
    absolute_tolerance: f64,
    maximum_iterations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePlacement {
    target: WireTarget,
    schedule: WireSchedule,
    assembly_execution: String,
    solve_execution: String,
    verification_execution: String,
    layout_artifacts: WireLayoutArtifacts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTarget {
    kind: String,
    threads: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSchedule {
    kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLayoutArtifacts {
    kind: String,
}
