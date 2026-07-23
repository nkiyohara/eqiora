//! Canonical in-memory projection of a resolved portable Realization.
//!
//! The accepted v1/v2/v3 plans remain compatibility authoring and wire
//! contracts.  After their existing validators succeed, this module lowers
//! them into one small, typed DAG.  It deliberately has no arbitrary node,
//! edge, payload, runtime handle, device ordinal, or allocation vocabulary.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DynQuantity, Id, OntologyId};
use eqiora_schema::Model;
use eqiora_solver::{LinearOperatorProperties, ScalarType, SolverPlan};

use crate::{
    AleGeometryQualityGate, AlgebraicBlock, AlgebraicConstraint, CellCenteredConvectionScheme,
    Discretization, ExecutionSchedule, NonlinearSolvePlan, PositiveMomentumDiagonal,
    PositivePhysicalScale, ResolutionSource, ResolvedCoupledFieldwiseRealization,
    ResolvedFieldwiseRealization, ResolvedFixedTopologyAleCoupledRealization, ResolvedRealization,
    ResolvedTransientCellCenteredIncompressibleFlowRealization,
    ResolvedTransientCellCenteredTransportRealization, ResolvedTransientFieldwiseRealization,
    Space, SymmetricCongruenceScaling, Target, TransientFaceFluxHistory, VectorLayoutKind,
    invalid_realization,
};

macro_rules! graph_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(usize);

        impl $name {
            const fn new(index: usize) -> Self {
                Self(index)
            }

            /// Canonical arena position inside this graph projection.
            #[must_use]
            pub const fn index(self) -> usize {
                self.0
            }
        }
    };
}

graph_id!(
    /// Typed reference to one Domain discretization node.
    DomainDiscretizationId
);
graph_id!(
    /// Typed reference to one Field representation node.
    FieldRepresentationId
);
graph_id!(
    /// Typed reference to one sealed moving-geometry action.
    GeometryActionId
);
graph_id!(
    /// Typed reference to one numerical transformation node.
    TransformationId
);
graph_id!(
    /// Typed reference to one algebraic-system node.
    AlgebraicSystemId
);
graph_id!(
    /// Typed reference to one linear-solve node.
    LinearSolveId
);
graph_id!(
    /// Typed reference to one nonlinear-solve node.
    NonlinearSolveId
);
graph_id!(
    /// Typed reference to one portable placement requirement.
    PlacementRequirementId
);

/// Semantic and independently revisioned Realization lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealizationLineage {
    model: OntologyId<Model>,
    semantic_revision: crate::SemanticRevision,
    source: ResolutionSource,
}

impl RealizationLineage {
    /// Exact Semantic Model identity.
    #[must_use]
    pub const fn model(self) -> OntologyId<Model> {
        self.model
    }

    /// Exact Semantic Model revision.
    #[must_use]
    pub const fn semantic_revision(self) -> crate::SemanticRevision {
        self.semantic_revision
    }

    /// Named-default or independent explicit Realization revision.
    #[must_use]
    pub const fn source(self) -> ResolutionSource {
        self.source
    }
}

/// How coordinates enter one portable spatial discretization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoordinateTreatment {
    /// Coordinates are consumed in the Model's declared reference values.
    Physical,
    /// Coordinates are normalized by one positive coherent-SI length.
    Scaled(PositivePhysicalScale),
}

/// Physical configuration in which one Domain-local weak action is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainConfiguration {
    /// Existing fixed-geometry projection with no moving-configuration role.
    FixedGeometry,
    /// Immutable material/reference configuration.
    ReferenceConfiguration,
    /// A Domain is evaluated through one exact current ALE geometry action.
    CurrentAleGeometry {
        /// Sole action deriving current coordinates and mesh kinematics.
        action: GeometryActionId,
    },
}

/// Equation-aware identity and operator claim for a sole semantic Field.
///
/// Compatibility `RealizationPlan` values predate Domain, Field, and operator
/// identity. A canonical lowerer supplies this structurally typed claim, then
/// the equation-aware execution finalizer must compare it with the exact
/// accepted lowering before execution. The portable projection seals its
/// operator property against the capability tuple retained at resolution; it
/// never invents anonymous identities or infers mathematics from the selected
/// solver, but it does not independently prove caller-supplied identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleFieldOperatorClaim {
    domain: Id<kinds::Domain>,
    field: Id<kinds::Field>,
    operator_properties: LinearOperatorProperties,
}

impl SingleFieldOperatorClaim {
    /// Claim one exact Domain/Field pair and its equation-aware operator class.
    #[must_use]
    pub const fn new(
        domain: Id<kinds::Domain>,
        field: Id<kinds::Field>,
        operator_properties: LinearOperatorProperties,
    ) -> Self {
        Self {
            domain,
            field,
            operator_properties,
        }
    }

    /// Exact Semantic Domain.
    #[must_use]
    pub const fn domain(self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Exact Semantic Field.
    #[must_use]
    pub const fn field(self) -> Id<kinds::Field> {
        self.field
    }

    /// Mathematical property claimed by the equation-aware lowerer.
    #[must_use]
    pub const fn operator_properties(self) -> LinearOperatorProperties {
        self.operator_properties
    }
}

/// One exact Semantic Domain and its portable spatial selection.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainDiscretizationNode {
    domain: Id<kinds::Domain>,
    coordinates: CoordinateTreatment,
    configuration: DomainConfiguration,
    discretization: Discretization,
}

impl DomainDiscretizationNode {
    /// Exact Semantic Domain.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Physical or explicitly scaled coordinate treatment.
    #[must_use]
    pub const fn coordinates(&self) -> CoordinateTreatment {
        self.coordinates
    }

    /// Fixed, reference, or current-ALE physical configuration.
    #[must_use]
    pub const fn configuration(&self) -> DomainConfiguration {
        self.configuration
    }

    /// Method, mesh policy, and quadrature choice.
    #[must_use]
    pub const fn discretization(&self) -> Discretization {
        self.discretization
    }
}

/// One exact Semantic Field represented in one Domain-local discrete space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldRepresentationNode {
    domain: DomainDiscretizationId,
    field: Id<kinds::Field>,
    space: Space,
}

impl FieldRepresentationNode {
    /// Domain-discretization dependency.
    #[must_use]
    pub const fn domain(self) -> DomainDiscretizationId {
        self.domain
    }

    /// Exact Semantic Field.
    #[must_use]
    pub const fn field(self) -> Id<kinds::Field> {
        self.field
    }

    /// Scalar basis applied to every semantic component.
    #[must_use]
    pub const fn space(self) -> Space {
        self.space
    }
}

/// One sealed moving-geometry action in the portable Realization DAG.
///
/// Coordinates, mesh velocity, its spatial gradient, and geometric-
/// conservation data are projections of this node.  They cannot be supplied
/// as independent graph inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeometryActionNode {
    /// Component-wise P1 harmonic extension on immutable reference topology.
    P1HarmonicExtension {
        /// Fluid Domain receiving the current ALE geometry.
        fluid_domain: DomainDiscretizationId,
        /// Solid Domain retained in its reference configuration.
        solid_domain: DomainDiscretizationId,
        /// Absolute solid-displacement Field driving the interface geometry.
        driver: FieldRepresentationId,
        /// Exact conforming FSI Connection defining the moving interface.
        interface: Id<kinds::Connection>,
        /// Shared physical duration used to derive mesh velocity.
        duration: DynQuantity,
        /// Trial and accepted geometry quality gate.
        quality_gate: AleGeometryQualityGate,
        /// Solver policy for the symmetric-positive-definite harmonic action.
        solver: SolverPlan,
    },
}

