use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora_artifact::{CartesianMeshEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1};
use eqiora_assembly::{AssemblyBackend, REFERENCE_ASSEMBLY_BACKEND};
use eqiora_core::Id;
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, GraphPath, RawId};
use eqiora_geometry::{CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{
    LineMesh, MeshTopology, OrientationCode, QuadratureRule, SimplicialMesh, simplex_centroid_rule,
};
use eqiora_realization::{
    Discretization, DiscretizationMethod, MeshArtifactReference, MeshPolicy,
    PlacementRequirementNode, QuadraturePolicy, ResolvedRealization, SingleFieldOperatorClaim,
    SolveRoot, Space, SpaceFamily, Target, VectorLayoutKind,
};
use eqiora_schema::kernel::{
    BoundarySide, DomainKind, ExprDag, ExprId, ExprNode, KernelNode, RepresentationKind, SymbolRef,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    CanonicalCsrSystemView, LinearOperatorProperties, LinearProblem, LinearSolution,
    LinearSolveRequest, LinearSolver, LinearSolverBackend, REFERENCE_LINEAR_SOLVER, ScalarType,
    SolverPlan,
};

use crate::assembled_linearization::AssembledLinearizedRelation;
use crate::cartesian_elliptic::{
    ScalarEllipticCartesianFemSolution, ScalarEllipticCartesianFvmSolution,
    linearize_scalar_elliptic_cartesian_fem, linearize_scalar_elliptic_cartesian_fem_output,
    linearize_scalar_elliptic_cartesian_fvm, linearize_scalar_elliptic_cartesian_fvm_output,
};
use crate::cartesian_elliptic::{
    finalize_scalar_elliptic_cartesian_fem, finalize_scalar_elliptic_cartesian_fvm,
};
use crate::elliptic::{
    ScalarBoundaryCondition1d, ScalarBoundaryPair1d, ScalarEllipticSolution1d,
    solve_scalar_elliptic_linear_fem, solve_scalar_elliptic_linear_fem_with_assembly,
};
use crate::finalized_spatial::FinalizedScalarEllipticCartesianProblem;
use crate::form_compiler::{DerivedScalarGalerkinForm, derive_candidate};
use crate::linearized_output::CartesianScalarFieldLinearization;
use crate::poisson::{
    DirichletBoundary1d, ScalarEllipticComparisonRow1d, ScalarEllipticFvmSolution1d,
};
use crate::poisson::{compare_scalar_elliptic_dirichlet_1d, solve_scalar_elliptic_cell_fvm};
use crate::simplicial_elliptic::{
    ScalarEllipticSimplicialFemSolution, solve_scalar_elliptic_simplicial_fem_with_assembly,
};
use crate::spatial_expression::{self, ScalarSpatialExpression};
use eqiora_meshing::CartesianMesh;

/// Boundary meaning of a canonical scalar elliptic Cartesian model.
#[derive(Debug, Clone, PartialEq)]
pub enum ScalarEllipticCartesianBoundary {
    /// Prescribed trace value.
    Essential(ScalarSpatialExpression),
    /// Prescribed outward constitutive flux.
    Natural(ScalarSpatialExpression),
}

impl ScalarEllipticCartesianBoundary {
    /// Canonical boundary-data expression in full physical coordinates.
    #[must_use]
    pub const fn value(&self) -> &ScalarSpatialExpression {
        match self {
            Self::Essential(value) | Self::Natural(value) => value,
        }
    }

    /// Whether this condition prescribes the field trace.
    #[must_use]
    pub const fn is_essential(&self) -> bool {
        matches!(self, Self::Essential(_))
    }
}

/// Method-neutral lowered scalar elliptic model on a Cartesian box.
///
/// Dimension is carried once by `bounds`; source and boundary tapes require
/// exactly that many physical coordinates. Numerical method, mesh density,
/// quadrature, solver, and backend remain outside this semantic lowering.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticCartesianModel {
    semantic_model: eqiora_core::OntologyId<eqiora_schema::Model>,
    semantic_revision: u64,
    domain: RawId,
    field: RawId,
    bounds: Vec<[f64; 2]>,
    coefficient: ScalarSpatialExpression,
    source: ScalarSpatialExpression,
    boundaries: BTreeMap<(usize, BoundarySide), ScalarEllipticCartesianBoundary>,
    parameter_fields: Vec<Id<kinds::Parameter>>,
    parameter_values: Vec<f64>,
    compiled_form: Option<DerivedScalarGalerkinForm>,
}

/// One lowered Parameter point paired with the exact finalized system it produced.
///
/// The wrapper prevents execution adapters from separating a bound model from
/// its finalized operator before solve and reconstruction.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedScalarEllipticParameterPoint {
    model: ScalarEllipticCartesianModel,
    realization: ResolvedRealization,
    problem: FinalizedScalarEllipticCartesianProblem,
}

impl FinalizedScalarEllipticParameterPoint {
    /// Exact portable execution graph retained by the finalized problem.
    #[must_use]
    pub const fn portable_realization(&self) -> &eqiora_realization::PortableRealizationGraph {
        self.problem.portable_realization()
    }

    /// Complete canonical sparse system admitted for execution.
    #[must_use]
    pub fn canonical_csr_system_view(&self) -> &CanonicalCsrSystemView {
        self.problem.canonical_csr_system_view()
    }

    /// Sole solver policy selected by the Realization.
    #[must_use]
    pub const fn solver_plan(&self) -> SolverPlan {
        self.problem.solver_plan()
    }

    /// Borrow the finalized system through the common solver contract.
    ///
    /// # Errors
    /// Preserves finalized-system consistency diagnostics.
    pub fn linear_problem(&self) -> Result<LinearProblem<'_>, Diagnostic> {
        self.problem.linear_problem()
    }

    /// Reaccept and reconstruct one solution without losing its bound model.
    ///
    /// # Errors
    /// Preserves the finalized problem's solution diagnostics.
    pub fn finish(
        self,
        solution: LinearSolution,
    ) -> Result<AcceptedScalarEllipticParameterPoint, Diagnostic> {
        let solution = self.problem.finish(solution)?;
        Ok(AcceptedScalarEllipticParameterPoint {
            model: self.model,
            realization: self.realization,
            solution,
        })
    }
}

/// One inseparable bound model, Realization, and accepted method-native solution.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedScalarEllipticParameterPoint {
    model: ScalarEllipticCartesianModel,
    realization: ResolvedRealization,
    solution: ResolvedScalarEllipticCartesianSolution,
}

impl AcceptedScalarEllipticParameterPoint {
    /// Accepted method-native solution at this exact point.
    #[must_use]
    pub const fn solution(&self) -> &ResolvedScalarEllipticCartesianSolution {
        &self.solution
    }

    /// Linearize the accepted relation and complete primary Field at this point.
    ///
    /// The bound model, Realization, and solution cannot be selected
    /// independently, so point identity never has to be inferred from
    /// potentially non-injective assembled bytes.
    ///
    /// # Errors
    /// Preserves Realization, selected-coordinate, and derivative-assembly
    /// diagnostics.
    pub fn linearize(
        &self,
        selected_coordinates: &[crate::spatial_design::SpatialDesignCoordinate],
    ) -> Result<
        (
            AssembledLinearizedRelation,
            CartesianScalarFieldLinearization,
        ),
        Diagnostic,
    > {
        linearize_owned_scalar_elliptic_parameter_point(
            &self.model,
            &self.realization,
            &self.solution,
            selected_coordinates,
        )
    }
}

impl ScalarEllipticCartesianModel {
    /// Canonical volume Domain.
    #[must_use]
    pub const fn domain(&self) -> RawId {
        self.domain
    }

    /// Typed canonical volume Domain identifier.
    #[must_use]
    pub fn domain_id(&self) -> Id<kinds::Domain> {
        self.domain
            .downcast()
            .expect("Cartesian lowering stores one Domain identifier")
    }

    /// Canonical scalar Field.
    #[must_use]
    pub const fn field(&self) -> RawId {
        self.field
    }

    /// Typed canonical scalar Field identifier.
    #[must_use]
    pub fn field_id(&self) -> Id<kinds::Field> {
        self.field
            .downcast()
            .expect("Cartesian lowering stores one Field identifier")
    }

    /// Physical Cartesian bounds in coherent SI coordinates.
    #[must_use]
    pub fn bounds(&self) -> &[[f64; 2]] {
        &self.bounds
    }

