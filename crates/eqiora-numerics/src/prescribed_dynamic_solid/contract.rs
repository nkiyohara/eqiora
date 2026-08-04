//! Exact immutable contract admitted by the prescribed-solid reference step.

use eqiora_artifact::{
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id};
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh, VertexId};
use eqiora_schema::kernel::BoundarySide;

use crate::canonical_boundary::PhysicalBoundaryDisposition;
use crate::canonical_elasticity::lower_isotropic_elastodynamics_cartesian_3d;

const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};

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

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PrescribedDynamicSolidContract {
    mesh: SimplicialMesh,
    time_step: f64,
    density: f64,
    shear_modulus: f64,
    first_lame_parameter: f64,
    prior_displacement: Vec<(VertexId, [f64; 3])>,
    prior_velocity: Vec<(VertexId, [f64; 3])>,
    fixed_vertices: Vec<VertexId>,
    driven_vertices: Vec<VertexId>,
}

impl PrescribedDynamicSolidContract {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        model: &impl ReplayableCanonicalModelArtifact,
        geometry: &GeometryIdentityEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        time_step: DynQuantity,
        prior_displacement: &[(VertexId, [f64; 3])],
        prior_velocity: &[(VertexId, [f64; 3])],
        driven_boundary: Id<kinds::Domain>,
    ) -> Result<Self, Diagnostic> {
        if time_step.dim() != TIME || !time_step.value().is_finite() || time_step.value() <= 0.0 {
            return Err(invalid(
                "prescribed dynamic-solid time step must be finite, strictly positive, and have time dimension",
            ));
        }

        let replay = model.replay_model()?;
        let canonical = lower_isotropic_elastodynamics_cartesian_3d(replay.program())?;
        if canonical.bounds() != &[[0.0, 1.0]; 3]
            || canonical.mass_density() != 2.0
            || canonical.shear_modulus() != 3.0
            || canonical.first_lame_parameter() != 0.0
            || canonical.load_potential_expression().constant_value() != Some(0.0)
        {
            return Err(invalid(
                "prescribed dynamic-solid reference admits only the exact unit-cube rho=2, mu=3, lambda=0, zero-load Model",
            ));
        }

        geometry.validate_against(model)?;
        correspondence.validate_against(geometry, model, mesh)?;
        require_reference_mesh(mesh.mesh())?;

        let body = canonical
            .domain()
            .downcast::<kinds::Domain>()
            .ok_or_else(|| {
                invalid("canonical dynamic-solid body does not retain a typed Domain identity")
            })?;
        let bodies = geometry.bodies();
        if bodies.len() != 1
            || bodies[0].domain() != body
            || bodies[0].bounds_m() != [(0.0, 1.0); 3]
            || correspondence.body_cells(body) != Some((0..REFERENCE_CELLS.len()).collect())
        {
            return Err(invalid(
                "prescribed dynamic-solid geometry and correspondence must realize the exact single unit-cube body and ordered cell inventory",
            ));
        }

        let fixed_boundary = require_boundary(
            &canonical,
            geometry,
            0,
            BoundarySide::Lower,
            PhysicalBoundaryDisposition::TraceZero,
        )?;
        let exact_driven_boundary = require_live_driven_boundary(&canonical, geometry)?;
        if driven_boundary != exact_driven_boundary {
            return Err(invalid(
                "prescribed dynamic-solid driven boundary is not the exact x-upper live PortBinding Domain",
            ));
        }
        for axis in 1..3 {
            for side in [BoundarySide::Lower, BoundarySide::Upper] {
                require_boundary(
                    &canonical,
                    geometry,
                    axis,
                    side,
                    PhysicalBoundaryDisposition::FluxZero,
                )?;
            }
        }

        let fixed_vertices =
            boundary_vertices(mesh.mesh(), correspondence, fixed_boundary, "fixed x-lower")?;
        let driven_vertices = boundary_vertices(
            mesh.mesh(),
            correspondence,
            driven_boundary,
            "driven x-upper",
        )?;
        if fixed_vertices
            .iter()
            .any(|vertex| driven_vertices.binary_search(vertex).is_ok())
        {
            return Err(invalid(
                "prescribed dynamic-solid fixed and driven boundary vertices overlap",
            ));
        }

        require_complete_field(mesh.mesh(), prior_displacement, "prior displacement")?;
        require_complete_field(mesh.mesh(), prior_velocity, "prior velocity")?;

        Ok(Self {
            mesh: mesh.mesh().clone(),
            time_step: time_step.value(),
            density: canonical.mass_density(),
            shear_modulus: canonical.shear_modulus(),
            first_lame_parameter: canonical.first_lame_parameter(),
            prior_displacement: prior_displacement.to_vec(),
            prior_velocity: prior_velocity.to_vec(),
            fixed_vertices,
            driven_vertices,
        })
    }

    pub(super) const fn mesh(&self) -> &SimplicialMesh {
        &self.mesh
    }

    pub(super) const fn time_step(&self) -> f64 {
        self.time_step
    }

    pub(super) const fn density(&self) -> f64 {
        self.density
    }

    pub(super) const fn shear_modulus(&self) -> f64 {
        self.shear_modulus
    }

    pub(super) const fn first_lame_parameter(&self) -> f64 {
        self.first_lame_parameter
    }

    pub(super) fn prior_displacement(&self) -> &[(VertexId, [f64; 3])] {
        &self.prior_displacement
    }

    pub(super) fn prior_velocity(&self) -> &[(VertexId, [f64; 3])] {
        &self.prior_velocity
    }

    pub(super) fn fixed_vertices(&self) -> &[VertexId] {
        &self.fixed_vertices
    }

    pub(super) fn driven_vertices(&self) -> &[VertexId] {
        &self.driven_vertices
    }

    pub(super) fn validate_candidate(
        &self,
        candidate: &[(VertexId, [f64; 3])],
    ) -> Result<(), Diagnostic> {
        if candidate.len() != self.driven_vertices.len() {
            return Err(invalid(
                "prescribed dynamic-solid candidate must contain every driven vertex exactly once",
            ));
        }
        for ((candidate_vertex, value), expected_vertex) in
            candidate.iter().zip(&self.driven_vertices)
        {
            if candidate_vertex != expected_vertex || value.iter().any(|entry| !entry.is_finite()) {
                return Err(invalid(
                    "prescribed dynamic-solid candidate must use exact canonical driven-vertex order and finite total displacement",
                ));
            }
        }
        Ok(())
    }
}

