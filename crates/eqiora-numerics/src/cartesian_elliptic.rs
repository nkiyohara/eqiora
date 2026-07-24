use std::sync::Arc;

use eqiora_assembly::{
    AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPlan, AssemblyReport, AssemblyTarget,
    DofId, IndexedAssemblyWork, LinearSystem, LocalContribution, LocalUnknown,
    REFERENCE_ASSEMBLY_BACKEND, TargetAssemblyMap,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_ir::LocalLinearActionIr;
use eqiora_meshing::{
    AffineGeometryLinearization, AffineGeometryMap, GeometryMap, MeshEntity, MeshGeometry,
    MeshTopology, QuadratureRule, ReferenceCell,
};
use eqiora_solver::{
    CanonicalCsrSystemView, LinearOperatorProperties, LinearSolution, LinearSolveRequest,
    SolveReport,
};

use crate::affine_fem::{dot, physical_gradient, weighted_gradient, weighted_gradient_tangent};
use crate::{
    AssembledLinearizedRelation, CartesianMesh, CartesianScalarFieldLinearization, DiscreteSpace,
    HypercubeQ1Space, LocalOperator, ScalarEllipticCartesianModel, SpatialDesignCoordinate,
};

/// Continuous scalar Q1 field on a Cartesian mesh.
///
/// Vertex values follow the mesh's canonical vertex stratum. Evaluation uses
/// the same reference basis and affine geometry contract as assembly.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianQ1Field {
    mesh: CartesianMesh,
    vertex_values: Vec<f64>,
}

impl CartesianQ1Field {
    /// Construct a finite field with exactly one value per mesh vertex.
    ///
    /// # Errors
    /// Returns `EQ0801` for an incompatible value shape or non-finite data.
    pub fn new(mesh: CartesianMesh, vertex_values: Vec<f64>) -> Result<Self, Diagnostic> {
        if Some(vertex_values.len()) != mesh.entity_count(0)
            || vertex_values.iter().any(|value| !value.is_finite())
        {
            return Err(invalid(
                "Cartesian Q1 field requires one finite value per mesh vertex",
            ));
        }
        Ok(Self {
            mesh,
            vertex_values,
        })
    }

    /// Mesh carrying this field.
    #[must_use]
    pub const fn mesh(&self) -> &CartesianMesh {
        &self.mesh
    }

    /// Values in canonical mesh-vertex order.
    #[must_use]
    pub fn vertex_values(&self) -> &[f64] {
        &self.vertex_values
    }

    /// Consume the field into values in canonical mesh-vertex order.
    #[must_use]
    pub(crate) fn into_vertex_values(self) -> Vec<f64> {
        self.vertex_values
    }

    /// Continuous L2 error against an independently supplied exact field.
    ///
    /// # Errors
    /// Returns a numerical diagnostic for incompatible quadrature, invalid
    /// exact values, geometry failure, or non-finite accumulation.
    pub fn l2_error<E>(&self, exact: &E, quadrature: &QuadratureRule) -> Result<f64, Diagnostic>
    where
        E: Fn(&[f64]) -> f64 + ?Sized,
    {
        require_cell_rule(&self.mesh, quadrature)?;
        let dimension = self.mesh.topological_dimension();
        let space = HypercubeQ1Space::new(dimension)?;
        let mut squared_error = 0.0;
        for cell_index in 0..self
            .mesh
            .entity_count(dimension)
            .expect("mesh owns its top stratum")
        {
            let cell = MeshEntity::new(dimension, cell_index);
            let geometry = self
                .mesh
                .geometry_map(cell)
                .expect("mesh top stratum owns geometry");
            let vertices = self
                .mesh
                .entity_vertices(cell)
                .expect("mesh top stratum owns a vertex closure");
            for point in quadrature.points() {
                let basis = space.tabulate(&point.coordinates)?;
                let mut physical = vec![0.0; dimension];
                geometry.map_point(&point.coordinates, &mut physical)?;
                let approximation = basis
                    .values()
                    .iter()
                    .zip(&vertices)
                    .map(|(basis, vertex)| basis * self.vertex_values[vertex.index()])
                    .sum::<f64>();
                let exact_value = exact(&physical);
                if !exact_value.is_finite() {
                    return Err(invalid("exact Cartesian field returned a non-finite value"));
                }
                squared_error +=
                    point.weight * geometry.measure_scale() * (approximation - exact_value).powi(2);
            }
        }
        if !squared_error.is_finite() || squared_error < 0.0 {
            return Err(invalid(
                "Cartesian L2 error accumulation is non-finite or negative",
            ));
        }
        Ok(squared_error.sqrt())
    }
}

/// Q1 finite-element solution with recovered boundary equilibrium evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticCartesianFemSolution {
    field: CartesianQ1Field,
    free_vertices: Vec<usize>,
    algebraic_values: Vec<f64>,
    canonical_system: Arc<CanonicalCsrSystemView>,
    boundary_reaction_sum: f64,
    integrated_source: f64,
    assembly_report: AssemblyReport,
    solve_report: SolveReport,
}

impl ScalarEllipticCartesianFemSolution {
    /// Continuous Q1 result field.
    #[must_use]
    pub const fn field(&self) -> &CartesianQ1Field {
        &self.field
    }

    /// Sum of residual reactions on all eliminated boundary vertices.
    #[must_use]
    pub const fn boundary_reaction_sum(&self) -> f64 {
        self.boundary_reaction_sum
    }

    /// Source integral represented by the assembled cell load vectors.
    #[must_use]
    pub const fn integrated_source(&self) -> f64 {
        self.integrated_source
    }

    /// Complete assembly placement and accepted packet-shape evidence.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    /// Complete solver/backend/execution evidence.
    #[must_use]
    pub const fn solve_report(&self) -> &SolveReport {
        &self.solve_report
    }

    pub(crate) fn free_vertices(&self) -> &[usize] {
        &self.free_vertices
    }

    /// Free-vertex algebraic unknowns in assembled equation order.
    #[must_use]
    pub fn algebraic_values(&self) -> &[f64] {
        &self.algebraic_values
    }

    pub(crate) const fn canonical_system(&self) -> &Arc<CanonicalCsrSystemView> {
        &self.canonical_system
    }

    /// Consume the accepted solution into its complete primary Field values.
    #[must_use]
    pub(crate) fn into_field_values(self) -> Vec<f64> {
        self.field.into_vertex_values()
    }
}

/// Cell-centered FVM solution and its explicit continuous reconstruction.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticCartesianFvmSolution {
    mesh: CartesianMesh,
    cell_centers: Vec<Vec<f64>>,
    cell_values: Vec<f64>,
    canonical_system: Arc<CanonicalCsrSystemView>,
    reconstruction: CartesianQ1Field,
    boundary_flux_sum: f64,
    integrated_source: f64,
    assembly_report: AssemblyReport,
    solve_report: SolveReport,
}

impl ScalarEllipticCartesianFvmSolution {
    /// Mesh carrying the cell-centred algebraic field.
    #[must_use]
    pub const fn mesh(&self) -> &CartesianMesh {
        &self.mesh
    }

    /// Physical center of every algebraic cell unknown.
    #[must_use]
    pub fn cell_centers(&self) -> &[Vec<f64>] {
        &self.cell_centers
    }

    /// Algebraic values in canonical cell order.
    #[must_use]
    pub fn cell_values(&self) -> &[f64] {
        &self.cell_values
    }

    /// Q1 dual-grid reconstruction through boundary samples and cell centers.
    #[must_use]
    pub const fn reconstruction(&self) -> &CartesianQ1Field {
        &self.reconstruction
    }

    /// Sum of reconstructed outward diffusive fluxes on boundary facets.
    #[must_use]
    pub const fn boundary_flux_sum(&self) -> f64 {
        self.boundary_flux_sum
    }

    /// Source integral represented by the assembled cell balances.
    #[must_use]
    pub const fn integrated_source(&self) -> f64 {
        self.integrated_source
    }

    /// Complete assembly placement and accepted packet-shape evidence.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    /// Complete solver/backend/execution evidence.
    #[must_use]
    pub const fn solve_report(&self) -> &SolveReport {
        &self.solve_report
    }

    pub(crate) const fn canonical_system(&self) -> &Arc<CanonicalCsrSystemView> {
        &self.canonical_system
    }

    /// Consume the accepted solution into canonical cell-centred Field values.
    #[must_use]
    pub(crate) fn into_field_values(self) -> Vec<f64> {
        self.cell_values
    }
}