    /// Runtime spatial dimension.
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.bounds.len()
    }

    /// Positive constant constitutive coefficient in coherent SI units.
    #[must_use]
    pub fn coefficient(&self) -> f64 {
        self.coefficient
            .constant_value()
            .expect("Cartesian lowerer accepts a spatially constant coefficient")
    }

    /// Canonical constitutive-coefficient expression at this revision.
    #[must_use]
    pub const fn coefficient_expression(&self) -> &ScalarSpatialExpression {
        &self.coefficient
    }

    /// Canonical scalar source expression.
    #[must_use]
    pub const fn source(&self) -> &ScalarSpatialExpression {
        &self.source
    }

    /// All canonical Parameters affecting coefficient, source, or boundary
    /// data, in deterministic dense differentiation order.
    #[must_use]
    pub fn parameter_fields(&self) -> &[Id<kinds::Parameter>] {
        &self.parameter_fields
    }

    /// Complete numerical point matching [`Self::parameter_fields`].
    ///
    /// Immediately after lowering this is the revision-local Model point.
    /// [`Self::bind_selected_parameters`] returns a new model with selected
    /// entries replaced and every unselected entry frozen here.
    #[must_use]
    pub fn parameter_values(&self) -> &[f64] {
        &self.parameter_values
    }

    /// Bind an ordered selection of canonical Parameters to one immutable
    /// numerical point.
    ///
    /// Parameters outside `selected` retain their values from this lowered
    /// model. The returned model owns the complete resulting point; neither
    /// this model nor the canonical Semantic Model is mutated.
    ///
    /// # Errors
    /// Returns `EQ0704` for an empty, mismatched, duplicate, foreign, or
    /// non-finite selection, or when the bound elliptic coefficient is not
    /// finite and positive.
    pub fn bind_selected_parameters(
        &self,
        selected: &[Id<kinds::Parameter>],
        values: &[f64],
    ) -> Result<Self, Diagnostic> {
        if selected.is_empty() {
            return Err(invalid_parameter_binding(
                "scalar elliptic Parameter binding requires at least one selected Parameter",
            ));
        }
        if selected.len() != values.len() {
            return Err(invalid_parameter_binding(format!(
                "scalar elliptic Parameter binding contains {} identities and {} values",
                selected.len(),
                values.len()
            )));
        }
        if selected
            .iter()
            .enumerate()
            .any(|(index, field)| selected[..index].contains(field))
        {
            return Err(invalid_parameter_binding(
                "scalar elliptic Parameter binding contains a duplicate identity",
            ));
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_parameter_binding(
                "scalar elliptic Parameter binding requires finite values",
            ));
        }

        let mut parameter_values = self.parameter_values.clone();
        for (field, value) in selected.iter().zip(values) {
            let Some(index) = self
                .parameter_fields
                .iter()
                .position(|candidate| candidate == field)
            else {
                return Err(invalid_parameter_binding(
                    "selected Parameter does not belong to this lowered scalar elliptic model",
                ));
            };
            parameter_values[index] = *value;
        }

        let coefficient = self
            .coefficient
            .bind_parameter_point(&self.parameter_fields, &parameter_values)?;
        let coefficient_value = coefficient.constant_value().ok_or_else(|| {
            invalid_parameter_binding(
                "bound scalar elliptic coefficient is not a finite spatial constant",
            )
        })?;
        if coefficient_value <= 0.0 {
            return Err(invalid_parameter_binding(
                "bound scalar elliptic coefficient must be positive",
            ));
        }
        let source = self
            .source
            .bind_parameter_point(&self.parameter_fields, &parameter_values)?;
        let boundaries = self
            .boundaries
            .iter()
            .map(|(&side, boundary)| {
                let value = boundary
                    .value()
                    .bind_parameter_point(&self.parameter_fields, &parameter_values)?;
                let boundary = match boundary {
                    ScalarEllipticCartesianBoundary::Essential(_) => {
                        ScalarEllipticCartesianBoundary::Essential(value)
                    }
                    ScalarEllipticCartesianBoundary::Natural(_) => {
                        ScalarEllipticCartesianBoundary::Natural(value)
                    }
                };
                Ok((side, boundary))
            })
            .collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;

        Ok(Self {
            coefficient,
            source,
            boundaries,
            parameter_values,
            ..self.clone()
        })
    }

    /// Boundary condition on one Cartesian axis side.
    #[must_use]
    pub fn boundary(
        &self,
        axis: usize,
        side: BoundarySide,
    ) -> Option<&ScalarEllipticCartesianBoundary> {
        self.boundaries.get(&(axis, side))
    }

    /// Evaluate the constitutive coefficient and one canonical-Parameter JVP.
    ///
    /// # Errors
    /// Preserves lowered-expression shape and finite-value diagnostics.
    pub fn coefficient_jvp(&self, parameter_tangent: &[f64]) -> Result<(f64, f64), Diagnostic> {
        self.evaluate_expression_jvp(
            &self.coefficient,
            &vec![0.0; self.dimension()],
            &vec![0.0; self.dimension()],
            parameter_tangent,
        )
    }

    /// Evaluate source meaning and its coordinate/Parameter JVP.
    ///
    /// `parameter_tangent` follows [`Self::parameter_fields`] order. This
    /// method is method-neutral and may be consumed by Cartesian or imported
    /// mesh realizations.
    ///
    /// # Errors
    /// Preserves lowered-expression shape and finite-value diagnostics.
    pub fn source_jvp(
        &self,
        coordinates: &[f64],
        coordinate_tangent: &[f64],
        parameter_tangent: &[f64],
    ) -> Result<(f64, f64), Diagnostic> {
        self.evaluate_expression_jvp(
            &self.source,
            coordinates,
            coordinate_tangent,
            parameter_tangent,
        )
    }

    /// Evaluate compatible essential trace data and its total JVP at a point
    /// on the canonical box boundary.
    ///
    /// # Errors
    /// Returns a lowering diagnostic for an interior point, nonessential data,
    /// incompatible edge/corner traces, shape mismatch, or non-finite data.
    pub fn essential_boundary_jvp(
        &self,
        coordinates: &[f64],
        coordinate_tangent: &[f64],
        parameter_tangent: &[f64],
    ) -> Result<(f64, f64), Diagnostic> {
        if coordinates.len() != self.dimension()
            || coordinate_tangent.len() != self.dimension()
            || parameter_tangent.len() != self.parameter_fields.len()
        {
            return Err(lowering_error(
                self.domain(),
                "Cartesian boundary action received an incompatible coordinate or tangent shape",
            ));
        }
        let mut value: Option<(f64, f64)> = None;
        for (axis, bounds) in self.bounds().iter().enumerate() {
            for (side, coordinate) in [
                (BoundarySide::Lower, bounds[0]),
                (BoundarySide::Upper, bounds[1]),
            ] {
                if coordinates[axis].to_bits() != coordinate.to_bits() {
                    continue;
                }
                let boundary = self
                    .boundary(axis, side)
                    .expect("Cartesian lowerer produces a complete boundary set");
                let ScalarEllipticCartesianBoundary::Essential(expression) = boundary else {
                    return Err(lowering_error(
                        self.domain(),
                        "Cartesian numerical path requires essential boundary data",
                    ));
                };
                let candidate = self.evaluate_expression_jvp(
                    expression,
                    coordinates,
                    coordinate_tangent,
                    parameter_tangent,
                )?;
                if let Some(previous) = value {
                    for (left, right) in [(previous.0, candidate.0), (previous.1, candidate.1)] {
                        let scale = left.abs().max(right.abs()).max(1.0);
                        if (left - right).abs() > 256.0 * f64::EPSILON * scale {
                            return Err(lowering_error(
                                self.domain(),
                                "Cartesian boundary expressions or their design actions disagree at an edge or corner",
                            ));
                        }
                    }
                } else {
                    value = Some(candidate);
                }
            }
        }
        value.ok_or_else(|| {
            lowering_error(
                self.domain(),
                "Cartesian boundary action point is not on the box boundary",
            )
        })
    }

    fn evaluate_expression_jvp(
        &self,
        expression: &ScalarSpatialExpression,
        coordinates: &[f64],
        coordinate_tangent: &[f64],
        parameter_tangent: &[f64],
    ) -> Result<(f64, f64), Diagnostic> {
        if parameter_tangent.len() != self.parameter_fields.len() {
            return Err(lowering_error(
                self.domain(),
                format!(
                    "Cartesian model expects {} Parameter tangents, received {}",
                    self.parameter_fields.len(),
                    parameter_tangent.len()
                ),
            ));
        }
        let local_tangent = expression
            .parameter_fields()
            .iter()
            .map(|field| {
                let index = self
                    .parameter_fields
                    .iter()
                    .position(|candidate| candidate == field)
                    .expect("model Parameter coordinates contain every expression Parameter");
                parameter_tangent[index]
            })
            .collect::<Vec<_>>();
        expression.evaluate_jvp(coordinates, coordinate_tangent, &local_tangent)
    }
}

/// Method-neutral lowered form of one canonical scalar elliptic model on an
/// interval.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarEllipticModel1d {
    domain: RawId,
    field: RawId,
    interval: [f64; 2],
    coefficient: f64,
    source: ScalarSpatialExpression,
    boundary: ScalarBoundaryPair1d,
}

impl ScalarEllipticModel1d {
    /// Canonical volume Domain.
    #[must_use]
    pub const fn domain(&self) -> RawId {
        self.domain
    }

    /// Typed canonical interval Domain identifier.
    #[must_use]
    pub fn domain_id(&self) -> Id<kinds::Domain> {
        self.domain
            .downcast()
            .expect("interval lowering stores one Domain identifier")
    }

    /// Canonical scalar Field.
    #[must_use]
    pub const fn field(&self) -> RawId {
        self.field
    }

    /// Typed canonical scalar Field identifier.
    #[must_use]
    pub fn field_id(&self) -> Id<kinds::Field> {
        self.field
            .downcast()
            .expect("interval lowering stores one Field identifier")
    }

    /// Physical interval bounds in metres.
    #[must_use]
    pub const fn interval(&self) -> [f64; 2] {
        self.interval
    }

    /// Constant coefficient in coherent SI units.
    #[must_use]
    pub const fn coefficient(&self) -> f64 {
        self.coefficient
    }

    /// Canonical scalar source expression in coherent SI coordinates.
    #[must_use]
    pub const fn source(&self) -> &ScalarSpatialExpression {
        &self.source
    }

    /// Essential/natural endpoint semantics.
    #[must_use]
    pub const fn boundary(&self) -> ScalarBoundaryPair1d {
        self.boundary
    }
}

/// Explicit policy for the built-in default interval realization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultScalarElliptic1dConfig {
    /// Equal affine cells in the generated line mesh.
    pub cells: usize,
    /// Backend-neutral linear solver policy.
    pub solver: SolverPlan,
}

impl Default for DefaultScalarElliptic1dConfig {
    fn default() -> Self {
        Self {
            cells: 16,
            solver: SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-13,
                1.0e-14,
                512.try_into().expect("512 is nonzero"),
            )
            .expect("default solver policy is valid"),
        }
    }
}

/// Method-specific result selected from one typed Realization plan.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedScalarEllipticSolution1d {
    /// Continuous P1 Galerkin finite-element result.
    FiniteElement(ScalarEllipticSolution1d),
    /// Cell-centred two-point-flux finite-volume result.
    FiniteVolume(ScalarEllipticFvmSolution1d),
}