/// Portable numerical transformation selected over exact semantic identities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformationNode {
    /// Replace one exact Relation derivative by a backward difference.
    BackwardEulerDerivative {
        /// Exact Semantic Relation.
        relation: Id<kinds::Relation>,
        /// Differential Field representation.
        state: FieldRepresentationId,
        /// Positive physical step duration.
        duration: DynQuantity,
    },
    /// Use the energy-skew weak form for one conservative convective term.
    EnergySkewConvection {
        /// Exact Semantic Relation.
        relation: Id<kinds::Relation>,
        /// Velocity Field used in both convective slots.
        velocity: FieldRepresentationId,
    },
    /// Apply one exact cell-centered convection reconstruction in time and space.
    CellCenteredConvection {
        /// Exact conservative Semantic Relation.
        relation: Id<kinds::Relation>,
        /// Cell-centered transported Field representation.
        state: FieldRepresentationId,
        /// Exact endpoint or previous-state reconstruction policy.
        scheme: CellCenteredConvectionScheme,
    },
    /// Use orthogonal two-point diffusive fluxes on cell-centered faces.
    OrthogonalTwoPointDiffusion {
        /// Exact conservative Semantic Relation.
        relation: Id<kinds::Relation>,
        /// Cell-centered transported Field representation.
        state: FieldRepresentationId,
    },
    /// Use the unique collocated face flux for implicit centered momentum convection.
    ImplicitCenteredMomentumConvection {
        /// Exact conservative momentum Relation.
        relation: Id<kinds::Relation>,
        /// Cell-centered velocity representation.
        velocity: FieldRepresentationId,
    },
    /// Apply centered Cartesian Newtonian velocity--pressure face traction.
    CartesianCentralNewtonianTraction {
        /// Exact conservative momentum Relation.
        relation: Id<kinds::Relation>,
        /// Cell-centered velocity representation.
        velocity: FieldRepresentationId,
        /// Cell-centered pressure representation.
        pressure: FieldRepresentationId,
    },
    /// Share one linearly exact momentum-weighted face flux between equations.
    MomentumWeightedLinearExactCoupling {
        /// Exact conservative momentum Relation.
        momentum_relation: Id<kinds::Relation>,
        /// Exact incompressibility Relation.
        incompressibility_relation: Id<kinds::Relation>,
        /// Cell-centered velocity representation.
        velocity: FieldRepresentationId,
        /// Cell-centered pressure representation.
        pressure: FieldRepresentationId,
        /// Exact positive momentum scale used by face interpolation.
        positive_diagonal: PositiveMomentumDiagonal,
        /// Exact previous-time face-flux closure.
        transient_history: TransientFaceFluxHistory,
    },
    /// Pull back one conservative fluid action through a sealed moving geometry.
    ///
    /// Relative energy-skew convection and the endpoint differential GCL
    /// correction are one transformation and cannot be selected separately.
    GclCompatibleAlePullback {
        /// Exact conservative transient fluid Relation.
        relation: Id<kinds::Relation>,
        /// Physical fluid velocity represented on the moving Domain.
        velocity: FieldRepresentationId,
        /// Sole source of current maps and mesh kinematics.
        geometry: GeometryActionId,
    },
    /// Eliminate one state in favour of its exact rate by backward Euler.
    BackwardEulerElimination {
        /// Exact kinematic Relation being transformed.
        relation: Id<kinds::Relation>,
        /// Represented state omitted from the algebraic unknowns.
        state: FieldRepresentationId,
        /// Algebraic rate retained in the system.
        rate: FieldRepresentationId,
        /// Positive physical step duration.
        duration: DynQuantity,
        /// Characteristic physical scale for state reconstruction.
        state_scale: PositivePhysicalScale,
    },
    /// Identify two exact conforming traces through one Semantic Connection.
    ConformingTraceQuotient {
        /// Exact conserving Connection selected by the lowerer.
        connection: Id<kinds::Connection>,
        /// Canonically ordered cross-Domain Field representations.
        endpoints: [FieldRepresentationId; 2],
    },
}

/// One unknown block in a monolithic algebraic system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemBlock {
    /// Coefficients of one represented Semantic Field.
    Field(FieldRepresentationId),
    /// Multiplier introduced by one exact algebraic constraint.
    ConstraintMultiplier(AlgebraicConstraint),
}

/// Scaling treatment of one portable algebraic system.
#[derive(Debug, Clone, PartialEq)]
pub enum SystemScaling {
    /// The compatibility path acts directly in its declared scalar values.
    Dimensional,
    /// One explicit physical-to-dimensionless symmetric congruence.
    SymmetricCongruence(SymmetricCongruenceScaling),
}

/// One connected algebraic operator selected by a portable Realization.
#[derive(Debug, Clone, PartialEq)]
pub struct AlgebraicSystemNode {
    blocks: Vec<SystemBlock>,
    transformations: Vec<TransformationId>,
    scaling: SystemScaling,
    operator_properties: LinearOperatorProperties,
    scalar_type: ScalarType,
    partition: VectorLayoutKind,
}

impl AlgebraicSystemNode {
    /// Canonical monolithic block order.
    #[must_use]
    pub fn blocks(&self) -> &[SystemBlock] {
        &self.blocks
    }

    /// Transformations contributing to this system.
    #[must_use]
    pub fn transformations(&self) -> &[TransformationId] {
        &self.transformations
    }

    /// Exact whole-system congruence scaling.
    #[must_use]
    pub const fn scaling(&self) -> &SystemScaling {
        &self.scaling
    }

    /// Explicit symmetric congruence, when selected by this system.
    #[must_use]
    pub const fn congruence_scaling(&self) -> Option<&SymmetricCongruenceScaling> {
        match &self.scaling {
            SystemScaling::Dimensional => None,
            SystemScaling::SymmetricCongruence(scaling) => Some(scaling),
        }
    }

    /// Mathematical property asserted for this realized operator.
    #[must_use]
    pub const fn operator_properties(&self) -> LinearOperatorProperties {
        self.operator_properties
    }

    /// Scalar storage and arithmetic policy.
    #[must_use]
    pub const fn scalar_type(&self) -> ScalarType {
        self.scalar_type
    }

    /// Replicated or explicitly partitioned global algebra.
    #[must_use]
    pub const fn partition(&self) -> VectorLayoutKind {
        self.partition
    }
}

/// Portable compute requirement; an observed machine is bound later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementRequirementNode {
    /// Host execution with an exact worker bound per partition.
    HostWorkers {
        /// Non-zero requested host workers.
        workers_per_partition: NonZeroUsize,
    },
    /// CUDA execution with a device count per partition, never an ordinal.
    CudaDevices {
        /// Non-zero requested devices for every partition.
        devices_per_partition: NonZeroUsize,
    },
}

impl PlacementRequirementNode {
    /// Requested host-worker count, when this is a host placement.
    #[must_use]
    pub const fn host_workers_per_partition(self) -> Option<NonZeroUsize> {
        match self {
            Self::HostWorkers {
                workers_per_partition,
            } => Some(workers_per_partition),
            Self::CudaDevices { .. } => None,
        }
    }

    /// Requested CUDA-device count, when this is a device placement.
    #[must_use]
    pub const fn cuda_devices_per_partition(self) -> Option<NonZeroUsize> {
        match self {
            Self::HostWorkers { .. } => None,
            Self::CudaDevices {
                devices_per_partition,
            } => Some(devices_per_partition),
        }
    }
}

