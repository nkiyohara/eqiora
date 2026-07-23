use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId};
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, PreconditionerPolicy, ReductionPolicy,
    SolverCapabilities, SolverCapability, SolverPlan,
};

use super::*;
use crate::{
    AlgebraicBlockScale, BackwardEulerRelationStep, DiscretizationMethod, FieldSpaceBinding,
    FieldwiseRealizationPlan, FieldwiseRealizationRequirements, FieldwiseSpatialDiscretization,
    MeshArtifactReference, MeshKind, MeshPolicy, QuadraturePolicy, RealizationCapabilities,
    RealizationRequirements, RealizationRevision, SemanticRevision, SpatialDimensionSupport,
    TargetCapabilities, TransientFieldwiseRealizationPlan, TransientFieldwiseRealizationRequest,
    TransientFieldwiseRealizationRequirements, resolve_transient_fieldwise,
};

const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};
const GAUGE: DimExponents = DimExponents {
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const FUNCTIONAL: DimExponents = DimExponents {
    mass: 1,
    length: 1,
    time: -3,
    ..DimExponents::DIMENSIONLESS
};

#[test]
fn transient_projection_is_one_connected_typed_solve_dag() {
    let fixture = Fixture::new();
    let resolved = fixture.resolve();
    let graph = resolved.portable_graph().unwrap();

    assert_eq!(graph.lineage().model(), resolved.model());
    assert_eq!(
        graph.lineage().source(),
        ResolutionSource::Explicit(RealizationRevision::new(9))
    );
    assert_eq!(graph.domains().len(), 1);
    assert_eq!(graph.domains()[0].domain(), fixture.domain);
    assert_eq!(graph.fields().len(), 2);
    assert_eq!(graph.systems().len(), 1);
    assert_eq!(graph.linear_solves().len(), 1);
    assert_eq!(graph.nonlinear_solves().len(), 1);
    assert_eq!(
        graph.placements(),
        [PlacementRequirementNode::HostWorkers {
            workers_per_partition: NonZeroUsize::MIN,
        }]
    );

    let SolveRoot::Nonlinear(root) = graph.root() else {
        panic!("transient projection must have a nonlinear root");
    };
    let nonlinear = graph.nonlinear_solve(root).unwrap();
    let linear = graph.linear_solve(nonlinear.linearization()).unwrap();
    assert_eq!(linear.plan(), resolved.plan().fieldwise().solver());
    assert_eq!(linear.schedule(), ExecutionSchedule::Offline);
    assert_eq!(nonlinear.plan(), resolved.plan().nonlinear());

    let velocity = graph
        .fields()
        .iter()
        .position(|field| field.field() == fixture.velocity)
        .map(FieldRepresentationId::new)
        .unwrap();
    assert_eq!(
        graph.transformations(),
        [
            TransformationNode::BackwardEulerDerivative {
                relation: fixture.relation,
                state: velocity,
                duration: DynQuantity::new(0.01, TIME),
            },
            TransformationNode::EnergySkewConvection {
                relation: fixture.relation,
                velocity,
            },
        ]
    );
}

#[test]
fn malformed_or_disconnected_projection_fails_closed() {
    let fixture = Fixture::new();
    let graph = fixture.resolve().portable_graph().unwrap();

    let mut orphan_domain = graph.clone();
    orphan_domain.domains.push(DomainDiscretizationNode {
        domain: Id::new(),
        coordinates: CoordinateTreatment::Physical,
        configuration: DomainConfiguration::FixedGeometry,
        discretization: orphan_domain.domains[0].discretization,
    });
    orphan_domain
        .domains
        .sort_by_key(|domain| domain.domain.ulid());
    assert_eq!(
        orphan_domain.validate().unwrap_err().code(),
        codes::INVALID_REALIZATION
    );

    let mut missing_state = graph.clone();
    missing_state.transformations[0] = TransformationNode::BackwardEulerDerivative {
        relation: fixture.relation,
        state: FieldRepresentationId::new(usize::MAX),
        duration: DynQuantity::new(0.01, TIME),
    };
    assert_eq!(
        missing_state.validate().unwrap_err().code(),
        codes::INVALID_REALIZATION
    );

    let mut orphan_transformation = graph;
    orphan_transformation.systems[0].transformations.pop();
    assert_eq!(
        orphan_transformation.validate().unwrap_err().code(),
        codes::INVALID_REALIZATION
    );
}

struct Fixture {
    domain: Id<kinds::Domain>,
    velocity: Id<kinds::Field>,
    relation: Id<kinds::Relation>,
    request: TransientFieldwiseRealizationRequest,
    requirements: TransientFieldwiseRealizationRequirements,
}

impl Fixture {
    fn new() -> Self {
        let domain = Id::new();
        let velocity = Id::new();
        let pressure = Id::new();
        let relation = Id::new();
        let spatial = FieldwiseSpatialDiscretization::new(
            domain,
            scale(LENGTH),
            [
                FieldSpaceBinding::new(velocity, Space::simplex_p1_bubble()),
                FieldSpaceBinding::new(pressure, Space::continuous_lagrange(NonZeroU16::MIN)),
            ],
            [AlgebraicConstraint::ZeroIntegral { field: pressure }],
            Discretization::new(
                DiscretizationMethod::ContinuousGalerkin,
                MeshPolicy::ImportedSimplicial {
                    artifact: MeshArtifactReference::from_sha256([7; 32]),
                },
                QuadraturePolicy::TriangleDuffyGaussLegendre {
                    points_per_axis: NonZeroUsize::new(5).unwrap(),
                },
            ),
        )
        .unwrap();
        let scaling = SymmetricCongruenceScaling::new(
            [
                AlgebraicBlockScale::new(AlgebraicBlock::Field(velocity), scale(VELOCITY)),
                AlgebraicBlockScale::new(AlgebraicBlock::Field(pressure), scale(PRESSURE)),
                AlgebraicBlockScale::new(
                    AlgebraicBlock::ConstraintMultiplier { field: pressure },
                    scale(GAUGE),
                ),
            ],
            scale(FUNCTIONAL),
        )
        .unwrap();
        let solver = SolverPlan::new(
            LinearSolver::BiConjugateGradientStabilized,
            1.0e-11,
            1.0e-13,
            NonZeroUsize::new(2_000).unwrap(),
        )
        .unwrap()
        .with_reduction(ReductionPolicy::Fast);
        let fieldwise = FieldwiseRealizationPlan::new(
            spatial,
            scaling,
            LinearOperatorProperties::General,
            solver,
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
            ExecutionSchedule::Offline,
        )
        .unwrap();
        let plan = TransientFieldwiseRealizationPlan::new(
            fieldwise,
            BackwardEulerRelationStep::new(relation, velocity, DynQuantity::new(0.01, TIME))
                .unwrap(),
            crate::EnergySkewConvection::new(relation, velocity),
            NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(12).unwrap(), 12).unwrap(),
        )
        .unwrap();
        let execution = RealizationRequirements::new(
            NonZeroUsize::new(2).unwrap(),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        );
        let requirements = TransientFieldwiseRealizationRequirements::new(
            FieldwiseRealizationRequirements::new(domain, [velocity, pressure], execution).unwrap(),
            relation,
            velocity,
        )
        .unwrap();
        let request = TransientFieldwiseRealizationRequest::explicit(
            OntologyId::new(),
            SemanticRevision::new(4),
            RealizationRevision::new(9),
            plan,
        );
        Self {
            domain,
            velocity,
            relation,
            request,
            requirements,
        }
    }

    fn resolve(&self) -> ResolvedTransientFieldwiseRealization {
        resolve_transient_fieldwise(&self.request, self.requirements.clone(), &capabilities())
            .unwrap()
    }
}

fn capabilities() -> RealizationCapabilities {
    let solver = SolverCapabilities::exact([SolverCapability {
        algorithm: LinearSolver::BiConjugateGradientStabilized,
        operator_properties: LinearOperatorProperties::General,
        preconditioner: PreconditionerPolicy::Identity,
        reduction: ReductionPolicy::Fast,
        scalar_type: ScalarType::F64,
    }])
    .unwrap();
    RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).unwrap()),
        )],
        [VectorLayoutKind::Replicated],
        solver,
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
    .unwrap()
}

fn scale(dimension: DimExponents) -> PositivePhysicalScale {
    PositivePhysicalScale::new(DynQuantity::new(1.0, dimension)).unwrap()
}