/// Method-specific Cartesian result selected from one typed Realization plan.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedScalarEllipticCartesianSolution {
    /// Continuous Q1 Galerkin finite-element result.
    FiniteElement(ScalarEllipticCartesianFemSolution),
    /// Cell-centred orthogonal two-point-flux finite-volume result.
    FiniteVolume(ScalarEllipticCartesianFvmSolution),
}

impl ResolvedScalarEllipticCartesianSolution {
    /// Consume the method-native solution into its complete primary Field.
    #[must_use]
    pub fn into_primary_field_values(self) -> Vec<f64> {
        match self {
            Self::FiniteElement(solution) => solution.into_field_values(),
            Self::FiniteVolume(solution) => solution.into_field_values(),
        }
    }
}

/// Lower a validated canonical model into a method-neutral interval problem.
///
/// V0 intentionally accepts one scalar continuum Field, one strong volume
/// Relation `-div(k grad(u)) - f = 0`, and one explicit essential or natural
/// Relation on each interval boundary. Its coefficient and boundary data are
/// spatially constant; its source may contain `coordinate(0)`, scalar
/// arithmetic, and dimensionally valid unary mathematics. Unsupported forms
/// fail rather than silently changing meaning.
///
/// # Errors
/// Returns `EQ0703` when the validated model is outside this precise lowering
/// subset or is ambiguous.
pub fn lower_scalar_elliptic_1d(
    program: &KernelProgram,
) -> Result<ScalarEllipticModel1d, Diagnostic> {
    let model = lower_scalar_elliptic_cartesian(program)?;
    if model.dimension() != 1 {
        return Err(lowering_error(
            model.domain(),
            format!(
                "scalar elliptic 1D lowering requires dimension one, received {}",
                model.dimension()
            ),
        ));
    }
    let ScalarEllipticCartesianModel {
        semantic_model: _,
        semantic_revision: _,
        domain,
        field,
        bounds,
        coefficient,
        source,
        boundaries,
        ..
    } = model;
    let lower = lower_constant_boundary_1d(
        boundaries
            .get(&(0, BoundarySide::Lower))
            .expect("Cartesian lowerer produces a complete boundary set"),
        domain,
    )?;
    let upper = lower_constant_boundary_1d(
        boundaries
            .get(&(0, BoundarySide::Upper))
            .expect("Cartesian lowerer produces a complete boundary set"),
        domain,
    )?;
    let boundary = ScalarBoundaryPair1d::new(lower, upper)
        .map_err(|diagnostic| lowering_error(domain, diagnostic.message()))?;

    Ok(ScalarEllipticModel1d {
        domain,
        field,
        interval: bounds[0],
        coefficient: coefficient
            .constant_value()
            .expect("Cartesian lowerer accepts a spatially constant coefficient"),
        source,
        boundary,
    })
}

/// Lower a validated canonical scalar elliptic Relation on one runtime-
/// dimensional Cartesian box.
///
/// The admitted volume form is `-div(k grad(u)) - f = 0` with one positive
/// constant coefficient. Exactly one explicit trace or normal-flux Relation
/// is required on every axis side. Source and boundary data lower to the same
/// immutable, dimension-checked spatial-expression contract.
///
/// # Errors
/// Returns `EQ0703` when the model is ambiguous or outside this precise
/// canonical subset.
pub fn lower_scalar_elliptic_cartesian(
    program: &KernelProgram,
) -> Result<ScalarEllipticCartesianModel, Diagnostic> {
    let (domain, bounds) = unique_cartesian_box(program)?;
    let boundary_domains = program
        .nodes()
        .filter_map(|node| {
            let KernelNode::Domain(boundary) = node else {
                return None;
            };
            let DomainKind::CartesianBoundary { axis, side } = boundary.kind() else {
                return None;
            };
            (boundary_parent(program, boundary.id().erase()) == Some(domain))
                .then_some(((*axis, *side), boundary.id().erase()))
        })
        .collect();
    lower_scalar_elliptic_cartesian_support(program, domain, bounds, boundary_domains)
}

pub(crate) fn lower_scalar_elliptic_cartesian_with_resources(
    program: &KernelProgram,
    geometry: &CanonicalGeometryV1,
    mesh: &CartesianMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
) -> Result<ScalarEllipticCartesianModel, Diagnostic> {
    let (domain, bounds, boundary_domains) =
        scalar_geometry_cartesian_support(program, geometry, mesh, correspondence)?;
    lower_scalar_elliptic_cartesian_support(program, domain, bounds, boundary_domains)
}

pub(crate) fn recognize_scalar_elliptic_geometry_mathematics(
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
            "geometry-backed scalar elliptic recognition requires exactly one GeometryRegion",
        ));
    };
    let compiled_form = derive_candidate(program, *domain)?;
    let field = unique_continuum_field(program, *domain)?;
    let volume_relation = unique_relation_on(program, *domain)?;
    let (coefficient, source) = lower_volume_relation(program, volume_relation, field, 2)?;
    let coefficient_value = coefficient
        .constant_value()
        .expect("flux coefficient is validated as spatially constant");
    if !coefficient_value.is_finite() || coefficient_value <= 0.0 {
        return Err(lowering_error(
            volume_relation,
            "elliptic coefficient must be finite and positive",
        ));
    }
    let boundaries = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(boundary)
                if matches!(boundary.kind(), DomainKind::GeometryBoundary { .. })
                    && boundary_parent(program, boundary.id().erase()) == Some(*domain) =>
            {
                Some(boundary.id().erase())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if boundaries.len() != 4 {
        return Err(lowering_error(
            *domain,
            "geometry-backed 2D scalar elliptic recognition requires four boundary supports",
        ));
    }
    let mut lowered = BTreeMap::new();
    for boundary in boundaries {
        let relation = unique_relation_on(program, boundary)?;
        lowered.insert(
            boundary,
            lower_cartesian_boundary_relation(program, relation, field, &coefficient, 2)?,
        );
    }
    let _ = (compiled_form, source, lowered);
    Ok(())
}

fn lower_scalar_elliptic_cartesian_support(
    program: &KernelProgram,
    domain: RawId,
    bounds: Vec<[f64; 2]>,
    boundary_domains: BTreeMap<(usize, BoundarySide), RawId>,
) -> Result<ScalarEllipticCartesianModel, Diagnostic> {
    let compiled_form = derive_candidate(program, domain)?;
    let dimension = bounds.len();
    let field = unique_continuum_field(program, domain)?;
    let volume_relation = unique_relation_on(program, domain)?;
    let (coefficient, source) = lower_volume_relation(program, volume_relation, field, dimension)?;
    let coefficient_value = coefficient
        .constant_value()
        .expect("flux coefficient is validated as spatially constant");
    if !coefficient_value.is_finite() || coefficient_value <= 0.0 {
        return Err(lowering_error(
            volume_relation,
            "elliptic coefficient must be finite and positive",
        ));
    }

    let mut boundaries = BTreeMap::new();
    for ((axis, side), boundary_domain) in boundary_domains {
        let relation = unique_relation_on(program, boundary_domain)?;
        let condition =
            lower_cartesian_boundary_relation(program, relation, field, &coefficient, dimension)?;
        if boundaries.insert((axis, side), condition).is_some() {
            return Err(lowering_error(
                boundary_domain,
                "scalar elliptic support has a duplicate canonical boundary side",
            ));
        }
    }
    for axis in 0..dimension {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            if !boundaries.contains_key(&(axis, side)) {
                return Err(lowering_error(
                    domain,
                    format!(
                        "scalar elliptic support requires an explicit Relation on axis {axis} {side:?} boundary"
                    ),
                ));
            }
        }
    }

    let (parameter_fields, parameter_values) =
        collect_parameter_coordinates(&coefficient, &source, &boundaries);
    Ok(ScalarEllipticCartesianModel {
        semantic_model: program.model(),
        semantic_revision: program.revision().0,
        domain,
        field,
        bounds,
        coefficient,
        source,
        boundaries,
        parameter_fields,
        parameter_values,
        compiled_form,
    })
}

type ScalarCartesianSupport = (RawId, Vec<[f64; 2]>, BTreeMap<(usize, BoundarySide), RawId>);