fn require_reference_mesh(mesh: &SimplicialMesh) -> Result<(), Diagnostic> {
    let exact_vertices = REFERENCE_VERTICES
        .iter()
        .map(|coordinates| coordinates.to_vec())
        .collect::<Vec<_>>();
    let exact_cells = REFERENCE_CELLS
        .iter()
        .map(|cell| cell.to_vec())
        .collect::<Vec<_>>();
    if mesh.topological_dimension() != 3
        || mesh.vertices() != exact_vertices
        || mesh.cells() != exact_cells
    {
        return Err(invalid(
            "prescribed dynamic-solid reference requires the exact ordered nine-vertex, twelve-tetrahedron unit-cube mesh",
        ));
    }
    Ok(())
}

fn require_boundary(
    canonical: &crate::canonical_elasticity::IsotropicElastodynamicsCartesianModel3d,
    geometry: &GeometryIdentityEnvelopeV1,
    axis: usize,
    side: BoundarySide,
    disposition: PhysicalBoundaryDisposition,
) -> Result<Id<kinds::Domain>, Diagnostic> {
    let entry = canonical
        .boundary_inventory()
        .boundary(axis, side)
        .ok_or_else(|| invalid("canonical dynamic-solid boundary inventory is incomplete"))?;
    if entry.disposition() != disposition {
        return Err(invalid(format!(
            "canonical dynamic-solid boundary on axis {axis} {side:?} has the wrong disposition"
        )));
    }
    let boundary = entry
        .boundary()
        .downcast::<kinds::Domain>()
        .ok_or_else(|| {
            invalid("canonical dynamic-solid boundary does not retain a typed Domain identity")
        })?;
    require_geometry_boundary(geometry, boundary, axis, side)?;
    Ok(boundary)
}

fn require_live_driven_boundary(
    canonical: &crate::canonical_elasticity::IsotropicElastodynamicsCartesianModel3d,
    geometry: &GeometryIdentityEnvelopeV1,
) -> Result<Id<kinds::Domain>, Diagnostic> {
    let entry = canonical
        .boundary_inventory()
        .boundary(0, BoundarySide::Upper)
        .ok_or_else(|| invalid("canonical dynamic-solid boundary inventory omits x-upper"))?;
    if !matches!(
        entry.disposition(),
        PhysicalBoundaryDisposition::PortBinding { .. }
    ) {
        return Err(invalid(
            "canonical dynamic-solid x-upper boundary must remain a live PortBinding",
        ));
    }
    let boundary = entry
        .boundary()
        .downcast::<kinds::Domain>()
        .ok_or_else(|| {
            invalid("canonical driven boundary does not retain a typed Domain identity")
        })?;
    require_geometry_boundary(geometry, boundary, 0, BoundarySide::Upper)?;
    Ok(boundary)
}

fn require_geometry_boundary(
    geometry: &GeometryIdentityEnvelopeV1,
    boundary: Id<kinds::Domain>,
    axis: usize,
    side: BoundarySide,
) -> Result<(), Diagnostic> {
    if !geometry.boundaries().iter().any(|candidate| {
        candidate.domain() == boundary && candidate.axis() == axis && candidate.side() == side
    }) {
        return Err(invalid(
            "canonical dynamic-solid boundary identity differs from the exact geometry catalog",
        ));
    }
    Ok(())
}

fn boundary_vertices(
    mesh: &SimplicialMesh,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    boundary: Id<kinds::Domain>,
    label: &'static str,
) -> Result<Vec<VertexId>, Diagnostic> {
    let facets = correspondence
        .boundary_facets(boundary)
        .filter(|facets| !facets.is_empty())
        .ok_or_else(|| {
            invalid(format!(
                "prescribed dynamic-solid {label} boundary has no facets"
            ))
        })?;
    let mut vertices = Vec::new();
    for facet in facets {
        let entity = MeshEntity::new(2, facet);
        let facet_vertices = mesh.entity_vertices(entity).ok_or_else(|| {
            invalid(format!(
                "prescribed dynamic-solid {label} correspondence names a foreign facet"
            ))
        })?;
        vertices.extend(
            facet_vertices
                .into_iter()
                .map(|vertex| VertexId::new(vertex.index())),
        );
    }
    vertices.sort_by_key(|vertex| vertex.index());
    vertices.dedup();
    if vertices.is_empty() {
        return Err(invalid(format!(
            "prescribed dynamic-solid {label} boundary has no vertex coefficients"
        )));
    }
    Ok(vertices)
}

fn require_complete_field(
    mesh: &SimplicialMesh,
    field: &[(VertexId, [f64; 3])],
    label: &'static str,
) -> Result<(), Diagnostic> {
    if field.len() != mesh.vertices().len()
        || field.iter().enumerate().any(|(index, (vertex, value))| {
            vertex.index() != index || value.iter().any(|entry| !entry.is_finite())
        })
    {
        return Err(invalid(format!(
            "prescribed dynamic-solid {label} must contain one finite vector in exact canonical vertex order"
        )));
    }
    Ok(())
}

pub(super) fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}
