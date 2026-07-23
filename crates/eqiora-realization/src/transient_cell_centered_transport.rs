use std::collections::BTreeSet;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id, OntologyId};
use eqiora_schema::Model;
use eqiora_solver::LinearOperatorProperties;

use crate::{
    BackwardEulerRelationStep, DiscretizationMethod, FieldwiseRealizationPlan,
    FieldwiseRealizationRequest, FieldwiseRealizationRequirements, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationRevision, ResolvedFieldwiseRealization, SemanticRevision,
    SpaceFamily, invalid_realization, resolve_fieldwise,
};

/// Closed convection treatments implemented by the cell-centered reference path.
///
/// Evaluation time is part of the choice: a limiter evaluated at the accepted
/// previous state must not masquerade as an implicit endpoint reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CellCenteredConvectionScheme {
    /// Endpoint-evaluated, first-order donor-cell reconstruction.
    ImplicitFirstOrderUpwind,
    /// Previous-state Cartesian MUSCL/minmod with one-axis CFL admission.
    ///
    /// The bounded reference profile uses a previous-state plus inflow-trace
    /// hull, exact inflow ghost closure, symmetric one-sided outflow closure,
    /// at most one active Cartesian velocity axis, and Courant number at most
    /// one half. A backend advertising this variant must preserve that complete
    /// method, not merely use a limiter named minmod.
    ExplicitPreviousStateCartesianMinmod,
}

impl CellCenteredConvectionScheme {
    /// Normative explicit advective Courant limit, when the scheme has one.
    #[must_use]
    pub const fn maximum_explicit_courant_number(self) -> Option<f64> {
        match self {
            Self::ImplicitFirstOrderUpwind => None,
            Self::ExplicitPreviousStateCartesianMinmod => Some(0.5),
        }
    }

    /// Minimum Cartesian cells required along an active advective axis.
    #[must_use]
    pub const fn minimum_cells_per_active_axis(self) -> usize {
        match self {
            Self::ImplicitFirstOrderUpwind => 1,
            Self::ExplicitPreviousStateCartesianMinmod => 3,
        }
    }
}

/// Numerical convection treatment for one conservative transported state.
///
/// The canonical Relation owns velocity, physical flux, and boundary meaning.
/// This value selects only how the exact convection term is evaluated and
/// reconstructed for one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellCenteredConvection {
    relation: Id<kinds::Relation>,
    state: Id<kinds::Field>,
    scheme: CellCenteredConvectionScheme,
}

impl CellCenteredConvection {
    /// Select one exact convection treatment for one Relation/state pair.
    #[must_use]
    pub const fn new(
        relation: Id<kinds::Relation>,
        state: Id<kinds::Field>,
        scheme: CellCenteredConvectionScheme,
    ) -> Self {
        Self {
            relation,
            state,
            scheme,
        }
    }

    /// Exact conservative Semantic Relation being realized.
    #[must_use]
    pub const fn relation(self) -> Id<kinds::Relation> {
        self.relation
    }

    /// Exact cell-centered transported Semantic Field.
    #[must_use]
    pub const fn state(self) -> Id<kinds::Field> {
        self.state
    }

    /// Exact reconstruction and evaluation-time policy.
    #[must_use]
    pub const fn scheme(self) -> CellCenteredConvectionScheme {
        self.scheme
    }
}

/// Orthogonal two-point diffusive face flux for one conservative state.
///
/// This selection is deliberately separate from both the canonical diffusion
/// Relation and the convection reconstruction. Nonorthogonal correction,
/// MPFA, and tensor diffusion require different typed choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrthogonalTwoPointDiffusion {
    relation: Id<kinds::Relation>,
    state: Id<kinds::Field>,
}

impl OrthogonalTwoPointDiffusion {
    /// Select orthogonal TPFA for one exact Relation/state pair.
    #[must_use]
    pub const fn new(relation: Id<kinds::Relation>, state: Id<kinds::Field>) -> Self {
        Self { relation, state }
    }

    /// Exact conservative Semantic Relation being realized.
    #[must_use]
    pub const fn relation(self) -> Id<kinds::Relation> {
        self.relation
    }

    /// Exact cell-centered transported Semantic Field.
    #[must_use]
    pub const fn state(self) -> Id<kinds::Field> {
        self.state
    }
}