fn scalar_geometry_cartesian_support(
    program: &KernelProgram,
    geometry: &CanonicalGeometryV1,
    mesh: &CartesianMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
) -> Result<ScalarCartesianSupport, Diagnostic> {
    let bounds = geometry.planar_rectangle_bounds().ok_or_else(|| {
        model_lowering_error(
            program,
            "geometry-backed scalar elliptic lowering requires exact PlanarRectangleV2",
        )
    })?;
    let regions = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::GeometryRegion { .. }) =>
            {
                Some(domain)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if regions.len() != 1 {
        return Err(model_lowering_error(
            program,
            format!(
                "geometry-backed scalar elliptic lowering requires one GeometryRegion, found {}",
                regions.len()
            ),
        ));
    }
    let region = regions[0];
    let DomainKind::GeometryRegion {
        geometry: digest,
        entity_set,
    } = region.kind()
    else {
        unreachable!("GeometryRegion filter is exact")
    };
    let region_set = geometry.entity_set(entity_set).ok_or_else(|| {
        lowering_error(
            region.id().erase(),
            "Model GeometryRegion entity set is absent from exact rectangle Geometry",
        )
    })?;
    if digest.bytes() != geometry.digest_bytes()
        || region_set.dimension() != FACE_DIMENSION
        || region_set.members() != [0]
    {
        return Err(lowering_error(
            region.id().erase(),
            "Model GeometryRegion differs from the exact rectangle source face",
        ));
    }
    let domain = region.id().erase();
    let mut boundary_domains = BTreeMap::new();
    for node in program.nodes() {
        let KernelNode::Domain(boundary) = node else {
            continue;
        };
        let DomainKind::GeometryBoundary { entity_set } = boundary.kind() else {
            continue;
        };
        if boundary_parent(program, boundary.id().erase()) != Some(domain) {
            continue;
        }
        let facets =
            correspondence.planar_rectangle_v2_entity_set_entities(geometry, entity_set)?;
        let mut side = None;
        for facet in facets {
            if facet.dimension() != EDGE_DIMENSION {
                return Err(lowering_error(
                    boundary.id().erase(),
                    "rectangle GeometryBoundary correspondence contains a non-facet entity",
                ));
            }
            let adjacent = mesh
                .mesh()
                .incidence(facet, FACE_DIMENSION)
                .ok_or_else(|| {
                    lowering_error(
                        boundary.id().erase(),
                        "rectangle boundary facet has no parent-cell incidence",
                    )
                })?;
            let [parent] = adjacent.as_slice() else {
                return Err(lowering_error(
                    boundary.id().erase(),
                    "rectangle boundary facet does not have exactly one parent cell",
                ));
            };
            if parent.orientation != OrientationCode::identity() {
                return Err(lowering_error(
                    boundary.id().erase(),
                    "rectangle boundary facet has noncanonical orientation",
                ));
            }
            let facet_side = match parent.local_ordinal {
                0 => (1, BoundarySide::Lower),
                1 => (1, BoundarySide::Upper),
                2 => (0, BoundarySide::Lower),
                3 => (0, BoundarySide::Upper),
                _ => {
                    return Err(lowering_error(
                        boundary.id().erase(),
                        "rectangle boundary facet has an unsupported local side ordinal",
                    ));
                }
            };
            if side
                .replace(facet_side)
                .is_some_and(|old| old != facet_side)
            {
                return Err(lowering_error(
                    boundary.id().erase(),
                    "one rectangle source boundary maps to multiple topology sides",
                ));
            }
        }
        let Some(side) = side else {
            return Err(lowering_error(
                boundary.id().erase(),
                "rectangle GeometryBoundary correspondence is empty",
            ));
        };
        if boundary_domains
            .insert(side, boundary.id().erase())
            .is_some()
        {
            return Err(lowering_error(
                boundary.id().erase(),
                "rectangle GeometryBoundary side is duplicated",
            ));
        }
    }
    Ok((domain, bounds.to_vec(), boundary_domains))
}

/// Select the built-in default realization and execute it.
///
/// The default is continuous P1 FEM on an equal-cell affine line mesh with
/// two-point Gauss integration. It is a declared policy, not canonical model
/// meaning.
///
/// # Errors
/// Returns a lowering or numerical diagnostic for unsupported semantics,
/// invalid configuration, assembly, or solution failure.
pub fn solve_default_scalar_elliptic_1d(
    program: &KernelProgram,
    config: DefaultScalarElliptic1dConfig,
) -> Result<(ScalarEllipticModel1d, ScalarEllipticSolution1d), Diagnostic> {
    let model = lower_scalar_elliptic_1d(program)?;
    let [start, end] = model.interval();
    let mesh = LineMesh::uniform(start, end, config.cells)?;
    let coefficient = model.coefficient();
    let source = model.source();
    let solution = solve_scalar_elliptic_linear_fem(
        &mesh,
        &move |_| coefficient,
        &move |coordinate| source.evaluate(&[coordinate]).unwrap_or(f64::NAN),
        model.boundary(),
        &QuadratureRule::gauss_legendre(2)?,
        LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, config.solver),
    )?;
    Ok((model, solution))
}

/// Execute one already-resolved plan against its exact Semantic Model revision.
///
/// The plan selects numerical method, mesh, quadrature, and solver without
/// changing the canonical interval, coefficient, source, or boundary meaning.
/// V0 supports P1 continuous Galerkin FEM and cell-constant, cell-centred FVM
/// on generated uniform line meshes. FVM presently requires two essential
/// endpoint conditions.
///
/// # Errors
/// Returns `EQ0807` when provenance does not identify `program` or when the
/// resolved plan is outside this lowerer's precise v0 capability. Canonical
/// lowering and numerical failures retain their own diagnostic codes.
pub fn solve_resolved_scalar_elliptic_1d(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    backend: &dyn LinearSolverBackend,
) -> Result<(ScalarEllipticModel1d, ResolvedScalarEllipticSolution1d), Diagnostic> {
    solve_resolved_scalar_elliptic_1d_impl(program, resolved, backend, None)
}

/// Execute one resolved P1 FEM plan through an explicit assembly backend.
///
/// Assembly placement and solver placement remain separate arguments and
/// separate evidence, even when both adapters share one run-owned pool. This
/// v0 entry point rejects FVM until its cell/facet work is migrated to the
/// indexed packet contract.
///
/// # Errors
/// Returns `EQ0807` for revision/dimension mismatch, a non-P1-FEM plan, or an
/// unsupported realization. Lowering, assembly, and solution diagnostics keep
/// their own stable codes.
pub fn solve_resolved_scalar_elliptic_1d_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    assembly: &dyn AssemblyBackend,
    backend: &dyn LinearSolverBackend,
) -> Result<(ScalarEllipticModel1d, ResolvedScalarEllipticSolution1d), Diagnostic> {
    solve_resolved_scalar_elliptic_1d_impl(program, resolved, backend, Some(assembly))
}

#[derive(Debug, Clone)]
struct SingleFieldGraphSelection {
    graph: eqiora_realization::PortableRealizationGraph,
    discretization: Discretization,
    space: Space,
    solver: SolverPlan,
    scalar_type: ScalarType,
    vector_layout: VectorLayoutKind,
    placement: PlacementRequirementNode,
}

fn project_scalar_operator(
    resolved: &ResolvedRealization,
    domain: Id<kinds::Domain>,
    field: Id<kinds::Field>,
) -> Result<SingleFieldGraphSelection, Diagnostic> {
    let graph = resolved.portable_graph(SingleFieldOperatorClaim::new(
        domain,
        field,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    ))?;
    let SolveRoot::Linear(root) = graph.root() else {
        return Err(invalid_realization(
            "scalar elliptic execution requires a linear portable Realization root",
        ));
    };
    let linear = graph
        .linear_solve(root)
        .ok_or_else(|| invalid_realization("scalar elliptic portable linear root is absent"))?;
    let system = graph.system(linear.system()).ok_or_else(|| {
        invalid_realization("scalar elliptic portable algebraic system is absent")
    })?;
    let domain_node = graph.domains().first().ok_or_else(|| {
        invalid_realization("scalar elliptic portable Domain selection is absent")
    })?;
    let field_node = graph.fields().first().ok_or_else(|| {
        invalid_realization("scalar elliptic portable Field representation is absent")
    })?;
    if graph.domains().len() != 1
        || graph.fields().len() != 1
        || domain_node.domain() != domain
        || field_node.field() != field
        || system.operator_properties() != LinearOperatorProperties::SymmetricPositiveDefinite
    {
        return Err(invalid_realization(
            "scalar elliptic lowerer identities differ from the portable Realization graph",
        ));
    }
    let placement = graph
        .placement(linear.placement())
        .ok_or_else(|| invalid_realization("scalar elliptic portable placement is absent"))?;
    let discretization = domain_node.discretization();
    let space = field_node.space();
    let solver = linear.plan();
    let scalar_type = system.scalar_type();
    let vector_layout = system.partition();
    Ok(SingleFieldGraphSelection {
        graph,
        discretization,
        space,
        solver,
        scalar_type,
        vector_layout,
        placement,
    })
}

fn require_legacy_deployment_binding(
    placement: PlacementRequirementNode,
    target: Target,
) -> Result<(), Diagnostic> {
    let matches = match (placement, target) {
        (
            PlacementRequirementNode::HostWorkers {
                workers_per_partition,
            },
            Target::HostCpu { threads },
        ) => workers_per_partition == threads,
        (PlacementRequirementNode::CudaDevices { .. }, Target::CudaGpu { .. }) => true,
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(invalid_realization(
            "portable placement requirement differs from its compatibility deployment binding",
        ))
    }
}

