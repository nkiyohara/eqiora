//! Two-dimensional Cartesian Q1 realization of isotropic linear elasticity.
//!
//! This module owns one deliberately narrow numerical realization. Material
//! coefficients and the conservative load potential arrive from canonical
//! lowering; mesh, quadrature, assembly, and solve policy remain explicit
//! realization inputs.

use std::num::NonZeroU32;
use std::sync::Arc;

use eqiora_assembly::{
    AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPlan, AssemblyReport, AssemblyTarget,
    DofId, IndexedAssemblyWork, LocalContribution, LocalUnknown, REFERENCE_ASSEMBLY_BACKEND,
    TargetAssemblyMap,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_ir::LocalLinearActionIr;
use eqiora_meshing::{
    AffineGeometryMap, GeometryMap, MeshEntity, MeshGeometry, MeshTopology, QuadratureRule,
    ReferenceCell,
};
use eqiora_meshing::{DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape};
use eqiora_solver::{
    CanonicalCsrSystemView, LinearOperatorProperties, LinearSolution, LinearSolveRequest,
    SolveReport,
};

use crate::affine_fem::physical_gradient;
use crate::cartesian_mesh::CartesianMesh;
use crate::discrete_space::{DiscreteSpace, HypercubeQ1Space};
use crate::operator::LocalOperator;
use crate::spatial_expression::ScalarSpatialExpression;

const DIMENSION: usize = 2;
const COMPONENTS: usize = 2;

mod pair;

pub use pair::{
    ConformingCartesianInterfaceMap2d, ConformingCartesianLinearElasticityPair2dSolution,
    ConformingElasticityInterfaceAction2d,
};
pub(crate) use pair::{
    FinalizedConformingCartesianElasticityPair2dAssembly,
    FinalizedConformingCartesianElasticityPair2dState,
    finalize_conforming_cartesian_q1_linear_elasticity_pair_2d,
};

/// Realization-private selection of complete homogeneous essential sides.
///
/// The Semantic Model owns boundary meaning. This compact value is the only
/// boundary information admitted by the Cartesian Q1 assembler after
/// canonical normalization and Realization validation have completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CartesianEssentialSides2d {
    sides: [[bool; 2]; DIMENSION],
}

impl CartesianEssentialSides2d {
    pub(crate) const fn new(sides: [[bool; 2]; DIMENSION]) -> Self {
        Self { sides }
    }

    const fn complete_boundary() -> Self {
        Self::new([[true; 2]; DIMENSION])
    }

    pub(crate) fn has_essential_side(self) -> bool {
        self.sides.iter().flatten().copied().any(|value| value)
    }

    fn constrains_vertex(self, mesh: &CartesianMesh, vertex: MeshEntity) -> bool {
        let index = mesh
            .vertex_multi_index(vertex)
            .expect("a Cartesian vertex owns one index per physical axis");
        (0..DIMENSION).any(|axis| {
            (self.sides[axis][0] && index[axis] == 0)
                || (self.sides[axis][1]
                    && index[axis]
                        == mesh
                            .axis_coordinates(axis)
                            .expect("a two-dimensional mesh owns both axes")
                            .len()
                            - 1)
        })
    }
}

/// Continuous two-component Q1 field on one two-dimensional Cartesian mesh.
///
/// Values use the repository-wide discrete-field order: mesh vertex first,
/// Cartesian component second. Thus component `c` of vertex `v` occupies
/// `2 * v + c`.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianQ1VectorField2d {
    mesh: CartesianMesh,
    payload: DiscreteFieldPayload,
}

impl CartesianQ1VectorField2d {
    /// Bind finite entity-major values to a two-dimensional Cartesian mesh.
    ///
    /// # Errors
    /// Returns `EQ0801` unless the mesh is exactly two-dimensional, or the
    /// discrete-field contract rejects the value shape or data.
    pub fn new(mesh: CartesianMesh, values: Vec<f64>) -> Result<Self, Diagnostic> {
        require_two_dimensional_mesh(&mesh)?;
        let payload = DiscreteFieldPayload::new(
            &mesh,
            DiscreteFieldAssociation::Vertex,
            DiscreteFieldShape::Vector {
                components: NonZeroU32::new(COMPONENTS as u32)
                    .expect("the two Cartesian components are nonzero"),
            },
            values,
        )?;
        Ok(Self { mesh, payload })
    }

    /// Mesh carrying the nodal field.
    #[must_use]
    pub const fn mesh(&self) -> &CartesianMesh {
        &self.mesh
    }

    /// Checked entity-major discrete values.
    #[must_use]
    pub const fn payload(&self) -> &DiscreteFieldPayload {
        &self.payload
    }

