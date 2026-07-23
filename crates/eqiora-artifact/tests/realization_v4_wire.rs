use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_artifact::{
    CanonicalModelArtifact, CanonicalRealizationArtifact, DecoderLimits, LayoutArtifacts,
    ModelEnvelopeV4, RealizationEnvelopeV4, RealizationEnvelopeV5, SimplicialMeshEnvelopeV1,
};
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id};
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};
use eqiora_realization::{
    AleGeometryQualityGate, AlgebraicBlock, AlgebraicBlockScale, AlgebraicConstraint,
    BackwardEulerRelationStep, BackwardEulerStateBinding, BackwardEulerStatePair,
    BackwardEulerStep, ConformingTraceQuotient, CoupledFieldwiseRealizationPlan,
    CoupledFieldwiseRealizationRequirements, CoupledFieldwiseSpatialDiscretization, Discretization,
    DiscretizationMethod, DomainFieldDiscretization, DomainFieldInventory, ExecutionSchedule,
    FieldSpaceBinding, FixedTopologyAleCoupledRealizationPlan,
    FixedTopologyAleCoupledRealizationRequest, FixedTopologyAleCoupledRealizationRequirements,
    GclCompatibleAlePullback, MeshArtifactReference, MeshKind, MeshPolicy, NonlinearSolvePlan,
    P1HarmonicMeshMotion, QuadraturePolicy, RealizationCapabilities, RealizationRequirements,
    RealizationRevision, ResolvedFixedTopologyAleCoupledRealization, ScalarType,
    SolverCapabilities, Space, SpatialDimensionSupport, SymmetricCongruenceScaling, Target,
    TargetCapabilities, TraceFieldEndpoint, VectorLayoutKind, resolve_fixed_topology_ale_coupled,
};
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, PreconditionerPolicy, ReductionPolicy,
    SolverCapability, SolverPlan,
};
use sha2::{Digest, Sha256};
use ulid::Ulid;

const MODEL: &[u8] =
    include_bytes!("../../../verify/fsi/fixed-reference-cuda-solve-2d/artifacts/model.json");
type JsonMutation = (&'static str, Box<dyn Fn(&mut serde_json::Value)>);

#[test]
fn realization_v4_round_trips_the_complete_ale_graph_and_typed_identity() {
    let fixture = Fixture::new();
    let envelope = fixture.envelope();
    let bytes = envelope.canonical_json().unwrap();
    let decoded = RealizationEnvelopeV4::from_json(&bytes, DecoderLimits::default()).unwrap();

    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
    assert_eq!(
        decoded.requirements().unwrap(),
        *fixture.resolved.requirements()
    );
    assert_eq!(decoded.plan().unwrap(), *fixture.resolved.plan());
    decoded.validate_model_artifact(&fixture.model).unwrap();
    decoded.validate_mesh_artifact(&fixture.mesh).unwrap();
    assert_eq!(
        decoded.mesh_artifact().unwrap(),
        fixture.mesh.digest().unwrap()
    );

    let reference = decoded.artifact_reference().unwrap();
    assert_eq!(reference.artifact(), &decoded.digest().unwrap());
    assert_eq!(reference.model_artifact(), &fixture.model.digest().unwrap());
    assert_eq!(reference.semantic_revision(), decoded.semantic_revision());

    let text = String::from_utf8(bytes).unwrap();
    for required in [
        "eqiora.realization-envelope/v4",
        "current-ale-geometry",
        "reference-configuration",
        "p1-harmonic-extension",
        "backward-euler-derivative",
        "backward-euler-elimination",
        "conforming-trace-quotient",
        "gcl-compatible-ale-pullback",
        "general",
        "nonlinear",
    ] {
        assert!(
            text.contains(required),
            "missing exact v4 graph role {required}"
        );
    }
}

#[test]
fn realization_v4_golden_digest_and_roundtrip_are_frozen() {
    const EXPECTED_SHA256: &str =
        "ba9efbdbca265dea0fdf9b1476ea2cae876eb2c97b4ac6f332f3755d866b5d9e";
    let actual = Fixture::new().envelope().canonical_json().unwrap();
    let sha256 = Sha256::digest(&actual)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(sha256, EXPECTED_SHA256);
    let decoded = RealizationEnvelopeV4::from_json(&actual, DecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), actual);
}