fn solve_resolved_scalar_elliptic_1d_impl(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    backend: &dyn LinearSolverBackend,
    assembly: Option<&dyn AssemblyBackend>,
) -> Result<(ScalarEllipticModel1d, ResolvedScalarEllipticSolution1d), Diagnostic> {
    if program.model() != resolved.model()
        || program.revision().0 != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved realization does not reference this exact Semantic Model revision",
        ));
    }
    if resolved.requirements().spatial_dimension().get() != 1 {
        return Err(invalid_realization(format!(
            "interval realization requires spatial dimension one, received {}",
            resolved.requirements().spatial_dimension()
        )));
    }
    let model = lower_scalar_elliptic_1d(program)?;
    if !matches!(
        model.boundary().lower(),
        ScalarBoundaryCondition1d::Essential(_)
    ) && !matches!(
        model.boundary().upper(),
        ScalarBoundaryCondition1d::Essential(_)
    ) {
        return Err(invalid_realization(
            "interval scalar elliptic execution requires an essential endpoint to prove an SPD operator",
        ));
    }
    let selection = project_scalar_operator(resolved, model.domain_id(), model.field_id())?;
    require_legacy_deployment_binding(selection.placement, resolved.plan().target())?;
    if selection.scalar_type != ScalarType::F64
        || selection.vector_layout != VectorLayoutKind::Replicated
    {
        return Err(invalid_realization(
            "interval scalar elliptic execution requires f64 replicated algebra",
        ));
    }
    let cells = match selection.discretization.mesh() {
        MeshPolicy::GeneratedUniform { cells_per_axis } => cells_per_axis.get(),
        MeshPolicy::ImportedSimplicial { .. } => {
            return Err(invalid_realization(
                "interval v0 execution requires a generated uniform mesh",
            ));
        }
    };
    let solver = LinearSolveRequest::new(backend, selection.solver);
    let [start, end] = model.interval();
    let mesh = LineMesh::uniform(start, end, cells)?;
    let coefficient = model.coefficient();
    let boundary_semantics = model.boundary();
    let source = model.source();
    let solution = match selection.discretization.method() {
        DiscretizationMethod::ContinuousGalerkin => {
            if selection.space.family()
                != (SpaceFamily::ContinuousLagrange {
                    order: 1.try_into().expect("one is non-zero"),
                })
            {
                return Err(invalid_realization(
                    "scalar elliptic v0 lowers only continuous P1 elements",
                ));
            }
            let QuadraturePolicy::GaussLegendre { points_per_axis } =
                selection.discretization.quadrature()
            else {
                return Err(invalid_realization(
                    "P1 FEM requires Gauss-Legendre quadrature",
                ));
            };
            let quadrature = QuadratureRule::gauss_legendre(points_per_axis.get())?;
            let diffusion = move |_| coefficient;
            let source = move |coordinate| source.evaluate(&[coordinate]).unwrap_or(f64::NAN);
            let solution = if let Some(assembly) = assembly {
                solve_scalar_elliptic_linear_fem_with_assembly(
                    &mesh,
                    &diffusion,
                    &source,
                    boundary_semantics,
                    &quadrature,
                    assembly,
                    solver,
                )?
            } else {
                solve_scalar_elliptic_linear_fem(
                    &mesh,
                    &diffusion,
                    &source,
                    boundary_semantics,
                    &quadrature,
                    solver,
                )?
            };
            ResolvedScalarEllipticSolution1d::FiniteElement(solution)
        }
        DiscretizationMethod::CellCenteredFiniteVolume => {
            if assembly.is_some() {
                return Err(invalid_realization(
                    "explicit indexed assembly v0 admits only continuous P1 FEM",
                ));
            }
            if selection.space.family() != SpaceFamily::CellConstant
                || selection.discretization.quadrature() != QuadraturePolicy::CellCentroid
            {
                return Err(invalid_realization(
                    "scalar elliptic FVM v0 requires cell-constant space and centroid quadrature",
                ));
            }
            let (
                ScalarBoundaryCondition1d::Essential(lower),
                ScalarBoundaryCondition1d::Essential(upper),
            ) = (boundary_semantics.lower(), boundary_semantics.upper())
            else {
                return Err(invalid_realization(
                    "scalar elliptic FVM v0 requires essential conditions on both endpoints",
                ));
            };
            let boundary = DirichletBoundary1d::new(lower, upper)
                .map_err(|diagnostic| invalid_realization(diagnostic.message()))?;
            let quadrature = QuadratureRule::gauss_legendre(1)?;
            ResolvedScalarEllipticSolution1d::FiniteVolume(solve_scalar_elliptic_cell_fvm(
                &mesh,
                coefficient,
                &move |coordinate| source.evaluate(&[coordinate]).unwrap_or(f64::NAN),
                boundary,
                &quadrature,
                solver,
            )?)
        }
    };
    Ok((model, solution))
}

/// Execute one resolved plan against a canonical Cartesian scalar elliptic
/// model with essential conditions on every box side.
///
/// The same model lowering feeds Q1 FEM or cell-centered TPFA. Generated mesh
/// density, quadrature, solver, and backend are read only from the resolved
/// Realization; they never enter canonical meaning.
///
/// # Errors
/// Returns `EQ0807` for revision/dimension mismatch, nonessential boundary
/// data, or an unsupported plan, and preserves lowering/numerical diagnostics
/// from the selected path.
pub fn solve_resolved_scalar_elliptic_cartesian(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        ScalarEllipticCartesianModel,
        ResolvedScalarEllipticCartesianSolution,
    ),
    Diagnostic,
> {
    solve_resolved_scalar_elliptic_cartesian_with_assembly(
        program,
        resolved,
        &REFERENCE_ASSEMBLY_BACKEND,
        backend,
    )
}

/// Execute a resolved Cartesian plan through explicit assembly and solver
/// backends while retaining their independent placement evidence.
///
/// Canonical lowering, mesh construction, and numerical-method selection are
/// identical to [`solve_resolved_scalar_elliptic_cartesian`]. Only the L2
/// assembly execution adapter is selected explicitly.
///
/// # Errors
/// Preserves all reference entry point diagnostics and the selected assembly
/// backend's complete-operation diagnostics.
pub fn solve_resolved_scalar_elliptic_cartesian_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    assembly: &dyn AssemblyBackend,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        ScalarEllipticCartesianModel,
        ResolvedScalarEllipticCartesianSolution,
    ),
    Diagnostic,
> {
    let (model, finalized) =
        finalize_resolved_scalar_elliptic_cartesian_with_assembly(program, resolved, assembly)?;
    let solution = backend.solve(&finalized.linear_problem()?, finalized.solver_plan())?;
    Ok((model, finalized.finish(solution)?))
}

/// Finalize one resolved Cartesian scalar-elliptic realization for an
/// independently selected linear execution adapter.
///
/// This is the default-assembly form of
/// [`finalize_resolved_scalar_elliptic_cartesian_with_assembly`]. It performs
/// canonical lowering, plan validation, mesh construction, local-operator
/// evaluation, constraint handling, and deterministic sparse finalization,
/// but deliberately does not select or invoke a solver backend.
///
/// # Errors
/// Preserves exact lowering, Realization, discretization, and assembly
/// diagnostics.
pub fn finalize_resolved_scalar_elliptic_cartesian(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
) -> Result<
    (
        ScalarEllipticCartesianModel,
        FinalizedScalarEllipticCartesianProblem,
    ),
    Diagnostic,
> {
    finalize_resolved_scalar_elliptic_cartesian_with_assembly(
        program,
        resolved,
        &REFERENCE_ASSEMBLY_BACKEND,
    )
}

/// Finalize one resolved Cartesian scalar-elliptic realization through an
/// explicit assembly adapter without selecting a solver backend.
///
/// The returned opaque problem is the only handoff: it owns the exact CSR and
/// right-hand side, the sole `SolverPlan`, mathematical properties, assembly
/// evidence, and private method-native reconstruction state.
///
/// # Errors
/// Returns `EQ0807` for revision, dimension, boundary, or policy mismatch and
/// preserves numerical diagnostics from assembly and sparse finalization.
pub fn finalize_resolved_scalar_elliptic_cartesian_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    assembly: &dyn AssemblyBackend,
) -> Result<
    (
        ScalarEllipticCartesianModel,
        FinalizedScalarEllipticCartesianProblem,
    ),
    Diagnostic,
> {
    if program.model() != resolved.model()
        || program.revision().0 != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved realization does not reference this exact Semantic Model revision",
        ));
    }
    let model = lower_scalar_elliptic_cartesian(program)?;
    let finalized =
        finalize_lowered_scalar_elliptic_cartesian_with_assembly(&model, resolved, assembly)?;
    Ok((model, finalized))
}

/// Finalize one already lowered Cartesian scalar-elliptic model without
/// returning to its canonical `KernelProgram`.
///
/// This entry point is shared by the canonical revision-local point and
/// immutable selected-Parameter bindings of that same lowered model. Exact
/// source Model and semantic-revision identity are retained privately by the
/// lowered value and revalidated against the Realization.
///
/// # Errors
/// Preserves the default finalizer's exact identity, dimension, boundary,
/// policy, discretization, and assembly diagnostics.
pub fn finalize_lowered_scalar_elliptic_cartesian(
    model: &ScalarEllipticCartesianModel,
    resolved: &ResolvedRealization,
) -> Result<FinalizedScalarEllipticCartesianProblem, Diagnostic> {
    finalize_lowered_scalar_elliptic_cartesian_with_assembly(
        model,
        resolved,
        &REFERENCE_ASSEMBLY_BACKEND,
    )
}

/// Finalize one owned lowered Parameter point as an inseparable execution handoff.
///
/// This is the preferred application path for repeated evaluation of a static
/// Model/Realization program. The exact bound model survives solver execution
/// and is the only model that can later linearize the accepted solution.
///
/// # Errors
/// Preserves [`finalize_lowered_scalar_elliptic_cartesian`] diagnostics.
pub fn finalize_scalar_elliptic_parameter_point(
    model: ScalarEllipticCartesianModel,
    resolved: &ResolvedRealization,
) -> Result<FinalizedScalarEllipticParameterPoint, Diagnostic> {
    let problem = finalize_lowered_scalar_elliptic_cartesian(&model, resolved)?;
    Ok(FinalizedScalarEllipticParameterPoint {
        model,
        realization: resolved.clone(),
        problem,
    })
}