    /// Flat vertex-major, component-minor values.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        self.payload.values()
    }

    /// Two Cartesian values at one mesh vertex.
    #[must_use]
    pub fn vertex_values(&self, vertex: usize) -> Option<&[f64]> {
        self.payload.entity_values(vertex)
    }

    /// Integrate continuous L2 and H1-seminorm errors in one pass.
    ///
    /// `exact` returns the physical vector and its row-major gradient
    /// `[component][physical_axis]` at one physical point.
    ///
    /// # Errors
    /// Returns a discretization diagnostic for incompatible quadrature,
    /// geometry, or non-finite exact/error data.
    pub fn error_norms<E>(
        &self,
        exact: &E,
        quadrature: &QuadratureRule,
    ) -> Result<CartesianVectorErrorNorms, Diagnostic>
    where
        E: Fn(&[f64]) -> ([f64; COMPONENTS], [[f64; DIMENSION]; COMPONENTS]) + ?Sized,
    {
        require_cell_rule(&self.mesh, quadrature)?;
        let space = HypercubeQ1Space::new(DIMENSION)?;
        let cell_count = self
            .mesh
            .entity_count(DIMENSION)
            .expect("a two-dimensional mesh owns its cell stratum");
        let mut l2_squared = 0.0;
        let mut h1_seminorm_squared = 0.0;
        let mut physical = [0.0; DIMENSION];

        for cell_index in 0..cell_count {
            let cell = MeshEntity::new(DIMENSION, cell_index);
            let geometry = self
                .mesh
                .geometry_map(cell)
                .expect("a Cartesian cell owns affine geometry");
            let inverse = geometry.inverse_jacobian()?;
            let vertices = self
                .mesh
                .entity_vertices(cell)
                .expect("a Cartesian cell owns its vertex closure");
            for point in quadrature.points() {
                let basis = space.tabulate(&point.coordinates)?;
                geometry.map_point(&point.coordinates, &mut physical)?;
                let gradients = physical_gradients(&basis, &inverse);
                let mut value = [0.0; COMPONENTS];
                let mut gradient = [[0.0; DIMENSION]; COMPONENTS];
                for (local_vertex, vertex) in vertices.iter().enumerate() {
                    let nodal = self
                        .vertex_values(vertex.index())
                        .expect("the accepted field owns every mesh vertex");
                    for component in 0..COMPONENTS {
                        value[component] += basis.values()[local_vertex] * nodal[component];
                        for axis in 0..DIMENSION {
                            gradient[component][axis] +=
                                gradients[local_vertex][axis] * nodal[component];
                        }
                    }
                }
                let (exact_value, exact_gradient) = exact(&physical);
                if exact_value
                    .iter()
                    .chain(exact_gradient.iter().flatten())
                    .any(|value| !value.is_finite())
                {
                    return Err(invalid("exact vector field returned a non-finite value"));
                }
                let scale = point.weight * geometry.measure_scale();
                l2_squared += scale
                    * value
                        .iter()
                        .zip(exact_value)
                        .map(|(actual, exact)| (actual - exact).powi(2))
                        .sum::<f64>();
                h1_seminorm_squared += scale
                    * gradient
                        .iter()
                        .flatten()
                        .zip(exact_gradient.iter().flatten())
                        .map(|(actual, exact)| (actual - exact).powi(2))
                        .sum::<f64>();
            }
        }
        CartesianVectorErrorNorms::new(l2_squared, h1_seminorm_squared)
    }
}

/// Continuous vector-field error norms from one explicit quadrature rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianVectorErrorNorms {
    l2: f64,
    h1_seminorm: f64,
}

impl CartesianVectorErrorNorms {
    fn new(l2_squared: f64, h1_seminorm_squared: f64) -> Result<Self, Diagnostic> {
        if !l2_squared.is_finite()
            || !h1_seminorm_squared.is_finite()
            || l2_squared < 0.0
            || h1_seminorm_squared < 0.0
        {
            return Err(invalid(
                "Cartesian vector error accumulation is non-finite or negative",
            ));
        }
        Ok(Self {
            l2: l2_squared.sqrt(),
            h1_seminorm: h1_seminorm_squared.sqrt(),
        })
    }

    /// Continuous vector L2 error.
    #[must_use]
    pub const fn l2(self) -> f64 {
        self.l2
    }

    /// Frobenius-gradient H1 seminorm error.
    #[must_use]
    pub const fn h1_seminorm(self) -> f64 {
        self.h1_seminorm
    }

    /// Complete H1 error.
    #[must_use]
    pub fn h1(self) -> f64 {
        self.l2.hypot(self.h1_seminorm)
    }
}

/// Accepted two-dimensional Q1 elasticity solution and balance evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianLinearElasticity2dSolution {
    displacement: CartesianQ1VectorField2d,
    algebraic_values: Vec<f64>,
    boundary_reaction: [f64; COMPONENTS],
    integrated_body_force: [f64; COMPONENTS],
    assembly_report: AssemblyReport,
    solve_report: SolveReport,
}

impl CartesianLinearElasticity2dSolution {
    /// Continuous nodal displacement field.
    #[must_use]
    pub const fn displacement(&self) -> &CartesianQ1VectorField2d {
        &self.displacement
    }

