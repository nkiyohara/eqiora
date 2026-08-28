use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id, OntologyId};
use eqiora_schema::Model;
use eqiora_solver::LinearOperatorProperties;

use crate::{
    AlgebraicConstraint, BackwardEulerRelationStep, DiscretizationMethod, FieldwiseRealizationPlan,
    FieldwiseRealizationRequest, FieldwiseRealizationRequirements, MeshPolicy, NonlinearSolvePlan,
    QuadraturePolicy, RealizationCapabilities, RealizationRevision, ResolvedFieldwiseRealization,
    SemanticRevision, SpaceFamily, invalid_realization, resolve_fieldwise,
};

/// Endpoint-centered conservative momentum convection for one velocity Field.
///
/// The actual face volume flux is owned by the collocated coupling below. This
/// transformation requires momentum convection to consume that unique flux;
/// it cannot independently interpolate a second velocity flux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplicitCenteredMomentumConvection {
    relation: Id<kinds::Relation>,
    velocity: Id<kinds::Field>,
}

impl ImplicitCenteredMomentumConvection {
    /// Select endpoint-centered convection for one exact momentum/velocity pair.
    #[must_use]
    pub const fn new(relation: Id<kinds::Relation>, velocity: Id<kinds::Field>) -> Self {
        Self { relation, velocity }
    }

    /// Exact conservative momentum Relation.
    #[must_use]
    pub const fn relation(self) -> Id<kinds::Relation> {
        self.relation
    }

    /// Exact transported velocity Field.
    #[must_use]
    pub const fn velocity(self) -> Id<kinds::Field> {
        self.velocity
    }
}

/// Centered Cartesian face realization of Newtonian velocity--pressure stress.
///
/// This is deliberately not scalar TPFA: the physical flux is the vector
/// traction `(2 mu sym(grad(u)) - I p) n` and couples all velocity components
/// to pressure through one oriented face action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CartesianCentralNewtonianTraction {
    relation: Id<kinds::Relation>,
    velocity: Id<kinds::Field>,
    pressure: Id<kinds::Field>,
}

impl CartesianCentralNewtonianTraction {
    /// Select the exact momentum, velocity, and pressure identities.
    #[must_use]
    pub const fn new(
        relation: Id<kinds::Relation>,
        velocity: Id<kinds::Field>,
        pressure: Id<kinds::Field>,
    ) -> Self {
        Self {
            relation,
            velocity,
            pressure,
        }
    }

    /// Exact momentum Relation carrying Newtonian stress.
    #[must_use]
    pub const fn relation(self) -> Id<kinds::Relation> {
        self.relation
    }

    /// Exact velocity Field.
    #[must_use]
    pub const fn velocity(self) -> Id<kinds::Field> {
        self.velocity
    }

    /// Exact pressure Field.
    #[must_use]
    pub const fn pressure(self) -> Id<kinds::Field> {
        self.pressure
    }
}

/// Positive momentum scale used only by collocated face interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PositiveMomentumDiagonal {
    /// Backward-Euler mass plus Cartesian local Newtonian normal traction.
    ///
    /// This positive scale deliberately excludes convection and nonlocal
    /// reconstructed-gradient terms. It weights interpolation; it is not a
    /// claim to be the diagonal of the complete Newton Jacobian.
    #[deprecated(note = "use BackwardEulerMassAndLocalNewtonian")]
    BackwardEulerMassAndLocalNewtonianV1,
}

/// Previous-time face state used by transient-consistent interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransientFaceFluxHistory {
    /// BDF1 uses the previous accepted face flux minus centered cell velocity.
    #[deprecated(note = "use Bdf1PreviousAccepted")]
    Bdf1PreviousAcceptedV1,
}

impl PositiveMomentumDiagonal {
    /// Backward-Euler mass plus Cartesian local Newtonian normal traction.
    #[allow(deprecated)]
    #[allow(non_upper_case_globals)]
    pub const BackwardEulerMassAndLocalNewtonian: Self = Self::BackwardEulerMassAndLocalNewtonianV1;
}

