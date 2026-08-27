//! Method-neutral lowering of the canonical isotropic-elasticity subset.

use std::collections::BTreeSet;

use eqiora_artifact::{CartesianMeshEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1};
use eqiora_assembly::{AssemblyBackend, REFERENCE_ASSEMBLY_BACKEND};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, GraphPath, RawId, ValueShape};
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_graph::EdgeKind;
use eqiora_ir::{OperatorApplicationProof, StandardPureOperator};
use eqiora_meshing::QuadratureRule;
use eqiora_realization::{
    DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy, ResolvedRealization,
    SpaceFamily, Target, VectorLayoutKind,
};
use eqiora_schema::kernel::typing::TypedResidual;
use eqiora_schema::kernel::{
    ActivationKind, BoundarySide, DomainKind, ExprDag, ExprId, ExprNode, KernelNode,
    RepresentationKind, SymbolRef, ValueFrame,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{LinearOperatorProperties, LinearSolverBackend, ScalarType};

use crate::canonical_boundary::BoundaryRelationBinding;
use crate::canonical_boundary::{CartesianBoundaryInventory2d, PhysicalBoundaryDisposition};
use crate::cartesian_elasticity::CartesianLinearElasticity2dSolution;
use crate::cartesian_elasticity::{
    CartesianEssentialSides2d, finalize_cartesian_q1_linear_elasticity_2d,
};
use crate::finalized_spatial::FinalizedIsotropicElasticityCartesian2dProblem;
use crate::spatial_expression::{self, ScalarSpatialExpression};
use eqiora_meshing::{CartesianMesh, MeshTopology};

mod block;
mod boundary;
mod dynamics;
mod pair;

pub use dynamics::{
    IsotropicElastodynamicsCartesianModel, IsotropicElastodynamicsCartesianModel2d,
    IsotropicElastodynamicsCartesianModel3d, lower_isotropic_elastodynamics_cartesian_2d,
    lower_isotropic_elastodynamics_cartesian_3d,
};
pub(crate) use dynamics::{
    LoweredIsotropicElastodynamicsSubdomain, LoweredIsotropicElastodynamicsSubdomain2d,
    lower_isotropic_elastodynamics_subdomain, lower_isotropic_elastodynamics_subdomain_2d,
    lower_isotropic_elastodynamics_subdomain_2d_with_boundaries,
};
pub use pair::{
    ConformingElasticityInterface2d, ConformingElasticityInterfaceSide2d,
    ConformingIsotropicElasticityCartesianPair2d,
    finalize_resolved_conforming_isotropic_elasticity_cartesian_pair_2d,
    finalize_resolved_conforming_isotropic_elasticity_cartesian_pair_2d_with_assembly,
    lower_conforming_isotropic_elasticity_cartesian_pair_2d,
    solve_resolved_conforming_isotropic_elasticity_cartesian_pair_2d,
    solve_resolved_conforming_isotropic_elasticity_cartesian_pair_2d_with_assembly,
};

const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};

/// Exact, method-neutral 2D isotropic small-strain elasticity model.
///
/// The admitted strong relation is
/// `-div(2 mu sym(grad(u)) + lambda I div(u)) - grad(q) = 0`,
/// where `q` is defined by one scalar spatial Relation and every Cartesian
/// side has one explicit normalized boundary disposition. Mesh, element,
/// quadrature, algebra, solver, and target choices are intentionally absent.
#[derive(Debug, Clone, PartialEq)]
pub struct IsotropicElasticityCartesianModel2d {
    domain: RawId,
    displacement: RawId,
    load_potential: RawId,
    load_definition_relation: RawId,
    balance_relation: RawId,
    bounds: [[f64; 2]; 2],
    shear_modulus: ScalarSpatialExpression,
    first_lame_parameter: ScalarSpatialExpression,
    load_potential_expression: ScalarSpatialExpression,
    boundary_inventory: CartesianBoundaryInventory2d,
    boundary_relations: Vec<BoundaryRelationBinding>,
}

impl IsotropicElasticityCartesianModel2d {
    /// Canonical volume Domain.
    #[must_use]
    pub const fn domain(&self) -> RawId {
        self.domain
    }

    /// Canonical spatial-vector displacement Field.
    #[must_use]
    pub const fn displacement(&self) -> RawId {
        self.displacement
    }

    /// Canonical scalar conservative-load potential Field.
    #[must_use]
    pub const fn load_potential(&self) -> RawId {
        self.load_potential
    }

    /// Exact Relation defining the conservative-load potential.
    #[must_use]
    pub(crate) const fn load_definition_relation(&self) -> RawId {
        self.load_definition_relation
    }

    /// Exact Relation witnessing isotropic momentum balance.
    #[must_use]
    pub(crate) const fn balance_relation(&self) -> RawId {
        self.balance_relation
    }

    /// Physical Cartesian bounds in coherent SI coordinates.
    #[must_use]
    pub const fn bounds(&self) -> &[[f64; 2]; 2] {
        &self.bounds
    }

    /// Shear modulus `mu` in coherent SI units.
    #[must_use]
    pub fn shear_modulus(&self) -> f64 {
        self.shear_modulus
            .constant_value()
            .expect("elastic lowerer retains a constant shear-modulus tape")
    }