/// Finalize one already lowered Cartesian scalar-elliptic model through an
/// explicit assembly adapter.
///
/// # Errors
/// Preserves [`finalize_lowered_scalar_elliptic_cartesian`] diagnostics and the
/// selected assembly backend's complete-operation diagnostics.
pub fn finalize_lowered_scalar_elliptic_cartesian_with_assembly(
    model: &ScalarEllipticCartesianModel,
    resolved: &ResolvedRealization,
    assembly: &dyn AssemblyBackend,
) -> Result<FinalizedScalarEllipticCartesianProblem, Diagnostic> {
    if model.semantic_model != resolved.model()
        || model.semantic_revision != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved realization does not reference the exact Semantic Model revision retained by the lowered scalar elliptic model",
        ));
    }
    let requirements = resolved.requirements();
    if model.dimension() != requirements.spatial_dimension().get() {
        return Err(invalid_realization(format!(
            "resolved spatial dimension {} differs from lowered model dimension {}",
            requirements.spatial_dimension(),
            model.dimension()
        )));
    }
    if model
        .boundaries
        .values()
        .any(|boundary| !boundary.is_essential())
    {
        return Err(invalid_realization(
            "Cartesian FEM/FVM v0 requires essential conditions on every box side",
        ));
    }

    let selection = project_scalar_operator(resolved, model.domain_id(), model.field_id())?;
    let target = resolved.plan().target();
    require_legacy_deployment_binding(selection.placement, target)?;
    if selection.scalar_type != ScalarType::F64 {
        return Err(invalid_realization(
            "Cartesian scalar-elliptic finalization admits f64 scalar storage only",
        ));
    }
    validate_cartesian_layout_target(selection.vector_layout, target)?;

    let cells = match selection.discretization.mesh() {
        MeshPolicy::GeneratedUniform { cells_per_axis } => cells_per_axis.get(),
        MeshPolicy::ImportedSimplicial { .. } => {
            return Err(invalid_realization(
                "Cartesian v0 execution requires a generated uniform mesh; use the simplicial realization entry point for an imported mesh",
            ));
        }
    };
    let mesh = CartesianMesh::uniform(model.bounds(), &vec![cells; model.dimension()])?;
    let source = |coordinates: &[f64]| model.source().evaluate(coordinates).unwrap_or(f64::NAN);
    let boundary =
        |coordinates: &[f64]| evaluate_essential_boundary(model, coordinates).unwrap_or(f64::NAN);
    let finalized = match selection.discretization.method() {
        DiscretizationMethod::ContinuousGalerkin => {
            if selection.space.family()
                != (SpaceFamily::ContinuousLagrange {
                    order: 1.try_into().expect("one is non-zero"),
                })
            {
                return Err(invalid_realization(
                    "Cartesian scalar elliptic FEM v0 lowers only continuous Q1 elements",
                ));
            }
            let QuadraturePolicy::GaussLegendre { points_per_axis } =
                selection.discretization.quadrature()
            else {
                return Err(invalid_realization(
                    "Cartesian Q1 FEM requires Gauss-Legendre quadrature",
                ));
            };
            let quadrature = QuadratureRule::tensor_product_gauss_legendre(
                model.dimension(),
                points_per_axis.get(),
            )?;
            FinalizedScalarEllipticCartesianProblem::finite_element(
                selection.graph,
                selection.solver,
                selection.vector_layout,
                target,
                finalize_scalar_elliptic_cartesian_fem(
                    &mesh,
                    model.coefficient(),
                    &source,
                    &boundary,
                    &quadrature,
                    assembly,
                    model.compiled_form.as_ref(),
                )?,
            )?
        }
        DiscretizationMethod::CellCenteredFiniteVolume => {
            if selection.space.family() != SpaceFamily::CellConstant
                || selection.discretization.quadrature() != QuadraturePolicy::CellCentroid
            {
                return Err(invalid_realization(
                    "Cartesian scalar elliptic FVM v0 requires cell-constant space and centroid quadrature",
                ));
            }
            let cell_quadrature =
                QuadratureRule::tensor_product_gauss_legendre(model.dimension(), 1)?;
            let facet_quadrature = if model.dimension() == 1 {
                QuadratureRule::point()
            } else {
                QuadratureRule::tensor_product_gauss_legendre(model.dimension() - 1, 1)?
            };
            FinalizedScalarEllipticCartesianProblem::finite_volume(
                selection.graph,
                selection.solver,
                selection.vector_layout,
                target,
                finalize_scalar_elliptic_cartesian_fvm(
                    &mesh,
                    model.coefficient(),
                    &source,
                    &boundary,
                    &cell_quadrature,
                    &facet_quadrature,
                    assembly,
                )?,
            )?
        }
    };
    Ok(finalized)
}

fn validate_cartesian_layout_target(
    vector_layout: VectorLayoutKind,
    target: Target,
) -> Result<(), Diagnostic> {
    match (vector_layout, target) {
        (VectorLayoutKind::Replicated, Target::HostCpu { .. } | Target::CudaGpu { .. })
        | (
            VectorLayoutKind::Distributed,
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        ) => Ok(()),
        (VectorLayoutKind::Distributed, Target::HostCpu { .. }) => Err(invalid_realization(
            "distributed Cartesian spatial execution admits exactly one host worker per partition in v0",
        )),
        (VectorLayoutKind::Distributed, Target::CudaGpu { .. }) => Err(invalid_realization(
            "distributed CUDA Cartesian spatial execution is not admitted in v0",
        )),
    }
}

/// Execute one imported affine-simplex Realization against the exact
/// canonical Cartesian scalar-elliptic model revision it references.
///
/// Artifact decoding remains outside numerics. The caller supplies a mesh
/// reconstructed by the shared meshing contract and its content identity;
/// this entry point proves that identity, plan, dimension, P1 space,
/// simplex-centroid quadrature, solver, and canonical model all agree before
/// assembly.
///
/// # Errors
/// Returns `EQ0807` for provenance, identity, dimension, boundary, or policy
/// mismatch, and preserves canonical-lowering/numerical diagnostics from the
/// selected P1 simplex path.
pub fn solve_resolved_scalar_elliptic_simplicial(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        ScalarEllipticCartesianModel,
        ScalarEllipticSimplicialFemSolution,
    ),
    Diagnostic,
> {
    solve_resolved_scalar_elliptic_simplicial_with_assembly(
        program,
        resolved,
        mesh_artifact,
        mesh,
        &REFERENCE_ASSEMBLY_BACKEND,
        backend,
    )
}

/// Execute an imported affine-simplex Realization through explicit assembly
/// and solver backends.
///
/// Model, Realization, and mesh-artifact identity are proven before the first
/// packet is evaluated. Adapter selection therefore cannot weaken imported
/// topology, geometry, quality, or provenance validation.
///
/// # Errors
/// Preserves all reference entry point diagnostics and the selected assembly
/// backend's complete-operation diagnostics.
pub fn solve_resolved_scalar_elliptic_simplicial_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    mesh_artifact: MeshArtifactReference,
    mesh: &SimplicialMesh,
    assembly: &dyn AssemblyBackend,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        ScalarEllipticCartesianModel,
        ScalarEllipticSimplicialFemSolution,
    ),
    Diagnostic,
> {
    if program.model() != resolved.model()
        || program.revision().0 != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved realization does not reference this exact Semantic Model revision",
        ));
    }
    let model = lower_scalar_elliptic_cartesian(program)?;
    let required_dimension = resolved.requirements().spatial_dimension().get();
    if model.dimension() != required_dimension || mesh.topological_dimension() != required_dimension
    {
        return Err(invalid_realization(format!(
            "canonical model, imported mesh, and realization require one shared spatial dimension; received model {}, mesh {}, realization {required_dimension}",
            model.dimension(),
            mesh.topological_dimension(),
        )));
    }
    if model
        .boundaries
        .values()
        .any(|boundary| !boundary.is_essential())
    {
        return Err(invalid_realization(
            "imported simplex P1 v0 requires essential conditions on every box side",
        ));
    }
    let selection = project_scalar_operator(resolved, model.domain_id(), model.field_id())?;
    require_legacy_deployment_binding(selection.placement, resolved.plan().target())?;
    if selection.scalar_type != ScalarType::F64 {
        return Err(invalid_realization(
            "imported simplex scalar elliptic execution requires f64 scalar storage",
        ));
    }
    validate_cartesian_layout_target(selection.vector_layout, resolved.plan().target())?;
    let expected_artifact = match selection.discretization.mesh() {
        MeshPolicy::ImportedSimplicial { artifact } => artifact,
        MeshPolicy::GeneratedUniform { .. } => {
            return Err(invalid_realization(
                "simplicial execution requires an imported mesh policy",
            ));
        }
    };
    if expected_artifact != mesh_artifact {
        return Err(invalid_realization(
            "supplied simplex mesh identity differs from the resolved Realization plan",
        ));
    }
    if selection.discretization.method() != DiscretizationMethod::ContinuousGalerkin
        || selection.space.family()
            != (SpaceFamily::ContinuousLagrange {
                order: 1.try_into().expect("one is non-zero"),
            })
        || selection.discretization.quadrature() != QuadraturePolicy::SimplexCentroid
    {
        return Err(invalid_realization(
            "imported simplex v0 executes only continuous P1 Galerkin with simplex-centroid quadrature",
        ));
    }
    let quadrature = simplex_centroid_rule(required_dimension)?;
    let solution = solve_scalar_elliptic_simplicial_fem_with_assembly(
        &model,
        mesh,
        &quadrature,
        assembly,
        LinearSolveRequest::new(backend, selection.solver),
    )?;
    Ok((model, solution))
}

/// Solve and assemble `R_w`/`R_p` for one resolved Cartesian realization.
///
/// FEM and FVM retain their method-native unknowns, but both return the same
/// [`AssembledLinearizedRelation`] contract. `selected_coordinates` is the
/// explicit analysis role/order; every unselected model or geometry
/// coordinate is frozen. No finite differences are used to form either action.
///
/// # Errors
/// Preserves exact canonical-lowering, realization, numerical-solve, and
/// accepted-point derivative-assembly diagnostics.
pub fn solve_and_linearize_resolved_scalar_elliptic_cartesian(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    backend: &dyn LinearSolverBackend,
    selected_coordinates: &[crate::spatial_design::SpatialDesignCoordinate],
) -> Result<
    (
        ScalarEllipticCartesianModel,
        ResolvedScalarEllipticCartesianSolution,
        AssembledLinearizedRelation,
    ),
    Diagnostic,
> {
    solve_and_linearize_resolved_scalar_elliptic_cartesian_with_assembly(
        program,
        resolved,
        &REFERENCE_ASSEMBLY_BACKEND,
        backend,
        selected_coordinates,
    )
}