#[test]
fn realization_v5_is_domain_separated_and_makes_legacy_triangle_dimension_explicit() {
    let fixture = Fixture::new();
    let v4 = fixture.envelope().canonical_json().unwrap();
    let envelope = fixture.envelope_v5();
    let v5 = envelope.canonical_json().unwrap();
    let decoded = RealizationEnvelopeV5::from_json(&v5, DecoderLimits::default()).unwrap();

    assert_eq!(decoded.canonical_json().unwrap(), v5);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
    assert_eq!(decoded.plan().unwrap(), *fixture.resolved.plan());
    decoded.validate_model_artifact(&fixture.model).unwrap();
    decoded.validate_mesh_artifact(&fixture.mesh).unwrap();
    assert_ne!(v4, v5);
    assert_ne!(
        fixture.envelope().digest().unwrap(),
        envelope.digest().unwrap()
    );
    assert!(RealizationEnvelopeV4::from_json(&v5, DecoderLimits::default()).is_err());
    assert!(RealizationEnvelopeV5::from_json(&v4, DecoderLimits::default()).is_err());

    let value: serde_json::Value = serde_json::from_slice(&v5).unwrap();
    let quadrature = &value["plan"]["coupled"]["spatial"]["discretization"]["quadrature"];
    assert_eq!(
        quadrature,
        &serde_json::json!({
            "kind": "triangle-duffy-gauss-legendre",
            "spatial_dimension": 2,
            "points_per_axis": 5,
        })
    );
}

#[test]
fn realization_v5_round_trips_dimension_explicit_tetrahedral_quadrature() {
    let fixture = Fixture::tetrahedral();
    let envelope = fixture.envelope_v5();
    let bytes = envelope.canonical_json().unwrap();
    let decoded = RealizationEnvelopeV5::from_json(&bytes, DecoderLimits::default()).unwrap();

    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.plan().unwrap(), *fixture.resolved.plan());
    assert_eq!(
        decoded.requirements().unwrap(),
        *fixture.resolved.requirements()
    );
    decoded.validate_mesh_artifact(&fixture.mesh).unwrap();

    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let quadrature = &value["plan"]["coupled"]["spatial"]["discretization"]["quadrature"];
    assert_eq!(
        quadrature,
        &serde_json::json!({
            "kind": "simplex-duffy-gauss-legendre",
            "spatial_dimension": 3,
            "points_per_axis": 7,
        })
    );
    assert!(
        RealizationEnvelopeV4::from_resolved(
            &fixture.model,
            &fixture.resolved,
            LayoutArtifacts::Replicated,
        )
        .is_err(),
        "the frozen V4 encoder must reject the V5-only quadrature policy",
    );
}

#[test]
fn ale_realization_rejects_one_bit_of_reference_mesh_quality_gate_drift() {
    let fixture = Fixture::new();
    let drifted_gate = f64::from_bits(0.05_f64.to_bits() + 1);

    let mut v4: serde_json::Value =
        serde_json::from_slice(&fixture.envelope().canonical_json().unwrap()).unwrap();
    v4["plan"]["geometry_action"]["minimum_mean_ratio"] = serde_json::Value::from(drifted_gate);
    let v4 = RealizationEnvelopeV4::from_json(
        &serde_json::to_vec(&v4).unwrap(),
        DecoderLimits::default(),
    )
    .unwrap();
    assert!(
        v4.validate_mesh_artifact(&fixture.mesh).is_err(),
        "V4 mesh validation must compare the complete binary64 quality gate",
    );

    let mut v5: serde_json::Value =
        serde_json::from_slice(&fixture.envelope_v5().canonical_json().unwrap()).unwrap();
    v5["plan"]["geometry_action"]["minimum_mean_ratio"] = serde_json::Value::from(drifted_gate);
    let v5 = RealizationEnvelopeV5::from_json(
        &serde_json::to_vec(&v5).unwrap(),
        DecoderLimits::default(),
    )
    .unwrap();
    assert!(
        v5.validate_mesh_artifact(&fixture.mesh).is_err(),
        "V5 mesh validation must compare the complete binary64 quality gate",
    );
}