    /// First Lamé parameter `lambda` in coherent SI units.
    #[must_use]
    pub fn first_lame_parameter(&self) -> f64 {
        self.first_lame_parameter
            .constant_value()
            .expect("elastic lowerer retains a constant first-Lame-parameter tape")
    }

    /// Canonical constant expression retaining `mu` Parameter identity.
    #[must_use]
    pub const fn shear_modulus_expression(&self) -> &ScalarSpatialExpression {
        &self.shear_modulus
    }

    /// Canonical constant expression retaining `lambda` Parameter identity.
    #[must_use]
    pub const fn first_lame_parameter_expression(&self) -> &ScalarSpatialExpression {
        &self.first_lame_parameter
    }

    /// Immutable scalar tape defining the canonical load potential `q`.
    #[must_use]
    pub const fn load_potential_expression(&self) -> &ScalarSpatialExpression {
        &self.load_potential_expression
    }

    /// Complete package-neutral meaning of the four exact Cartesian sides.
    #[must_use]
    pub const fn boundary_inventory(&self) -> &CartesianBoundaryInventory2d {
        &self.boundary_inventory
    }

    /// Canonically ordered exact Relations admitted by boundary normalization.
    #[must_use]
    pub(crate) fn boundary_relations(&self) -> &[BoundaryRelationBinding] {
        &self.boundary_relations
    }

    /// Evaluate `grad(q)` from the same canonical tape as the model Relation.
    ///
    /// # Errors
    /// Preserves the tape's exact shape and finite-evaluation diagnostics.
    pub fn conservative_body_force(&self, coordinates: &[f64]) -> Result<[f64; 2], Diagnostic> {
        let zero_parameters = vec![0.0; self.load_potential_expression.parameter_fields().len()];
        let mut gradient = [0.0; 2];
        for axis in 0..2 {
            let mut direction = [0.0; 2];
            direction[axis] = 1.0;
            gradient[axis] = self
                .load_potential_expression
                .evaluate_jvp(coordinates, &direction, &zero_parameters)?
                .1;
        }
        Ok(gradient)
    }
}

/// Lower the exact canonical 2D isotropic-elasticity subset.
///
/// This lowerer is identity-parametric: it recognizes typed structure rather
/// than source names. It admits exactly one Cartesian box, one spatial-vector
/// displacement, one scalar load-potential definition, one balance Relation,
/// and one exact direct or conserving-interface boundary law on every side.
/// Closed zero trace/traction terminals normalize to method-neutral
/// dispositions; live bindings remain explicit for a later Realization gate.
///
/// # Errors
/// Returns `EQ0703` when the admitted Model is ambiguous or differs from this
/// deliberately narrow semantic subset.
pub fn lower_isotropic_elasticity_cartesian_2d(
    program: &KernelProgram,
) -> Result<IsotropicElasticityCartesianModel2d, Diagnostic> {
    let (domain, bounds) = unique_box_2d(program)?;
    let lowered =
        lower_isotropic_elasticity_subdomain_2d_with_boundaries(program, domain, bounds, None)?;
    require_closed_elasticity_models(program, std::slice::from_ref(&lowered))?;
    Ok(lowered.model)
}

pub(crate) fn lower_isotropic_elasticity_geometry_2d(
    program: &KernelProgram,
    geometry: &CanonicalGeometryV1,
    mesh: &CartesianMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
) -> Result<IsotropicElasticityCartesianModel2d, Diagnostic> {
    let (domain, bounds, boundaries) = crate::canonical::geometry_rectangle_cartesian_support(
        program,
        geometry,
        mesh,
        correspondence,
    )?;
    let bounds: [[f64; 2]; 2] = bounds
        .try_into()
        .map_err(|_| model_lowering_error(program, "elasticity requires two rectangle axes"))?;
    let lowered = lower_isotropic_elasticity_subdomain_2d_with_boundaries(
        program,
        domain,
        bounds,
        Some(boundaries),
    )?;
    require_closed_elasticity_models(program, std::slice::from_ref(&lowered))?;
    Ok(lowered.model)
}