impl TransientFaceFluxHistory {
    /// BDF1 uses the previous accepted face flux minus centered cell velocity.
    #[allow(deprecated)]
    #[allow(non_upper_case_globals)]
    pub const Bdf1PreviousAccepted: Self = Self::Bdf1PreviousAcceptedV1;
}

/// Linearly exact momentum-weighted collocated face-flux coupling.
///
/// The transformation owns one face mass flux shared by continuity and
/// momentum convection. Its pressure correction vanishes for affine pressure
/// on the admitted Cartesian mesh and acts nontrivially on checkerboard modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MomentumWeightedLinearExactCoupling {
    momentum_relation: Id<kinds::Relation>,
    incompressibility_relation: Id<kinds::Relation>,
    velocity: Id<kinds::Field>,
    pressure: Id<kinds::Field>,
    positive_diagonal: PositiveMomentumDiagonal,
    transient_history: TransientFaceFluxHistory,
}

impl MomentumWeightedLinearExactCoupling {
    /// Bind the two exact Relations and their velocity/pressure Fields.
    #[must_use]
    pub const fn new(
        momentum_relation: Id<kinds::Relation>,
        incompressibility_relation: Id<kinds::Relation>,
        velocity: Id<kinds::Field>,
        pressure: Id<kinds::Field>,
        positive_diagonal: PositiveMomentumDiagonal,
        transient_history: TransientFaceFluxHistory,
    ) -> Self {
        Self {
            momentum_relation,
            incompressibility_relation,
            velocity,
            pressure,
            positive_diagonal,
            transient_history,
        }
    }

    /// Exact momentum Relation supplying the momentum diagonal and convection.
    #[must_use]
    pub const fn momentum_relation(self) -> Id<kinds::Relation> {
        self.momentum_relation
    }

    /// Exact incompressibility Relation consuming the unique face flux.
    #[must_use]
    pub const fn incompressibility_relation(self) -> Id<kinds::Relation> {
        self.incompressibility_relation
    }

    /// Exact cell-centered velocity Field.
    #[must_use]
    pub const fn velocity(self) -> Id<kinds::Field> {
        self.velocity
    }

    /// Exact cell-centered pressure Field.
    #[must_use]
    pub const fn pressure(self) -> Id<kinds::Field> {
        self.pressure
    }

    /// Exact positive scale used by the pressure--velocity interpolation.
    #[must_use]
    pub const fn positive_diagonal(self) -> PositiveMomentumDiagonal {
        self.positive_diagonal
    }

    /// Exact previous-time face-flux closure for transient consistency.
    #[must_use]
    pub const fn transient_history(self) -> TransientFaceFluxHistory {
        self.transient_history
    }
}

/// Explicit adapter witness for the complete bounded collocated-flow method.
///
/// Constructing this value claims more than generic cell-centered spaces: the
/// adapter implements the exact centered convection, Newtonian traction,
/// linearly exact momentum-weighted coupling, and monolithic nonlinear action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientCellCenteredIncompressibleFlowCapabilities {
    fieldwise: RealizationCapabilities,
}

impl TransientCellCenteredIncompressibleFlowCapabilities {
    /// Claim the bounded collocated-flow method over one ordinary capability.
    #[must_use]
    pub const fn new(fieldwise: RealizationCapabilities) -> Self {
        Self { fieldwise }
    }

    /// Generic spatial, scalar, layout, solver, target, and schedule support.
    #[must_use]
    pub const fn fieldwise(&self) -> &RealizationCapabilities {
        &self.fieldwise
    }
}