#[test]
fn realization_v5_rejects_quadrature_dimension_drift_and_unknown_fields() {
    let bytes = Fixture::tetrahedral()
        .envelope_v5()
        .canonical_json()
        .unwrap();

    let mut drift: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    drift["plan"]["coupled"]["spatial"]["discretization"]["quadrature"]["spatial_dimension"] =
        serde_json::json!(2);
    assert!(
        RealizationEnvelopeV5::from_json(
            &serde_json::to_vec(&drift).unwrap(),
            DecoderLimits::default(),
        )
        .is_err(),
        "quadrature policy dimension cannot drift from requirements",
    );

    let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    unknown["plan"]["coupled"]["spatial"]["discretization"]["quadrature"]["reference_cell"] =
        serde_json::json!("tetrahedron");
    assert!(
        RealizationEnvelopeV5::from_json(
            &serde_json::to_vec(&unknown).unwrap(),
            DecoderLimits::default(),
        )
        .is_err(),
        "quadrature wire variants must deny unknown fields",
    );
}

#[test]
fn realization_v4_rejects_ale_role_transformation_and_system_drift() {
    let bytes = Fixture::new().envelope().canonical_json().unwrap();
    let mutations: Vec<JsonMutation> = vec![
        (
            "driver",
            Box::new(|value| {
                value["plan"]["geometry_action"]["driver_field_ulid"] =
                    serde_json::json!(fixed_ulid(90).to_string());
            }),
        ),
        (
            "action",
            Box::new(|value| {
                value["plan"]["domain_configurations"][0]["configuration"]["geometry_action"] =
                    serde_json::json!(1);
            }),
        ),
        (
            "gcl",
            Box::new(|value| {
                value["plan"]["transformations"][3]["relation_ulid"] =
                    serde_json::json!(fixed_ulid(91).to_string());
            }),
        ),
        (
            "duration",
            Box::new(|value| {
                value["plan"]["geometry_action"]["duration"]["coherent_si_value"] =
                    serde_json::json!(0.2);
            }),
        ),
        (
            "configuration",
            Box::new(|value| {
                value["plan"]["domain_configurations"]
                    .as_array_mut()
                    .unwrap()
                    .reverse();
            }),
        ),
        (
            "solver",
            Box::new(|value| {
                value["plan"]["linear_solve"]["solver"]["maximum_iterations"] =
                    serde_json::json!(1999);
            }),
        ),
        (
            "operator",
            Box::new(|value| {
                value["plan"]["system"]["operator_properties"] =
                    serde_json::json!("symmetric-indefinite");
            }),
        ),
    ];
    for (label, mutate) in mutations {
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        mutate(&mut value);
        assert!(
            RealizationEnvelopeV4::from_json(
                &serde_json::to_vec(&value).unwrap(),
                DecoderLimits::default(),
            )
            .is_err(),
            "{label} drift must fail closed",
        );
    }
}