pub(crate) fn recognize_isotropic_elasticity_geometry_mathematics(
    program: &KernelProgram,
) -> Result<(), Diagnostic> {
    let regions = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::GeometryRegion { .. }) =>
            {
                Some(domain.id().erase())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [domain] = regions.as_slice() else {
        return Err(model_lowering_error(
            program,
            "geometry-backed elasticity recognition requires exactly one GeometryRegion",
        ));
    };
    let boundaries = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(boundary)
                if matches!(boundary.kind(), DomainKind::GeometryBoundary { .. })
                    && crate::canonical::boundary_parent(program, boundary.id().erase())
                        == Some(*domain) =>
            {
                Some(boundary.id().erase())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if boundaries.len() != 4 {
        return Err(lowering_error(
            *domain,
            "geometry-backed 2D elasticity recognition requires four boundary supports",
        ));
    }
    let sides = [
        (0, BoundarySide::Lower),
        (0, BoundarySide::Upper),
        (1, BoundarySide::Lower),
        (1, BoundarySide::Upper),
    ];
    let boundary_map = sides.into_iter().zip(boundaries).collect();
    let lowered = lower_isotropic_elasticity_subdomain_2d_with_boundaries(
        program,
        *domain,
        [[0.0, 1.0], [0.0, 1.0]],
        Some(boundary_map),
    )?;
    require_closed_elasticity_models(program, std::slice::from_ref(&lowered))?;
    Ok(())
}

#[derive(Debug)]
struct LoweredIsotropicElasticitySubdomain2d {
    model: IsotropicElasticityCartesianModel2d,
    boundary: boundary::LoweredElasticityBoundary2d,
}

fn lower_isotropic_elasticity_subdomain_2d(
    program: &KernelProgram,
    domain: RawId,
    bounds: [[f64; 2]; 2],
) -> Result<LoweredIsotropicElasticitySubdomain2d, Diagnostic> {
    lower_isotropic_elasticity_subdomain_2d_with_boundaries(program, domain, bounds, None)
}

fn lower_isotropic_elasticity_subdomain_2d_with_boundaries(
    program: &KernelProgram,
    domain: RawId,
    bounds: [[f64; 2]; 2],
    boundaries: Option<std::collections::BTreeMap<(usize, BoundarySide), RawId>>,
) -> Result<LoweredIsotropicElasticitySubdomain2d, Diagnostic> {
    let (displacement, load_potential) = exact_fields(program, domain)?;
    let volume_relations = relations_on(program, domain);
    if volume_relations.len() != 2 {
        return Err(lowering_error(
            domain,
            format!(
                "2D isotropic elasticity requires one load-potential definition and one balance Relation, found {} volume Relations",
                volume_relations.len()
            ),
        ));
    }

    let mut load_definition = None;
    let mut balance = None;
    for relation in volume_relations {
        require_continuous_relation(program, relation)?;
        let typed = typed_relation(program, relation)?;
        let expression = typed.expression();
        let root = unique_root(expression, relation)?;
        if let Some(source) = load_definition_root(expression, root, load_potential) {
            if load_definition.replace((relation, source)).is_some() {
                return Err(lowering_error(
                    relation,
                    "load-potential definition is not unique",
                ));
            }
        } else if let Some(stress) = balance_root(expression, root, load_potential) {
            if balance.replace((relation, stress, typed)).is_some() {
                return Err(lowering_error(relation, "elastic balance is not unique"));
            }
        } else {
            return Err(lowering_error(
                relation,
                "volume Relation is neither `q - expression = 0` nor the canonical isotropic balance",
            ));
        }
    }
    let (load_relation, load_root) = load_definition
        .ok_or_else(|| lowering_error(domain, "load-potential definition is missing"))?;
    let (balance_relation, stress_root, balance_typed) =
        balance.ok_or_else(|| lowering_error(domain, "elastic balance is missing"))?;

    let load_expression = relation_expression(program, load_relation)?;
    let load_potential_expression =
        spatial_expression::lower(program, load_expression, load_root, load_relation, 2)?;
    let (two_mu, lambda) = lower_isotropic_stress_coefficients(
        program,
        &balance_typed,
        stress_root,
        displacement,
        balance_relation,
    )?;
    let boundary_inventory = match boundaries {
        Some(boundaries) => boundary::lower_with_boundaries(
            program,
            domain,
            displacement,
            displacement,
            &two_mu,
            &lambda,
            boundaries,
        )?,
        None => boundary::lower(
            program,
            domain,
            displacement,
            displacement,
            &two_mu,
            &lambda,
        )?,
    };
    let shear_modulus = two_mu.multiply(ScalarSpatialExpression::constant(2, 0.5));
    let Some(shear_value) = shear_modulus.constant_value() else {
        return Err(lowering_error(
            balance_relation,
            "shear-modulus expression is not finitely evaluable",
        ));
    };
    let Some(lambda_value) = lambda.constant_value() else {
        return Err(lowering_error(
            balance_relation,
            "first-Lame-parameter expression is not finitely evaluable",
        ));
    };
    let coercivity = lambda_value + shear_value;
    if !shear_value.is_finite()
        || !lambda_value.is_finite()
        || !coercivity.is_finite()
        || shear_value <= 0.0
        || coercivity <= 0.0
    {
        return Err(lowering_error(
            balance_relation,
            "2D isotropic elasticity requires finite `mu > 0` and `lambda + mu > 0`",
        ));
    }
    Ok(LoweredIsotropicElasticitySubdomain2d {
        model: IsotropicElasticityCartesianModel2d {
            domain,
            displacement,
            load_potential,
            load_definition_relation: load_relation,
            balance_relation,
            bounds,
            shear_modulus,
            first_lame_parameter: lambda,
            load_potential_expression,
            boundary_inventory: boundary_inventory.inventory.clone(),
            boundary_relations: boundary_inventory.boundary_relations.clone(),
        },
        boundary: boundary_inventory,
    })
}

/// Execute one exact resolved Q1 plan against canonical 2D elasticity.
///
/// The Semantic Model supplies only physical meaning. Cell count, Q1 space,
/// Gauss rule, solver plan, and execution target must already have crossed the
/// typed Realization capability gate.
///
/// # Errors
/// Returns `EQ0807` when provenance or the plan differs from the verified
/// reference envelope, and preserves lowering, assembly, and solver failures.
pub fn solve_resolved_isotropic_elasticity_cartesian_2d(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        IsotropicElasticityCartesianModel2d,
        CartesianLinearElasticity2dSolution,
    ),
    Diagnostic,
> {
    solve_resolved_isotropic_elasticity_cartesian_2d_with_assembly(
        program,
        resolved,
        &REFERENCE_ASSEMBLY_BACKEND,
        backend,
    )
}

/// Execute the same resolved elasticity plan through an explicit assembly
/// adapter and an independently selected linear-solver backend.
///
/// # Errors
/// Preserves the reference entry point diagnostics and each adapter's
/// complete-operation failure.
pub fn solve_resolved_isotropic_elasticity_cartesian_2d_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    assembly: &dyn AssemblyBackend,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        IsotropicElasticityCartesianModel2d,
        CartesianLinearElasticity2dSolution,
    ),
    Diagnostic,