/// Solve and assemble `R_w`/`R_p` through explicit primal assembly and solver
/// backends.
///
/// The accepted primal retains its assembly placement independently from
/// solve placement. Linearization consumes that exact accepted state; it does
/// not silently re-solve or select another execution adapter.
///
/// # Errors
/// Preserves exact canonical-lowering, realization, assembly, solve, and
/// accepted-point derivative diagnostics.
pub fn solve_and_linearize_resolved_scalar_elliptic_cartesian_with_assembly(
    program: &KernelProgram,
    resolved: &ResolvedRealization,
    assembly: &dyn AssemblyBackend,
    backend: &dyn LinearSolverBackend,
    selected_coordinates: &[crate::spatial_design::SpatialDesignCoordinate],
) -> Result<
    (
        ScalarEllipticCartesianModel,
        ResolvedScalarEllipticCartesianSolution,
        AssembledLinearizedRelation,
    ),
    Diagnostic,
> {
    let (model, solution) = solve_resolved_scalar_elliptic_cartesian_with_assembly(
        program, resolved, assembly, backend,
    )?;
    let (linearization, output) = linearize_accepted_scalar_elliptic_cartesian(
        &model,
        resolved,
        &solution,
        selected_coordinates,
        LinearizationProducts::RelationOnly,
    )?;
    debug_assert!(output.is_none());
    Ok((model, solution, linearization))
}

fn linearize_owned_scalar_elliptic_parameter_point(
    model: &ScalarEllipticCartesianModel,
    resolved: &ResolvedRealization,
    solution: &ResolvedScalarEllipticCartesianSolution,
    selected_coordinates: &[crate::spatial_design::SpatialDesignCoordinate],
) -> Result<
    (
        AssembledLinearizedRelation,
        CartesianScalarFieldLinearization,
    ),
    Diagnostic,
> {
    if model.semantic_model != resolved.model()
        || model.semantic_revision != resolved.semantic_revision().get()
    {
        return Err(invalid_realization(
            "resolved realization does not reference the exact Semantic Model revision retained by the accepted Parameter point",
        ));
    }
    let (relation, output) = linearize_accepted_scalar_elliptic_cartesian(
        model,
        resolved,
        solution,
        selected_coordinates,
        LinearizationProducts::RelationAndPrimaryField,
    )?;
    Ok((
        relation,
        output.expect("complete-output product must construct one Field projection"),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinearizationProducts {
    RelationOnly,
    RelationAndPrimaryField,
}

fn linearize_accepted_scalar_elliptic_cartesian(
    model: &ScalarEllipticCartesianModel,
    resolved: &ResolvedRealization,
    solution: &ResolvedScalarEllipticCartesianSolution,
    selected_coordinates: &[crate::spatial_design::SpatialDesignCoordinate],
    products: LinearizationProducts,
) -> Result<
    (
        AssembledLinearizedRelation,
        Option<CartesianScalarFieldLinearization>,
    ),
    Diagnostic,
> {
    let cells = match resolved.plan().discretization().mesh() {
        MeshPolicy::GeneratedUniform { cells_per_axis } => cells_per_axis.get(),
        MeshPolicy::ImportedSimplicial { .. } => {
            return Err(invalid_realization(
                "Cartesian linearization requires the generated mesh used by its successful primal solve",
            ));
        }
    };
    let mesh = CartesianMesh::uniform(model.bounds(), &vec![cells; model.dimension()])?;
    let (linearization, output) = match (resolved.plan().discretization().method(), solution) {
        (
            DiscretizationMethod::ContinuousGalerkin,
            ResolvedScalarEllipticCartesianSolution::FiniteElement(solution),
        ) => {
            let QuadraturePolicy::GaussLegendre { points_per_axis } =
                resolved.plan().discretization().quadrature()
            else {
                unreachable!("successful FEM solve validated its quadrature policy")
            };
            let quadrature = QuadratureRule::tensor_product_gauss_legendre(
                model.dimension(),
                points_per_axis.get(),
            )?;
            let relation = linearize_scalar_elliptic_cartesian_fem(
                model,
                &mesh,
                solution,
                &quadrature,
                selected_coordinates,
            )?;
            let output = (products == LinearizationProducts::RelationAndPrimaryField)
                .then(|| {
                    linearize_scalar_elliptic_cartesian_fem_output(
                        model,
                        &mesh,
                        solution,
                        selected_coordinates,
                    )
                })
                .transpose()?;
            (relation, output)
        }
        (
            DiscretizationMethod::CellCenteredFiniteVolume,
            ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution),
        ) => {
            let cell_quadrature =
                QuadratureRule::tensor_product_gauss_legendre(model.dimension(), 1)?;
            let facet_quadrature = if model.dimension() == 1 {
                QuadratureRule::point()
            } else {
                QuadratureRule::tensor_product_gauss_legendre(model.dimension() - 1, 1)?
            };
            let relation = linearize_scalar_elliptic_cartesian_fvm(
                model,
                &mesh,
                solution,
                &cell_quadrature,
                &facet_quadrature,
                selected_coordinates,
            )?;
            let output = (products == LinearizationProducts::RelationAndPrimaryField)
                .then(|| {
                    linearize_scalar_elliptic_cartesian_fvm_output(
                        model,
                        &mesh,
                        solution,
                        selected_coordinates,
                    )
                })
                .transpose()?;
            (relation, output)
        }
        _ => {
            return Err(invalid_realization(
                "accepted scalar-elliptic solution method differs from its Realization",
            ));
        }
    };
    Ok((linearization, output))
}

/// Realize the same canonical interval model with P1 FEM and cell-centered
/// two-point-flux FVM over a refinement sequence.
///
/// The analytic reference is verification evidence and deliberately remains
/// outside canonical model meaning. Both numerical methods receive the same
/// lowered coefficient, source tape, interval, boundary values, mesh levels,
/// quadrature policy, solver policy, and error view.
///
/// # Errors
/// Returns `EQ0703` when canonical semantics fall outside the current scalar
/// Dirichlet comparison subset, or a numerical diagnostic when realization or
/// evidence calculation fails.
pub fn compare_canonical_scalar_elliptic_1d<E>(
    program: &KernelProgram,
    cell_counts: &[usize],
    exact: &E,
) -> Result<(ScalarEllipticModel1d, Vec<ScalarEllipticComparisonRow1d>), Diagnostic>
where
    E: Fn(f64) -> f64 + ?Sized,
{
    let model = lower_scalar_elliptic_1d(program)?;
    let (ScalarBoundaryCondition1d::Essential(lower), ScalarBoundaryCondition1d::Essential(upper)) =
        (model.boundary().lower(), model.boundary().upper())
    else {
        return Err(lowering_error(
            model.domain(),
            "FEM/FVM comparison currently requires essential conditions on both endpoints",
        ));
    };
    let boundary = DirichletBoundary1d::new(lower, upper)
        .map_err(|diagnostic| lowering_error(model.domain(), diagnostic.message()))?;
    let source = model.source();
    let rows = compare_scalar_elliptic_dirichlet_1d(
        model.interval(),
        model.coefficient(),
        &|coordinate| source.evaluate(&[coordinate]).unwrap_or(f64::NAN),
        boundary,
        exact,
        cell_counts,
    )?;
    Ok((model, rows))
}

pub(crate) fn unique_cartesian_box(
    program: &KernelProgram,
) -> Result<(RawId, Vec<[f64; 2]>), Diagnostic> {
    let domains = program
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
    if domains.len() != 1 {
        let count = domains.len();
        return Err(model_lowering_error(
            program,
            format!("scalar elliptic Cartesian lowering requires one box Domain, found {count}"),
        ));
    }
    let domain = domains[0];
    let bounds = program.resolved_cartesian_bounds(domain.id())?;
    Ok((
        domain.id().erase(),
        bounds
            .iter()
            .map(|axis| [axis.lower().value(), axis.upper().value()])
            .collect(),
    ))
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn invalid_parameter_binding(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LINEARIZATION, message)
}

fn unique_continuum_field(program: &KernelProgram, domain: RawId) -> Result<RawId, Diagnostic> {
    let fields = continuum_fields_on(program, domain);
    if fields.len() == 1 {
        Ok(fields[0])
    } else {
        Err(lowering_error(
            domain,
            format!(
                "default scalar elliptic lowering requires one continuum Field, found {}",
                fields.len()
            ),
        ))
    }
}

pub(crate) fn continuum_fields_on(program: &KernelProgram, domain: RawId) -> Vec<RawId> {
    program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Field(field)
                if has_edge(program, field.id().erase(), domain, EdgeKind::DefinedOn)
                    && field_has_continuum_representation(program, field.id().erase()) =>
            {
                Some(field.id().erase())
            }
            _ => None,
        })
        .collect()
}

fn field_has_continuum_representation(program: &KernelProgram, field: RawId) -> bool {
    program.edges().iter().any(|edge| {
        edge.from() == field
            && edge.kind() == EdgeKind::DefinedOn
            && matches!(
                program.node(edge.to()),
                Some(KernelNode::Representation(representation))
                    if representation.kind() == RepresentationKind::Continuum
            )
    })
}

pub(crate) fn relations_on(program: &KernelProgram, domain: RawId) -> Vec<RawId> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::AppliesOn && edge.to() == domain)
        .map(|edge| edge.from())
        .collect()
}

pub(crate) fn unique_relation_on(
    program: &KernelProgram,
    domain: RawId,
) -> Result<RawId, Diagnostic> {
    let relations = relations_on(program, domain);
    if relations.len() == 1 {
        Ok(relations[0])
    } else {
        Err(lowering_error(
            domain,
            format!(
                "default scalar elliptic lowering requires one Relation on this Domain, found {}",
                relations.len()
            ),
        ))
    }
}