/// One linear solver role attached to one algebraic system and placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearSolveNode {
    system: AlgebraicSystemId,
    plan: SolverPlan,
    placement: PlacementRequirementId,
    schedule: ExecutionSchedule,
}

impl LinearSolveNode {
    /// Algebraic system being solved.
    #[must_use]
    pub const fn system(self) -> AlgebraicSystemId {
        self.system
    }

    /// Sole linear-solver policy.
    #[must_use]
    pub const fn plan(self) -> SolverPlan {
        self.plan
    }

    /// Portable compute requirement.
    #[must_use]
    pub const fn placement(self) -> PlacementRequirementId {
        self.placement
    }

    /// Deployment scheduling requirement, separate from model time.
    #[must_use]
    pub const fn schedule(self) -> ExecutionSchedule {
        self.schedule
    }
}

/// One nonlinear solve whose linearization uses an explicit linear-solve node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonlinearSolveNode {
    residual_system: AlgebraicSystemId,
    linearization: LinearSolveId,
    plan: NonlinearSolvePlan,
}

impl NonlinearSolveNode {
    /// Nonlinear residual system.
    #[must_use]
    pub const fn residual_system(self) -> AlgebraicSystemId {
        self.residual_system
    }

    /// Linear solve used for every admitted linearization.
    #[must_use]
    pub const fn linearization(self) -> LinearSolveId {
        self.linearization
    }

    /// Nonlinear convergence and globalization policy.
    #[must_use]
    pub const fn plan(self) -> NonlinearSolvePlan {
        self.plan
    }
}

/// Sole accepted solve root of one connected Phase-A graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveRoot {
    /// A linear solve is the accepted output boundary.
    Linear(LinearSolveId),
    /// A nonlinear solve is the accepted output boundary.
    Nonlinear(NonlinearSolveId),
}

/// Canonical portable DAG projected from one already resolved Realization.
///
/// This type is intentionally in-memory only. Frozen artifact envelopes retain
/// their bytes and digests; a graph-native wire is considered only after all
/// accepted plan families prove lossless projection. Graph validation proves
/// structural closure and solver compatibility. An equation-aware consumer
/// must additionally compare all claimed Semantic identities and mathematical
/// properties with its accepted lowering before this graph can authorize a
/// run or become evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct PortableRealizationGraph {
    lineage: RealizationLineage,
    domains: Vec<DomainDiscretizationNode>,
    fields: Vec<FieldRepresentationNode>,
    geometry_actions: Vec<GeometryActionNode>,
    transformations: Vec<TransformationNode>,
    systems: Vec<AlgebraicSystemNode>,
    linear_solves: Vec<LinearSolveNode>,
    nonlinear_solves: Vec<NonlinearSolveNode>,
    placements: Vec<PlacementRequirementNode>,
    root: SolveRoot,
}

impl PortableRealizationGraph {
    /// Exact Model and Realization lineage.
    #[must_use]
    pub const fn lineage(&self) -> RealizationLineage {
        self.lineage
    }

    /// Canonically ordered Domain discretizations.
    #[must_use]
    pub fn domains(&self) -> &[DomainDiscretizationNode] {
        &self.domains
    }

    /// Canonically ordered Field representations.
    #[must_use]
    pub fn fields(&self) -> &[FieldRepresentationNode] {
        &self.fields
    }

    /// Canonically ordered sealed moving-geometry actions.
    #[must_use]
    pub fn geometry_actions(&self) -> &[GeometryActionNode] {
        &self.geometry_actions
    }

    /// Canonically ordered numerical transformations.
    #[must_use]
    pub fn transformations(&self) -> &[TransformationNode] {
        &self.transformations
    }

    /// Connected algebraic systems.
    #[must_use]
    pub fn systems(&self) -> &[AlgebraicSystemNode] {
        &self.systems
    }

    /// Linear-solver roles.
    #[must_use]
    pub fn linear_solves(&self) -> &[LinearSolveNode] {
        &self.linear_solves
    }

    /// Nonlinear-solver roles.
    #[must_use]
    pub fn nonlinear_solves(&self) -> &[NonlinearSolveNode] {
        &self.nonlinear_solves
    }

    /// Portable compute requirements without environment-local ordinals.
    #[must_use]
    pub fn placements(&self) -> &[PlacementRequirementNode] {
        &self.placements
    }

    /// Sole connected solve root.
    #[must_use]
    pub const fn root(&self) -> SolveRoot {
        self.root
    }

    /// Resolve a typed Domain reference.
    #[must_use]
    pub fn domain(&self, id: DomainDiscretizationId) -> Option<&DomainDiscretizationNode> {
        self.domains.get(id.index())
    }

    /// Resolve a typed Field reference.
    #[must_use]
    pub fn field(&self, id: FieldRepresentationId) -> Option<&FieldRepresentationNode> {
        self.fields.get(id.index())
    }

    /// Resolve a typed moving-geometry action reference.
    #[must_use]
    pub fn geometry_action(&self, id: GeometryActionId) -> Option<&GeometryActionNode> {
        self.geometry_actions.get(id.index())
    }

    /// Resolve a typed transformation reference.
    #[must_use]
    pub fn transformation(&self, id: TransformationId) -> Option<&TransformationNode> {
        self.transformations.get(id.index())
    }

    /// Resolve a typed algebraic-system reference.
    #[must_use]
    pub fn system(&self, id: AlgebraicSystemId) -> Option<&AlgebraicSystemNode> {
        self.systems.get(id.index())
    }

    /// Resolve a typed linear-solve reference.
    #[must_use]
    pub fn linear_solve(&self, id: LinearSolveId) -> Option<&LinearSolveNode> {
        self.linear_solves.get(id.index())
    }

    /// Resolve a typed nonlinear-solve reference.
    #[must_use]
    pub fn nonlinear_solve(&self, id: NonlinearSolveId) -> Option<&NonlinearSolveNode> {
        self.nonlinear_solves.get(id.index())
    }