/// Explicit execution capability for bounded transient transport profiles.
///
/// A generic Field-wise backend capability does not imply that its adapter can
/// execute transient conservative transport. Constructing this witness is the
/// adapter's explicit claim that the enclosed Field-wise capability also
/// implements each enclosed convection scheme together with the exact
/// backward-difference and orthogonal-TPFA composition admitted by this
/// module. The transport resolver still validates the complete spatial,
/// algebraic, solver, and placement envelope; this type prevents generic axes
/// from silently standing in for method-specific transformations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientCellCenteredTransportCapabilities {
    fieldwise: RealizationCapabilities,
    convection_schemes: BTreeSet<CellCenteredConvectionScheme>,
    spatial_periodic_translation: bool,
}

impl TransientCellCenteredTransportCapabilities {
    /// Claim exact convection treatments over one ordinary Field-wise capability.
    ///
    /// # Errors
    /// Returns `EQ0807` when the adapter claims no executable treatment.
    pub fn new(
        fieldwise: RealizationCapabilities,
        convection_schemes: impl IntoIterator<Item = CellCenteredConvectionScheme>,
    ) -> Result<Self, Diagnostic> {
        let convection_schemes = convection_schemes.into_iter().collect::<BTreeSet<_>>();
        if convection_schemes.is_empty() {
            return Err(invalid_realization(
                "transient cell-centered transport capability must name at least one exact convection scheme",
            ));
        }
        Ok(Self {
            fieldwise,
            convection_schemes,
            spatial_periodic_translation: false,
        })
    }

    /// Add the exact Cartesian translation seam implemented by this adapter.
    #[must_use]
    pub fn with_spatial_periodic_translation(mut self) -> Self {
        self.spatial_periodic_translation = true;
        self
    }

    /// Generic spatial, solver (including scalar), layout, and target
    /// capability that remains part of this exact transport execution profile.
    #[must_use]
    pub const fn fieldwise(&self) -> &RealizationCapabilities {
        &self.fieldwise
    }

    /// Whether the adapter implements one exact convection treatment.
    #[must_use]
    pub fn supports_convection(&self, scheme: CellCenteredConvectionScheme) -> bool {
        self.convection_schemes.contains(&scheme)
    }

    /// Whether the adapter pairs conforming Cartesian periodic facets and
    /// executes them as one conservative coupled-face action.
    #[must_use]
    pub const fn supports_spatial_periodic_translation(&self) -> bool {
        self.spatial_periodic_translation
    }
}

/// Complete linear transient cell-centered transport selection.
///
/// Spatial representation, scaling, linear solver, placement, and schedule
/// remain in the ordinary Field-wise plan. This sibling of the nonlinear
/// mixed-FEM transient plan adds exactly one backward difference, one typed
/// convection treatment, and orthogonal TPFA; it does not own run length or
/// physical boundary meaning.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientCellCenteredTransportRealizationPlan {
    fieldwise: FieldwiseRealizationPlan,
    time_step: BackwardEulerRelationStep,
    convection: CellCenteredConvection,
    diffusion: OrthogonalTwoPointDiffusion,
}

impl TransientCellCenteredTransportRealizationPlan {
    /// Compose and cross-check one linear cell-centered transport plan.
    ///
    /// # Errors
    /// Returns `EQ0807` unless the composed plan selects exactly one
    /// cell-constant Field over generated uniform Cartesian cells with
    /// cell-centroid quadrature, no algebraic constraint, a general linear
    /// operator, and one exact Relation/state pair for all three transformations.
    pub fn new(
        fieldwise: FieldwiseRealizationPlan,
        time_step: BackwardEulerRelationStep,
        convection: CellCenteredConvection,
        diffusion: OrthogonalTwoPointDiffusion,
    ) -> Result<Self, Diagnostic> {
        let value = Self {
            fieldwise,
            time_step,
            convection,
            diffusion,
        };
        value.validate()?;
        Ok(value)
    }

    /// Ordinary spatial, scaling, linear-solver, and execution selection.
    #[must_use]
    pub const fn fieldwise(&self) -> &FieldwiseRealizationPlan {
        &self.fieldwise
    }

    /// Exact one-step backward-difference transformation.
    #[must_use]
    pub const fn time_step(&self) -> BackwardEulerRelationStep {
        self.time_step
    }

    /// Exact convection reconstruction and evaluation-time policy.
    #[must_use]
    pub const fn convection(&self) -> CellCenteredConvection {
        self.convection
    }