/// Method-private state retained between finalized FEM assembly and field
/// reconstruction.
///
/// This is crate-visible only so canonical realization can expose one opaque,
/// method-neutral algebraic handoff without making constraint bookkeeping part
/// of the public contract.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedCartesianFemAssembly {
    mesh: CartesianMesh,
    fixed_values: Vec<Option<f64>>,
    free_indices: Vec<Option<DofId>>,
    free_vertices: Vec<usize>,
    linear_system: LinearSystem,
    full_system: LinearSystem,
    integrated_source: f64,
    assembly_report: AssemblyReport,
}

impl FinalizedCartesianFemAssembly {
    pub(crate) fn into_canonical(
        self,
    ) -> Result<(Arc<CanonicalCsrSystemView>, FinalizedCartesianFemState), Diagnostic> {
        let Self {
            mesh,
            fixed_values,
            free_indices,
            free_vertices,
            linear_system,
            full_system,
            integrated_source,
            assembly_report,
        } = self;
        let canonical_system = Arc::new(CanonicalCsrSystemView::new(
            &linear_system,
            LinearOperatorProperties::SymmetricPositiveDefinite,
        )?);
        Ok((
            canonical_system,
            FinalizedCartesianFemState {
                mesh,
                fixed_values,
                free_indices,
                free_vertices,
                full_system,
                integrated_source,
                assembly_report,
            },
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedCartesianFemState {
    mesh: CartesianMesh,
    fixed_values: Vec<Option<f64>>,
    free_indices: Vec<Option<DofId>>,
    free_vertices: Vec<usize>,
    full_system: LinearSystem,
    integrated_source: f64,
    assembly_report: AssemblyReport,
}

impl FinalizedCartesianFemState {
    pub(crate) const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    pub(crate) fn finish(
        self,
        solved: LinearSolution,
        canonical_system: Arc<CanonicalCsrSystemView>,
    ) -> Result<ScalarEllipticCartesianFemSolution, Diagnostic> {
        if solved.values().len() != canonical_system.rows() {
            return Err(invalid(
                "Cartesian FEM solution shape differs from its finalized system",
            ));
        }
        let (algebraic_values, solve_report) = solved.into_parts();
        let mut values = fallible_zeroed(
            self.fixed_values.len(),
            "Cartesian FEM field allocation exceeds platform capacity",
        )?;
        for (vertex, value) in values.iter_mut().enumerate() {
            *value = if let Some(fixed) = self.fixed_values[vertex] {
                fixed
            } else {
                let equation = self
                    .free_indices
                    .get(vertex)
                    .copied()
                    .flatten()
                    .ok_or_else(|| {
                        invalid("Cartesian FEM free vertex has no finalized equation")
                    })?;
                *algebraic_values.get(equation.index()).ok_or_else(|| {
                    invalid("Cartesian FEM equation exceeds its finalized solution")
                })?
            };
        }
        let mut equilibrium = fallible_zeroed(
            self.full_system.matrix().rows(),
            "Cartesian FEM equilibrium allocation exceeds platform capacity",
        )?;
        self.full_system
            .matrix()
            .multiply_into(&values, &mut equilibrium)?;
        for (residual, rhs) in equilibrium.iter_mut().zip(self.full_system.rhs()) {
            *residual -= rhs;
        }
        let boundary_reaction_sum = equilibrium
            .iter()
            .zip(&self.fixed_values)
            .filter_map(|(reaction, fixed)| fixed.map(|_| reaction))
            .sum::<f64>();
        if !boundary_reaction_sum.is_finite() {
            return Err(invalid("Cartesian FEM reaction sum is non-finite"));
        }

        Ok(ScalarEllipticCartesianFemSolution {
            field: CartesianQ1Field::new(self.mesh, values)?,
            free_vertices: self.free_vertices,
            algebraic_values,
            canonical_system,
            boundary_reaction_sum,
            integrated_source: self.integrated_source,
            assembly_report: self.assembly_report,
            solve_report,
        })
    }
}

/// Method-private state retained between finalized TPFA assembly and field
/// reconstruction.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedCartesianFvmAssembly {
    mesh: CartesianMesh,
    cell_centers: Vec<Vec<f64>>,
    linear_system: LinearSystem,
    reconstruction_mesh: CartesianMesh,
    reconstruction_boundary_values: Vec<Option<f64>>,
    facets: Vec<CartesianFacetPacket>,
    diffusion: f64,
    facet_quadrature: QuadratureRule,
    assembly_report: AssemblyReport,
}

impl FinalizedCartesianFvmAssembly {
    pub(crate) fn into_canonical(
        self,
    ) -> Result<(Arc<CanonicalCsrSystemView>, FinalizedCartesianFvmState), Diagnostic> {
        let Self {
            mesh,
            cell_centers,
            linear_system,
            reconstruction_mesh,
            reconstruction_boundary_values,
            facets,
            diffusion,
            facet_quadrature,
            assembly_report,
        } = self;
        let canonical_system = Arc::new(CanonicalCsrSystemView::new(
            &linear_system,
            LinearOperatorProperties::SymmetricPositiveDefinite,
        )?);
        Ok((
            canonical_system,
            FinalizedCartesianFvmState {
                mesh,
                cell_centers,
                reconstruction_mesh,
                reconstruction_boundary_values,
                facets,
                diffusion,
                facet_quadrature,
                assembly_report,
            },
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedCartesianFvmState {
    mesh: CartesianMesh,
    cell_centers: Vec<Vec<f64>>,
    reconstruction_mesh: CartesianMesh,
    reconstruction_boundary_values: Vec<Option<f64>>,
    facets: Vec<CartesianFacetPacket>,
    diffusion: f64,
    facet_quadrature: QuadratureRule,
    assembly_report: AssemblyReport,
}

impl FinalizedCartesianFvmState {
    pub(crate) const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    pub(crate) fn finish(
        self,
        solved: LinearSolution,
        canonical_system: Arc<CanonicalCsrSystemView>,
    ) -> Result<ScalarEllipticCartesianFvmSolution, Diagnostic> {
        if solved.values().len() != canonical_system.rows() {
            return Err(invalid(
                "Cartesian FVM solution shape differs from its finalized system",
            ));
        }
        let (cell_values, solve_report) = solved.into_parts();
        let (boundary_flux_sum, boundary_load_sum) = self
            .facets
            .iter()
            .filter_map(|facet| match facet.kind {
                CartesianFacetKind::Interior { .. } => None,
                CartesianFacetKind::Boundary { cell, value } => Some((facet, cell, value)),
            })
            .try_fold((0.0, 0.0), |(flux, load), (facet, cell, value)| {
                let context = FluxContext {
                    facet_geometry: &facet.geometry,
                    distance: facet.distance,
                    diffusion: self.diffusion,
                };
                let transmissibility = transmissibility(&context, &self.facet_quadrature)?;
                let cell_value = cell_values.get(cell).copied().ok_or_else(|| {
                    invalid("Cartesian FVM boundary facet exceeds its finalized field")
                })?;
                Ok::<_, Diagnostic>((
                    flux + transmissibility * (value - cell_value),
                    load + transmissibility * value,
                ))
            })?;
        if !boundary_flux_sum.is_finite() {
            return Err(invalid("Cartesian FVM boundary flux sum is non-finite"));
        }
        let integrated_source =
            canonical_system.right_hand_side().iter().sum::<f64>() - boundary_load_sum;
        if !integrated_source.is_finite() {
            return Err(invalid("Cartesian FVM source integral is non-finite"));
        }
        let reconstruction = reconstruct_cell_field_from_boundary_values(
            self.reconstruction_mesh,
            &self.mesh,
            &cell_values,
            self.reconstruction_boundary_values,
        )?;

        Ok(ScalarEllipticCartesianFvmSolution {
            mesh: self.mesh,
            cell_centers: self.cell_centers,
            cell_values,
            canonical_system,
            reconstruction,
            boundary_flux_sum,
            integrated_source,
            assembly_report: self.assembly_report,
            solve_report,
        })
    }
}

/// Solve `-div(k grad(u)) = source` with continuous Cartesian Q1 FEM and
/// prescribed values on the complete box boundary.
///
/// # Errors
/// Returns a numerical diagnostic for invalid coefficients, quadrature,
/// boundary/source evaluation, mesh constraints, assembly, or solve failure.
pub fn solve_scalar_elliptic_cartesian_fem<S, B>(
    mesh: &CartesianMesh,
    diffusion: f64,
    source: &S,
    boundary: &B,
    quadrature: &QuadratureRule,
    solver: LinearSolveRequest<'_>,
) -> Result<ScalarEllipticCartesianFemSolution, Diagnostic>
where
    S: Fn(&[f64]) -> f64 + Sync + ?Sized,
    B: Fn(&[f64]) -> f64 + ?Sized,
{
    solve_scalar_elliptic_cartesian_fem_with_assembly(
        mesh,
        diffusion,
        source,
        boundary,
        quadrature,
        &REFERENCE_ASSEMBLY_BACKEND,
        solver,
    )
}

/// Lower the Cartesian Q1 diffusion cell operator to one shape-homogeneous
/// device-neutral local-action batch.
///
/// The returned inputs and outputs are packed in canonical cell order, then
/// local Q1 basis order. This is deliberately only the anonymous entity-local
/// action: global gather, constraint handling, scatter, and reduction remain
/// separate execution contracts.
///
/// # Errors
/// Returns a numerical or Operator-IR diagnostic for an invalid coefficient,
/// quadrature rule, cell geometry, local contribution, or batch shape.
pub fn lower_cartesian_q1_diffusion_local_action(
    mesh: &CartesianMesh,
    diffusion: f64,
    quadrature: &QuadratureRule,
) -> Result<LocalLinearActionIr, Diagnostic> {
    validate_problem(mesh, diffusion, quadrature)?;
    let dimension = mesh.topological_dimension();
    let cell_count = mesh.entity_count(dimension).expect("mesh owns cells");
    let local_width = HypercubeQ1Space::new(dimension)?.local_dofs().len();
    let coefficients_per_cell = local_width
        .checked_mul(local_width)
        .ok_or_else(|| invalid("Cartesian Q1 local-action shape overflows usize"))?;
    let coefficient_count = cell_count
        .checked_mul(coefficients_per_cell)
        .ok_or_else(|| invalid("Cartesian Q1 local-action batch overflows usize"))?;
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(|_| {
            invalid("Cartesian Q1 local-action batch exceeds platform allocation capacity")
        })?;
    let zero_source = |_: &[f64]| 0.0;
    let operator = CartesianEllipticCell {
        diffusion,
        source: &zero_source,
    };
    for cell_index in 0..cell_count {
        let cell = MeshEntity::new(dimension, cell_index);
        let geometry = mesh
            .geometry_map(cell)
            .expect("mesh cell has affine geometry");
        let local = operator.evaluate(&geometry, quadrature)?;
        debug_assert_eq!(local.rows(), local_width);
        debug_assert_eq!(local.columns(), local_width);
        coefficients.extend_from_slice(local.matrix());
    }
    LocalLinearActionIr::new(local_width, local_width, coefficients)
}

/// Solve Cartesian Q1 FEM through an explicit ordered assembly backend.
///
/// Each cell evaluates its local operator exactly once and maps that single
/// contribution to both the reduced solve system and full reaction system.
/// The backend may schedule cell evaluation freely, but accepted scatter order
/// and floating-point accumulation remain the canonical cell order.
///
/// # Errors
/// Preserves the reference entry point diagnostics and the selected assembly
/// backend's complete-operation diagnostics.
pub fn solve_scalar_elliptic_cartesian_fem_with_assembly<S, B>(
    mesh: &CartesianMesh,
    diffusion: f64,
    source: &S,
    boundary: &B,
    quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
    solver: LinearSolveRequest<'_>,
) -> Result<ScalarEllipticCartesianFemSolution, Diagnostic>
where
    S: Fn(&[f64]) -> f64 + Sync + ?Sized,
    B: Fn(&[f64]) -> f64 + ?Sized,
{
    let finalized = finalize_scalar_elliptic_cartesian_fem(
        mesh, diffusion, source, boundary, quadrature, assembly,
    )?;
    let (canonical_system, state) = finalized.into_canonical()?;
    let solved = solver.solve(&canonical_system.linear_problem()?)?;
    state.finish(solved, canonical_system)
}

pub(crate) fn finalize_scalar_elliptic_cartesian_fem<S, B>(
    mesh: &CartesianMesh,
    diffusion: f64,
    source: &S,
    boundary: &B,
    quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
) -> Result<FinalizedCartesianFemAssembly, Diagnostic>
where
    S: Fn(&[f64]) -> f64 + Sync + ?Sized,
    B: Fn(&[f64]) -> f64 + ?Sized,
{
    validate_problem(mesh, diffusion, quadrature)?;
    let dimension = mesh.topological_dimension();
    let vertex_count = mesh.entity_count(0).expect("mesh owns vertices");
    let mut fixed_values = Vec::with_capacity(vertex_count);
    let mut free_indices = vec![None; vertex_count];
    let mut free_count = 0_usize;
    for (vertex_index, free_index) in free_indices.iter_mut().enumerate() {
        let vertex = MeshEntity::new(0, vertex_index);
        if mesh
            .is_boundary_entity(vertex)
            .expect("mesh vertex has boundary classification")
        {
            let coordinates = mesh
                .vertex_coordinates(vertex)
                .expect("mesh vertex has geometry");
            let value = boundary(&coordinates);
            if !value.is_finite() {
                return Err(invalid("Cartesian boundary returned a non-finite value"));
            }
            fixed_values.push(Some(value));
        } else {
            fixed_values.push(None);
            *free_index = Some(DofId::new(free_count));
            free_count += 1;
        }
    }
    if free_count == 0 {
        return Err(invalid(
            "Cartesian Q1 system requires at least one unconstrained interior vertex",
        ));
    }

    let operator = CartesianEllipticCell { diffusion, source };
    let cell_count = mesh.entity_count(dimension).expect("mesh owns cells");
    let assembly_plan = AssemblyPlan::new(vec![
        AssemblyTarget::new(free_count)?,
        AssemblyTarget::new(vertex_count)?,
    ])?;
    let reduced_target = assembly_plan
        .target_id(0)
        .expect("two-target FEM assembly plan owns its reduced target");
    let full_target = assembly_plan
        .target_id(1)
        .expect("two-target FEM assembly plan owns its full target");
    let work = IndexedAssemblyWork::new(cell_count, |cell_index| {
        let cell = MeshEntity::new(dimension, cell_index);
        let geometry = mesh
            .geometry_map(cell)
            .expect("mesh cell has affine geometry");
        let local = operator.evaluate(&geometry, quadrature)?;
        let vertices = mesh
            .entity_vertices(cell)
            .expect("mesh cell has a vertex closure");
        let equations = vertices
            .iter()
            .map(|vertex| free_indices[vertex.index()])
            .collect::<Vec<_>>();
        let unknowns = vertices
            .iter()
            .map(|vertex| {
                fixed_values[vertex.index()].map_or_else(
                    || {
                        LocalUnknown::Free(
                            free_indices[vertex.index()]
                                .expect("unfixed vertex owns a free equation"),
                        )
                    },
                    LocalUnknown::Fixed,
                )
            })
            .collect::<Vec<_>>();
        let reduced = AssemblyMap::new(equations, unknowns)?;
        let full = AssemblyMap::new(
            vertices
                .iter()
                .map(|vertex| Some(DofId::new(vertex.index())))
                .collect(),
            vertices
                .iter()
                .map(|vertex| LocalUnknown::Free(DofId::new(vertex.index())))
                .collect(),
        )?;
        AssemblyPacket::new(
            local,
            vec![
                TargetAssemblyMap::new(reduced_target, reduced),
                TargetAssemblyMap::new(full_target, full),
            ],
        )
    });
    let (systems, assembly_report) = assembly.assemble(&assembly_plan, &work)?.into_parts();
    let mut systems = systems.into_iter();
    let reduced_system = systems
        .next()
        .expect("two-target FEM assembly returns its reduced system");
    let full_system = systems
        .next()
        .expect("two-target FEM assembly returns its full system");
    debug_assert!(systems.next().is_none());

    let integrated_source = full_system.rhs().iter().sum::<f64>();
    if !integrated_source.is_finite() {
        return Err(invalid("Cartesian FEM source integral is non-finite"));
    }
    let mut free_vertices = vec![0; free_count];
    for (vertex, dof) in free_indices.iter().enumerate() {
        if let Some(dof) = dof {
            free_vertices[dof.index()] = vertex;
        }
    }
    Ok(FinalizedCartesianFemAssembly {
        mesh: mesh.clone(),
        fixed_values,
        free_indices,
        free_vertices,
        linear_system: reduced_system,
        full_system,
        integrated_source,
        assembly_report,
    })
}

/// Solve `-div(k grad(u)) = source` with orthogonal cell-centered TPFA and
/// prescribed values on the complete Cartesian-box boundary.
///
/// Cell, interior-facet, and boundary-facet contributions all scatter through
/// the same [`AssemblyMap`] contract as FEM.
///
/// # Errors
/// Returns a numerical diagnostic for invalid coefficients, quadrature,
/// boundary/source evaluation, topology, assembly, or solve failure.
pub fn solve_scalar_elliptic_cartesian_fvm<S, B>(
    mesh: &CartesianMesh,
    diffusion: f64,
    source: &S,
    boundary: &B,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    solver: LinearSolveRequest<'_>,
) -> Result<ScalarEllipticCartesianFvmSolution, Diagnostic>
where
    S: Fn(&[f64]) -> f64 + Sync + ?Sized,
    B: Fn(&[f64]) -> f64 + ?Sized,
{
    solve_scalar_elliptic_cartesian_fvm_with_assembly(
        mesh,
        diffusion,
        source,
        boundary,
        cell_quadrature,
        facet_quadrature,
        &REFERENCE_ASSEMBLY_BACKEND,
        solver,
    )
}

/// Solve Cartesian cell-centred TPFA through an explicit assembly backend.
///
/// Logical packets are ordered as all source cells followed by all canonical
/// facets. Interior and boundary facets therefore share the same pure local-
/// contribution path without making topology traversal or physics metadata an
/// assembly-backend responsibility.
///
/// # Errors
/// Preserves the reference entry point diagnostics and the selected assembly
/// backend's complete-operation diagnostics.
#[allow(clippy::too_many_arguments)]
pub fn solve_scalar_elliptic_cartesian_fvm_with_assembly<S, B>(
    mesh: &CartesianMesh,
    diffusion: f64,
    source: &S,
    boundary: &B,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
    solver: LinearSolveRequest<'_>,
) -> Result<ScalarEllipticCartesianFvmSolution, Diagnostic>
where
    S: Fn(&[f64]) -> f64 + Sync + ?Sized,
    B: Fn(&[f64]) -> f64 + ?Sized,
{
    let finalized = finalize_scalar_elliptic_cartesian_fvm(
        mesh,
        diffusion,
        source,
        boundary,
        cell_quadrature,
        facet_quadrature,
        assembly,
    )?;
    let (canonical_system, state) = finalized.into_canonical()?;
    let solved = solver.solve(&canonical_system.linear_problem()?)?;
    state.finish(solved, canonical_system)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_scalar_elliptic_cartesian_fvm<S, B>(
    mesh: &CartesianMesh,
    diffusion: f64,
    source: &S,
    boundary: &B,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
) -> Result<FinalizedCartesianFvmAssembly, Diagnostic>
where
    S: Fn(&[f64]) -> f64 + Sync + ?Sized,
    B: Fn(&[f64]) -> f64 + ?Sized,
{
    validate_problem(mesh, diffusion, cell_quadrature)?;
    let dimension = mesh.topological_dimension();
    require_facet_rule(dimension, facet_quadrature)?;
    let cell_count = mesh.entity_count(dimension).expect("mesh owns cells");
    let cell_centers = (0..cell_count)
        .map(|cell_index| {
            mesh.geometry_map(MeshEntity::new(dimension, cell_index))
                .expect("mesh cell has affine geometry")
                .origin()
                .to_vec()
        })
        .collect::<Vec<_>>();

    let facet_dimension = dimension - 1;
    let facet_count = mesh
        .entity_count(facet_dimension)
        .expect("mesh owns its facet stratum");
    let facets = (0..facet_count)
        .map(|facet_index| {
            let facet = MeshEntity::new(facet_dimension, facet_index);
            let facet_geometry = mesh
                .geometry_map(facet)
                .expect("mesh facet has affine geometry");
            let free_axes = mesh
                .entity_free_axes(facet)
                .expect("mesh facet exposes its tangent axes");
            let normal_axis = (0..dimension)
                .find(|axis| free_axes.binary_search(axis).is_err())
                .ok_or_else(|| invalid("Cartesian facet has no normal axis"))?;
            let cells = mesh
                .incidence(facet, dimension)
                .ok_or_else(|| invalid("Cartesian facet adjacency is unavailable"))?;
            let (kind, distance) = match cells.as_slice() {
                [left, right] => {
                    let left_center = &cell_centers[left.entity.index()];
                    let right_center = &cell_centers[right.entity.index()];
                    let distance = (right_center[normal_axis] - left_center[normal_axis]).abs();
                    require_positive_distance(distance)?;
                    (
                        CartesianFacetKind::Interior {
                            left: left.entity.index(),
                            right: right.entity.index(),
                        },
                        distance,
                    )
                }
                [cell] => {
                    let center = &cell_centers[cell.entity.index()];
                    let boundary_coordinates = facet_geometry.origin();
                    let distance = (boundary_coordinates[normal_axis] - center[normal_axis]).abs();
                    let boundary_value = boundary(boundary_coordinates);
                    if !boundary_value.is_finite() {
                        return Err(invalid("Cartesian boundary returned a non-finite value"));
                    }
                    require_positive_distance(distance)?;
                    (
                        CartesianFacetKind::Boundary {
                            cell: cell.entity.index(),
                            value: boundary_value,
                        },
                        distance,
                    )
                }
                _ => {
                    return Err(invalid(
                        "Cartesian facet requires exactly one or two adjacent cells",
                    ));
                }
            };
            Ok(CartesianFacetPacket {
                geometry: facet_geometry,
                distance,
                kind,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let (reconstruction_mesh, reconstruction_boundary_values) =
        prepare_cell_field_reconstruction(mesh, boundary)?;

    let assembly_plan = AssemblyPlan::new(vec![AssemblyTarget::new(cell_count)?])?;
    let target = assembly_plan
        .target_id(0)
        .expect("one-target FVM assembly plan owns its system");
    let packet_count = cell_count
        .checked_add(facet_count)
        .ok_or_else(|| invalid("Cartesian FVM packet count overflows usize"))?;
    let source_operator = CartesianSourceCell { source };
    let work = IndexedAssemblyWork::new(packet_count, |packet_index| {
        let (local, map) = if packet_index < cell_count {
            let cell = MeshEntity::new(dimension, packet_index);
            let geometry = mesh
                .geometry_map(cell)
                .expect("mesh cell has affine geometry");
            let local = source_operator.evaluate(&geometry, cell_quadrature)?;
            let dof = DofId::new(packet_index);
            let map = AssemblyMap::new(vec![Some(dof)], vec![LocalUnknown::Free(dof)])?;
            (local, map)
        } else {
            let facet = &facets[packet_index - cell_count];
            let context = FluxContext {
                facet_geometry: &facet.geometry,
                distance: facet.distance,
                diffusion,
            };
            match facet.kind {
                CartesianFacetKind::Interior { left, right } => {
                    let left = DofId::new(left);
                    let right = DofId::new(right);
                    let local = CartesianInteriorFlux.evaluate(&context, facet_quadrature)?;
                    let map = AssemblyMap::new(
                        vec![Some(left), Some(right)],
                        vec![LocalUnknown::Free(left), LocalUnknown::Free(right)],
                    )?;
                    (local, map)
                }
                CartesianFacetKind::Boundary { cell, value } => {
                    let cell = DofId::new(cell);
                    let local = CartesianBoundaryFlux.evaluate(&context, facet_quadrature)?;
                    let map = AssemblyMap::new(
                        vec![Some(cell)],
                        vec![LocalUnknown::Free(cell), LocalUnknown::Fixed(value)],
                    )?;
                    (local, map)
                }
            }
        };
        AssemblyPacket::new(local, vec![TargetAssemblyMap::new(target, map)])
    });
    let (systems, assembly_report) = assembly.assemble(&assembly_plan, &work)?.into_parts();
    let mut systems = systems.into_iter();
    let system = systems
        .next()
        .expect("one-target FVM assembly returns its system");
    debug_assert!(systems.next().is_none());

    Ok(FinalizedCartesianFvmAssembly {
        mesh: mesh.clone(),
        cell_centers,
        linear_system: system,
        reconstruction_mesh,
        reconstruction_boundary_values,
        facets,
        diffusion,
        facet_quadrature: facet_quadrature.clone(),
        assembly_report,
    })
}

/// Assemble accepted-point design actions for a Cartesian Q1 FEM solve.
///
/// The resulting relation uses only free vertex unknowns. Its `R_p` includes
/// constitutive-coefficient, source, and eliminated essential-boundary
/// derivatives for explicitly selected model-Parameter and Domain-bound
/// coordinates.
///
/// # Errors
/// Returns a numerical/lowering diagnostic for a mismatched solution, invalid
/// quadrature, expression action, geometry, or non-finite assembly.
pub fn linearize_scalar_elliptic_cartesian_fem(
    model: &ScalarEllipticCartesianModel,
    mesh: &CartesianMesh,
    solution: &ScalarEllipticCartesianFemSolution,
    quadrature: &QuadratureRule,
    selected_coordinates: &[SpatialDesignCoordinate],
) -> Result<AssembledLinearizedRelation, Diagnostic> {
    validate_linearization_inputs(model, mesh, solution.field().mesh(), quadrature)?;
    let dimension = mesh.topological_dimension();
    let selected = select_design_coordinates(model, selected_coordinates)?;
    let design_dimension = selected.coordinates.len();
    let unknown_dimension = solution.algebraic_values().len();
    let mut free_indices = vec![None; mesh.entity_count(0).expect("mesh owns vertices")];
    for (dof, vertex) in solution.free_vertices().iter().copied().enumerate() {
        let Some(slot) = free_indices.get_mut(vertex) else {
            return Err(invalid(
                "Cartesian FEM solution contains an invalid free vertex",
            ));
        };
        if slot.replace(dof).is_some() {
            return Err(invalid(
                "Cartesian FEM solution contains a duplicate free vertex",
            ));
        }
    }
    let mut design_jacobian = vec![0.0; unknown_dimension * design_dimension];
    let space = HypercubeQ1Space::new(dimension)?;
    let mut parameter_tangent = vec![0.0; model.parameter_fields().len()];

    for (coordinate, action) in selected.actions.iter().copied().enumerate() {
        activate_model_parameter(action, &mut parameter_tangent);
        let (diffusion, diffusion_tangent) = model.coefficient_jvp(&parameter_tangent)?;
        for cell_index in 0..mesh.entity_count(dimension).expect("mesh owns cells") {
            let cell = MeshEntity::new(dimension, cell_index);
            let geometry = design_geometry(mesh, cell, action)?;
            let map = geometry.map();
            require_geometry_rule(map, quadrature)?;
            let inverse = map.inverse_jacobian()?;
            let inverse_tangent = geometry.inverse_jacobian_tangent()?;
            let vertices = mesh
                .entity_vertices(cell)
                .expect("mesh cell has a vertex closure");
            let fixed_tangents = vertices
                .iter()
                .map(|vertex| {
                    if free_indices[vertex.index()].is_some() {
                        Ok(0.0)
                    } else {
                        let vertex_geometry = design_geometry(mesh, *vertex, action)?;
                        let coordinates = vertex_geometry.map().origin();
                        model
                            .essential_boundary_jvp(
                                coordinates,
                                vertex_geometry.origin_tangent(),
                                &parameter_tangent,
                            )
                            .map(|(_, tangent)| tangent)
                    }
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let mut physical = vec![0.0; dimension];
            let mut physical_tangent = vec![0.0; dimension];
            for point in quadrature.points() {
                let basis = space.tabulate(&point.coordinates)?;
                geometry.map_point_jvp(&point.coordinates, &mut physical, &mut physical_tangent)?;
                let (source, source_tangent) =
                    model.source_jvp(&physical, &physical_tangent, &parameter_tangent)?;
                let scale = point.weight * map.measure_scale();
                let scale_tangent = point.weight * geometry.measure_scale_tangent();
                let physical_gradients = (0..vertices.len())
                    .map(|dof| {
                        physical_gradient(
                            basis
                                .gradient(dof)
                                .expect("tabulation owns every basis gradient"),
                            &inverse,
                            dimension,
                        )
                    })
                    .collect::<Vec<_>>();
                let physical_gradient_tangents = (0..vertices.len())
                    .map(|dof| {
                        physical_gradient(
                            basis
                                .gradient(dof)
                                .expect("tabulation owns every basis gradient"),
                            &inverse_tangent,
                            dimension,
                        )
                    })
                    .collect::<Vec<_>>();
                let accepted_values = vertices
                    .iter()
                    .map(|vertex| solution.field().vertex_values()[vertex.index()])
                    .collect::<Vec<_>>();
                let state_gradient = weighted_gradient(&accepted_values, &physical_gradients);
                let state_gradient_tangent = weighted_gradient_tangent(
                    &accepted_values,
                    &fixed_tangents,
                    &physical_gradients,
                    &physical_gradient_tangents,
                );
                for (local_row, vertex) in vertices.iter().enumerate() {
                    let Some(global_row) = free_indices[vertex.index()] else {
                        continue;
                    };
                    let gradient_product = dot(&physical_gradients[local_row], &state_gradient);
                    let energy = diffusion * gradient_product;
                    let energy_tangent = diffusion_tangent * gradient_product
                        + diffusion
                            * (dot(&physical_gradient_tangents[local_row], &state_gradient)
                                + dot(&physical_gradients[local_row], &state_gradient_tangent));
                    let load = source * basis.values()[local_row];
                    let load_tangent = source_tangent * basis.values()[local_row];
                    let residual_tangent =
                        scale_tangent * (energy - load) + scale * (energy_tangent - load_tangent);
                    design_jacobian[global_row * design_dimension + coordinate] += residual_tangent;
                }
            }
        }
        parameter_tangent.fill(0.0);
    }
    ensure_finite_design_assembly(&design_jacobian)?;
    AssembledLinearizedRelation::from_shared_canonical(
        Arc::clone(solution.canonical_system()),
        solution.algebraic_values().to_vec(),
        selected.coordinates,
        selected.values,
        design_jacobian,
    )
}

/// Assemble accepted-point design actions for a Cartesian TPFA solve.
///
/// The method-native cell unknowns and balances are retained. Constitutive,
/// source, essential-boundary, and orthogonality-preserving geometry actions
/// use the caller's exact selected coordinate order, matching the FEM relation.
///
/// # Errors
/// Returns a numerical/lowering diagnostic for a mismatched solution, invalid
/// quadrature, topology, expression action, or non-finite assembly.
pub fn linearize_scalar_elliptic_cartesian_fvm(
    model: &ScalarEllipticCartesianModel,
    mesh: &CartesianMesh,
    solution: &ScalarEllipticCartesianFvmSolution,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    selected_coordinates: &[SpatialDesignCoordinate],
) -> Result<AssembledLinearizedRelation, Diagnostic> {
    validate_linearization_inputs(model, mesh, solution.mesh(), cell_quadrature)?;
    let dimension = mesh.topological_dimension();
    require_facet_rule(dimension, facet_quadrature)?;
    let selected = select_design_coordinates(model, selected_coordinates)?;
    let design_dimension = selected.coordinates.len();
    let cell_count = mesh.entity_count(dimension).expect("mesh owns cells");
    let mut design_jacobian = vec![0.0; cell_count * design_dimension];
    let mut parameter_tangent = vec![0.0; model.parameter_fields().len()];

    for (coordinate, action) in selected.actions.iter().copied().enumerate() {
        activate_model_parameter(action, &mut parameter_tangent);
        let (diffusion, diffusion_tangent) = model.coefficient_jvp(&parameter_tangent)?;
        for cell_index in 0..cell_count {
            let geometry = design_geometry(mesh, MeshEntity::new(dimension, cell_index), action)?;
            let map = geometry.map();
            require_geometry_rule(map, cell_quadrature)?;
            let mut physical = vec![0.0; dimension];
            let mut physical_tangent = vec![0.0; dimension];
            let mut source_tangent_integral = 0.0;
            for point in cell_quadrature.points() {
                geometry.map_point_jvp(&point.coordinates, &mut physical, &mut physical_tangent)?;
                let (source, source_tangent) =
                    model.source_jvp(&physical, &physical_tangent, &parameter_tangent)?;
                source_tangent_integral += point.weight
                    * (geometry.measure_scale_tangent() * source
                        + map.measure_scale() * source_tangent);
            }
            design_jacobian[cell_index * design_dimension + coordinate] -= source_tangent_integral;
        }

        let facet_dimension = dimension - 1;
        for facet_index in 0..mesh
            .entity_count(facet_dimension)
            .expect("mesh owns facets")
        {
            let facet = MeshEntity::new(facet_dimension, facet_index);
            let geometry = design_geometry(mesh, facet, action)?;
            let map = geometry.map();
            let free_axes = mesh
                .entity_free_axes(facet)
                .expect("mesh facet exposes its tangent axes");
            let normal_axis = (0..dimension)
                .find(|axis| free_axes.binary_search(axis).is_err())
                .ok_or_else(|| invalid("Cartesian facet has no normal axis"))?;
            let cells = mesh
                .incidence(facet, dimension)
                .ok_or_else(|| invalid("Cartesian facet adjacency is unavailable"))?;
            let (area, area_tangent) = facet_measure_jvp(&geometry, facet_quadrature)?;
            match cells.as_slice() {
                [left, right] => {
                    let left_index = left.entity.index();
                    let right_index = right.entity.index();
                    let left_geometry = design_geometry(mesh, left.entity, action)?;
                    let right_geometry = design_geometry(mesh, right.entity, action)?;
                    let delta = solution.cell_centers()[right_index][normal_axis]
                        - solution.cell_centers()[left_index][normal_axis];
                    let distance = delta.abs();
                    require_positive_distance(distance)?;
                    let distance_tangent = delta.signum()
                        * (right_geometry.origin_tangent()[normal_axis]
                            - left_geometry.origin_tangent()[normal_axis]);
                    let transmissibility_tangent = transmissibility_jvp(
                        diffusion,
                        diffusion_tangent,
                        area,
                        area_tangent,
                        distance,
                        distance_tangent,
                    )?;
                    let jump =
                        solution.cell_values()[left_index] - solution.cell_values()[right_index];
                    let contribution = transmissibility_tangent * jump;
                    design_jacobian[left_index * design_dimension + coordinate] += contribution;
                    design_jacobian[right_index * design_dimension + coordinate] -= contribution;
                }
                [cell] => {
                    let cell_index = cell.entity.index();
                    let cell_geometry = design_geometry(mesh, cell.entity, action)?;
                    let boundary_coordinates = map.origin();
                    let delta = boundary_coordinates[normal_axis]
                        - solution.cell_centers()[cell_index][normal_axis];
                    let distance = delta.abs();
                    require_positive_distance(distance)?;
                    let distance_tangent = delta.signum()
                        * (geometry.origin_tangent()[normal_axis]
                            - cell_geometry.origin_tangent()[normal_axis]);
                    let (boundary, boundary_tangent) = model.essential_boundary_jvp(
                        boundary_coordinates,
                        geometry.origin_tangent(),
                        &parameter_tangent,
                    )?;
                    let transmissibility = diffusion * area / distance;
                    let transmissibility_tangent = transmissibility_jvp(
                        diffusion,
                        diffusion_tangent,
                        area,
                        area_tangent,
                        distance,
                        distance_tangent,
                    )?;
                    design_jacobian[cell_index * design_dimension + coordinate] +=
                        transmissibility_tangent * (solution.cell_values()[cell_index] - boundary)
                            - transmissibility * boundary_tangent;
                }
                _ => {
                    return Err(invalid(
                        "Cartesian facet requires exactly one or two adjacent cells",
                    ));
                }
            }
        }
        parameter_tangent.fill(0.0);
    }
    ensure_finite_design_assembly(&design_jacobian)?;
    AssembledLinearizedRelation::from_shared_canonical(
        Arc::clone(solution.canonical_system()),
        solution.cell_values().to_vec(),
        selected.coordinates,
        selected.values,
        design_jacobian,
    )
}

/// Linearize the complete Q1 Field published by one accepted FEM solution.
///
/// Free vertices project their algebraic tangent directly. Eliminated
/// essential-boundary vertices retain the direct Parameter/geometry action of
/// their canonical trace expression, so the output remains the full semantic
/// Field rather than the shorter method-native unknown vector.
///
/// # Errors
/// Returns a numerical/lowering diagnostic for a mismatched solution,
/// unsupported design coordinate, boundary action, or non-finite projection.
pub fn linearize_scalar_elliptic_cartesian_fem_output(
    model: &ScalarEllipticCartesianModel,
    mesh: &CartesianMesh,
    solution: &ScalarEllipticCartesianFemSolution,
    selected_coordinates: &[SpatialDesignCoordinate],
) -> Result<CartesianScalarFieldLinearization, Diagnostic> {
    if solution.field().mesh() != mesh {
        return Err(invalid(
            "Cartesian FEM output linearization requires the accepted solution mesh",
        ));
    }
    let selected = select_design_coordinates(model, selected_coordinates)?;
    let output_dimension = solution.field().vertex_values().len();
    let parameter_dimension = selected.coordinates.len();
    let unknown_dimension = solution.algebraic_values().len();
    let mut output_unknowns = vec![None; output_dimension];
    for (dof, &vertex) in solution.free_vertices().iter().enumerate() {
        let Some(slot) = output_unknowns.get_mut(vertex) else {
            return Err(invalid(
                "Cartesian FEM output contains an invalid free vertex",
            ));
        };
        if slot.replace(dof).is_some() {
            return Err(invalid(
                "Cartesian FEM output contains a duplicate free vertex",
            ));
        }
    }

    let mut direct_parameter_jacobian = vec![0.0; output_dimension * parameter_dimension];
    let mut parameter_tangent = vec![0.0; model.parameter_fields().len()];
    for (column, action) in selected.actions.iter().copied().enumerate() {
        activate_model_parameter(action, &mut parameter_tangent);
        for (vertex, unknown) in output_unknowns.iter().enumerate() {
            if unknown.is_some() {
                continue;
            }
            let geometry = design_geometry(mesh, MeshEntity::new(0, vertex), action)?;
            let (_, tangent) = model.essential_boundary_jvp(
                geometry.map().origin(),
                geometry.origin_tangent(),
                &parameter_tangent,
            )?;
            direct_parameter_jacobian[vertex * parameter_dimension + column] = tangent;
        }
        parameter_tangent.fill(0.0);
    }
    CartesianScalarFieldLinearization::new(
        solution.field().vertex_values().to_vec(),
        unknown_dimension,
        parameter_dimension,
        output_unknowns,
        direct_parameter_jacobian,
    )
}

/// Linearize the complete cell-centred Field published by one accepted FVM
/// solution.
///
/// The current TPFA primary output is exactly its algebraic cell vector; its
/// geometry and Parameter dependence therefore arrives through the implicit
/// state action, with no direct `O_p` term.
///
/// # Errors
/// Returns a numerical/lowering diagnostic for a mismatched solution or
/// unsupported design coordinate.
pub fn linearize_scalar_elliptic_cartesian_fvm_output(
    model: &ScalarEllipticCartesianModel,
    mesh: &CartesianMesh,
    solution: &ScalarEllipticCartesianFvmSolution,
    selected_coordinates: &[SpatialDesignCoordinate],
) -> Result<CartesianScalarFieldLinearization, Diagnostic> {
    if solution.mesh() != mesh {
        return Err(invalid(
            "Cartesian FVM output linearization requires the accepted solution mesh",
        ));
    }
    let selected = select_design_coordinates(model, selected_coordinates)?;
    let output_dimension = solution.cell_values().len();
    let parameter_dimension = selected.coordinates.len();
    CartesianScalarFieldLinearization::new(
        solution.cell_values().to_vec(),
        output_dimension,
        parameter_dimension,
        (0..output_dimension).map(Some).collect(),
        vec![0.0; output_dimension * parameter_dimension],
    )
}

fn validate_linearization_inputs(
    model: &ScalarEllipticCartesianModel,
    mesh: &CartesianMesh,
    solution_mesh: &CartesianMesh,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    let bounds_match = (0..model.dimension()).all(|axis| {
        mesh.axis_bounds(axis).is_some_and(|bounds| {
            bounds[0].to_bits() == model.bounds()[axis][0].to_bits()
                && bounds[1].to_bits() == model.bounds()[axis][1].to_bits()
        })
    });
    if mesh != solution_mesh || mesh.topological_dimension() != model.dimension() || !bounds_match {
        return Err(invalid(
            "Cartesian linearization requires the exact primal mesh, model dimension, and Domain bounds",
        ));
    }
    validate_problem(mesh, model.coefficient(), quadrature)
}

struct SelectedDesignCoordinates {
    coordinates: Vec<crate::SpatialDesignCoordinate>,
    values: Vec<f64>,
    actions: Vec<SpatialDesignAction>,
}

#[derive(Debug, Clone, Copy)]
enum SpatialDesignAction {
    ModelParameter(usize),
    CartesianBound {
        axis: usize,
        side: eqiora_schema::kernel::BoundarySide,
    },
}

fn select_design_coordinates(
    model: &ScalarEllipticCartesianModel,
    selected: &[SpatialDesignCoordinate],
) -> Result<SelectedDesignCoordinates, Diagnostic> {
    if selected.is_empty() {
        return Err(invalid(
            "Cartesian differentiation requires at least one explicitly selected design coordinate",
        ));
    }
    if selected
        .iter()
        .enumerate()
        .any(|(index, field)| selected[..index].contains(field))
    {
        return Err(invalid(
            "Cartesian differentiation design selection contains a duplicate",
        ));
    }
    let mut values = Vec::with_capacity(selected.len());
    let mut actions = Vec::with_capacity(selected.len());
    for coordinate in selected {
        match *coordinate {
            SpatialDesignCoordinate::ModelParameter(field) => {
                let Some(index) = model
                    .parameter_fields()
                    .iter()
                    .position(|candidate| candidate == &field)
                else {
                    return Err(invalid(
                        "selected model Parameter does not affect the lowered Cartesian relation",
                    ));
                };
                values.push(model.parameter_values()[index]);
                actions.push(SpatialDesignAction::ModelParameter(index));
            }
            SpatialDesignCoordinate::CartesianBound { domain, axis, side } => {
                if domain != model.domain_id() || axis >= model.dimension() {
                    return Err(invalid(
                        "selected Cartesian bound does not belong to the lowered Domain",
                    ));
                }
                let bound = match side {
                    eqiora_schema::kernel::BoundarySide::Lower => model.bounds()[axis][0],
                    eqiora_schema::kernel::BoundarySide::Upper => model.bounds()[axis][1],
                };
                values.push(bound);
                actions.push(SpatialDesignAction::CartesianBound { axis, side });
            }
        }
    }
    Ok(SelectedDesignCoordinates {
        coordinates: selected.to_vec(),
        values,
        actions,
    })
}

fn activate_model_parameter(action: SpatialDesignAction, tangent: &mut [f64]) {
    if let SpatialDesignAction::ModelParameter(index) = action {
        tangent[index] = 1.0;
    }
}

fn design_geometry(
    mesh: &CartesianMesh,
    entity: MeshEntity,
    action: SpatialDesignAction,
) -> Result<AffineGeometryLinearization, Diagnostic> {
    match action {
        SpatialDesignAction::ModelParameter(_) => AffineGeometryLinearization::stationary(
            mesh.geometry_map(entity)
                .ok_or_else(|| invalid("Cartesian entity geometry is unavailable"))?,
        ),
        SpatialDesignAction::CartesianBound { axis, side } => {
            mesh.linearize_box_bound(entity, axis, side)
        }
    }
}

fn facet_measure_jvp(
    geometry: &AffineGeometryLinearization,
    quadrature: &QuadratureRule,
) -> Result<(f64, f64), Diagnostic> {
    require_geometry_rule(geometry.map(), quadrature)?;
    let area = quadrature
        .points()
        .iter()
        .map(|point| point.weight * geometry.map().measure_scale())
        .sum::<f64>();
    let tangent = quadrature
        .points()
        .iter()
        .map(|point| point.weight * geometry.measure_scale_tangent())
        .sum::<f64>();
    if !area.is_finite() || area <= 0.0 || !tangent.is_finite() {
        return Err(invalid(
            "Cartesian facet measure/JVP must be finite with positive primal measure",
        ));
    }
    Ok((area, tangent))
}

fn transmissibility_jvp(
    diffusion: f64,
    diffusion_tangent: f64,
    area: f64,
    area_tangent: f64,
    distance: f64,
    distance_tangent: f64,
) -> Result<f64, Diagnostic> {
    let tangent = (diffusion_tangent * area + diffusion * area_tangent) / distance
        - diffusion * area * distance_tangent / distance.powi(2);
    if !tangent.is_finite() {
        return Err(invalid(
            "Cartesian transmissibility JVP produced a non-finite value",
        ));
    }
    Ok(tangent)
}

fn require_positive_distance(distance: f64) -> Result<(), Diagnostic> {
    if !distance.is_finite() || distance <= 0.0 {
        Err(invalid(
            "Cartesian two-point flux requires a positive finite normal distance",
        ))
    } else {
        Ok(())
    }
}

fn ensure_finite_design_assembly(values: &[f64]) -> Result<(), Diagnostic> {
    if values.iter().any(|value| !value.is_finite()) {
        Err(invalid(
            "Cartesian design derivative assembly produced a non-finite value",
        ))
    } else {
        Ok(())
    }
}

struct CartesianEllipticCell<'a, S: ?Sized> {
    diffusion: f64,
    source: &'a S,
}

impl<S> LocalOperator<AffineGeometryMap> for CartesianEllipticCell<'_, S>
where
    S: Fn(&[f64]) -> f64 + ?Sized,
{
    fn evaluate(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        require_geometry_rule(geometry, quadrature)?;
        let dimension = geometry.reference_cell().dimension();
        if geometry.physical_dimension() != dimension {
            return Err(invalid(
                "Cartesian elliptic cell requires matching physical and reference dimensions",
            ));
        }
        let inverse = geometry.inverse_jacobian()?;
        let space = HypercubeQ1Space::new(dimension)?;
        let dof_count = space.local_dofs().len();
        let mut matrix = vec![0.0; dof_count * dof_count];
        let mut rhs = vec![0.0; dof_count];
        let mut physical = vec![0.0; dimension];
        for point in quadrature.points() {
            let basis = space.tabulate(&point.coordinates)?;
            geometry.map_point(&point.coordinates, &mut physical)?;
            let source = (self.source)(&physical);
            if !source.is_finite() {
                return Err(invalid("Cartesian source returned a non-finite value"));
            }
            let scale = point.weight * geometry.measure_scale();
            let physical_gradients = (0..dof_count)
                .map(|dof| {
                    physical_gradient(
                        basis
                            .gradient(dof)
                            .expect("tabulation owns every basis gradient"),
                        &inverse,
                        dimension,
                    )
                })
                .collect::<Vec<_>>();
            for row in 0..dof_count {
                rhs[row] += scale * source * basis.values()[row];
                for column in 0..dof_count {
                    matrix[row * dof_count + column] += scale
                        * self.diffusion
                        * physical_gradients[row]
                            .iter()
                            .zip(&physical_gradients[column])
                            .map(|(left, right)| left * right)
                            .sum::<f64>();
                }
            }
        }
        LocalContribution::new(dof_count, dof_count, matrix, rhs)
    }
}

struct CartesianSourceCell<'a, S: ?Sized> {
    source: &'a S,
}

impl<S> LocalOperator<AffineGeometryMap> for CartesianSourceCell<'_, S>
where
    S: Fn(&[f64]) -> f64 + ?Sized,
{
    fn evaluate(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        require_geometry_rule(geometry, quadrature)?;
        let mut physical = vec![0.0; geometry.physical_dimension()];
        let mut integral = 0.0;
        for point in quadrature.points() {
            geometry.map_point(&point.coordinates, &mut physical)?;
            let source = (self.source)(&physical);
            if !source.is_finite() {
                return Err(invalid("Cartesian source returned a non-finite value"));
            }
            integral += point.weight * geometry.measure_scale() * source;
        }
        LocalContribution::new(1, 1, vec![0.0], vec![integral])
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CartesianFacetPacket {
    geometry: AffineGeometryMap,
    distance: f64,
    kind: CartesianFacetKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CartesianFacetKind {
    Interior { left: usize, right: usize },
    Boundary { cell: usize, value: f64 },
}

struct FluxContext<'a> {
    facet_geometry: &'a AffineGeometryMap,
    distance: f64,
    diffusion: f64,
}

struct CartesianInteriorFlux;

impl LocalOperator<FluxContext<'_>> for CartesianInteriorFlux {
    fn evaluate(
        &self,
        context: &FluxContext<'_>,
        quadrature: &QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        let transmissibility = transmissibility(context, quadrature)?;
        LocalContribution::new(
            2,
            2,
            vec![
                transmissibility,
                -transmissibility,
                -transmissibility,
                transmissibility,
            ],
            vec![0.0, 0.0],
        )
    }
}

struct CartesianBoundaryFlux;

impl LocalOperator<FluxContext<'_>> for CartesianBoundaryFlux {
    fn evaluate(
        &self,
        context: &FluxContext<'_>,
        quadrature: &QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        let transmissibility = transmissibility(context, quadrature)?;
        LocalContribution::new(1, 2, vec![transmissibility, -transmissibility], vec![0.0])
    }
}

fn transmissibility(
    context: &FluxContext<'_>,
    quadrature: &QuadratureRule,
) -> Result<f64, Diagnostic> {
    require_geometry_rule(context.facet_geometry, quadrature)?;
    if !context.distance.is_finite() || context.distance <= 0.0 {
        return Err(invalid(
            "Cartesian two-point flux requires a positive finite normal distance",
        ));
    }
    let area = quadrature
        .points()
        .iter()
        .map(|point| point.weight * context.facet_geometry.measure_scale())
        .sum::<f64>();
    let transmissibility = context.diffusion * area / context.distance;
    if !transmissibility.is_finite() || transmissibility <= 0.0 {
        return Err(invalid(
            "Cartesian two-point transmissibility must be finite and positive",
        ));
    }
    Ok(transmissibility)
}

fn prepare_cell_field_reconstruction<B>(
    mesh: &CartesianMesh,
    boundary: &B,
) -> Result<(CartesianMesh, Vec<Option<f64>>), Diagnostic>
where
    B: Fn(&[f64]) -> f64 + ?Sized,
{
    let dimension = mesh.topological_dimension();
    let axes = (0..dimension)
        .map(|axis| {
            let coordinates = mesh
                .axis_coordinates(axis)
                .expect("mesh owns every physical axis");
            let mut dual = Vec::with_capacity(coordinates.len() + 1);
            dual.push(coordinates[0]);
            dual.extend(
                coordinates
                    .windows(2)
                    .map(|pair| pair[0] + 0.5 * (pair[1] - pair[0])),
            );
            dual.push(coordinates[coordinates.len() - 1]);
            dual
        })
        .collect::<Vec<_>>();
    let reconstruction_mesh = CartesianMesh::from_axes(axes)?;
    let vertex_count = reconstruction_mesh
        .entity_count(0)
        .expect("reconstruction mesh owns vertices");
    let mut boundary_values = Vec::with_capacity(vertex_count);
    for vertex_index in 0..vertex_count {
        let vertex = MeshEntity::new(0, vertex_index);
        if reconstruction_mesh
            .is_boundary_entity(vertex)
            .expect("reconstruction vertex has boundary classification")
        {
            let coordinates = reconstruction_mesh
                .vertex_coordinates(vertex)
                .expect("reconstruction vertex has geometry");
            let value = boundary(&coordinates);
            if !value.is_finite() {
                return Err(invalid("Cartesian boundary returned a non-finite value"));
            }
            boundary_values.push(Some(value));
        } else {
            boundary_values.push(None);
        }
    }
    Ok((reconstruction_mesh, boundary_values))
}

fn reconstruct_cell_field_from_boundary_values(
    reconstruction_mesh: CartesianMesh,
    source_mesh: &CartesianMesh,
    cell_values: &[f64],
    boundary_values: Vec<Option<f64>>,
) -> Result<CartesianQ1Field, Diagnostic> {
    let dimension = source_mesh.topological_dimension();
    let reconstruction_vertex_count = reconstruction_mesh
        .entity_count(0)
        .ok_or_else(|| invalid("Cartesian reconstruction mesh has no vertex stratum"))?;
    if Some(cell_values.len()) != source_mesh.entity_count(dimension)
        || boundary_values.len() != reconstruction_vertex_count
    {
        return Err(invalid(
            "Cartesian reconstruction state differs from its finalized system",
        ));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(boundary_values.len())
        .map_err(|_| {
            finish_allocation("Cartesian FVM reconstruction allocation exceeds platform capacity")
        })?;
    for (vertex_index, boundary_value) in boundary_values.into_iter().enumerate() {
        if let Some(value) = boundary_value {
            values.push(value);
            continue;
        }
        let vertex = MeshEntity::new(0, vertex_index);
        let source_indices = reconstruction_mesh
            .vertex_multi_index(vertex)
            .ok_or_else(|| invalid("Cartesian reconstruction vertex has no multi-index"))?;
        let source_cell_index = source_indices.iter().enumerate().try_fold(
            0_usize,
            |linear, (axis, &dual_index)| {
                let cell_count = source_mesh.axis_cell_count(axis).ok_or_else(|| {
                    invalid("Cartesian reconstruction source axis is unavailable")
                })?;
                let source_index = dual_index.checked_sub(1).ok_or_else(|| {
                    invalid("Cartesian reconstruction interior index is on the boundary")
                })?;
                if source_index >= cell_count {
                    return Err(invalid(
                        "Cartesian reconstruction interior index exceeds its source axis",
                    ));
                }
                linear
                    .checked_mul(cell_count)
                    .and_then(|linear| linear.checked_add(source_index))
                    .ok_or_else(|| invalid("Cartesian reconstruction cell index overflows usize"))
            },
        )?;
        let value = cell_values.get(source_cell_index).copied().ok_or_else(|| {
            invalid("Cartesian reconstruction cell index exceeds its finalized field")
        })?;
        values.push(value);
    }
    CartesianQ1Field::new(reconstruction_mesh, values)
}

fn fallible_zeroed(length: usize, allocation_error: &'static str) -> Result<Vec<f64>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| finish_allocation(allocation_error))?;
    values.resize(length, 0.0);
    Ok(values)
}

fn finish_allocation(message: &'static str) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}

fn validate_problem(
    mesh: &CartesianMesh,
    diffusion: f64,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    if !diffusion.is_finite() || diffusion <= 0.0 {
        return Err(invalid(
            "Cartesian scalar diffusion coefficient must be finite and positive",
        ));
    }
    require_cell_rule(mesh, quadrature)
}

fn require_cell_rule(mesh: &CartesianMesh, quadrature: &QuadratureRule) -> Result<(), Diagnostic> {
    let expected = ReferenceCell::hypercube(mesh.topological_dimension())?;
    if quadrature.reference_cell() != expected {
        return Err(invalid(format!(
            "Cartesian cell requires {:?} quadrature, received {:?}",
            expected,
            quadrature.reference_cell()
        )));
    }
    Ok(())
}

fn require_facet_rule(dimension: usize, quadrature: &QuadratureRule) -> Result<(), Diagnostic> {
    let expected = if dimension == 1 {
        ReferenceCell::point()
    } else {
        ReferenceCell::hypercube(dimension - 1)?
    };
    if quadrature.reference_cell() != expected {
        return Err(invalid(format!(
            "Cartesian facet requires {:?} quadrature, received {:?}",
            expected,
            quadrature.reference_cell()
        )));
    }
    Ok(())
}

fn require_geometry_rule(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    if geometry.reference_cell() != quadrature.reference_cell() {
        return Err(invalid(
            "Cartesian local geometry and quadrature reference cells differ",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use eqiora_solver::{LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};

    use super::*;

    #[test]
    fn method_native_capacity_failure_is_a_stable_diagnostic() {
        let diagnostic = fallible_zeroed(
            usize::MAX,
            "Cartesian finish allocation exceeds platform capacity",
        )
        .unwrap_err();

        assert_eq!(diagnostic.code(), codes::NUMERICAL_SOLVE_FAILED);
        assert_eq!(
            diagnostic.message(),
            "Cartesian finish allocation exceeds platform capacity"
        );
    }

    #[test]
    fn q1_diffusion_lowers_to_anonymous_uniform_local_action() {
        let mesh = CartesianMesh::from_axes(vec![vec![0.0, 0.2, 1.0], vec![-1.0, 0.5]]).unwrap();
        let rule = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
        let action = lower_cartesian_q1_diffusion_local_action(&mesh, 1.7, &rule).unwrap();
        let input = vec![1.0; action.input_len()];
        let mut output = vec![f64::NAN; action.output_len()];

        action.apply_reference(&input, &mut output).unwrap();

        assert_eq!(action.entity_count(), 2);
        assert_eq!((action.rows(), action.columns()), (4, 4));
        assert!(output.iter().all(|value| value.abs() < 8.0e-15));
    }

    #[test]
    fn both_methods_reproduce_a_linear_harmonic_field_on_a_nonuniform_grid() {
        let mesh =
            CartesianMesh::from_axes(vec![vec![0.0, 0.2, 0.65, 1.0], vec![-1.0, -0.1, 0.4, 2.0]])
                .unwrap();
        let cell_rule = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
        let facet_rule = QuadratureRule::gauss_legendre(2).unwrap();
        let plan = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(256).unwrap(),
        )
        .unwrap();
        let solver = LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, plan);
        let exact = |coordinate: &[f64]| 2.0 + coordinate[0] - 0.5 * coordinate[1];
        let source = |_: &[f64]| 0.0;

        let fem =
            solve_scalar_elliptic_cartesian_fem(&mesh, 1.0, &source, &exact, &cell_rule, solver)
                .unwrap();
        let fvm = solve_scalar_elliptic_cartesian_fvm(
            &mesh,
            1.0,
            &source,
            &exact,
            &cell_rule,
            &facet_rule,
            solver,
        )
        .unwrap();

        assert!(fem.field().l2_error(&exact, &cell_rule).unwrap() < 2.0e-13);
        let dual_rule = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
        assert!(fvm.reconstruction().l2_error(&exact, &dual_rule).unwrap() < 2.0e-13);
        assert!(fem.boundary_reaction_sum().abs() < 2.0e-12);
        assert!(fvm.boundary_flux_sum().abs() < 2.0e-12);
        assert!((fem.boundary_reaction_sum() + fem.integrated_source()).abs() < 2.0e-12);
        assert!((fvm.boundary_flux_sum() + fvm.integrated_source()).abs() < 2.0e-12);
    }
}