> {
    let (model, finalized) = finalize_resolved_isotropic_elasticity_cartesian_2d_with_assembly(
        program, resolved, assembly,
    )?;
    let solved = backend.solve(&finalized.linear_problem()?, finalized.solver_plan())?;
    Ok((model, finalized.finish(solved)?))
}

/// Finalize one resolved Cartesian Q1 elasticity realization without
/// selecting a linear execution adapter.
///
/// # Errors
/// Preserves canonical lowering, Realization, discretization, boundary, and
/// assembly diagnostics.
pub fn finalize_resolved_isotropic_elasticity_cartesian_2d(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
) -> Result<
    (
        IsotropicElasticityCartesianModel2d,
        FinalizedIsotropicElasticityCartesian2dProblem,
    ),
    Diagnostic,
> {
    finalize_resolved_isotropic_elasticity_cartesian_2d_with_assembly(
        program,
        resolved,
        &REFERENCE_ASSEMBLY_BACKEND,
    )
}

/// Finalize one resolved Cartesian Q1 elasticity realization through an
/// explicit assembly adapter without selecting a solver backend.
///
/// Live physical Port bindings are rejected before mesh construction. Closed
/// package and direct boundary laws have already normalized to the same exact
/// essential-side inventory at this boundary.
///
/// # Errors
/// Returns `EQ0807` for provenance, plan, or unresolved-boundary mismatch and
/// preserves exact lowering and assembly diagnostics.
pub fn finalize_resolved_isotropic_elasticity_cartesian_2d_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    assembly: &dyn AssemblyBackend,
) -> Result<
    (
        IsotropicElasticityCartesianModel2d,
        FinalizedIsotropicElasticityCartesian2dProblem,
    ),
    Diagnostic,
> {
    let execution = require_resolved_cartesian_elasticity_q1_plan_2d(program, resolved)?;
    let model = lower_isotropic_elasticity_cartesian_2d(program)?;
    let mesh = CartesianMesh::uniform(model.bounds(), &[execution.cells; 2])?;
    let finalized = finalize_isotropic_elasticity_cartesian_q1_on_mesh(
        &model,
        &mesh,
        execution.solver,
        assembly,
    )?;
    Ok((model, finalized))
}

pub(crate) fn finalize_isotropic_elasticity_cartesian_q1_on_mesh(
    model: &IsotropicElasticityCartesianModel2d,
    mesh: &CartesianMesh,
    solver: eqiora_solver::SolverPlan,
    assembly: &dyn AssemblyBackend,
) -> Result<FinalizedIsotropicElasticityCartesian2dProblem, Diagnostic> {
    require_two_dimensional_exact_bounds(mesh, model.bounds())?;
    let essential_sides = cartesian_essential_sides(model)?;
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2)?;
    FinalizedIsotropicElasticityCartesian2dProblem::new(
        solver,
        VectorLayoutKind::Replicated,
        Target::HostCpu {
            threads: std::num::NonZeroUsize::MIN,
        },
        finalize_cartesian_q1_linear_elasticity_2d(
            mesh,
            model.shear_modulus(),
            model.first_lame_parameter(),
            model.load_potential_expression(),
            &quadrature,
            essential_sides,
            assembly,
        )?,
    )
}