    /// Exact orthogonal two-point diffusive-flux selection.
    #[must_use]
    pub const fn diffusion(&self) -> OrthogonalTwoPointDiffusion {
        self.diffusion
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
                MeshPolicy::GeneratedUniform { .. },
                QuadraturePolicy::CellCentroid,
            )
        ) || spatial.field_spaces().len() != 1
            || spatial.field_spaces()[0].space().family() != SpaceFamily::CellConstant
            || !spatial.constraints().is_empty()
        {
            return Err(invalid_realization(
                "transient cell-centered transport requires one unconstrained cell-constant Field on generated uniform Cartesian cells with cell-centroid quadrature",
            ));
        }
        if self.fieldwise.operator_properties() != LinearOperatorProperties::General {
            return Err(invalid_realization(
                "cell-centered convection transport requires a general linear operator contract",
            ));
        }
        if self.time_step.relation() != self.convection.relation() {
            return Err(invalid_realization(
                "backward difference and convection transformations must select one exact Relation",
            ));
        }
        if self.time_step.relation() != self.diffusion.relation() {
            return Err(invalid_realization(
                "Backward Euler and orthogonal two-point diffusion must select one exact Relation",
            ));
        }
        if self.time_step.state() != self.convection.state() {
            return Err(invalid_realization(
                "backward difference and convection transformations must select one exact state Field",
            ));
        }
        if self.time_step.state() != self.diffusion.state() {
            return Err(invalid_realization(
                "Backward Euler and orthogonal two-point diffusion must select one exact state Field",
            ));
        }
        if spatial.field_spaces()[0].field() != self.time_step.state() {
            return Err(invalid_realization(
                "transported state Field must be the sole composed Field-wise unknown",
            ));
        }
        Ok(())
    }
}

/// Exact lowerer facts required by one cell-centered transport realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientCellCenteredTransportRealizationRequirements {
    fieldwise: FieldwiseRealizationRequirements,
    relation: Id<kinds::Relation>,
    state: Id<kinds::Field>,
    spatial_periodic_connections: Vec<Id<kinds::Connection>>,
}

impl TransientCellCenteredTransportRealizationRequirements {
    /// Bind ordinary Field-wise requirements to one exact transported state.
    ///
    /// # Errors
    /// Returns `EQ0807` unless the lowerer inventory contains exactly that
    /// transported state.
    pub fn new(
        fieldwise: FieldwiseRealizationRequirements,
        relation: Id<kinds::Relation>,
        state: Id<kinds::Field>,
    ) -> Result<Self, Diagnostic> {
        if fieldwise.unknown_fields() != [state] {
            return Err(invalid_realization(
                "cell-centered transport requirements must contain exactly the transported state Field",
            ));
        }
        Ok(Self {
            fieldwise,
            relation,
            state,
            spatial_periodic_connections: Vec::new(),
        })
    }

    /// Bind the exact spatial-periodic Connections retained by the lowerer.
    #[must_use]
    pub fn with_spatial_periodic_connections(
        mut self,
        connections: impl IntoIterator<Item = Id<kinds::Connection>>,
    ) -> Self {
        self.spatial_periodic_connections = connections.into_iter().collect();
        self.spatial_periodic_connections
            .sort_unstable_by_key(|connection| connection.erase());
        self.spatial_periodic_connections.dedup();
        self
    }

    /// Ordinary exact Domain, Field, and execution requirements.
    #[must_use]
    pub const fn fieldwise(&self) -> &FieldwiseRealizationRequirements {
        &self.fieldwise
    }

    /// Exact Semantic Relation carrying the conservative transient balance.
    #[must_use]
    pub const fn relation(&self) -> Id<kinds::Relation> {
        self.relation
    }

    /// Exact transported state Field selected by the lowerer.
    #[must_use]
    pub const fn state(&self) -> Id<kinds::Field> {
        self.state
    }

    /// Exact Model-owned spatial-periodic identifications to be realized.
    #[must_use]
    pub fn spatial_periodic_connections(&self) -> &[Id<kinds::Connection>] {
        &self.spatial_periodic_connections
    }
}

/// Explicit cell-centered transport request at an independent revision.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientCellCenteredTransportRealizationRequest {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    realization_revision: RealizationRevision,
    plan: TransientCellCenteredTransportRealizationPlan,
}