    /// Free displacement components in assembled equation order.
    #[must_use]
    pub fn algebraic_values(&self) -> &[f64] {
        &self.algebraic_values
    }

    /// Sum of full-system residual reactions over constrained vertices.
    #[must_use]
    pub const fn boundary_reaction(&self) -> [f64; COMPONENTS] {
        self.boundary_reaction
    }

    /// Integral of `grad(q)` represented by the complete nodal load vector.
    #[must_use]
    pub const fn integrated_body_force(&self) -> [f64; COMPONENTS] {
        self.integrated_body_force
    }

    /// Complete assembly placement and packet-shape evidence.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    /// Complete solver/backend/execution evidence.
    #[must_use]
    pub const fn solve_report(&self) -> &SolveReport {
        &self.solve_report
    }
}

/// Lower the body-free local stiffness of every Cartesian cell into one
/// shape-homogeneous local-action batch.
///
/// # Errors
/// Returns a discretization diagnostic for a non-2D mesh, non-coercive Lamé
/// coefficients, incompatible quadrature, geometry, or allocation failure.
pub fn lower_cartesian_q1_linear_elasticity_local_action_2d(
    mesh: &CartesianMesh,
    shear_modulus: f64,
    first_lame_parameter: f64,
    quadrature: &QuadratureRule,
) -> Result<LocalLinearActionIr, Diagnostic> {
    validate_problem(mesh, shear_modulus, first_lame_parameter, quadrature)?;
    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("a two-dimensional mesh owns its cell stratum");
    let local_width = HypercubeQ1Space::new(DIMENSION)?
        .local_dofs()
        .len()
        .checked_mul(COMPONENTS)
        .ok_or_else(|| invalid("elasticity local width overflows usize"))?;
    let coefficient_count = cell_count
        .checked_mul(local_width)
        .and_then(|count| count.checked_mul(local_width))
        .ok_or_else(|| invalid("elasticity local-action shape overflows usize"))?;
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(|_| {
            invalid("elasticity local-action batch exceeds platform allocation capacity")
        })?;
    let operator = CartesianElasticityCell {
        shear_modulus,
        first_lame_parameter,
        body_force_potential: None,
    };
    for cell_index in 0..cell_count {
        let geometry = mesh
            .geometry_map(MeshEntity::new(DIMENSION, cell_index))
            .expect("a Cartesian cell owns affine geometry");
        let local = operator.evaluate(&geometry, quadrature)?;
        debug_assert_eq!(local.rows(), local_width);
        debug_assert_eq!(local.columns(), local_width);
        coefficients.extend_from_slice(local.matrix());
    }
    LocalLinearActionIr::new(local_width, local_width, coefficients)
}

/// Solve homogeneous-Dirichlet isotropic elasticity using the deterministic
/// reference assembly backend.
///
/// The conservative body force is not supplied as a parallel vector closure.
/// It is the exact coordinate derivative of the same lowered scalar `q` tape
/// at every quadrature point.
///
/// # Errors
/// Returns a diagnostic for incompatible mesh/material/quadrature data,
/// potential evaluation, assembly, constraints, or solve failure.
pub fn solve_cartesian_q1_linear_elasticity_2d(
    mesh: &CartesianMesh,
    shear_modulus: f64,
    first_lame_parameter: f64,
    body_force_potential: &ScalarSpatialExpression,
    quadrature: &QuadratureRule,
    solver: LinearSolveRequest<'_>,
) -> Result<CartesianLinearElasticity2dSolution, Diagnostic> {
    solve_cartesian_q1_linear_elasticity_2d_with_assembly(
        mesh,
        shear_modulus,
        first_lame_parameter,
        body_force_potential,
        quadrature,
        &REFERENCE_ASSEMBLY_BACKEND,
        solver,
    )
}