fn require_two_dimensional_exact_bounds(
    mesh: &CartesianMesh,
    bounds: &[[f64; 2]; 2],
) -> Result<(), Diagnostic> {
    if mesh.topological_dimension() != 2 {
        return Err(invalid_realization(
            "Cartesian Q1 elasticity requires an exact two-dimensional supplied Mesh",
        ));
    }
    for (axis, expected) in bounds.iter().enumerate() {
        let coordinates = mesh
            .axis_coordinates(axis)
            .ok_or_else(|| invalid_realization("supplied Mesh omitted a Cartesian axis"))?;
        let observed = [coordinates[0], coordinates[coordinates.len() - 1]];
        if !observed
            .iter()
            .zip(expected)
            .all(|(observed, expected)| observed.to_bits() == expected.to_bits())
        {
            return Err(invalid_realization(
                "supplied Cartesian Mesh bounds differ from exact elasticity Model bounds",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ResolvedCartesianElasticityQ1Plan {
    cells: usize,
    points_per_axis: usize,
    solver: eqiora_solver::SolverPlan,
    vector_layout: VectorLayoutKind,
    target: Target,
}

fn require_resolved_cartesian_elasticity_q1_plan_2d(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
) -> Result<ResolvedCartesianElasticityQ1Plan, Diagnostic> {
    if program.model() != resolved.model()
        || program.revision().0 != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved realization does not reference this exact Semantic Model revision",
        ));
    }
    resolved.require_admitted_operator_properties(
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )?;
    let requirements = resolved.requirements();
    if requirements.spatial_dimension().get() != 2
        || requirements.scalar_type() != ScalarType::F64
        || requirements.vector_layout() != VectorLayoutKind::Replicated
    {
        return Err(invalid_realization(
            "2D elasticity reference execution requires dimension two, f64, and replicated storage",
        ));
    }
    let plan = resolved.plan();
    if plan.discretization().method() != DiscretizationMethod::ContinuousGalerkin
        || plan.space().family()
            != (SpaceFamily::ContinuousLagrange {
                order: 1.try_into().expect("one is non-zero"),
            })
    {
        return Err(invalid_realization(
            "2D elasticity reference execution requires continuous Q1",
        ));
    }
    let cells = match plan.discretization().mesh() {
        MeshPolicy::GeneratedUniform { cells_per_axis } => cells_per_axis.get(),
        MeshPolicy::ImportedSimplicial { .. } => {
            return Err(invalid_realization(
                "2D elasticity reference execution requires a generated Cartesian mesh",
            ));
        }
        MeshPolicy::SuppliedCartesian { .. } => {
            return Err(invalid_realization(
                "2D elasticity reference execution does not admit a supplied Cartesian mesh",
            ));
        }
    };
    let QuadraturePolicy::GaussLegendre { points_per_axis } = plan.discretization().quadrature()
    else {
        return Err(invalid_realization(
            "Cartesian Q1 elasticity requires Gauss-Legendre quadrature",
        ));
    };
    if points_per_axis.get() != 2 {
        return Err(invalid_realization(
            "Cartesian Q1 elasticity reference execution requires exactly two Gauss-Legendre points per axis",
        ));
    }
    if plan.target()
        != (Target::HostCpu {
            threads: std::num::NonZeroUsize::MIN,
        })
        || plan.schedule() != ExecutionSchedule::Offline
    {
        return Err(invalid_realization(
            "2D elasticity reference execution requires one offline host worker",
        ));
    }

    Ok(ResolvedCartesianElasticityQ1Plan {
        cells,
        points_per_axis: points_per_axis.get(),
        solver: plan.solver(),
        vector_layout: requirements.vector_layout(),
        target: plan.target(),
    })
}

fn cartesian_essential_sides(
    model: &IsotropicElasticityCartesianModel2d,
) -> Result<CartesianEssentialSides2d, Diagnostic> {
    let mut essential = [[false; 2]; 2];
    for (axis, axis_sides) in essential.iter_mut().enumerate() {
        for (side_index, side) in [BoundarySide::Lower, BoundarySide::Upper]
            .into_iter()
            .enumerate()
        {
            axis_sides[side_index] = match model
                .boundary_inventory()
                .boundary(axis, side)
                .expect("a lowered 2D inventory owns every Cartesian side")
                .disposition()
            {
                PhysicalBoundaryDisposition::TraceZero => true,
                PhysicalBoundaryDisposition::FluxZero => false,
                PhysicalBoundaryDisposition::Prescribed(law) => {
                    return Err(invalid_realization(format!(
                        "prescribed elasticity {:?} law {} on axis {axis} {side:?} requires an explicit boundary-data Realization",
                        law.quantity(),
                        law.relation()
                    )));
                }
                PhysicalBoundaryDisposition::PortBinding { connection, .. } => {
                    return Err(invalid_realization(format!(
                        "live elasticity PortBinding {connection} on axis {axis} {side:?} requires a coincident trace-space Realization"
                    )));
                }
            };
        }
    }
    if !essential.iter().flatten().copied().any(|value| value) {
        return Err(invalid_realization(
            "Cartesian Q1 elasticity requires at least one complete homogeneous essential side to remove rigid modes",
        ));
    }
    Ok(CartesianEssentialSides2d::new(essential))
}

fn unique_box_2d(program: &KernelProgram) -> Result<(RawId, [[f64; 2]; 2]), Diagnostic> {
    unique_box::<2>(program)
}

fn unique_box<const D: usize>(
    program: &KernelProgram,
) -> Result<(RawId, [[f64; 2]; D]), Diagnostic> {
    if !matches!(D, 2 | 3) {
        return Err(model_lowering_error(
            program,
            format!("Cartesian elasticity lowering supports dimension two or three, received {D}"),
        ));
    }
    let boxes = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::CartesianBox { .. }) =>
            {
                Some(domain)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if boxes.len() != 1 {
        return Err(model_lowering_error(
            program,
            format!(
                "{D}D isotropic elasticity requires exactly one Cartesian box, found {}",
                boxes.len()
            ),
        ));
    }
    let box_domain = boxes[0];
    let bounds = program.resolved_cartesian_bounds(box_domain.id())?;
    if bounds.len() != D {
        return Err(lowering_error(
            box_domain.id().erase(),
            format!(
                "isotropic elasticity reference lowering requires dimension {D}, received {}",
                bounds.len()
            ),
        ));
    }
    let bounds = bounds
        .iter()
        .map(|bound| [bound.lower().value(), bound.upper().value()])
        .collect::<Vec<_>>()
        .try_into()
        .expect("dimension equality establishes Cartesian bound count");
    Ok((box_domain.id().erase(), bounds))
}

fn exact_fields(program: &KernelProgram, domain: RawId) -> Result<(RawId, RawId), Diagnostic> {
    let fields = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Field(field)
                if has_edge(program, field.id().erase(), domain, EdgeKind::DefinedOn)
                    && continuum_representation(program, field.id().erase()).is_some() =>
            {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err(lowering_error(
            domain,
            format!(
                "2D isotropic elasticity requires exactly two continuum Fields, found {}",
                fields.len()
            ),
        ));
    }
    let vector_shape = ValueShape::new([2]).expect("two components are representable");
    let displacement = fields
        .iter()
        .filter(|field| {
            field.shape() == &vector_shape
                && field.frame() == ValueFrame::SpatialCartesian
                && field.dimension() == LENGTH
        })
        .map(|field| field.id().erase())
        .collect::<Vec<_>>();
    let potential = fields
        .iter()
        .filter(|field| {
            field.shape().is_scalar()
                && field.frame() == ValueFrame::Invariant
                && field.dimension() == PRESSURE
        })
        .map(|field| field.id().erase())
        .collect::<Vec<_>>();
    if displacement.len() == 1
        && potential.len() == 1
        && continuum_representation(program, displacement[0])
            == continuum_representation(program, potential[0])
    {
        Ok((displacement[0], potential[0]))
    } else {
        Err(lowering_error(
            domain,
            "Fields must be exactly one length-valued spatial Cartesian `[2]` displacement and one pressure-valued invariant scalar load potential on the same continuum Representation",
        ))
    }
}

fn balance_root(expression: &ExprDag, root: ExprId, potential: RawId) -> Option<ExprId> {
    let ExprNode::Sub(operator, load) = expression.node(root)? else {
        return None;
    };
    let ExprNode::Neg(divergence) = expression.node(*operator)? else {
        return None;
    };
    let ExprNode::Divergence(stress) = expression.node(*divergence)? else {
        return None;
    };
    let ExprNode::Gradient(argument) = expression.node(*load)? else {
        return None;
    };
    is_field(expression, *argument, potential).then_some(*stress)
}

fn load_definition_root(expression: &ExprDag, root: ExprId, potential: RawId) -> Option<ExprId> {
    let ExprNode::Sub(left, right) = expression.node(root)? else {
        return None;
    };
    is_field(expression, *left, potential).then_some(*right)
}

fn lower_isotropic_stress_coefficients(
    program: &KernelProgram,
    residual: &TypedResidual<RawId>,
    stress: ExprId,
    displacement: RawId,
    owner: RawId,
) -> Result<(ScalarSpatialExpression, ScalarSpatialExpression), Diagnostic> {
    let spatial_dimension = vector_field_dimension(program, displacement, owner)?;
    let expression = residual.expression();
    let ExprNode::Add(left, right) = expression.node(stress).ok_or_else(|| {
        lowering_error(owner, "elastic stress must add shear and volumetric terms")
    })?
    else {
        return Err(lowering_error(
            owner,
            "elastic stress must add shear and volumetric terms",
        ));
    };
    let candidates = [(*left, *right), (*right, *left)];
    for (shear, volumetric) in candidates {
        let Some(two_mu) = lower_scaled_target(
            program,
            residual,
            shear,
            owner,
            spatial_dimension,
            |typed, value| is_symmetric_gradient(typed, value, displacement, owner),
        )?
        else {
            continue;
        };
        let Some(lambda) = lower_scaled_target(
            program,
            residual,
            volumetric,
            owner,
            spatial_dimension,
            |typed, value| is_isotropic_divergence(typed, value, displacement, owner),
        )?
        else {
            continue;
        };
        return Ok((two_mu, lambda));
    }
    Err(lowering_error(
        owner,
        "stress must be `2 * mu * symmetric_part(grad(u)) + lambda * isotropic_lift(div(u))`",
    ))
}

fn lower_scaled_target<F>(
    program: &KernelProgram,
    residual: &TypedResidual<RawId>,
    value: ExprId,
    owner: RawId,
    spatial_dimension: usize,
    target: F,
) -> Result<Option<ScalarSpatialExpression>, Diagnostic>
where
    F: Copy + Fn(&TypedResidual<RawId>, ExprId) -> Result<bool, Diagnostic>,
{
    let expression = residual.expression();
    if target(residual, value)? {
        return Ok(Some(ScalarSpatialExpression::constant(
            spatial_dimension,
            1.0,
        )));
    }
    let Some(ExprNode::Mul(left, right)) = expression.node(value) else {
        return Ok(None);
    };
    for (operator, factor) in [(*left, *right), (*right, *left)] {
        let Some(coefficient) = lower_scaled_target(
            program,
            residual,
            operator,
            owner,
            spatial_dimension,
            target,
        )?
        else {
            continue;
        };
        let factor =
            spatial_expression::lower(program, expression, factor, owner, spatial_dimension)?;
        if factor.constant_value().is_none() {
            return Err(lowering_error(
                owner,
                "Lamé coefficients must be spatially constant scalar expressions",
            ));
        }
        return Ok(Some(coefficient.multiply(factor)));
    }
    Ok(None)
}

fn vector_field_dimension(
    program: &KernelProgram,
    field: RawId,
    owner: RawId,
) -> Result<usize, Diagnostic> {
    let Some(KernelNode::Field(definition)) = program.node(field) else {
        return Err(lowering_error(
            owner,
            "elastic displacement Field is missing",
        ));
    };
    let [extent] = definition.shape().extents() else {
        return Err(lowering_error(
            owner,
            "elastic displacement must have one vector-shape extent",
        ));
    };
    let dimension = usize::try_from(extent.get()).map_err(|_| {
        lowering_error(
            owner,
            "elastic displacement component count exceeds the local target",
        )
    })?;
    if !matches!(dimension, 2 | 3) {
        return Err(lowering_error(
            owner,
            format!("elastic displacement requires dimension two or three, received {dimension}"),
        ));
    }
    Ok(dimension)
}

fn is_symmetric_gradient(
    residual: &TypedResidual<RawId>,
    value: ExprId,
    field: RawId,
    owner: RawId,
) -> Result<bool, Diagnostic> {
    let Some(proof) =
        OperatorApplicationProof::classify(residual, value, StandardPureOperator::SymmetricPart)
            .map_err(|error| calculus_lowering_error(owner, value, "symmetric_part", error))?
    else {
        return Ok(false);
    };
    Ok(matches!(
        residual.expression().node(proof.operand()),
        Some(ExprNode::Gradient(argument))
            if is_field(residual.expression(), *argument, field)
    ))
}

fn is_isotropic_divergence(
    residual: &TypedResidual<RawId>,
    value: ExprId,
    field: RawId,
    owner: RawId,
) -> Result<bool, Diagnostic> {
    let Some(proof) =
        OperatorApplicationProof::classify(residual, value, StandardPureOperator::IsotropicLift)
            .map_err(|error| calculus_lowering_error(owner, value, "isotropic_lift", error))?
    else {
        return Ok(false);
    };
    Ok(matches!(
        residual.expression().node(proof.operand()),
        Some(ExprNode::Divergence(argument))
            if is_field(residual.expression(), *argument, field)
    ))
}

fn typed_relation(
    program: &KernelProgram,
    relation: RawId,
) -> Result<TypedResidual<RawId>, Diagnostic> {
    let relation_id = relation
        .downcast::<kinds::Relation>()
        .ok_or_else(|| lowering_error(relation, "calculus typing owner is not a Relation"))?;
    program
        .typed_relation_residual(relation_id)
        .map_err(|diagnostics| {
            diagnostics.into_iter().next().unwrap_or_else(|| {
                lowering_error(relation, "calculus typing failed without a diagnostic")
            })
        })
}

fn calculus_lowering_error(
    owner: RawId,
    value: ExprId,
    operation: &str,
    error: impl std::fmt::Display,
) -> Diagnostic {
    lowering_error(
        owner,
        format!(
            "{operation} calculus proof failed at expression node {}: {error}",
            value.index()
        ),
    )
}

fn require_continuous_relation(program: &KernelProgram, relation: RawId) -> Result<(), Diagnostic> {
    let activations = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Activates && edge.to() == relation)
        .filter_map(|edge| match program.node(edge.from()) {
            Some(KernelNode::Activation(activation)) => Some(activation),
            _ => None,
        })
        .collect::<Vec<_>>();
    if activations.len() == 1 && matches!(activations[0].kind(), ActivationKind::Continuous) {
        Ok(())
    } else {
        Err(lowering_error(
            relation,
            "elastic-solid Relations require exactly one continuous Activation",
        ))
    }
}

fn require_closed_elasticity_models(
    program: &KernelProgram,
    subdomains: &[LoweredIsotropicElasticitySubdomain2d],
) -> Result<(), Diagnostic> {
    let closures = subdomains
        .iter()
        .map(|lowered| ElasticityClosure {
            domain: lowered.model.domain(),
            fields: vec![lowered.model.displacement(), lowered.model.load_potential()],
            volume_relations: vec![
                lowered.model.load_definition_relation(),
                lowered.model.balance_relation(),
            ],
            boundary_relations: lowered.model.boundary_relations(),
            boundary: &lowered.boundary,
        })
        .collect::<Vec<_>>();
    require_closed_elasticity_parts(program, &closures)
}

struct ElasticityClosure<'a, const D: usize> {
    domain: RawId,
    fields: Vec<RawId>,
    volume_relations: Vec<RawId>,
    boundary_relations: &'a [BoundaryRelationBinding],
    boundary: &'a boundary::LoweredElasticityBoundary<D>,
}