/// Complete fixed-domain collocated incompressible-flow Realization selection.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientCellCenteredIncompressibleFlowRealizationPlan {
    fieldwise: FieldwiseRealizationPlan,
    time_step: BackwardEulerRelationStep,
    convection: ImplicitCenteredMomentumConvection,
    traction: CartesianCentralNewtonianTraction,
    coupling: MomentumWeightedLinearExactCoupling,
    nonlinear: NonlinearSolvePlan,
}

impl TransientCellCenteredIncompressibleFlowRealizationPlan {
    /// Compose and cross-check the complete bounded collocated method.
    ///
    /// # Errors
    /// Returns `EQ0807` unless the plan has exactly cell-constant velocity and
    /// pressure on generated Cartesian cells, one zero-integral pressure
    /// constraint, a general linearization, and exact shared identities.
    pub fn new(
        fieldwise: FieldwiseRealizationPlan,
        time_step: BackwardEulerRelationStep,
        convection: ImplicitCenteredMomentumConvection,
        traction: CartesianCentralNewtonianTraction,
        coupling: MomentumWeightedLinearExactCoupling,
        nonlinear: NonlinearSolvePlan,
    ) -> Result<Self, Diagnostic> {
        let value = Self {
            fieldwise,
            time_step,
            convection,
            traction,
            coupling,
            nonlinear,
        };
        value.validate()?;
        Ok(value)
    }

    /// Ordinary field-wise spatial, scaling, solver, and placement selection.
    #[must_use]
    pub const fn fieldwise(&self) -> &FieldwiseRealizationPlan {
        &self.fieldwise
    }

    /// Exact backward-Euler momentum derivative.
    #[must_use]
    pub const fn time_step(&self) -> BackwardEulerRelationStep {
        self.time_step
    }

    /// Exact endpoint-centered momentum convection.
    #[must_use]
    pub const fn convection(&self) -> ImplicitCenteredMomentumConvection {
        self.convection
    }

    /// Exact centered Newtonian face traction.
    #[must_use]
    pub const fn traction(&self) -> CartesianCentralNewtonianTraction {
        self.traction
    }

    /// Exact shared momentum-weighted face-flux coupling.
    #[must_use]
    pub const fn coupling(&self) -> MomentumWeightedLinearExactCoupling {
        self.coupling
    }

    /// Bounded monolithic nonlinear policy.
    #[must_use]
    pub const fn nonlinear(&self) -> NonlinearSolvePlan {
        self.nonlinear
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        self.fieldwise.validate()?;
        let spatial = self.fieldwise.spatial();
        if !matches!(
            (
                spatial.discretization().method(),
                spatial.discretization().mesh(),
                spatial.discretization().quadrature(),
            ),
            (
                DiscretizationMethod::CellCenteredFiniteVolume,
                MeshPolicy::GeneratedUniform { .. } | MeshPolicy::SuppliedCartesian { .. },
                QuadraturePolicy::CellCentroid,
            )
        ) || spatial.field_spaces().len() != 2
            || spatial
                .field_spaces()
                .iter()
                .any(|binding| binding.space().family() != SpaceFamily::CellConstant)
        {
            return Err(invalid_realization(
                "collocated incompressible flow requires exactly two cell-constant Fields on a generated or supplied 2D Cartesian mesh with cell-centroid quadrature",
            ));
        }
        let cells = match spatial.discretization().mesh() {
            MeshPolicy::GeneratedUniform { cells_per_axis } => [cells_per_axis; 2],
            MeshPolicy::SuppliedCartesian { cells, .. } => cells,
            MeshPolicy::ImportedSimplicial { .. } => {
                unreachable!("the exact Cartesian-mesh match above already succeeded")
            }
        };
        if cells.into_iter().any(|count| count.get() < 2) {
            return Err(invalid_realization(
                "linearly exact collocated pressure coupling requires at least two cells per Cartesian axis",
            ));
        }
        if self.fieldwise.operator_properties() != LinearOperatorProperties::General {
            return Err(invalid_realization(
                "collocated incompressible-flow Newton linearization requires a general operator contract",
            ));
        }
        let momentum = self.coupling.momentum_relation;
        let velocity = self.coupling.velocity;
        let pressure = self.coupling.pressure;
        if velocity == pressure
            || self.time_step.relation() != momentum
            || self.time_step.state() != velocity
            || self.convection.relation != momentum
            || self.convection.velocity != velocity
            || self.traction.relation != momentum
            || self.traction.velocity != velocity
            || self.traction.pressure != pressure
        {
            return Err(invalid_realization(
                "collocated incompressible-flow transformations must share exact momentum, velocity, and pressure identities",
            ));
        }
        if !spatial
            .field_spaces()
            .iter()
            .any(|binding| binding.field() == velocity)
            || !spatial
                .field_spaces()
                .iter()
                .any(|binding| binding.field() == pressure)
        {
            return Err(invalid_realization(
                "collocated velocity and pressure must be the complete Field-wise unknown inventory",
            ));
        }
        if spatial.constraints() != [AlgebraicConstraint::ZeroIntegral { field: pressure }] {
            return Err(invalid_realization(
                "bounded collocated incompressible flow requires exactly one zero-integral pressure constraint",
            ));
        }
        Ok(())
    }
}