fn lower_volume_relation(
    program: &KernelProgram,
    relation: RawId,
    field: RawId,
    coordinate_dimension: usize,
) -> Result<(ScalarSpatialExpression, ScalarSpatialExpression), Diagnostic> {
    let expression = relation_expression(program, relation)?;
    let root = unique_root(expression, relation)?;
    let (operator, source) = match expression.node(root) {
        Some(ExprNode::Sub(operator, source)) => (*operator, Some(*source)),
        _ => (root, None),
    };
    let divergence = match expression.node(operator) {
        Some(ExprNode::Neg(value)) => *value,
        _ => {
            return Err(lowering_error(
                relation,
                "volume residual must start with `-div(...)`",
            ));
        }
    };
    let flux = match expression.node(divergence) {
        Some(ExprNode::Divergence(value)) => *value,
        _ => {
            return Err(lowering_error(
                relation,
                "volume residual must contain physical divergence",
            ));
        }
    };
    let coefficient = lower_flux_coefficient(
        program,
        expression,
        flux,
        field,
        relation,
        coordinate_dimension,
    )?;
    let source = match source {
        Some(source) => {
            spatial_expression::lower(program, expression, source, relation, coordinate_dimension)?
        }
        None => ScalarSpatialExpression::constant(coordinate_dimension, 0.0),
    };
    Ok((coefficient, source))
}

fn lower_cartesian_boundary_relation(
    program: &KernelProgram,
    relation: RawId,
    field: RawId,
    volume_coefficient: &ScalarSpatialExpression,
    coordinate_dimension: usize,
) -> Result<ScalarEllipticCartesianBoundary, Diagnostic> {
    let expression = relation_expression(program, relation)?;
    let root = unique_root(expression, relation)?;
    let (operator, value) = match expression.node(root) {
        Some(ExprNode::Sub(operator, value)) => (*operator, Some(*value)),
        _ => (root, None),
    };
    let value = match value {
        Some(value) => {
            spatial_expression::lower(program, expression, value, relation, coordinate_dimension)?
        }
        None => ScalarSpatialExpression::constant(coordinate_dimension, 0.0),
    };
    match expression.node(operator) {
        Some(ExprNode::Trace(trace_operand)) if is_field(expression, *trace_operand, field) => {
            Ok(ScalarEllipticCartesianBoundary::Essential(value))
        }
        Some(ExprNode::NormalComponent(flux)) => {
            let coefficient = lower_flux_coefficient(
                program,
                expression,
                *flux,
                field,
                relation,
                coordinate_dimension,
            )?;
            if coefficient != *volume_coefficient {
                return Err(lowering_error(
                    relation,
                    "boundary flux coefficient is not structurally identical to the volume constitutive flux",
                ));
            }
            Ok(ScalarEllipticCartesianBoundary::Natural(value))
        }
        _ => Err(lowering_error(
            relation,
            "boundary residual must be `trace(field) - value` or `normal(flux) - value`",
        )),
    }
}

fn lower_constant_boundary_1d(
    boundary: &ScalarEllipticCartesianBoundary,
    owner: RawId,
) -> Result<ScalarBoundaryCondition1d, Diagnostic> {
    let value = boundary.value().constant_value().ok_or_else(|| {
        lowering_error(
            owner,
            "scalar elliptic 1D realization currently requires spatially constant boundary data",
        )
    })?;
    Ok(match boundary {
        ScalarEllipticCartesianBoundary::Essential(_) => {
            ScalarBoundaryCondition1d::Essential(value)
        }
        ScalarEllipticCartesianBoundary::Natural(_) => ScalarBoundaryCondition1d::Natural(value),
    })
}

fn collect_parameter_coordinates(
    coefficient: &ScalarSpatialExpression,
    source: &ScalarSpatialExpression,
    boundaries: &BTreeMap<(usize, BoundarySide), ScalarEllipticCartesianBoundary>,
) -> (Vec<Id<kinds::Parameter>>, Vec<f64>) {
    let mut fields = Vec::new();
    let mut values = Vec::new();
    for expression in std::iter::once(coefficient)
        .chain(std::iter::once(source))
        .chain(
            boundaries
                .values()
                .map(ScalarEllipticCartesianBoundary::value),
        )
    {
        for (field, value) in expression
            .parameter_fields()
            .iter()
            .zip(expression.parameter_values())
        {
            if let Some(index) = fields.iter().position(|existing| existing == field) {
                debug_assert_eq!(values[index], *value);
            } else {
                fields.push(*field);
                values.push(*value);
            }
        }
    }
    (fields, values)
}

fn evaluate_essential_boundary(
    model: &ScalarEllipticCartesianModel,
    coordinates: &[f64],
) -> Result<f64, Diagnostic> {
    if coordinates.len() != model.dimension() {
        return Err(lowering_error(
            model.domain(),
            "Cartesian boundary evaluation received the wrong coordinate dimension",
        ));
    }
    let mut value: Option<f64> = None;
    for (axis, bounds) in model.bounds().iter().enumerate() {
        for (side, coordinate) in [
            (BoundarySide::Lower, bounds[0]),
            (BoundarySide::Upper, bounds[1]),
        ] {
            if coordinates[axis].to_bits() != coordinate.to_bits() {
                continue;
            }
            let boundary = model
                .boundary(axis, side)
                .expect("Cartesian lowerer produces a complete boundary set");
            let ScalarEllipticCartesianBoundary::Essential(expression) = boundary else {
                return Err(lowering_error(
                    model.domain(),
                    "Cartesian numerical path requires essential boundary data",
                ));
            };
            let candidate = expression.evaluate(coordinates)?;
            if let Some(previous) = value {
                let scale = previous.abs().max(candidate.abs()).max(1.0);
                if (previous - candidate).abs() > 256.0 * f64::EPSILON * scale {
                    return Err(lowering_error(
                        model.domain(),
                        "Cartesian boundary expressions disagree at an edge or corner",
                    ));
                }
            } else {
                value = Some(candidate);
            }
        }
    }
    value.ok_or_else(|| {
        lowering_error(
            model.domain(),
            "Cartesian boundary evaluation point is not on the box boundary",
        )
    })
}

pub(crate) fn lower_flux_coefficient(
    program: &KernelProgram,
    expression: &ExprDag,
    value: ExprId,
    field: RawId,
    owner: RawId,
    coordinate_dimension: usize,
) -> Result<ScalarSpatialExpression, Diagnostic> {
    if let Some(ExprNode::Gradient(argument)) = expression.node(value)
        && is_field(expression, *argument, field)
    {
        return Ok(ScalarSpatialExpression::constant(coordinate_dimension, 1.0));
    }
    let Some(ExprNode::Mul(left, right)) = expression.node(value) else {
        return Err(lowering_error(
            owner,
            "constitutive flux must be a scalar coefficient times `grad(field)`",
        ));
    };
    if contains_gradient_of(expression, *left, field) {
        let coefficient = lower_flux_coefficient(
            program,
            expression,
            *left,
            field,
            owner,
            coordinate_dimension,
        )?;
        let factor = lower_constant_spatial_factor(
            program,
            expression,
            *right,
            owner,
            coordinate_dimension,
        )?;
        Ok(coefficient.multiply(factor))
    } else if contains_gradient_of(expression, *right, field) {
        let factor =
            lower_constant_spatial_factor(program, expression, *left, owner, coordinate_dimension)?;
        let coefficient = lower_flux_coefficient(
            program,
            expression,
            *right,
            field,
            owner,
            coordinate_dimension,
        )?;
        Ok(factor.multiply(coefficient))
    } else {
        Err(lowering_error(
            owner,
            "constitutive flux does not contain `grad(field)`",
        ))
    }
}

fn lower_constant_spatial_factor(
    program: &KernelProgram,
    expression: &ExprDag,
    value: ExprId,
    owner: RawId,
    coordinate_dimension: usize,
) -> Result<ScalarSpatialExpression, Diagnostic> {
    let factor =
        spatial_expression::lower(program, expression, value, owner, coordinate_dimension)?;
    if factor.is_coordinate_dependent() {
        return Err(lowering_error(
            owner,
            "Cartesian scalar elliptic coefficient must be spatially constant",
        ));
    }
    Ok(factor)
}

fn contains_gradient_of(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    match expression.node(value) {
        Some(ExprNode::Gradient(argument)) => is_field(expression, *argument, field),
        Some(ExprNode::Mul(left, right)) => {
            contains_gradient_of(expression, *left, field)
                || contains_gradient_of(expression, *right, field)
        }
        _ => false,
    }
}

pub(crate) fn is_field(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    matches!(
        expression.node(value),
        Some(ExprNode::Symbol(SymbolRef::Field(id))) if id.erase() == field
    )
}

pub(crate) fn relation_expression(
    program: &KernelProgram,
    relation: RawId,
) -> Result<&ExprDag, Diagnostic> {
    match program.node(relation) {
        Some(KernelNode::Relation(relation)) => Ok(relation.residuals()),
        _ => Err(lowering_error(
            relation,
            "AppliesOn source has no Relation definition",
        )),
    }
}

pub(crate) fn unique_root(expression: &ExprDag, owner: RawId) -> Result<ExprId, Diagnostic> {
    if expression.roots().len() == 1 {
        Ok(expression.roots()[0])
    } else {
        Err(lowering_error(
            owner,
            "default scalar elliptic Relation requires exactly one residual root",
        ))
    }
}

pub(crate) fn boundary_parent(program: &KernelProgram, boundary: RawId) -> Option<RawId> {
    let parents = program
        .edges()
        .iter()
        .filter(|edge| edge.from() == boundary && edge.kind() == EdgeKind::BoundaryOf)
        .map(|edge| edge.to())
        .collect::<Vec<_>>();
    (parents.len() == 1).then(|| parents[0])
}

fn has_edge(program: &KernelProgram, from: RawId, to: RawId, kind: EdgeKind) -> bool {
    program
        .edges()
        .iter()
        .any(|edge| edge.from() == from && edge.to() == to && edge.kind() == kind)
}

pub(crate) fn lowering_error(owner: RawId, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        owner.kind().graph().name().to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
    ]))
}

pub(crate) fn model_lowering_error(
    program: &KernelProgram,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        "ontology-view".to_owned(),
        "eqiora.model/v1".to_owned(),
        program.model().to_string(),
    ]))
}