impl TransientCellCenteredTransportRealizationRequest {
    /// Bind one explicit transport plan to exact semantic and Realization revisions.
    #[must_use]
    pub const fn explicit(
        model: OntologyId<Model>,
        semantic_revision: SemanticRevision,
        realization_revision: RealizationRevision,
        plan: TransientCellCenteredTransportRealizationPlan,
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

    /// Explicit unresolved transport plan.
    #[must_use]
    pub const fn plan(&self) -> &TransientCellCenteredTransportRealizationPlan {
        &self.plan
    }
}

/// Validated transport plan, exact lowerer requirements, and two-layer provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTransientCellCenteredTransportRealization {
    fieldwise: ResolvedFieldwiseRealization,
    requirements: TransientCellCenteredTransportRealizationRequirements,
    plan: TransientCellCenteredTransportRealizationPlan,
}

impl ResolvedTransientCellCenteredTransportRealization {
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

    /// Resolved ordinary Field-wise admission reused by this contract.
    #[must_use]
    pub const fn fieldwise(&self) -> &ResolvedFieldwiseRealization {
        &self.fieldwise
    }

    /// Exact lowerer facts used during admission.
    #[must_use]
    pub const fn requirements(&self) -> &TransientCellCenteredTransportRealizationRequirements {
        &self.requirements
    }

    /// Complete validated transport plan.
    #[must_use]
    pub const fn plan(&self) -> &TransientCellCenteredTransportRealizationPlan {
        &self.plan
    }
}

/// Resolve one explicit cell-centered transport request without fallback.
///
/// Ordinary Domain/Field/capability admission is delegated to
/// [`resolve_fieldwise`]. This layer additionally requires an explicit
/// [`TransientCellCenteredTransportCapabilities`] witness, adds exact
/// Relation/state identity checks, and never substitutes another
/// reconstruction or diffusion method.
///
/// # Errors
/// Returns `EQ0807` for an invalid plan, Relation/state identity drift, or any
/// ordinary Field-wise capability-admission error.
pub fn resolve_transient_cell_centered_transport(
    request: &TransientCellCenteredTransportRealizationRequest,
    requirements: TransientCellCenteredTransportRealizationRequirements,
    capabilities: &TransientCellCenteredTransportCapabilities,
) -> Result<ResolvedTransientCellCenteredTransportRealization, Diagnostic> {
    request.plan.validate()?;
    if request.plan.time_step.relation() != requirements.relation
        || request.plan.convection.relation() != requirements.relation
        || request.plan.diffusion.relation() != requirements.relation
    {
        return Err(invalid_realization(
            "cell-centered transport plan Relation differs from the exact lowerer requirement",
        ));
    }
    if request.plan.time_step.state() != requirements.state
        || request.plan.convection.state() != requirements.state
        || request.plan.diffusion.state() != requirements.state
    {
        return Err(invalid_realization(
            "cell-centered transport plan state differs from the exact lowerer requirement",
        ));
    }
    if !capabilities.supports_convection(request.plan.convection.scheme()) {
        return Err(invalid_realization(format!(
            "cell-centered transport adapter does not support requested convection scheme {:?}",
            request.plan.convection.scheme()
        )));
    }
    if !requirements.spatial_periodic_connections.is_empty()
        && !capabilities.supports_spatial_periodic_translation()
    {
        return Err(invalid_realization(
            "cell-centered transport adapter does not support the required spatial-periodic Cartesian translation",
        ));
    }
    if !requirements.spatial_periodic_connections.is_empty()
        && request.plan.convection.scheme()
            != CellCenteredConvectionScheme::ImplicitFirstOrderUpwind
    {
        return Err(invalid_realization(
            "spatial-periodic transport currently requires implicit first-order upwind reconstruction",
        ));
    }
    let fieldwise_request = FieldwiseRealizationRequest::explicit(
        request.model,
        request.semantic_revision,
        request.realization_revision,
        request.plan.fieldwise.clone(),
    );
    let fieldwise = resolve_fieldwise(
        &fieldwise_request,
        requirements.fieldwise.clone(),
        capabilities.fieldwise(),
    )?;
    Ok(ResolvedTransientCellCenteredTransportRealization {
        fieldwise,
        requirements,
        plan: request.plan.clone(),
    })
}

#[cfg(test)]
mod tests;