/// Exact lowerer facts required by the collocated incompressible-flow adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientCellCenteredIncompressibleFlowRealizationRequirements {
    fieldwise: FieldwiseRealizationRequirements,
    momentum_relation: Id<kinds::Relation>,
    incompressibility_relation: Id<kinds::Relation>,
    velocity: Id<kinds::Field>,
    pressure: Id<kinds::Field>,
}

impl TransientCellCenteredIncompressibleFlowRealizationRequirements {
    /// Bind ordinary requirements to exact canonical fluid identities.
    ///
    /// # Errors
    /// Returns `EQ0807` unless velocity and pressure are distinct and exactly
    /// cover the lowerer's unknown inventory.
    pub fn new(
        fieldwise: FieldwiseRealizationRequirements,
        momentum_relation: Id<kinds::Relation>,
        incompressibility_relation: Id<kinds::Relation>,
        velocity: Id<kinds::Field>,
        pressure: Id<kinds::Field>,
    ) -> Result<Self, Diagnostic> {
        if velocity == pressure
            || fieldwise.unknown_fields().len() != 2
            || !fieldwise.unknown_fields().contains(&velocity)
            || !fieldwise.unknown_fields().contains(&pressure)
        {
            return Err(invalid_realization(
                "collocated incompressible-flow requirements need exactly distinct velocity and pressure Fields",
            ));
        }
        Ok(Self {
            fieldwise,
            momentum_relation,
            incompressibility_relation,
            velocity,
            pressure,
        })
    }

    /// Ordinary Domain, Field, scalar, layout, and target requirements.
    #[must_use]
    pub const fn fieldwise(&self) -> &FieldwiseRealizationRequirements {
        &self.fieldwise
    }

    /// Exact transient momentum Relation.
    #[must_use]
    pub const fn momentum_relation(&self) -> Id<kinds::Relation> {
        self.momentum_relation
    }

    /// Exact incompressibility Relation.
    #[must_use]
    pub const fn incompressibility_relation(&self) -> Id<kinds::Relation> {
        self.incompressibility_relation
    }

    /// Exact velocity Field.
    #[must_use]
    pub const fn velocity(&self) -> Id<kinds::Field> {
        self.velocity
    }

    /// Exact pressure Field.
    #[must_use]
    pub const fn pressure(&self) -> Id<kinds::Field> {
        self.pressure
    }
}

/// Explicit collocated incompressible-flow request at an independent revision.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientCellCenteredIncompressibleFlowRealizationRequest {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    realization_revision: RealizationRevision,
    plan: TransientCellCenteredIncompressibleFlowRealizationPlan,
}