fn require_closed_elasticity_parts<const D: usize>(
    program: &KernelProgram,
    subdomains: &[ElasticityClosure<'_, D>],
) -> Result<(), Diagnostic> {
    let mut expected_domains = BTreeSet::new();
    let mut expected_fields = BTreeSet::new();
    let mut expected_relations = BTreeSet::new();
    let mut expected_representations = BTreeSet::new();
    let mut expected_ports = BTreeSet::new();
    let mut expected_connections = BTreeSet::new();

    for lowered in subdomains {
        expected_domains.insert(lowered.domain);
        expected_domains.extend(
            lowered
                .boundary_relations
                .iter()
                .map(|binding| binding.boundary()),
        );
        expected_domains.extend(lowered.boundary.connector_domains.iter().copied());
        expected_fields.extend(lowered.fields.iter().copied());
        expected_relations.extend(lowered.volume_relations.iter().copied());
        expected_relations.extend(
            lowered
                .boundary_relations
                .iter()
                .map(|binding| binding.relation()),
        );
        expected_representations.insert(
            continuum_representation(program, lowered.fields[0])
                .expect("field validation establishes one continuum Representation"),
        );
        expected_ports.extend(lowered.boundary.ports.iter().copied());
        expected_connections.extend(lowered.boundary.connections.iter().copied());
    }

    let expected_activations = program
        .edges()
        .iter()
        .filter(|edge| {
            edge.kind() == EdgeKind::Activates && expected_relations.contains(&edge.to())
        })
        .map(|edge| edge.from())
        .collect::<BTreeSet<_>>();
    let expected_parameters = expected_relations
        .iter()
        .copied()
        .flat_map(|relation| {
            relation_expression(program, relation)
                .expect("admitted Relations were already inspected")
                .nodes()
                .iter()
        })
        .filter_map(|node| match node {
            ExprNode::Symbol(SymbolRef::Parameter(parameter)) => Some(parameter.erase()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    for node in program.nodes() {
        let admitted = match node {
            KernelNode::Domain(value) => expected_domains.contains(&value.id().erase()),
            KernelNode::Representation(value) => {
                expected_representations.contains(&value.id().erase())
            }
            KernelNode::Field(value) => expected_fields.contains(&value.id().erase()),
            KernelNode::Parameter(value) => expected_parameters.contains(&value.id().erase()),
            KernelNode::Relation(value) => expected_relations.contains(&value.id().erase()),
            KernelNode::Activation(value) => expected_activations.contains(&value.id().erase()),
            KernelNode::Port(value) => expected_ports.contains(&value.id().erase()),
            KernelNode::Connection(value) => expected_connections.contains(&value.id().erase()),
            KernelNode::ClockDomain(_) => false,
            _ => false,
        };
        if !admitted {
            return Err(model_lowering_error(
                program,
                format!(
                    "closed 2D elasticity-family lowering would ignore unexpected {:?} node {}",
                    node.kind(),
                    node.id()
                ),
            ));
        }
    }
    Ok(())
}

fn continuum_representation(program: &KernelProgram, field: RawId) -> Option<RawId> {
    let representations = program
        .edges()
        .iter()
        .filter(|edge| edge.from() == field && edge.kind() == EdgeKind::DefinedOn)
        .filter_map(|edge| match program.node(edge.to()) {
            Some(KernelNode::Representation(representation))
                if representation.kind() == RepresentationKind::Continuum =>
            {
                Some(edge.to())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    (representations.len() == 1).then(|| representations[0])
}

fn relations_on(program: &KernelProgram, domain: RawId) -> Vec<RawId> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::AppliesOn && edge.to() == domain)
        .map(|edge| edge.from())
        .collect()
}

fn relation_expression(program: &KernelProgram, relation: RawId) -> Result<&ExprDag, Diagnostic> {
    match program.node(relation) {
        Some(KernelNode::Relation(relation)) => Ok(relation.residuals()),
        _ => Err(lowering_error(
            relation,
            "AppliesOn source has no Relation definition",
        )),
    }
}

fn unique_root(expression: &ExprDag, owner: RawId) -> Result<ExprId, Diagnostic> {
    if expression.roots().len() == 1 {
        Ok(expression.roots()[0])
    } else {
        Err(lowering_error(
            owner,
            "elasticity Relation requires exactly one residual root",
        ))
    }
}

fn is_field(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    matches!(
        expression.node(value),
        Some(ExprNode::Symbol(SymbolRef::Field(id))) if id.erase() == field
    )
}

fn has_edge(program: &KernelProgram, from: RawId, to: RawId, kind: EdgeKind) -> bool {
    program
        .edges()
        .iter()
        .any(|edge| edge.from() == from && edge.to() == to && edge.kind() == kind)
}

fn lowering_error(owner: RawId, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        owner.kind().graph().name().to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
    ]))
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn model_lowering_error(program: &KernelProgram, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        "ontology-view".to_owned(),
        "eqiora.model/v1".to_owned(),
        program.model().to_string(),
    ]))
}