#[test]
fn realization_v4_rejects_unknown_fields_layout_drift_and_resource_excess() {
    let fixture = Fixture::new();
    let bytes = fixture.envelope().canonical_json().unwrap();

    let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    unknown["plan"]["geometry_action"]["mesh_velocity"] = serde_json::json!([0.0, 0.0]);
    assert!(
        RealizationEnvelopeV4::from_json(
            &serde_json::to_vec(&unknown).unwrap(),
            DecoderLimits::default(),
        )
        .is_err(),
        "the geometry action cannot accept an independent mesh velocity",
    );

    let mut layout: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    layout["layout_artifacts"] = serde_json::json!({
        "kind": "distributed",
        "layout_sha256": "11".repeat(32),
        "partition_sha256": "22".repeat(32)
    });
    assert!(
        RealizationEnvelopeV4::from_json(
            &serde_json::to_vec(&layout).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );

    for limits in [
        DecoderLimits {
            max_realization_fields: 1,
            ..DecoderLimits::default()
        },
        DecoderLimits {
            max_realization_constraints: 3,
            ..DecoderLimits::default()
        },
        DecoderLimits {
            max_realization_blocks: 1,
            ..DecoderLimits::default()
        },
    ] {
        assert!(RealizationEnvelopeV4::from_json(&bytes, limits).is_err());
    }
}

struct Fixture {
    model: ModelEnvelopeV4,
    mesh: SimplicialMeshEnvelopeV1,
    resolved: ResolvedFixedTopologyAleCoupledRealization,
}

impl Fixture {
    fn new() -> Self {
        Self::with_dimension(2)
    }

    fn tetrahedral() -> Self {
        Self::with_dimension(3)
    }

    fn with_dimension(dimension: usize) -> Self {
        let model = ModelEnvelopeV4::from_json(MODEL, DecoderLimits::default()).unwrap();
        let (points, cells, quadrature) = match dimension {
            2 => (
                vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]],
                vec![vec![0, 1, 2]],
                QuadraturePolicy::TriangleDuffyGaussLegendre {
                    points_per_axis: NonZeroUsize::new(5).unwrap(),
                },
            ),
            3 => (
                vec![
                    vec![0.0, 0.0, 0.0],
                    vec![1.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0],
                    vec![0.0, 0.0, 1.0],
                ],
                vec![vec![0, 1, 2, 3]],
                QuadraturePolicy::SimplexDuffyGaussLegendre {
                    spatial_dimension: NonZeroUsize::new(3).unwrap(),
                    points_per_axis: NonZeroUsize::new(7).unwrap(),
                },
            ),
            _ => unreachable!("fixture admits only triangle and tetrahedron dimensions"),
        };
        let mesh = SimplicialMeshEnvelopeV1::from_mesh(
            &SimplicialMesh::new(
                dimension,
                points,
                cells,
                MeshQualityGate::new(0.05).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let ids = Ids::new();
        let plan = ids.plan(&mesh, quadrature);
        let model_reference = model.artifact_reference().unwrap();
        let request = FixedTopologyAleCoupledRealizationRequest::explicit(
            model_reference.model(),
            model_reference.semantic_revision(),
            RealizationRevision::new(6),
            plan,
        );
        let resolved = resolve_fixed_topology_ale_coupled(
            &request,
            ids.requirements(dimension),
            &capabilities(dimension),
        )
        .unwrap();
        Self {
            model,
            mesh,
            resolved,
        }
    }

    fn envelope(&self) -> RealizationEnvelopeV4 {
        RealizationEnvelopeV4::from_resolved(
            &self.model,
            &self.resolved,
            LayoutArtifacts::Replicated,
        )
        .unwrap()
    }

    fn envelope_v5(&self) -> RealizationEnvelopeV5 {
        RealizationEnvelopeV5::from_resolved(
            &self.model,
            &self.resolved,
            LayoutArtifacts::Replicated,
        )
        .unwrap()
    }
}

#[derive(Clone, Copy)]
struct Ids {
    fluid_domain: Id<kinds::Domain>,
    solid_domain: Id<kinds::Domain>,
    fluid_velocity: Id<kinds::Field>,
    pressure: Id<kinds::Field>,
    solid_velocity: Id<kinds::Field>,
    displacement: Id<kinds::Field>,
    connection: Id<kinds::Connection>,
    fluid_relation: Id<kinds::Relation>,
    solid_relation: Id<kinds::Relation>,
}

impl Ids {
    fn new() -> Self {
        Self {
            fluid_domain: parsed_id("1XQ46C76NKJ4HKBT3AYHZK7248"),
            solid_domain: parsed_id("36BFQQER8GMSMJC4E9CMC40GX1"),
            fluid_velocity: parsed_id("5KZKW30PAM3D5XVG2RXCSZ89TX"),
            pressure: parsed_id("2YSJB8SQ3YEQCJ66YYJW3CF38X"),
            solid_velocity: parsed_id("06JRV1N1F26VSVZFFEKAG12S5J"),
            displacement: parsed_id("6Z27MZJEAZ8GT73BXWN5THG8YW"),
            connection: parsed_id("655GJQQW1FFC2PWC1MEMVW0EFZ"),
            fluid_relation: parsed_id("66QS6PD17TSR2KM23HXEFZ0J7Q"),
            solid_relation: parsed_id("4S2893DG9Y358XB61VRX7CZYVV"),
        }
    }

    fn trace(self) -> ConformingTraceQuotient {
        ConformingTraceQuotient::new(
            self.connection,
            TraceFieldEndpoint::new(self.fluid_domain, self.fluid_velocity),
            TraceFieldEndpoint::new(self.solid_domain, self.solid_velocity),
        )
        .unwrap()
    }

    fn state_pair(self) -> BackwardEulerStatePair {
        BackwardEulerStatePair::new(self.displacement, self.solid_velocity).unwrap()
    }

    fn plan(
        self,
        mesh: &SimplicialMeshEnvelopeV1,
        quadrature: QuadraturePolicy,
    ) -> FixedTopologyAleCoupledRealizationPlan {
        let spatial = CoupledFieldwiseSpatialDiscretization::new(
            scale(length_dimension()),
            [
                DomainFieldDiscretization::new(
                    self.fluid_domain,
                    [
                        FieldSpaceBinding::new(self.fluid_velocity, Space::simplex_p1_bubble()),
                        FieldSpaceBinding::new(
                            self.pressure,
                            Space::continuous_lagrange(NonZeroU16::MIN),
                        ),
                    ],
                    [AlgebraicConstraint::ZeroIntegral {
                        field: self.pressure,
                    }],
                )
                .unwrap(),
                DomainFieldDiscretization::new(
                    self.solid_domain,
                    [FieldSpaceBinding::new(
                        self.solid_velocity,
                        Space::continuous_lagrange(NonZeroU16::MIN),
                    )],
                    [],
                )
                .unwrap(),
            ],
            self.trace(),
            Discretization::new(
                DiscretizationMethod::ContinuousGalerkin,
                MeshPolicy::ImportedSimplicial {
                    artifact: MeshArtifactReference::from_sha256(
                        mesh.digest().unwrap().sha256_bytes(),
                    ),
                },
                quadrature,
            ),
        )
        .unwrap();
        let duration = DynQuantity::new(0.1, time_dimension());
        let coupled = CoupledFieldwiseRealizationPlan::new(
            spatial,
            BackwardEulerStep::new(
                duration,
                BackwardEulerStateBinding::new(
                    self.state_pair(),
                    Space::continuous_lagrange(NonZeroU16::MIN),
                    scale(length_dimension()),
                ),
            )
            .unwrap(),
            self.scaling(),
            LinearOperatorProperties::General,
            solver(LinearSolver::BiConjugateGradientStabilized),
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
            ExecutionSchedule::Offline,
        )
        .unwrap();
        FixedTopologyAleCoupledRealizationPlan::new(
            coupled,
            BackwardEulerRelationStep::new(self.fluid_relation, self.fluid_velocity, duration)
                .unwrap(),
            self.solid_relation,
            P1HarmonicMeshMotion::new(
                self.fluid_domain,
                self.solid_domain,
                self.displacement,
                self.connection,
                AleGeometryQualityGate::new(0.05).unwrap(),
                solver(LinearSolver::ConjugateGradient),
            )
            .unwrap(),
            GclCompatibleAlePullback::new(self.fluid_relation, self.fluid_velocity),
            NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(12).unwrap(), 12).unwrap(),
        )
        .unwrap()
    }

    fn scaling(self) -> SymmetricCongruenceScaling {
        SymmetricCongruenceScaling::new(
            [
                AlgebraicBlockScale::new(
                    AlgebraicBlock::Field(self.fluid_velocity),
                    scale(velocity_dimension()),
                ),
                AlgebraicBlockScale::new(
                    AlgebraicBlock::Field(self.pressure),
                    scale(pressure_dimension()),
                ),
                AlgebraicBlockScale::new(
                    AlgebraicBlock::Field(self.solid_velocity),
                    scale(velocity_dimension()),
                ),
                AlgebraicBlockScale::new(
                    AlgebraicBlock::ConstraintMultiplier {
                        field: self.pressure,
                    },
                    scale(gauge_dimension()),
                ),
            ],
            scale(functional_dimension()),
        )
        .unwrap()
    }

    fn requirements(
        self,
        spatial_dimension: usize,
    ) -> FixedTopologyAleCoupledRealizationRequirements {
        FixedTopologyAleCoupledRealizationRequirements::new(
            CoupledFieldwiseRealizationRequirements::new(
                [
                    DomainFieldInventory::new(
                        self.fluid_domain,
                        [self.fluid_velocity, self.pressure],
                    )
                    .unwrap(),
                    DomainFieldInventory::new(
                        self.solid_domain,
                        [self.solid_velocity, self.displacement],
                    )
                    .unwrap(),
                ],
                self.trace(),
                self.state_pair(),
                RealizationRequirements::new(
                    NonZeroUsize::new(spatial_dimension).unwrap(),
                    ScalarType::F64,
                    VectorLayoutKind::Replicated,
                ),
            )
            .unwrap(),
            self.fluid_domain,
            self.solid_domain,
            self.fluid_relation,
            self.solid_relation,
            self.fluid_velocity,
            self.displacement,
        )
        .unwrap()
    }
}