    /// Resolve a typed placement reference.
    #[must_use]
    pub fn placement(&self, id: PlacementRequirementId) -> Option<PlacementRequirementNode> {
        self.placements.get(id.index()).copied()
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        if self.domains.is_empty() || self.fields.is_empty() {
            return Err(invalid_realization(
                "portable Realization graph requires a Domain and a Field representation",
            ));
        }
        if !strictly_sorted_unique_by(&self.domains, |node| node.domain.ulid()) {
            return Err(invalid_realization(
                "portable Realization Domain nodes must be unique and canonically ordered",
            ));
        }
        if !strictly_sorted_unique_by(&self.fields, |node| node.field.ulid()) {
            return Err(invalid_realization(
                "portable Realization Field nodes must be unique and canonically ordered",
            ));
        }
        for field in &self.fields {
            if self.domain(field.domain).is_none() {
                return Err(invalid_realization(
                    "portable Realization Field references an absent Domain node",
                ));
            }
        }
        validate_geometry_actions(self)?;
        if self.systems.len() != 1
            || self.linear_solves.len() != 1
            || self.placements.len() != 1
            || self.nonlinear_solves.len() > 1
        {
            return Err(invalid_realization(
                "portable Realization Phase A admits one connected system, linear solve, placement, and optional nonlinear root",
            ));
        }
        let system = &self.systems[0];
        validate_system(self, system)?;
        let linear = self.linear_solves[0];
        if linear.system != AlgebraicSystemId::new(0)
            || linear.placement != PlacementRequirementId::new(0)
        {
            return Err(invalid_realization(
                "portable linear solve must reference the connected system and placement",
            ));
        }
        if !linear.plan.algorithm().accepts(system.operator_properties) {
            return Err(invalid_realization(
                "portable linear solver algorithm is incompatible with the asserted operator properties",
            ));
        }
        match self.root {
            SolveRoot::Linear(id) => {
                if id != LinearSolveId::new(0) || !self.nonlinear_solves.is_empty() {
                    return Err(invalid_realization(
                        "portable linear root leaves an unreachable solve node",
                    ));
                }
            }
            SolveRoot::Nonlinear(id) => {
                let Some(nonlinear) = self.nonlinear_solve(id) else {
                    return Err(invalid_realization(
                        "portable nonlinear root references an absent node",
                    ));
                };
                if self.nonlinear_solves.len() != 1
                    || nonlinear.residual_system != AlgebraicSystemId::new(0)
                    || nonlinear.linearization != LinearSolveId::new(0)
                {
                    return Err(invalid_realization(
                        "portable nonlinear root must own the sole residual system and linearization",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl ResolvedRealization {
    /// Normalize an accepted compatibility plan for one exact semantic Field.
    ///
    /// The claim is deliberately supplied by an equation-aware lowerer because
    /// the old plan contains neither Semantic identities nor operator facts.
    /// This projection validates its structure and seals its operator property
    /// against the exact candidate set retained by compatibility resolution.
    /// The execution finalizer still owns comparison with the accepted
    /// equation identity and coefficients.
    ///
    /// # Errors
    /// Returns `EQ0807` if the resolved value cannot form one connected,
    /// portable linear-solve DAG.
    pub fn portable_graph(
        &self,
        claim: SingleFieldOperatorClaim,
    ) -> Result<PortableRealizationGraph, Diagnostic> {
        self.require_admitted_operator_properties(claim.operator_properties())?;
        let plan = self.plan();
        let requirements = self.requirements();
        let graph = PortableRealizationGraph {
            lineage: RealizationLineage {
                model: self.model(),
                semantic_revision: self.semantic_revision(),
                source: self.source(),
            },
            domains: vec![DomainDiscretizationNode {
                domain: claim.domain(),
                coordinates: CoordinateTreatment::Physical,
                configuration: DomainConfiguration::FixedGeometry,
                discretization: plan.discretization(),
            }],
            fields: vec![FieldRepresentationNode {
                domain: DomainDiscretizationId::new(0),
                field: claim.field(),
                space: plan.space(),
            }],
            geometry_actions: Vec::new(),
            transformations: Vec::new(),
            systems: vec![AlgebraicSystemNode {
                blocks: vec![SystemBlock::Field(FieldRepresentationId::new(0))],
                transformations: Vec::new(),
                scaling: SystemScaling::Dimensional,
                operator_properties: claim.operator_properties(),
                scalar_type: requirements.scalar_type(),
                partition: requirements.vector_layout(),
            }],
            linear_solves: vec![LinearSolveNode {
                system: AlgebraicSystemId::new(0),
                plan: plan.solver(),
                placement: PlacementRequirementId::new(0),
                schedule: plan.schedule(),
            }],
            nonlinear_solves: Vec::new(),
            placements: vec![portable_placement(plan.target())],
            root: SolveRoot::Linear(LinearSolveId::new(0)),
        };
        graph.validate()?;
        Ok(graph)
    }
}

impl ResolvedFieldwiseRealization {
    /// Normalize an accepted single-Domain field-wise plan into the portable DAG.
    ///
    /// # Errors
    /// Returns `EQ0807` if the resolved compatibility value cannot be
    /// represented losslessly by the connected Phase-A graph.
    pub fn portable_graph(&self) -> Result<PortableRealizationGraph, Diagnostic> {
        let plan = self.plan();
        let spatial = plan.spatial();
        let fields = spatial
            .field_spaces()
            .iter()
            .map(|binding| FieldRepresentationNode {
                domain: DomainDiscretizationId::new(0),
                field: binding.field(),
                space: binding.space(),
            })
            .collect::<Vec<_>>();
        let blocks = blocks_from_scaling(&fields, plan.scaling())?;
        let execution = self.requirements().execution();
        let graph = PortableRealizationGraph {
            lineage: RealizationLineage {
                model: self.model(),
                semantic_revision: self.semantic_revision(),
                source: ResolutionSource::Explicit(self.realization_revision()),
            },
            domains: vec![DomainDiscretizationNode {
                domain: spatial.domain(),
                coordinates: CoordinateTreatment::Scaled(spatial.coordinate_length_scale()),
                configuration: DomainConfiguration::FixedGeometry,
                discretization: spatial.discretization(),
            }],
            fields,
            geometry_actions: Vec::new(),
            transformations: Vec::new(),
            systems: vec![AlgebraicSystemNode {
                blocks,
                transformations: Vec::new(),
                scaling: SystemScaling::SymmetricCongruence(plan.scaling().clone()),
                operator_properties: plan.operator_properties(),
                scalar_type: execution.scalar_type(),
                partition: execution.vector_layout(),
            }],
            linear_solves: vec![LinearSolveNode {
                system: AlgebraicSystemId::new(0),
                plan: plan.solver(),
                placement: PlacementRequirementId::new(0),
                schedule: plan.schedule(),
            }],
            nonlinear_solves: Vec::new(),
            placements: vec![portable_placement(plan.target())],
            root: SolveRoot::Linear(LinearSolveId::new(0)),
        };
        graph.validate()?;
        Ok(graph)
    }
}

impl ResolvedCoupledFieldwiseRealization {
    /// Normalize an accepted multi-Domain plan using a claimed kinematic Relation.
    ///
    /// The compatibility plan predates Relation-bound transformations, so the
    /// equation-aware lowerer supplies the Relation it accepted. No anonymous
    /// or inferred Relation is fabricated by this projection, but the
    /// equation-aware execution finalizer owns the exact identity comparison.
    ///
    /// # Errors
    /// Returns `EQ0807` if any Domain, Field, quotient, state/rate, block, or
    /// solve reference cannot be represented losslessly.
    pub fn portable_graph(
        &self,
        claimed_eliminated_state_relation: Id<kinds::Relation>,
    ) -> Result<PortableRealizationGraph, Diagnostic> {
        let plan = self.plan();
        let spatial = plan.spatial();
        let domains = spatial
            .domains()
            .iter()
            .map(|selection| DomainDiscretizationNode {
                domain: selection.domain(),
                coordinates: CoordinateTreatment::Scaled(spatial.coordinate_length_scale()),
                configuration: DomainConfiguration::FixedGeometry,
                discretization: spatial.discretization(),
            })
            .collect::<Vec<_>>();
        let mut fields = spatial
            .domains()
            .iter()
            .enumerate()
            .flat_map(|(domain_index, selection)| {
                selection
                    .field_spaces()
                    .iter()
                    .map(move |binding| FieldRepresentationNode {
                        domain: DomainDiscretizationId::new(domain_index),
                        field: binding.field(),
                        space: binding.space(),
                    })
            })
            .collect::<Vec<_>>();
        let eliminated = plan.time_step().eliminated_state();
        let pair = eliminated.pair();
        let rate_domain = fields
            .iter()
            .find(|field| field.field == pair.rate())
            .map(|field| field.domain)
            .ok_or_else(|| {
                invalid_realization(
                    "coupled portable graph cannot locate the eliminated state's rate Domain",
                )
            })?;
        fields.push(FieldRepresentationNode {
            domain: rate_domain,
            field: pair.state(),
            space: eliminated.state_space(),
        });
        fields.sort_by_key(|field| field.field.ulid());
        let state = field_reference(&fields, pair.state())?;
        let rate = field_reference(&fields, pair.rate())?;
        let quotient = spatial.trace_quotient();
        let endpoints = quotient.endpoints().map(|endpoint| {
            field_reference(&fields, endpoint.field())
                .expect("resolved trace endpoint is present in the exact Field inventory")
        });
        let transformations = vec![
            TransformationNode::BackwardEulerElimination {
                relation: claimed_eliminated_state_relation,
                state,
                rate,
                duration: plan.time_step().duration(),
                state_scale: eliminated.state_scale(),
            },
            TransformationNode::ConformingTraceQuotient {
                connection: quotient.connection(),
                endpoints,
            },
        ];
        let blocks = blocks_from_scaling(&fields, plan.scaling())?;
        let execution = self.requirements().execution();
        let graph = PortableRealizationGraph {
            lineage: RealizationLineage {
                model: self.model(),
                semantic_revision: self.semantic_revision(),
                source: ResolutionSource::Explicit(self.realization_revision()),
            },
            domains,
            fields,
            geometry_actions: Vec::new(),
            transformations,
            systems: vec![AlgebraicSystemNode {
                blocks,
                transformations: vec![TransformationId::new(0), TransformationId::new(1)],
                scaling: SystemScaling::SymmetricCongruence(plan.scaling().clone()),
                operator_properties: plan.operator_properties(),
                scalar_type: execution.scalar_type(),
                partition: execution.vector_layout(),
            }],
            linear_solves: vec![LinearSolveNode {
                system: AlgebraicSystemId::new(0),
                plan: plan.solver(),
                placement: PlacementRequirementId::new(0),
                schedule: plan.schedule(),
            }],
            nonlinear_solves: Vec::new(),
            placements: vec![portable_placement(plan.target())],
            root: SolveRoot::Linear(LinearSolveId::new(0)),
        };
        graph.validate()?;
        Ok(graph)
    }
}

impl ResolvedTransientFieldwiseRealization {
    /// Normalize this accepted transient plan into the common portable DAG.
    ///
    /// The existing resolver remains the sole compatibility validator. This
    /// projection cannot add a step count, runtime backend, buffer, device
    /// ordinal, or another source of numerical policy.
    ///
    /// # Errors
    /// Returns `EQ0807` if the resolved compatibility value cannot be
    /// represented losslessly by the connected Phase-A graph.
    pub fn portable_graph(&self) -> Result<PortableRealizationGraph, Diagnostic> {
        let plan = self.plan();
        let fieldwise = plan.fieldwise();
        let spatial = fieldwise.spatial();
        let domain_id = DomainDiscretizationId::new(0);
        let domains = vec![DomainDiscretizationNode {
            domain: spatial.domain(),
            coordinates: CoordinateTreatment::Scaled(spatial.coordinate_length_scale()),
            configuration: DomainConfiguration::FixedGeometry,
            discretization: spatial.discretization(),
        }];
        let fields = spatial
            .field_spaces()
            .iter()
            .map(|binding| FieldRepresentationNode {
                domain: domain_id,
                field: binding.field(),
                space: binding.space(),
            })
            .collect::<Vec<_>>();
        let field_id = |field| field_reference(&fields, field);
        let time_step = plan.time_step();
        let convection = plan.convection();
        let transformations = vec![
            TransformationNode::BackwardEulerDerivative {
                relation: time_step.relation(),
                state: field_id(time_step.state())?,
                duration: time_step.duration(),
            },
            TransformationNode::EnergySkewConvection {
                relation: convection.relation(),
                velocity: field_id(convection.velocity())?,
            },
        ];
        let blocks = fieldwise
            .scaling()
            .block_scales()
            .iter()
            .map(|entry| match entry.block() {
                AlgebraicBlock::Field(field) => field_id(field).map(SystemBlock::Field),
                AlgebraicBlock::ConstraintMultiplier { field } => Ok(
                    SystemBlock::ConstraintMultiplier(AlgebraicConstraint::ZeroIntegral { field }),
                ),
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let execution = self.requirements().fieldwise().execution();
        let placement = portable_placement(fieldwise.target());
        let graph = PortableRealizationGraph {
            lineage: RealizationLineage {
                model: self.model(),
                semantic_revision: self.semantic_revision(),
                source: ResolutionSource::Explicit(self.realization_revision()),
            },
            domains,
            fields,
            geometry_actions: Vec::new(),
            transformations,
            systems: vec![AlgebraicSystemNode {
                blocks,
                transformations: vec![TransformationId::new(0), TransformationId::new(1)],
                scaling: SystemScaling::SymmetricCongruence(fieldwise.scaling().clone()),
                operator_properties: fieldwise.operator_properties(),
                scalar_type: execution.scalar_type(),
                partition: execution.vector_layout(),
            }],
            linear_solves: vec![LinearSolveNode {
                system: AlgebraicSystemId::new(0),
                plan: fieldwise.solver(),
                placement: PlacementRequirementId::new(0),
                schedule: fieldwise.schedule(),
            }],
            nonlinear_solves: vec![NonlinearSolveNode {
                residual_system: AlgebraicSystemId::new(0),
                linearization: LinearSolveId::new(0),
                plan: plan.nonlinear(),
            }],
            placements: vec![placement],
            root: SolveRoot::Nonlinear(NonlinearSolveId::new(0)),
        };
        graph.validate()?;
        Ok(graph)
    }
}

impl ResolvedTransientCellCenteredTransportRealization {
    /// Normalize this accepted linear transport plan into the common portable DAG.
    ///
    /// The graph records exactly one backward difference, one selected
    /// convection treatment, and orthogonal two-point diffusive flux over the
    /// same Relation/state pair. It cannot add run length, boundary meaning,
    /// or nonlinear policy.
    ///
    /// # Errors
    /// Returns `EQ0807` if the resolved compatibility value cannot be
    /// represented losslessly by the connected Phase-A graph.
    pub fn portable_graph(&self) -> Result<PortableRealizationGraph, Diagnostic> {
        let plan = self.plan();
        let fieldwise = plan.fieldwise();
        let spatial = fieldwise.spatial();
        let fields = spatial
            .field_spaces()
            .iter()
            .map(|binding| FieldRepresentationNode {
                domain: DomainDiscretizationId::new(0),
                field: binding.field(),
                space: binding.space(),
            })
            .collect::<Vec<_>>();
        let state = field_reference(&fields, plan.time_step().state())?;
        let transformations = vec![
            TransformationNode::BackwardEulerDerivative {
                relation: plan.time_step().relation(),
                state,
                duration: plan.time_step().duration(),
            },
            TransformationNode::CellCenteredConvection {
                relation: plan.convection().relation(),
                state,
                scheme: plan.convection().scheme(),
            },
            TransformationNode::OrthogonalTwoPointDiffusion {
                relation: plan.diffusion().relation(),
                state,
            },
        ];
        let blocks = blocks_from_scaling(&fields, fieldwise.scaling())?;
        let execution = self.requirements().fieldwise().execution();
        let graph = PortableRealizationGraph {
            lineage: RealizationLineage {
                model: self.model(),
                semantic_revision: self.semantic_revision(),
                source: ResolutionSource::Explicit(self.realization_revision()),
            },
            domains: vec![DomainDiscretizationNode {
                domain: spatial.domain(),
                coordinates: CoordinateTreatment::Scaled(spatial.coordinate_length_scale()),
                configuration: DomainConfiguration::FixedGeometry,
                discretization: spatial.discretization(),
            }],
            fields,
            geometry_actions: Vec::new(),
            transformations,
            systems: vec![AlgebraicSystemNode {
                blocks,
                transformations: vec![
                    TransformationId::new(0),
                    TransformationId::new(1),
                    TransformationId::new(2),
                ],
                scaling: SystemScaling::SymmetricCongruence(fieldwise.scaling().clone()),
                operator_properties: fieldwise.operator_properties(),
                scalar_type: execution.scalar_type(),
                partition: execution.vector_layout(),
            }],
            linear_solves: vec![LinearSolveNode {
                system: AlgebraicSystemId::new(0),
                plan: fieldwise.solver(),
                placement: PlacementRequirementId::new(0),
                schedule: fieldwise.schedule(),
            }],
            nonlinear_solves: Vec::new(),
            placements: vec![portable_placement(fieldwise.target())],
            root: SolveRoot::Linear(LinearSolveId::new(0)),
        };
        graph.validate()?;
        Ok(graph)
    }
}

impl ResolvedTransientCellCenteredIncompressibleFlowRealization {
    /// Normalize the accepted collocated flow plan into the portable DAG.
    ///
    /// One nonlinear root owns backward Euler, centered momentum convection,
    /// Newtonian traction, and the shared momentum-weighted face-flux coupling.
    /// Run length and physical boundary meaning remain outside the graph.
    ///
    /// # Errors
    /// Returns `EQ0807` when the accepted compatibility value cannot be
    /// represented losslessly by this connected graph.
    pub fn portable_graph(&self) -> Result<PortableRealizationGraph, Diagnostic> {
        let plan = self.plan();
        let fieldwise = plan.fieldwise();
        let spatial = fieldwise.spatial();
        let fields = spatial
            .field_spaces()
            .iter()
            .map(|binding| FieldRepresentationNode {
                domain: DomainDiscretizationId::new(0),
                field: binding.field(),
                space: binding.space(),
            })
            .collect::<Vec<_>>();
        let velocity = field_reference(&fields, plan.coupling().velocity())?;
        let pressure = field_reference(&fields, plan.coupling().pressure())?;
        let transformations = vec![
            TransformationNode::BackwardEulerDerivative {
                relation: plan.time_step().relation(),
                state: velocity,
                duration: plan.time_step().duration(),
            },
            TransformationNode::ImplicitCenteredMomentumConvection {
                relation: plan.convection().relation(),
                velocity,
            },
            TransformationNode::CartesianCentralNewtonianTraction {
                relation: plan.traction().relation(),
                velocity,
                pressure,
            },
            TransformationNode::MomentumWeightedLinearExactCoupling {
                momentum_relation: plan.coupling().momentum_relation(),
                incompressibility_relation: plan.coupling().incompressibility_relation(),
                velocity,
                pressure,
                positive_diagonal: plan.coupling().positive_diagonal(),
                transient_history: plan.coupling().transient_history(),
            },
        ];
        let blocks = blocks_from_scaling(&fields, fieldwise.scaling())?;
        let execution = self.requirements().fieldwise().execution();
        let graph = PortableRealizationGraph {
            lineage: RealizationLineage {
                model: self.model(),
                semantic_revision: self.semantic_revision(),
                source: ResolutionSource::Explicit(self.realization_revision()),
            },
            domains: vec![DomainDiscretizationNode {
                domain: spatial.domain(),
                coordinates: CoordinateTreatment::Scaled(spatial.coordinate_length_scale()),
                configuration: DomainConfiguration::FixedGeometry,
                discretization: spatial.discretization(),
            }],
            fields,
            geometry_actions: Vec::new(),
            transformations,
            systems: vec![AlgebraicSystemNode {
                blocks,
                transformations: vec![
                    TransformationId::new(0),
                    TransformationId::new(1),
                    TransformationId::new(2),
                    TransformationId::new(3),
                ],
                scaling: SystemScaling::SymmetricCongruence(fieldwise.scaling().clone()),
                operator_properties: fieldwise.operator_properties(),
                scalar_type: execution.scalar_type(),
                partition: execution.vector_layout(),
            }],
            linear_solves: vec![LinearSolveNode {
                system: AlgebraicSystemId::new(0),
                plan: fieldwise.solver(),
                placement: PlacementRequirementId::new(0),
                schedule: fieldwise.schedule(),
            }],
            nonlinear_solves: vec![NonlinearSolveNode {
                residual_system: AlgebraicSystemId::new(0),
                linearization: LinearSolveId::new(0),
                plan: plan.nonlinear(),
            }],
            placements: vec![portable_placement(fieldwise.target())],
            root: SolveRoot::Nonlinear(NonlinearSolveId::new(0)),
        };
        graph.validate()?;
        Ok(graph)
    }
}

impl ResolvedFixedTopologyAleCoupledRealization {
    /// Normalize the accepted fixed-topology ALE plan into one portable DAG.
    ///
    /// The graph contains one sealed geometry action.  The fluid Domain,
    /// endpoint ALE pullback, mesh velocity, and GCL correction all refer to
    /// that action; the solid Domain remains explicitly in the reference
    /// configuration.
    ///
    /// # Errors
    /// Returns `EQ0807` if any exact Domain, Field, transformation, geometry,
    /// or solve reference cannot form the closed nonlinear graph.
    pub fn portable_graph(&self) -> Result<PortableRealizationGraph, Diagnostic> {
        let plan = self.plan();
        let coupled = plan.coupled();
        let spatial = coupled.spatial();
        let action_id = GeometryActionId::new(0);
        let motion = plan.mesh_motion();
        let domains = spatial
            .domains()
            .iter()
            .map(|selection| {
                let configuration = if selection.domain() == motion.fluid_domain() {
                    DomainConfiguration::CurrentAleGeometry { action: action_id }
                } else if selection.domain() == motion.solid_domain() {
                    DomainConfiguration::ReferenceConfiguration
                } else {
                    unreachable!("validated ALE plan has exactly two covered Domains")
                };
                DomainDiscretizationNode {
                    domain: selection.domain(),
                    coordinates: CoordinateTreatment::Scaled(spatial.coordinate_length_scale()),
                    configuration,
                    discretization: spatial.discretization(),
                }
            })
            .collect::<Vec<_>>();
        let fluid_domain = domain_reference(&domains, motion.fluid_domain())?;
        let solid_domain = domain_reference(&domains, motion.solid_domain())?;

        let mut fields = spatial
            .domains()
            .iter()
            .enumerate()
            .flat_map(|(domain_index, selection)| {
                selection
                    .field_spaces()
                    .iter()
                    .map(move |binding| FieldRepresentationNode {
                        domain: DomainDiscretizationId::new(domain_index),
                        field: binding.field(),
                        space: binding.space(),
                    })
            })
            .collect::<Vec<_>>();
        let eliminated = coupled.time_step().eliminated_state();
        fields.push(FieldRepresentationNode {
            domain: solid_domain,
            field: eliminated.pair().state(),
            space: eliminated.state_space(),
        });
        fields.sort_by_key(|field| field.field.ulid());

        let driver = field_reference(&fields, motion.solid_displacement())?;
        let fluid_velocity = field_reference(&fields, plan.pullback().velocity())?;
        let solid_rate = field_reference(&fields, eliminated.pair().rate())?;
        let geometry_actions = vec![GeometryActionNode::P1HarmonicExtension {
            fluid_domain,
            solid_domain,
            driver,
            interface: motion.interface(),
            duration: plan.fluid_time_step().duration(),
            quality_gate: motion.quality_gate(),
            solver: motion.solver(),
        }];
        let quotient = spatial.trace_quotient();
        let endpoints = quotient.endpoints().map(|endpoint| {
            field_reference(&fields, endpoint.field())
                .expect("validated ALE trace endpoint is represented")
        });
        let transformations = vec![
            TransformationNode::BackwardEulerDerivative {
                relation: plan.fluid_time_step().relation(),
                state: fluid_velocity,
                duration: plan.fluid_time_step().duration(),
            },
            TransformationNode::BackwardEulerElimination {
                relation: plan.solid_kinematic_relation(),
                state: driver,
                rate: solid_rate,
                duration: coupled.time_step().duration(),
                state_scale: eliminated.state_scale(),
            },
            TransformationNode::ConformingTraceQuotient {
                connection: quotient.connection(),
                endpoints,
            },
            TransformationNode::GclCompatibleAlePullback {
                relation: plan.pullback().relation(),
                velocity: fluid_velocity,
                geometry: action_id,
            },
        ];
        let blocks = blocks_from_scaling(&fields, coupled.scaling())?;
        let execution = self.requirements().coupled().execution();
        let graph = PortableRealizationGraph {
            lineage: RealizationLineage {
                model: self.model(),
                semantic_revision: self.semantic_revision(),
                source: ResolutionSource::Explicit(self.realization_revision()),
            },
            domains,
            fields,
            geometry_actions,
            transformations,
            systems: vec![AlgebraicSystemNode {
                blocks,
                transformations: vec![
                    TransformationId::new(0),
                    TransformationId::new(1),
                    TransformationId::new(2),
                    TransformationId::new(3),
                ],
                scaling: SystemScaling::SymmetricCongruence(coupled.scaling().clone()),
                operator_properties: coupled.operator_properties(),
                scalar_type: execution.scalar_type(),
                partition: execution.vector_layout(),
            }],
            linear_solves: vec![LinearSolveNode {
                system: AlgebraicSystemId::new(0),
                plan: coupled.solver(),
                placement: PlacementRequirementId::new(0),
                schedule: coupled.schedule(),
            }],
            nonlinear_solves: vec![NonlinearSolveNode {
                residual_system: AlgebraicSystemId::new(0),
                linearization: LinearSolveId::new(0),
                plan: plan.nonlinear(),
            }],
            placements: vec![portable_placement(coupled.target())],
            root: SolveRoot::Nonlinear(NonlinearSolveId::new(0)),
        };
        graph.validate()?;
        Ok(graph)
    }
}

fn blocks_from_scaling(
    fields: &[FieldRepresentationNode],
    scaling: &SymmetricCongruenceScaling,
) -> Result<Vec<SystemBlock>, Diagnostic> {
    scaling
        .block_scales()
        .iter()
        .map(|entry| match entry.block() {
            AlgebraicBlock::Field(field) => field_reference(fields, field).map(SystemBlock::Field),
            AlgebraicBlock::ConstraintMultiplier { field } => {
                if fields.iter().any(|node| node.field == field) {
                    Ok(SystemBlock::ConstraintMultiplier(
                        AlgebraicConstraint::ZeroIntegral { field },
                    ))
                } else {
                    Err(invalid_realization(
                        "portable constraint multiplier refers to an unrepresented Field",
                    ))
                }
            }
        })
        .collect()
}

fn domain_reference(
    domains: &[DomainDiscretizationNode],
    domain: Id<kinds::Domain>,
) -> Result<DomainDiscretizationId, Diagnostic> {
    domains
        .binary_search_by_key(&domain.ulid(), |node| node.domain.ulid())
        .map(DomainDiscretizationId::new)
        .map_err(|_| {
            invalid_realization(
                "portable geometry action references an unrepresented Semantic Domain",
            )
        })
}

fn field_reference(
    fields: &[FieldRepresentationNode],
    field: Id<kinds::Field>,
) -> Result<FieldRepresentationId, Diagnostic> {
    fields
        .binary_search_by_key(&field.ulid(), |node| node.field.ulid())
        .map(FieldRepresentationId::new)
        .map_err(|_| {
            invalid_realization(
                "portable transformation references an unrepresented Semantic Field",
            )
        })
}

fn portable_placement(target: Target) -> PlacementRequirementNode {
    match target {
        Target::HostCpu { threads } => PlacementRequirementNode::HostWorkers {
            workers_per_partition: threads,
        },
        Target::CudaGpu { .. } => PlacementRequirementNode::CudaDevices {
            devices_per_partition: NonZeroUsize::MIN,
        },
    }
}

fn validate_geometry_actions(graph: &PortableRealizationGraph) -> Result<(), Diagnostic> {
    let mut domain_references = vec![0_usize; graph.geometry_actions.len()];
    for domain in &graph.domains {
        if let DomainConfiguration::CurrentAleGeometry { action } = domain.configuration {
            let Some(count) = domain_references.get_mut(action.index()) else {
                return Err(invalid_realization(
                    "portable current ALE geometry references an absent Geometry Action",
                ));
            };
            *count += 1;
        }
    }
    if domain_references.iter().any(|count| *count != 1) {
        return Err(invalid_realization(
            "every portable Geometry Action must drive exactly one current ALE Domain",
        ));
    }

    for (index, action) in graph.geometry_actions.iter().enumerate() {
        match *action {
            GeometryActionNode::P1HarmonicExtension {
                fluid_domain,
                solid_domain,
                driver,
                interface,
                duration,
                solver,
                ..
            } => {
                let (Some(fluid), Some(solid), Some(driver_field)) = (
                    graph.domain(fluid_domain),
                    graph.domain(solid_domain),
                    graph.field(driver),
                ) else {
                    return Err(invalid_realization(
                        "portable P1 harmonic Geometry Action references an absent Domain or driver Field",
                    ));
                };
                if fluid_domain == solid_domain || driver_field.domain != solid_domain {
                    return Err(invalid_realization(
                        "portable P1 harmonic Geometry Action requires distinct fluid/solid Domains and a solid driver",
                    ));
                }
                if !matches!(
                    fluid.configuration,
                    DomainConfiguration::CurrentAleGeometry { action }
                        if action == GeometryActionId::new(index)
                ) || !matches!(
                    solid.configuration,
                    DomainConfiguration::ReferenceConfiguration
                ) {
                    return Err(invalid_realization(
                        "portable ALE fluid and solid Domains require current and reference configurations respectively",
                    ));
                }
                if duration.value() <= 0.0 || !duration.value().is_finite() {
                    return Err(invalid_realization(
                        "portable Geometry Action duration must be finite and strictly positive",
                    ));
                }
                if !solver
                    .algorithm()
                    .accepts(LinearOperatorProperties::SymmetricPositiveDefinite)
                {
                    return Err(invalid_realization(
                        "portable P1 harmonic Geometry Action requires an SPD-admissible solver",
                    ));
                }
                if !graph.transformations.iter().any(|transformation| {
                    matches!(
                        transformation,
                        TransformationNode::ConformingTraceQuotient { connection, .. }
                            if *connection == interface
                    )
                }) {
                    return Err(invalid_realization(
                        "portable Geometry Action interface has no exact conforming trace quotient",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_system(
    graph: &PortableRealizationGraph,
    system: &AlgebraicSystemNode,
) -> Result<(), Diagnostic> {
    if system.blocks.is_empty() {
        return Err(invalid_realization(
            "portable algebraic system requires at least one block",
        ));
    }
    let mut field_blocks = BTreeSet::new();
    let mut constraints = BTreeSet::new();
    let mut actual_scales = Vec::with_capacity(system.blocks.len());
    for block in &system.blocks {
        match *block {
            SystemBlock::Field(id) => {
                let Some(field) = graph.field(id) else {
                    return Err(invalid_realization(
                        "portable algebraic block references an absent Field node",
                    ));
                };
                if !field_blocks.insert(field.field().ulid()) {
                    return Err(invalid_realization(
                        "portable algebraic system contains a duplicate Field block",
                    ));
                }
                actual_scales.push(AlgebraicBlock::Field(field.field()));
            }
            SystemBlock::ConstraintMultiplier(constraint) => {
                if !constraints.insert(constraint.field().ulid()) {
                    return Err(invalid_realization(
                        "portable algebraic system contains a duplicate constraint multiplier",
                    ));
                }
                if !graph
                    .fields
                    .iter()
                    .any(|field| field.field == constraint.field())
                {
                    return Err(invalid_realization(
                        "portable constraint multiplier refers to an absent Field node",
                    ));
                }
                actual_scales.push(AlgebraicBlock::ConstraintMultiplier {
                    field: constraint.field(),
                });
            }
        }
    }
    if let SystemScaling::SymmetricCongruence(scaling) = &system.scaling {
        let expected_scales = scaling
            .block_scales()
            .iter()
            .map(|entry| entry.block())
            .collect::<Vec<_>>();
        if actual_scales != expected_scales {
            return Err(invalid_realization(
                "portable algebraic blocks and congruence scales must have exact equal coverage and order",
            ));
        }
    }
    let mut seen_transformations = BTreeSet::new();
    for id in &system.transformations {
        if graph.transformation(*id).is_none() || !seen_transformations.insert(*id) {
            return Err(invalid_realization(
                "portable algebraic system has an absent or duplicate transformation",
            ));
        }
    }
    if seen_transformations.len() != graph.transformations.len() {
        return Err(invalid_realization(
            "portable Realization graph contains an unreachable transformation",
        ));
    }
    let mut represented = BTreeSet::new();
    let mut used_geometry_actions = BTreeSet::new();
    for block in &system.blocks {
        if let SystemBlock::Field(id) = *block {
            represented.insert(id);
        }
    }
    for transformation in &graph.transformations {
        match *transformation {
            TransformationNode::BackwardEulerDerivative { state, .. } => {
                if graph.field(state).is_none() {
                    return Err(invalid_realization(
                        "portable Backward Euler transformation references an absent Field node",
                    ));
                }
                represented.insert(state);
            }
            TransformationNode::EnergySkewConvection { velocity, .. } => {
                if graph.field(velocity).is_none() {
                    return Err(invalid_realization(
                        "portable energy-skew transformation references an absent Field node",
                    ));
                }
                represented.insert(velocity);
            }
            TransformationNode::CellCenteredConvection { state, .. } => {
                if graph.field(state).is_none() {
                    return Err(invalid_realization(
                        "portable cell-centered convection transformation references an absent Field node",
                    ));
                }
                represented.insert(state);
            }
            TransformationNode::OrthogonalTwoPointDiffusion { state, .. } => {
                if graph.field(state).is_none() {
                    return Err(invalid_realization(
                        "portable orthogonal two-point diffusion references an absent Field node",
                    ));
                }
                represented.insert(state);
            }
            TransformationNode::ImplicitCenteredMomentumConvection { velocity, .. } => {
                if graph.field(velocity).is_none() {
                    return Err(invalid_realization(
                        "portable centered momentum convection references an absent velocity Field node",
                    ));
                }
                represented.insert(velocity);
            }
            TransformationNode::CartesianCentralNewtonianTraction {
                velocity, pressure, ..
            }
            | TransformationNode::MomentumWeightedLinearExactCoupling {
                velocity, pressure, ..
            } => {
                if graph.field(velocity).is_none() || graph.field(pressure).is_none() {
                    return Err(invalid_realization(
                        "portable collocated fluid transformation references an absent velocity or pressure Field node",
                    ));
                }
                if velocity == pressure {
                    return Err(invalid_realization(
                        "portable collocated fluid transformation requires distinct velocity and pressure Field nodes",
                    ));
                }
                represented.insert(velocity);
                represented.insert(pressure);
            }
            TransformationNode::GclCompatibleAlePullback {
                relation,
                velocity,
                geometry,
            } => {
                let (Some(velocity_field), Some(action)) =
                    (graph.field(velocity), graph.geometry_action(geometry))
                else {
                    return Err(invalid_realization(
                        "portable GCL-compatible ALE pullback references an absent Field or Geometry Action",
                    ));
                };
                if !used_geometry_actions.insert(geometry) {
                    return Err(invalid_realization(
                        "a portable Geometry Action must feed exactly one GCL-compatible ALE pullback",
                    ));
                }
                let GeometryActionNode::P1HarmonicExtension {
                    fluid_domain,
                    duration,
                    ..
                } = *action;
                if velocity_field.domain != fluid_domain {
                    return Err(invalid_realization(
                        "portable GCL-compatible ALE velocity must belong to the action's fluid Domain",
                    ));
                }
                if !graph.transformations.iter().any(|candidate| {
                    matches!(
                        candidate,
                        TransformationNode::BackwardEulerDerivative {
                            relation: candidate_relation,
                            state,
                            duration: candidate_duration,
                        } if *candidate_relation == relation
                            && *state == velocity
                            && *candidate_duration == duration
                    )
                }) {
                    return Err(invalid_realization(
                        "portable GCL-compatible ALE pullback must share its Relation, velocity, and duration with Backward Euler",
                    ));
                }
                represented.insert(velocity);
            }
            TransformationNode::BackwardEulerElimination { state, rate, .. } => {
                let (Some(state_field), Some(rate_field)) = (graph.field(state), graph.field(rate))
                else {
                    return Err(invalid_realization(
                        "portable Backward Euler elimination references an absent Field node",
                    ));
                };
                if state == rate || state_field.domain != rate_field.domain {
                    return Err(invalid_realization(
                        "portable Backward Euler state and rate must be distinct Fields on one Domain",
                    ));
                }
                if represented.contains(&state) || !represented.contains(&rate) {
                    return Err(invalid_realization(
                        "portable Backward Euler elimination requires a non-algebraic state and algebraic rate",
                    ));
                }
                represented.insert(state);
                represented.insert(rate);
            }
            TransformationNode::ConformingTraceQuotient { endpoints, .. } => {
                let [Some(first), Some(second)] =
                    [graph.field(endpoints[0]), graph.field(endpoints[1])]
                else {
                    return Err(invalid_realization(
                        "portable trace quotient references an absent Field node",
                    ));
                };
                if first.domain == second.domain {
                    return Err(invalid_realization(
                        "portable conforming trace quotient must join distinct Domains",
                    ));
                }
                represented.extend(endpoints);
            }
        }
    }
    if used_geometry_actions.len() != graph.geometry_actions.len() {
        return Err(invalid_realization(
            "portable Realization graph contains an unreachable Geometry Action",
        ));
    }
    if represented.len() != graph.fields.len() {
        return Err(invalid_realization(
            "portable Realization graph contains an unreachable Field representation",
        ));
    }
    let represented_domains = graph
        .fields
        .iter()
        .map(|field| field.domain)
        .collect::<BTreeSet<_>>();
    if represented_domains.len() != graph.domains.len() {
        return Err(invalid_realization(
            "portable Realization graph contains an unreachable Domain discretization",
        ));
    }
    Ok(())
}

fn strictly_sorted_unique_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

#[cfg(test)]
#[path = "portable_graph/tests.rs"]
mod tests;