impl TransientCellCenteredIncompressibleFlowRealizationRequest {
    /// Bind one explicit plan to exact semantic and Realization revisions.
    #[must_use]
    pub const fn explicit(
        model: OntologyId<Model>,
        semantic_revision: SemanticRevision,
        realization_revision: RealizationRevision,
        plan: TransientCellCenteredIncompressibleFlowRealizationPlan,
    ) -> Self {
        Self {
            model,
            semantic_revision,
            realization_revision,
            plan,
        }
    }

    /// Semantic Model identity.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Semantic Model revision.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Independently selected Realization revision.
    #[must_use]
    pub const fn realization_revision(&self) -> RealizationRevision {
        self.realization_revision
    }

    /// Complete unresolved plan.
    #[must_use]
    pub const fn plan(&self) -> &TransientCellCenteredIncompressibleFlowRealizationPlan {
        &self.plan
    }
}

/// Accepted plan, exact lowerer requirements, and two-layer provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTransientCellCenteredIncompressibleFlowRealization {
    fieldwise: ResolvedFieldwiseRealization,
    requirements: TransientCellCenteredIncompressibleFlowRealizationRequirements,
    plan: TransientCellCenteredIncompressibleFlowRealizationPlan,
}

impl ResolvedTransientCellCenteredIncompressibleFlowRealization {
    /// Semantic Model identity.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.fieldwise.model()
    }

    /// Semantic Model revision.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.fieldwise.semantic_revision()
    }

    /// Independently selected Realization revision.
    #[must_use]
    pub const fn realization_revision(&self) -> RealizationRevision {
        self.fieldwise.realization_revision()
    }

    /// Accepted ordinary field-wise admission.
    #[must_use]
    pub const fn fieldwise(&self) -> &ResolvedFieldwiseRealization {
        &self.fieldwise
    }

    /// Exact canonical lowerer facts used for admission.
    #[must_use]
    pub const fn requirements(
        &self,
    ) -> &TransientCellCenteredIncompressibleFlowRealizationRequirements {
        &self.requirements
    }

    /// Complete accepted collocated plan.
    #[must_use]
    pub const fn plan(&self) -> &TransientCellCenteredIncompressibleFlowRealizationPlan {
        &self.plan
    }
}

/// Resolve one collocated incompressible-flow request without substitution.
///
/// # Errors
/// Returns `EQ0807` for any plan, lowerer identity, or ordinary capability
/// mismatch. No alternate coupling or solver policy is inferred.
pub fn resolve_transient_cell_centered_incompressible_flow(
    request: &TransientCellCenteredIncompressibleFlowRealizationRequest,
    requirements: TransientCellCenteredIncompressibleFlowRealizationRequirements,
    capabilities: &TransientCellCenteredIncompressibleFlowCapabilities,
) -> Result<ResolvedTransientCellCenteredIncompressibleFlowRealization, Diagnostic> {
    request.plan.validate()?;
    let plan = &request.plan;
    if plan.coupling.momentum_relation != requirements.momentum_relation
        || plan.coupling.incompressibility_relation != requirements.incompressibility_relation
        || plan.coupling.velocity != requirements.velocity
        || plan.coupling.pressure != requirements.pressure
    {
        return Err(invalid_realization(
            "collocated incompressible-flow plan differs from exact canonical lowerer identities",
        ));
    }
    let fieldwise_request = FieldwiseRealizationRequest::explicit(
        request.model,
        request.semantic_revision,
        request.realization_revision,
        plan.fieldwise.clone(),
    );
    let fieldwise = resolve_fieldwise(
        &fieldwise_request,
        requirements.fieldwise.clone(),
        capabilities.fieldwise(),
    )?;
    Ok(ResolvedTransientCellCenteredIncompressibleFlowRealization {
        fieldwise,
        requirements,
        plan: plan.clone(),
    })
}

#[cfg(test)]
#[path = "transient_cell_centered_incompressible/tests.rs"]
mod tests;
