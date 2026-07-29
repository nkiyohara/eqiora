//! Exact-geometry binding for the shared steady MINI/P1 Stokes path.

use std::collections::BTreeMap;

use eqiora_artifact::{
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, SimplicialMeshEnvelopeV1,
};
use eqiora_assembly::REFERENCE_ASSEMBLY_BACKEND;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{CanonicalCircularHoleGeometryV1, CircularHoleChordalMeshV1};
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh};
use eqiora_realization::{
    FieldwiseRealizationPlan, FieldwiseRealizationRequirements, MeshArtifactReference,
    ResolvedFieldwiseRealization,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{LinearSolverBackend, SolverPlan};

use super::api::{SteadyIncompressibleStokesModel2d, StokesBoundaryKey2d};
use super::physical::SteadyStokesMiniSolution2d;
use super::realization::{
    SteadyStokesScaleProfile2d, finalize_lowered_steady_stokes_mini_2d_with_assembly,
    resolved_stokes_scales, steady_stokes_fieldwise_requirements_for_model_2d,
    steady_stokes_mini_plan_for_model_2d,
};
use super::recognize::lower_steady_incompressible_stokes_geometry_2d;
use crate::canonical_boundary::{PhysicalBoundaryDisposition, PhysicalBoundaryQuantity};
use crate::simplicial_stokes::{
    SimplicialMiniStokesBoundary2d, SimplicialMiniStokesBoundaryCondition2d,
    SimplicialMiniStokesBoundaryFacet2d,
};

const DIMENSION: usize = 2;
const REQUIRED_BOUNDARY_SETS: [&str; 4] = ["cylinder", "inlet", "outlet", "walls"];

struct GeometryBoundary2d {
    boundary: SimplicialMiniStokesBoundary2d,
    fixed_velocity: Vec<Option<[f64; DIMENSION]>>,
}

/// Replay-validated exact-circle to affine-mesh binding for steady Stokes.
///
/// This value is deliberately narrower than a geometry framework. It accepts
/// the one bounded circular-hole owner, its ordinary region and mesh artifacts,
/// and the authored-region correspondence. Named sets are resolved once
/// through that correspondence and never recovered from coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct SteadyStokesGeometryBinding2d {
    model: SteadyIncompressibleStokesModel2d,
    source: CanonicalCircularHoleGeometryV1,
    owner: CircularHoleChordalMeshV1,
    geometry: GeometryDefinitionV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    entity_sets: BTreeMap<String, Vec<MeshEntity>>,
}

impl SteadyStokesGeometryBinding2d {
    /// Validate the complete in-process exact-source realization chain.
    ///
    /// # Errors
    /// Rejects source, region, mesh, correspondence, named-set, or complete
    /// fluid-cell inventory drift before any Stokes assembly can begin.
    pub fn new(
        program: &KernelProgram,
        source: CanonicalCircularHoleGeometryV1,
        owner: CircularHoleChordalMeshV1,
        geometry: GeometryDefinitionV1,
        mesh: SimplicialMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        if owner.source().digest_bytes() != source.digest_bytes() {
            return Err(invalid(
                "circular-hole chordal owner belongs to another exact source revision",
            ));
        }
        let expected_geometry = GeometryDefinitionV1::from_region(owner.region());
        if geometry != expected_geometry {
            return Err(invalid(
                "geometry artifact differs from the chordal owner's authored region",
            ));
        }
        let expected_mesh = SimplicialMeshEnvelopeV1::from_mesh(owner.mesh())?;
        if mesh != expected_mesh {
            return Err(invalid(
                "mesh artifact differs from the chordal owner's exact mesh inventory",
            ));
        }
        correspondence.validate_against_region(&geometry, &mesh)?;
        let mut entity_sets = BTreeMap::new();
        for name in REQUIRED_BOUNDARY_SETS.into_iter().chain(["fluid"]) {
            entity_sets.insert(
                name.to_owned(),
                correspondence.region_entity_set_entities(&geometry, name)?,
            );
        }
        let expected_cells = (0..mesh.mesh().entity_count(DIMENSION).expect("2D mesh cells"))
            .map(|index| MeshEntity::new(DIMENSION, index))
            .collect::<Vec<_>>();
        if entity_sets["fluid"] != expected_cells {
            return Err(invalid(
                "the exact `fluid` entity set does not realize every mesh cell exactly once",
            ));
        }
        let model = lower_steady_incompressible_stokes_geometry_2d(program, &source)?;
        if model.geometry_source_digest() != Some(owner.source().digest_bytes()) {
            return Err(invalid(
                "Model GeometryRegion digest differs from the chordal owner's exact source revision",
            ));
        }
        Ok(Self {
            model,
            source,
            owner,
            geometry,
            mesh,
            correspondence,
            entity_sets,
        })
    }