fn capabilities(spatial_dimension: usize) -> RealizationCapabilities {
    RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(NonZeroUsize::new(spatial_dimension).unwrap()),
        )],
        [VectorLayoutKind::Replicated],
        SolverCapabilities::exact([
            SolverCapability {
                algorithm: LinearSolver::BiConjugateGradientStabilized,
                operator_properties: LinearOperatorProperties::General,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Reproducible,
                scalar_type: ScalarType::F64,
            },
            SolverCapability {
                algorithm: LinearSolver::ConjugateGradient,
                operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Reproducible,
                scalar_type: ScalarType::F64,
            },
        ])
        .unwrap(),
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
    .unwrap()
}

fn solver(algorithm: LinearSolver) -> SolverPlan {
    SolverPlan::new(
        algorithm,
        1.0e-11,
        1.0e-13,
        NonZeroUsize::new(2_000).unwrap(),
    )
    .unwrap()
}

fn fixed_ulid(byte: u8) -> Ulid {
    Ulid::from_bytes([byte; 16])
}

fn parsed_id<E: eqiora_core::Entity>(value: &str) -> Id<E> {
    Id::from_ulid(value.parse().unwrap())
}

fn scale(dimension: DimExponents) -> eqiora_realization::PositivePhysicalScale {
    eqiora_realization::PositivePhysicalScale::new(DynQuantity::new(1.0, dimension)).unwrap()
}

const fn length_dimension() -> DimExponents {
    DimExponents {
        length: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

const fn time_dimension() -> DimExponents {
    DimExponents {
        time: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

const fn velocity_dimension() -> DimExponents {
    DimExponents {
        length: 1,
        time: -1,
        ..DimExponents::DIMENSIONLESS
    }
}

const fn pressure_dimension() -> DimExponents {
    DimExponents {
        mass: 1,
        length: -1,
        time: -2,
        ..DimExponents::DIMENSIONLESS
    }
}

const fn gauge_dimension() -> DimExponents {
    DimExponents {
        time: -1,
        ..DimExponents::DIMENSIONLESS
    }
}

const fn functional_dimension() -> DimExponents {
    DimExponents {
        mass: 1,
        length: 1,
        time: -3,
        ..DimExponents::DIMENSIONLESS
    }
}