/// Solve the same problem through one explicit ordered assembly backend.
///
/// # Errors
/// Preserves the reference entry point diagnostics and the selected assembly
/// backend's complete-operation diagnostics.
#[allow(clippy::too_many_arguments)]
pub fn solve_cartesian_q1_linear_elasticity_2d_with_assembly(
    mesh: &CartesianMesh,
    shear_modulus: f64,
    first_lame_parameter: f64,
    body_force_potential: &ScalarSpatialExpression,
    quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
    solver: LinearSolveRequest<'_>,
) -> Result<CartesianLinearElasticity2dSolution, Diagnostic> {
    let assembled = finalize_cartesian_q1_linear_elasticity_2d(
        mesh,
        shear_modulus,
        first_lame_parameter,
        body_force_potential,
        quadrature,
        CartesianEssentialSides2d::complete_boundary(),
        assembly,
    )?;
    let (canonical_system, state) = assembled.into_canonical()?;
    let solved = solver.solve(&canonical_system.linear_problem()?)?;
    state.finish(solved, canonical_system)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedCartesianElasticity2dAssembly {
    mesh: CartesianMesh,
    free_indices: Vec<Option<DofId>>,
    linear_system: eqiora_assembly::LinearSystem,
    full_system: eqiora_assembly::LinearSystem,
    integrated_body_force: [f64; COMPONENTS],
    assembly_report: AssemblyReport,
}

impl FinalizedCartesianElasticity2dAssembly {
    pub(crate) fn into_canonical(
        self,
    ) -> Result<
        (
            Arc<CanonicalCsrSystemView>,
            FinalizedCartesianElasticity2dState,
        ),
        Diagnostic,
    > {
        let Self {
            mesh,
            free_indices,
            linear_system,
            full_system,
            integrated_body_force,
            assembly_report,
        } = self;
        let canonical_system = Arc::new(CanonicalCsrSystemView::new(
            &linear_system,
            LinearOperatorProperties::SymmetricPositiveDefinite,
        )?);
        Ok((
            canonical_system,
            FinalizedCartesianElasticity2dState {
                mesh,
                free_indices,
                full_system,
                integrated_body_force,
                assembly_report,
            },
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedCartesianElasticity2dState {
    mesh: CartesianMesh,
    free_indices: Vec<Option<DofId>>,
    full_system: eqiora_assembly::LinearSystem,
    integrated_body_force: [f64; COMPONENTS],
    assembly_report: AssemblyReport,
}

impl FinalizedCartesianElasticity2dState {
    pub(crate) const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    pub(crate) fn finish(
        self,
        solved: LinearSolution,
        canonical_system: Arc<CanonicalCsrSystemView>,
    ) -> Result<CartesianLinearElasticity2dSolution, Diagnostic> {
        if solved.values().len() != canonical_system.rows() {
            return Err(invalid(
                "elasticity solution shape differs from its finalized system",
            ));
        }
        let (algebraic_values, solve_report) = solved.into_parts();
        let mut displacement = vec![0.0; self.free_indices.len()];
        for (global, value) in displacement.iter_mut().enumerate() {
            if let Some(equation) = self.free_indices[global] {
                *value = algebraic_values[equation.index()];
            }
        }

        let mut residual = self.full_system.matrix().multiply(&displacement)?;
        for (value, right_hand_side) in residual.iter_mut().zip(self.full_system.rhs()) {
            *value -= right_hand_side;
        }
        let mut boundary_reaction = [0.0; COMPONENTS];
        for (global, reaction) in residual.iter().enumerate() {
            if self.free_indices[global].is_none() {
                boundary_reaction[global % COMPONENTS] += reaction;
            }
        }
        if boundary_reaction
            .iter()
            .chain(&self.integrated_body_force)
            .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "elasticity reaction or integrated body force is non-finite",
            ));
        }

        Ok(CartesianLinearElasticity2dSolution {
            displacement: CartesianQ1VectorField2d::new(self.mesh, displacement)?,
            algebraic_values,
            boundary_reaction,
            integrated_body_force: self.integrated_body_force,
            assembly_report: self.assembly_report,
            solve_report,
        })
    }
}

pub(crate) fn finalize_cartesian_q1_linear_elasticity_2d(
    mesh: &CartesianMesh,
    shear_modulus: f64,
    first_lame_parameter: f64,
    body_force_potential: &ScalarSpatialExpression,
    quadrature: &QuadratureRule,
    essential_sides: CartesianEssentialSides2d,
    assembly: &dyn AssemblyBackend,
) -> Result<FinalizedCartesianElasticity2dAssembly, Diagnostic> {
    validate_problem(mesh, shear_modulus, first_lame_parameter, quadrature)?;
    if !essential_sides.has_essential_side() {
        return Err(invalid(
            "Cartesian Q1 elasticity requires at least one complete homogeneous essential side to remove rigid modes",
        ));
    }
    if body_force_potential.coordinate_dimension() != DIMENSION {
        return Err(invalid(format!(
            "elasticity body-force potential expects {} coordinates, not {DIMENSION}",
            body_force_potential.coordinate_dimension(),
        )));
    }

    let vertex_count = mesh
        .entity_count(0)
        .expect("a Cartesian mesh owns its vertex stratum");
    let global_width = vertex_count
        .checked_mul(COMPONENTS)
        .ok_or_else(|| invalid("elasticity global DOF count overflows usize"))?;
    let mut free_indices = vec![None; global_width];
    let mut free_count = 0_usize;
    for vertex_index in 0..vertex_count {
        let vertex = MeshEntity::new(0, vertex_index);
        if !essential_sides.constrains_vertex(mesh, vertex) {
            for component in 0..COMPONENTS {
                free_indices[global_dof(vertex_index, component)?] = Some(DofId::new(free_count));
                free_count = free_count
                    .checked_add(1)
                    .ok_or_else(|| invalid("elasticity free DOF count overflows usize"))?;
            }
        }
    }
    if free_count == 0 {
        return Err(invalid(
            "Cartesian Q1 elasticity requires at least one unconstrained interior vertex",
        ));
    }

    let operator = CartesianElasticityCell {
        shear_modulus,
        first_lame_parameter,
        body_force_potential: Some(body_force_potential),
    };
    let plan = AssemblyPlan::new(vec![
        AssemblyTarget::new(free_count)?,
        AssemblyTarget::new(global_width)?,
    ])?;
    let reduced_target = plan
        .target_id(0)
        .expect("the elasticity assembly plan owns its reduced target");
    let full_target = plan
        .target_id(1)
        .expect("the elasticity assembly plan owns its full target");
    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("a two-dimensional mesh owns its cell stratum");
    let work = IndexedAssemblyWork::new(cell_count, |cell_index| {
        let cell = MeshEntity::new(DIMENSION, cell_index);
        let geometry = mesh
            .geometry_map(cell)
            .expect("a Cartesian cell owns affine geometry");
        let local = operator.evaluate(&geometry, quadrature)?;
        let vertices = mesh
            .entity_vertices(cell)
            .expect("a Cartesian cell owns its vertex closure");
        let global_dofs = local_global_dofs(&vertices)?;
        let reduced = AssemblyMap::new(
            global_dofs
                .iter()
                .map(|global| free_indices[*global])
                .collect(),
            global_dofs
                .iter()
                .map(|global| {
                    free_indices[*global]
                        .map(LocalUnknown::Free)
                        .unwrap_or(LocalUnknown::Fixed(0.0))
                })
                .collect(),
        )?;
        let full = AssemblyMap::new(
            global_dofs
                .iter()
                .map(|global| Some(DofId::new(*global)))
                .collect(),
            global_dofs
                .iter()
                .map(|global| LocalUnknown::Free(DofId::new(*global)))
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
    let (systems, assembly_report) = assembly.assemble(&plan, &work)?.into_parts();
    let mut systems = systems.into_iter();
    let reduced_system = systems
        .next()
        .expect("two-target elasticity assembly returns a reduced system");
    let full_system = systems
        .next()
        .expect("two-target elasticity assembly returns a full system");
    debug_assert!(systems.next().is_none());

    let mut integrated_body_force = [0.0; COMPONENTS];
    for values in full_system.rhs().chunks_exact(COMPONENTS) {
        for component in 0..COMPONENTS {
            integrated_body_force[component] += values[component];
        }
    }
    if integrated_body_force.iter().any(|value| !value.is_finite()) {
        return Err(invalid("integrated elasticity body force is non-finite"));
    }

    Ok(FinalizedCartesianElasticity2dAssembly {
        mesh: mesh.clone(),
        free_indices,
        linear_system: reduced_system,
        full_system,
        integrated_body_force,
        assembly_report,
    })
}

struct CartesianElasticityCell<'a> {
    shear_modulus: f64,
    first_lame_parameter: f64,
    body_force_potential: Option<&'a ScalarSpatialExpression>,
}

impl LocalOperator<AffineGeometryMap> for CartesianElasticityCell<'_> {
    fn evaluate(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        require_geometry_rule(geometry, quadrature)?;
        if geometry.reference_cell().dimension() != DIMENSION
            || geometry.physical_dimension() != DIMENSION
        {
            return Err(invalid(
                "Cartesian elasticity cell requires a square two-dimensional geometry map",
            ));
        }
        let inverse = geometry.inverse_jacobian()?;
        let space = HypercubeQ1Space::new(DIMENSION)?;
        let scalar_dofs = space.local_dofs().len();
        let dof_count = scalar_dofs
            .checked_mul(COMPONENTS)
            .ok_or_else(|| invalid("elasticity local DOF count overflows usize"))?;
        let mut matrix = vec![0.0; dof_count * dof_count];
        let mut rhs = vec![0.0; dof_count];
        let mut physical = [0.0; DIMENSION];
        let zero_parameter_tangent = self
            .body_force_potential
            .map_or_else(Vec::new, |potential| {
                vec![0.0; potential.parameter_fields().len()]
            });

        for point in quadrature.points() {
            let basis = space.tabulate(&point.coordinates)?;
            geometry.map_point(&point.coordinates, &mut physical)?;
            let gradients = physical_gradients(&basis, &inverse);
            let body_force = match self.body_force_potential {
                Some(potential) => {
                    potential_gradient(potential, &physical, &zero_parameter_tangent)?
                }
                None => [0.0; COMPONENTS],
            };
            let scale = point.weight * geometry.measure_scale();
            for local_row in 0..scalar_dofs {
                let row_gradient = &gradients[local_row];
                for row_component in 0..COMPONENTS {
                    let row = local_dof(local_row, row_component)?;
                    rhs[row] += scale * basis.values()[local_row] * body_force[row_component];
                    for (local_column, column_gradient) in gradients.iter().enumerate() {
                        let gradient_dot = row_gradient
                            .iter()
                            .zip(column_gradient)
                            .map(|(left, right)| left * right)
                            .sum::<f64>();
                        for column_component in 0..COMPONENTS {
                            let column = local_dof(local_column, column_component)?;
                            let delta_term = if row_component == column_component {
                                gradient_dot
                            } else {
                                0.0
                            };
                            let value = self.shear_modulus
                                * (delta_term
                                    + row_gradient[column_component]
                                        * column_gradient[row_component])
                                + self.first_lame_parameter
                                    * row_gradient[row_component]
                                    * column_gradient[column_component];
                            matrix[row * dof_count + column] += scale * value;
                        }
                    }
                }
            }
        }
        LocalContribution::new(dof_count, dof_count, matrix, rhs)
    }
}

fn potential_gradient(
    potential: &ScalarSpatialExpression,
    coordinates: &[f64; DIMENSION],
    zero_parameter_tangent: &[f64],
) -> Result<[f64; COMPONENTS], Diagnostic> {
    let mut gradient = [0.0; COMPONENTS];
    for axis in 0..DIMENSION {
        let mut coordinate_tangent = [0.0; DIMENSION];
        coordinate_tangent[axis] = 1.0;
        let (_, derivative) =
            potential.evaluate_jvp(coordinates, &coordinate_tangent, zero_parameter_tangent)?;
        gradient[axis] = derivative;
    }
    Ok(gradient)
}

fn physical_gradients(
    basis: &crate::discrete_space::BasisTabulation,
    inverse_jacobian: &[f64],
) -> Vec<Vec<f64>> {
    basis
        .reference_gradients()
        .chunks_exact(DIMENSION)
        .map(|gradient| physical_gradient(gradient, inverse_jacobian, DIMENSION))
        .collect()
}

fn local_global_dofs(vertices: &[MeshEntity]) -> Result<Vec<usize>, Diagnostic> {
    let capacity = vertices
        .len()
        .checked_mul(COMPONENTS)
        .ok_or_else(|| invalid("elasticity local-to-global shape overflows usize"))?;
    let mut dofs = Vec::new();
    dofs.try_reserve_exact(capacity)
        .map_err(|_| invalid("elasticity local-to-global map exceeds allocation capacity"))?;
    for vertex in vertices {
        for component in 0..COMPONENTS {
            dofs.push(global_dof(vertex.index(), component)?);
        }
    }
    Ok(dofs)
}

fn global_dof(vertex: usize, component: usize) -> Result<usize, Diagnostic> {
    vertex
        .checked_mul(COMPONENTS)
        .and_then(|base| base.checked_add(component))
        .ok_or_else(|| invalid("elasticity global DOF index overflows usize"))
}

fn local_dof(vertex: usize, component: usize) -> Result<usize, Diagnostic> {
    vertex
        .checked_mul(COMPONENTS)
        .and_then(|base| base.checked_add(component))
        .ok_or_else(|| invalid("elasticity local DOF index overflows usize"))
}

fn validate_problem(
    mesh: &CartesianMesh,
    shear_modulus: f64,
    first_lame_parameter: f64,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    require_two_dimensional_mesh(mesh)?;
    if !shear_modulus.is_finite()
        || !first_lame_parameter.is_finite()
        || shear_modulus <= 0.0
        || first_lame_parameter + shear_modulus <= 0.0
    {
        return Err(invalid(
            "two-dimensional isotropic elasticity requires finite mu > 0 and lambda + mu > 0",
        ));
    }
    require_cell_rule(mesh, quadrature)
}

fn require_two_dimensional_mesh(mesh: &CartesianMesh) -> Result<(), Diagnostic> {
    if mesh.topological_dimension() != DIMENSION || mesh.geometric_dimension() != DIMENSION {
        Err(invalid(
            "this Cartesian Q1 vector realization requires exactly two dimensions",
        ))
    } else {
        Ok(())
    }
}

fn require_cell_rule(mesh: &CartesianMesh, quadrature: &QuadratureRule) -> Result<(), Diagnostic> {
    require_two_dimensional_mesh(mesh)?;
    let expected = ReferenceCell::hypercube(DIMENSION)?;
    if quadrature.reference_cell() != expected {
        return Err(invalid(format!(
            "two-dimensional Cartesian cell requires {expected:?} quadrature, received {:?}",
            quadrature.reference_cell(),
        )));
    }
    Ok(())
}

fn require_geometry_rule(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    if geometry.reference_cell() != quadrature.reference_cell() {
        Err(invalid(
            "elasticity local geometry and quadrature reference cells differ",
        ))
    } else {
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use eqiora_assembly::CooAssembler;
    use eqiora_compiler::compile;
    use eqiora_graph::{GraphStore, InMemoryGraphStore};
    use eqiora_sem::KernelProgram;
    use eqiora_solver::{LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};

    use super::*;
    use crate::canonical::lower_scalar_elliptic_cartesian;

    fn unit_cell_action(mu: f64, lambda: f64) -> (CartesianMesh, LocalLinearActionIr) {
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[1, 1]).unwrap();
        let quadrature = QuadratureRule::tensor_product_gauss_legendre(DIMENSION, 2).unwrap();
        let action =
            lower_cartesian_q1_linear_elasticity_local_action_2d(&mesh, mu, lambda, &quadrature)
                .unwrap();
        (mesh, action)
    }

    fn nodal_vector(mesh: &CartesianMesh, value: impl Fn(&[f64]) -> [f64; COMPONENTS]) -> Vec<f64> {
        (0..mesh.entity_count(0).unwrap())
            .flat_map(|vertex| value(&mesh.vertex_coordinates(MeshEntity::new(0, vertex)).unwrap()))
            .collect()
    }

    fn cell_nodal_vector(
        mesh: &CartesianMesh,
        value: impl Fn(&[f64]) -> [f64; COMPONENTS],
    ) -> Vec<f64> {
        mesh.entity_vertices(MeshEntity::new(DIMENSION, 0))
            .unwrap()
            .into_iter()
            .flat_map(|vertex| value(&mesh.vertex_coordinates(vertex).unwrap()))
            .collect()
    }

    fn energy(action: &LocalLinearActionIr, values: &[f64]) -> f64 {
        let mut applied = vec![0.0; action.output_len()];
        action.apply_reference(values, &mut applied).unwrap();
        0.5 * values
            .iter()
            .zip(applied)
            .map(|(left, right)| left * right)
            .sum::<f64>()
    }

    #[test]
    fn every_essential_side_uses_exact_cartesian_vertex_topology() {
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[2, 3]).unwrap();
        for axis in 0..DIMENSION {
            for side in 0..2 {
                let mut sides = [[false; 2]; DIMENSION];
                sides[axis][side] = true;
                let essential = CartesianEssentialSides2d::new(sides);
                let boundary_index = if side == 0 {
                    0
                } else {
                    mesh.axis_cell_count(axis).unwrap()
                };
                for vertex_index in 0..mesh.entity_count(0).unwrap() {
                    let vertex = MeshEntity::new(0, vertex_index);
                    let index = mesh.vertex_multi_index(vertex).unwrap();
                    assert_eq!(
                        essential.constrains_vertex(&mesh, vertex),
                        index[axis] == boundary_index,
                        "axis {axis}, side {side}, vertex {index:?}",
                    );
                }
            }
        }
    }

    #[test]
    fn local_stiffness_is_symmetric_and_contains_cross_component_coupling() {
        let (_, action) = unit_cell_action(2.0, 3.0);
        let width = action.rows();
        let matrix = action.coefficients();
        for row in 0..width {
            for column in 0..width {
                assert!(
                    (matrix[row * width + column] - matrix[column * width + row]).abs() < 2.0e-15
                );
            }
        }
        assert!(
            (0..width).any(|row| {
                (0..width).any(|column| {
                    row % COMPONENTS != column % COMPONENTS
                        && matrix[row * width + column].abs() > 1.0e-12
                })
            }),
            "vector elasticity must not degenerate into uncoupled scalar diffusion",
        );
    }

    #[test]
    fn rigid_translation_and_infinitesimal_rotation_have_zero_energy() {
        let (mesh, action) = unit_cell_action(2.0, 3.0);
        let translation = cell_nodal_vector(&mesh, |_| [2.5, -4.0]);
        let rotation = cell_nodal_vector(&mesh, |point| [-point[1], point[0]]);
        assert!(energy(&action, &translation).abs() < 2.0e-14);
        assert!(energy(&action, &rotation).abs() < 2.0e-14);
    }

    #[test]
    fn pure_shear_and_dilatation_match_analytical_energy() {
        let mu = 2.0;
        let lambda = 3.0;
        let (mesh, action) = unit_cell_action(mu, lambda);
        let pure_shear = cell_nodal_vector(&mesh, |point| [point[1], point[0]]);
        let dilatation = cell_nodal_vector(&mesh, |point| [point[0], point[1]]);
        assert!((energy(&action, &pure_shear) - 2.0 * mu).abs() < 2.0e-14);
        assert!((energy(&action, &dilatation) - 2.0 * (mu + lambda)).abs() < 4.0e-14);
    }

    #[test]
    fn two_by_two_affine_patch_has_exact_center_and_boundary_equilibrium() {
        let lambda = 2.0;
        let mu = 3.0;
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[2, 2]).unwrap();
        let quadrature = QuadratureRule::tensor_product_gauss_legendre(DIMENSION, 2).unwrap();
        let action =
            lower_cartesian_q1_linear_elasticity_local_action_2d(&mesh, mu, lambda, &quadrature)
                .unwrap();
        let vertex_count = mesh.entity_count(0).unwrap();
        let mut assembler = CooAssembler::new(vertex_count * COMPONENTS).unwrap();
        let local_width = action.rows();
        for cell_index in 0..mesh.entity_count(DIMENSION).unwrap() {
            let offset = cell_index * local_width * local_width;
            let local = LocalContribution::new(
                local_width,
                local_width,
                action.coefficients()[offset..offset + local_width * local_width].to_vec(),
                vec![0.0; local_width],
            )
            .unwrap();
            let vertices = mesh
                .entity_vertices(MeshEntity::new(DIMENSION, cell_index))
                .unwrap();
            let global = local_global_dofs(&vertices).unwrap();
            let map = AssemblyMap::new(
                global
                    .iter()
                    .map(|index| Some(DofId::new(*index)))
                    .collect(),
                global
                    .iter()
                    .map(|index| LocalUnknown::Free(DofId::new(*index)))
                    .collect(),
            )
            .unwrap();
            assembler.scatter(&map, &local).unwrap();
        }
        let system = assembler.finish().unwrap();
        let displacement = nodal_vector(&mesh, |point| {
            [
                2.0 * point[0] + 3.0 * point[1] + 1.0,
                5.0 * point[0] + 7.0 * point[1] - 2.0,
            ]
        });
        let reactions = system.matrix().multiply(&displacement).unwrap();
        let mut resultant = [0.0; COMPONENTS];
        let mut moment = 0.0;

        for vertex in 0..vertex_count {
            let entity = MeshEntity::new(0, vertex);
            let point = mesh.vertex_coordinates(entity).unwrap();
            let index = mesh.vertex_multi_index(entity).unwrap();
            let values = &displacement[vertex * COMPONENTS..(vertex + 1) * COMPONENTS];
            let reaction = &reactions[vertex * COMPONENTS..(vertex + 1) * COMPONENTS];
            if index == [1, 1] {
                assert!((values[0] - 3.5).abs() < 2.0e-15);
                assert!((values[1] - 4.0).abs() < 2.0e-15);
                assert!(reaction.iter().all(|value| value.abs() < 2.0e-14));
                continue;
            }

            let expected = match (index[0], index[1]) {
                (0, 0) => [-13.5, -21.0],
                (1, 0) => [-12.0, -30.0],
                (2, 0) => [1.5, -9.0],
                (0, 1) => [-15.0, -12.0],
                (2, 1) => [15.0, 12.0],
                (0, 2) => [-1.5, 9.0],
                (1, 2) => [12.0, 30.0],
                (2, 2) => [13.5, 21.0],
                _ => panic!("unexpected two-by-two patch vertex {index:?}"),
            };
            for component in 0..COMPONENTS {
                assert!((reaction[component] - expected[component]).abs() < 3.0e-14);
                resultant[component] += reaction[component];
            }
            moment += point[0] * reaction[1] - point[1] * reaction[0];
        }
        assert!(resultant.iter().all(|value| value.abs() < 6.0e-14));
        assert!(moment.abs() < 6.0e-14);
    }

    #[test]
    fn vector_field_error_uses_value_and_frobenius_gradient_contracts() {
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[2, 3]).unwrap();
        let values = nodal_vector(&mesh, |point| [-point[1], point[0]]);
        let field = CartesianQ1VectorField2d::new(mesh, values).unwrap();
        let quadrature = QuadratureRule::tensor_product_gauss_legendre(DIMENSION, 2).unwrap();
        let error = field
            .error_norms(
                &|point| ([-point[1], point[0]], [[0.0, -1.0], [1.0, 0.0]]),
                &quadrature,
            )
            .unwrap();
        assert!(error.l2() < 2.0e-15);
        assert!(error.h1_seminorm() < 3.0e-15);
        assert!(error.h1() < 4.0e-15);
    }

    #[test]
    fn canonical_potential_jvp_drives_load_and_full_reaction_balance() {
        let source = r#"
model potential_probe {
  domain body = box(0, 1, 0, 1);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  representation space = continuum;
  field probe on body as space: m ^ 3 = 0;
  relation balance continuous on body {
    -div(grad(probe)) - (coordinate(0) + 2 * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(probe) = 0; }
  relation x_upper_value continuous on x_upper { trace(probe) = 0; }
  relation y_lower_value continuous on y_lower { trace(probe) = 0; }
  relation y_upper_value continuous on y_upper { trace(probe) = 0; }
}
"#;
        let mut compiled = compile("potential-probe.eqi", source).unwrap();
        let (transaction, model, _) = compiled.remove(0).into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
        let lowered = lower_scalar_elliptic_cartesian(&program).unwrap();

        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[2, 2]).unwrap();
        let quadrature = QuadratureRule::tensor_product_gauss_legendre(DIMENSION, 2).unwrap();
        let plan = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-13,
            1.0e-14,
            NonZeroUsize::new(64).unwrap(),
        )
        .unwrap();
        let solution = solve_cartesian_q1_linear_elasticity_2d(
            &mesh,
            2.0,
            3.0,
            lowered.source(),
            &quadrature,
            LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, plan),
        )
        .unwrap();
        for (actual, exact) in solution.integrated_body_force().into_iter().zip([1.0, 2.0]) {
            assert!((actual - exact).abs() < 2.0e-14);
        }
        for component in 0..COMPONENTS {
            assert!(
                (solution.boundary_reaction()[component]
                    + solution.integrated_body_force()[component])
                    .abs()
                    < 2.0e-12
            );
        }
        assert_eq!(solution.assembly_report().target_count(), 2);
    }
}