    /// Exact field-wise requirements derived from the bound Model.
    #[must_use]
    pub fn fieldwise_requirements(&self) -> FieldwiseRealizationRequirements {
        steady_stokes_fieldwise_requirements_for_model_2d(&self.model)
    }

    /// Build the bounded MINI/P1 plan for this source-bound Model.
    ///
    /// # Errors
    /// Preserves exact scale, solver-tuple, and plan-construction diagnostics.
    pub fn mini_plan(
        &self,
        mesh: MeshArtifactReference,
        scales: SteadyStokesScaleProfile2d,
        solver: SolverPlan,
    ) -> Result<FieldwiseRealizationPlan, Diagnostic> {
        steady_stokes_mini_plan_for_model_2d(&self.model, mesh, scales, solver)
    }

    fn entities(&self, name: &str) -> Result<&[MeshEntity], Diagnostic> {
        self.entity_sets
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid(format!("geometry binding has no entity set named `{name}`")))
    }

    fn mesh_reference(&self) -> Result<MeshArtifactReference, Diagnostic> {
        self.mesh.artifact_reference()
    }
}

/// Solve the exact geometry-backed steady Stokes model through reference paths.
///
/// The Model source digest, chordal owner, artifacts and correspondence are
/// all reaccepted before assembly. The returned named `cylinder` reaction uses
/// the existing constrained-residual convention: force on the fluid. Retained
/// inlet and outlet fluxes use the physical parent-outward convention.
///
/// # Errors
/// Preserves semantic replay, exact-source binding, field-wise Realization,
/// reference assembly, backend execution, coherent-SI reconstruction and
/// named-boundary flux reconstruction diagnostics.
pub fn solve_resolved_steady_stokes_geometry_mini_2d(
    program: &KernelProgram,
    resolved: &ResolvedFieldwiseRealization,
    binding: &SteadyStokesGeometryBinding2d,
    backend: &dyn LinearSolverBackend,
) -> Result<SteadyStokesMiniSolution2d, Diagnostic> {
    // Replay at the use site so a future decoded binding cannot bypass the
    // source/region/mesh relationship established by `new`.
    if binding.owner.source().digest_bytes() != binding.source.digest_bytes()
        || binding.geometry != GeometryDefinitionV1::from_region(binding.owner.region())
        || binding.mesh != SimplicialMeshEnvelopeV1::from_mesh(binding.owner.mesh())?
    {
        return Err(invalid(
            "steady Stokes geometry binding no longer replays to its exact owner",
        ));
    }
    binding
        .correspondence
        .validate_against_region(&binding.geometry, &binding.mesh)?;
    let model = lower_steady_incompressible_stokes_geometry_2d(program, &binding.source)?;
    if model != binding.model
        || model.geometry_source_digest() != Some(binding.owner.source().digest_bytes())
    {
        return Err(invalid(
            "Model meaning or GeometryRegion digest differs from the source-bound Stokes binding",
        ));
    }
    let mesh_reference = binding.mesh_reference()?;
    let scales = resolved_stokes_scales(program, resolved, mesh_reference, &model)?;
    let normalized =
        normalize_geometry_mesh(model.bounds(), binding.mesh.mesh(), scales.length_value())?;
    let geometry_boundary = geometry_boundary(&model, binding, &normalized, scales)?;
    let lookup = normalized
        .vertices()
        .iter()
        .enumerate()
        .filter_map(|(index, coordinate)| {
            geometry_boundary.fixed_velocity[index]
                .map(|value| ((coordinate[0].to_bits(), coordinate[1].to_bits()), value))
        })
        .collect::<BTreeMap<_, _>>();
    let essential_velocity = |coordinate: [f64; DIMENSION]| {
        lookup
            .get(&(coordinate[0].to_bits(), coordinate[1].to_bits()))
            .copied()
            .ok_or_else(|| {
                invalid(
                    "an essential geometry vertex is absent from correspondence-derived trace data",
                )
            })
    };
    let finalized = finalize_lowered_steady_stokes_mini_2d_with_assembly(
        resolved,
        mesh_reference,
        &model,
        binding.mesh.mesh(),
        &normalized,
        &geometry_boundary.boundary,
        scales,
        &essential_velocity,
        &REFERENCE_ASSEMBLY_BACKEND,
    )?;
    let solved = backend.solve(&finalized.linear_problem()?, finalized.solver_plan())?;
    let solution = finalized.finish(solved)?;
    let named_boundary_fluxes = ["inlet", "outlet"]
        .into_iter()
        .map(|name| {
            boundary_flux(
                binding.mesh.mesh(),
                solution.velocity().vertex_values(),
                binding.entities(name)?,
            )
            .map(|flux| (name.to_owned(), flux))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    solution.with_named_boundary_fluxes(named_boundary_fluxes)
}

fn boundary_flux(
    mesh: &SimplicialMesh,
    velocity: &[[f64; DIMENSION]],
    facets: &[MeshEntity],
) -> Result<f64, Diagnostic> {
    if velocity.len() != mesh.vertices().len() {
        return Err(invalid(
            "named Stokes boundary flux requires one velocity per mesh vertex",
        ));
    }
    if facets.is_empty() {
        return Err(invalid(
            "named Stokes boundary flux requires at least one facet",
        ));
    }
    let mut total = 0.0;
    for &facet in facets {
        let vertices = mesh
            .entity_vertices(facet)
            .filter(|vertices| {
                facet.dimension() == DIMENSION - 1
                    && vertices.len() == DIMENSION
                    && vertices.iter().all(|vertex| vertex.dimension() == 0)
            })
            .ok_or_else(|| invalid("named Stokes flux set contains a non-edge entity"))?;
        let [first, second] = [vertices[0].index(), vertices[1].index()];
        let first_coordinate = &mesh.vertices()[first];
        let second_coordinate = &mesh.vertices()[second];
        let length = (second_coordinate[0] - first_coordinate[0])
            .hypot(second_coordinate[1] - first_coordinate[1]);
        let normal = facet_outward_unit_normal(mesh, facet)?;
        let average_velocity = [
            0.5 * (velocity[first][0] + velocity[second][0]),
            0.5 * (velocity[first][1] + velocity[second][1]),
        ];
        total += length * (average_velocity[0] * normal[0] + average_velocity[1] * normal[1]);
    }
    if !total.is_finite() {
        return Err(invalid(
            "named Stokes boundary flux reconstruction is non-finite",
        ));
    }
    Ok(total)
}

fn normalize_geometry_mesh(
    bounds: &[[f64; 2]; DIMENSION],
    mesh: &SimplicialMesh,
    length: f64,
) -> Result<SimplicialMesh, Diagnostic> {
    if mesh.topological_dimension() != DIMENSION {
        return Err(invalid(
            "geometry-backed coherent-SI MINI Stokes requires an intrinsic 2D mesh",
        ));
    }
    let vertices = mesh
        .vertices()
        .iter()
        .map(|coordinate| {
            if coordinate.len() != DIMENSION
                || coordinate
                    .iter()
                    .enumerate()
                    .any(|(axis, value)| *value < bounds[axis][0] || *value > bounds[axis][1])
            {
                return Err(invalid(
                    "geometry-backed Stokes mesh has a vertex outside the exact source bounds",
                ));
            }
            Ok(vec![
                (coordinate[0] - bounds[0][0]) / length,
                (coordinate[1] - bounds[1][0]) / length,
            ])
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    SimplicialMesh::new(
        DIMENSION,
        vertices,
        mesh.cells().to_vec(),
        mesh.quality_gate(),
    )
    .map_err(|error| invalid(error.message()))
}

fn geometry_boundary(
    model: &SteadyIncompressibleStokesModel2d,
    binding: &SteadyStokesGeometryBinding2d,
    normalized: &SimplicialMesh,
    scales: SteadyStokesScaleProfile2d,
) -> Result<GeometryBoundary2d, Diagnostic> {
    let mut facet_owners = BTreeMap::new();
    let mut fixed_velocity = vec![None; normalized.vertices().len()];
    let physical = binding.mesh.mesh();
    for (key, entry) in model.boundary_entries() {
        let StokesBoundaryKey2d::NamedEntitySet(name) = key else {
            return Err(invalid(
                "geometry-backed Stokes model contains a Cartesian boundary key",
            ));
        };
        let facets = binding.entities(name)?;
        for facet in facets {
            if facet.dimension() != DIMENSION - 1
                || facet_owners.insert(*facet, key.clone()).is_some()
            {
                return Err(invalid(
                    "correspondence-derived Stokes boundary partition overlaps or contains a non-facet",
                ));
            }
        }
        let essential = entry.disposition == PhysicalBoundaryDisposition::TraceZero
            || matches!(
                entry.disposition,
                PhysicalBoundaryDisposition::Prescribed(law)
                    if law.quantity() == PhysicalBoundaryQuantity::Trace
            );
        if !essential {
            continue;
        }
        let outward = match entry.disposition {
            PhysicalBoundaryDisposition::TraceZero => None,
            PhysicalBoundaryDisposition::Prescribed(_) => Some(
                eqiora_geometry::CanonicalGeometryRef::from(&binding.source)
                    .constant_parent_outward_normal(name)
                    .ok_or_else(|| {
                        invalid(format!(
                            "prescribed normal velocity on `{name}` has no exact fixed-side normal"
                        ))
                    })?,
            ),
            _ => unreachable!("essential predicate is exact"),
        };
        for facet in facets {
            for vertex in physical
                .entity_vertices(*facet)
                .expect("validated correspondence facet owns vertices")
            {
                let index = vertex.index();
                let coordinate = &physical.vertices()[index];
                let value = if let Some(outward) = outward {
                    model
                        .prescribed_normal_velocity(key, outward, coordinate)?
                        .ok_or_else(|| {
                            invalid(format!(
                                "prescribed normal velocity on `{name}` has no retained expression"
                            ))
                        })?
                        .map(|component| component / scales.velocity_value())
                } else {
                    [0.0; DIMENSION]
                };
                if fixed_velocity[index].is_some_and(|existing| existing != value) {
                    return Err(invalid(format!(
                        "essential velocity prescriptions disagree at a vertex shared by `{name}`"
                    )));
                }
                fixed_velocity[index] = Some(value);
            }
        }
    }

    let facets = facet_owners
        .iter()
        .map(|(facet, key)| {
            let entry = model
                .boundary_entry(key)
                .expect("facet owner came from the exact model inventory");
            let condition = match entry.disposition {
                PhysicalBoundaryDisposition::TraceZero => {
                    SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
                }
                PhysicalBoundaryDisposition::Prescribed(law)
                    if law.quantity() == PhysicalBoundaryQuantity::Trace =>
                {
                    SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
                }
                PhysicalBoundaryDisposition::FluxZero
                | PhysicalBoundaryDisposition::Prescribed(_) => {
                    let pressure = model
                        .normal_pressure_for(key)
                        .ok_or_else(|| {
                            invalid(format!(
                                "traction boundary {key:?} has no retained normal-pressure law"
                            ))
                        })?
                        .expression()
                        .constant_value()
                        .ok_or_else(|| {
                            invalid(
                                "geometry-backed Stokes admits only constant normal pressure",
                            )
                        })?;
                    let outward = facet_outward_unit_normal(normalized, *facet)?;
                    SimplicialMiniStokesBoundaryCondition2d::ConstantTraction {
                        value: outward.map(|normal| -pressure * normal / scales.pressure_value()),
                    }
                }
                PhysicalBoundaryDisposition::PortBinding { connection, port } => {
                    return Err(invalid(format!(
                        "live Stokes PortBinding {connection} through Port {port} requires an explicit trace-space interface Realization"
                    )));
                }
            };
            Ok(SimplicialMiniStokesBoundaryFacet2d::new(
                *facet, condition,
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let boundary = SimplicialMiniStokesBoundary2d::new(normalized, facets)
        .map_err(|error| invalid(error.message()))?
        .with_named_reaction_surface(
            normalized,
            "cylinder",
            binding.entities("cylinder")?.iter().copied(),
        )
        .map_err(|error| invalid(error.message()))?;
    Ok(GeometryBoundary2d {
        boundary,
        fixed_velocity,
    })
}

fn facet_outward_unit_normal(
    mesh: &SimplicialMesh,
    facet: MeshEntity,
) -> Result<[f64; DIMENSION], Diagnostic> {
    let vertices = mesh
        .entity_vertices(facet)
        .ok_or_else(|| invalid("correspondence facet is absent from the bound mesh"))?;
    let adjacent = mesh
        .incidence(facet, DIMENSION)
        .ok_or_else(|| invalid("correspondence facet has no cell incidence"))?;
    let ([left, right], [cell]) = (vertices.as_slice(), adjacent.as_slice()) else {
        return Err(invalid(
            "boundary facet requires two vertices and exactly one adjacent fluid cell",
        ));
    };
    let a = &mesh.vertices()[left.index()];
    let b = &mesh.vertices()[right.index()];
    let tangent = [b[0] - a[0], b[1] - a[1]];
    let length = tangent[0].hypot(tangent[1]);
    if !length.is_finite() || length <= 0.0 {
        return Err(invalid("boundary facet has non-positive finite length"));
    }
    let cell_vertices = mesh
        .entity_vertices(cell.entity)
        .expect("validated adjacent cell owns vertices");
    let mut centroid = [0.0; DIMENSION];
    for vertex in cell_vertices {
        centroid[0] += mesh.vertices()[vertex.index()][0] / 3.0;
        centroid[1] += mesh.vertices()[vertex.index()][1] / 3.0;
    }
    let midpoint = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
    let mut normal = [tangent[1] / length, -tangent[0] / length];
    if normal[0] * (midpoint[0] - centroid[0]) + normal[1] * (midpoint[1] - centroid[1]) < 0.0 {
        normal = [-normal[0], -normal[1]];
    }
    Ok(normal)
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}
