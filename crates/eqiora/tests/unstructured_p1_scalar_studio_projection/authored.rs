use std::num::{NonZeroU16, NonZeroUsize};

use eqiora::api::UnstructuredP1ScalarFieldProjection2d;
use eqiora::artifact::{
    AcceptedCircularHoleChordalRealizationV1, DiscreteFieldEnvelopeV1, ExecutionProvenanceV1,
    ExecutionTopologyV1, FieldSnapshotEnvelopeV1, LayoutArtifacts, ModelEnvelope,
    RealizationEnvelopeV2, RunManifestV2, SimplicialMeshEnvelopeV1,
};
use eqiora::geometry::{CanonicalGeometryV1, FACE_DIMENSION, NamedEntitySet};
use eqiora::graph::{GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora::kernel::{DomainDef, DomainKind, KernelNode};
use eqiora::meshing::{
    DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape, MeshQualityGate,
};
use eqiora::ontology::ModelView;
use eqiora::realization::{
    AlgebraicBlock, AlgebraicBlockScale, Discretization, DiscretizationMethod, ExecutionSchedule,
    FieldSpaceBinding, FieldwiseRealizationPlan, FieldwiseRealizationRequest,
    FieldwiseRealizationRequirements, FieldwiseSpatialDiscretization, MeshPolicy,
    PositivePhysicalScale, QuadraturePolicy, RealizationCapabilities, RealizationRequirements,
    RealizationRevision, SemanticRevision, Space, SymmetricCongruenceScaling, Target,
    VectorLayoutKind, resolve_fieldwise,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{
    LinearOperatorProperties, LinearSolver, ReductionPolicy, ScalarType, SolverPlan,
};
use eqiora::{DimExponents, DynQuantity};

const SOURCE: &str = r#"
model Main {
  domain body = box(0, 2.2, 0, 0.41);
  representation space = continuum;
  field pressure on body as space: kg / (m * s ^ 2) = 0;
  relation anchor continuous on body { pressure = 0; }
}
"#;

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

struct AcceptedAuthoredField {
    model: ModelEnvelope,
    realization: RealizationEnvelopeV2,
    accepted: AcceptedCircularHoleChordalRealizationV1,
    run: RunManifestV2,
    snapshot: FieldSnapshotEnvelopeV1,
    block: DiscreteFieldEnvelopeV1,
}

impl AcceptedAuthoredField {
    fn inputs(&self) -> AuthoredProjectionInputs<'_> {
        AuthoredProjectionInputs {
            model: &self.model,
            realization: &self.realization,
            accepted: &self.accepted,
            run: &self.run,
            snapshot: &self.snapshot,
            block: &self.block,
        }
    }

    fn project(&self) -> Result<UnstructuredP1ScalarFieldProjection2d, eqiora::Diagnostic> {
        self.inputs().project()
    }
}

#[derive(Clone, Copy)]
struct AuthoredProjectionInputs<'a> {
    model: &'a ModelEnvelope,
    realization: &'a RealizationEnvelopeV2,
    accepted: &'a AcceptedCircularHoleChordalRealizationV1,
    run: &'a RunManifestV2,
    snapshot: &'a FieldSnapshotEnvelopeV1,
    block: &'a DiscreteFieldEnvelopeV1,
}

impl AuthoredProjectionInputs<'_> {
    fn project(self) -> Result<UnstructuredP1ScalarFieldProjection2d, eqiora::Diagnostic> {
        UnstructuredP1ScalarFieldProjection2d::from_authored_fieldwise_snapshot(
            self.model,
            self.realization,
            self.accepted,
            self.run,
            self.snapshot,
            self.block,
        )
    }
}

#[test]
fn exact_circle_authored_fieldwise_snapshot_projects_through_the_existing_value() {
    let accepted = accepted_authored_field([0.2, 0.2]);
    let projection = accepted
        .project()
        .expect("accepted authored P1 snapshot projects");

    assert_eq!(
        projection.model_artifact(),
        &accepted.model.digest().unwrap()
    );
    assert_eq!(
        projection.realization_artifact(),
        &accepted.realization.digest().unwrap()
    );
    assert_eq!(projection.run_artifact(), &accepted.run.digest().unwrap());
    assert_eq!(
        projection.snapshot_artifact(),
        &accepted.snapshot.digest().unwrap()
    );
    assert_eq!(
        projection.mesh_artifact(),
        &accepted.accepted.mesh().digest().unwrap()
    );
    assert_eq!(
        accepted.snapshot.geometry_artifact(),
        accepted.accepted.realized_geometry().digest().unwrap()
    );
    assert_eq!(
        projection.vertices_m().len(),
        accepted.accepted.mesh().mesh().vertices().len()
    );
    assert_eq!(
        projection.triangles().len(),
        accepted.accepted.mesh().mesh().cells().len()
    );
    assert_eq!(projection.values(), accepted.block.values());
}

#[test]
fn foreign_and_same_named_authored_resources_reject_before_array_publication() {
    let accepted = accepted_authored_field([0.2, 0.2]);
    let foreign = accepted_authored_field([0.3, 0.2]);

    assert!(
        FieldSnapshotEnvelopeV1::new_authored_fieldwise(
            &accepted.model,
            &accepted.realization,
            &accepted.accepted,
            eqiora::Id::new(),
            std::slice::from_ref(&accepted.block),
        )
        .is_err(),
        "an unrepresented selected Field must fail before a snapshot exists"
    );

    let accepted_inputs = accepted.inputs();
    let rejects = |name: &str, inputs: AuthoredProjectionInputs<'_>| {
        assert!(
            inputs.project().is_err(),
            "{name} must reject before array publication"
        );
    };
    rejects(
        "foreign Model",
        AuthoredProjectionInputs {
            model: &foreign.model,
            ..accepted_inputs
        },
    );
    rejects(
        "foreign Realization",
        AuthoredProjectionInputs {
            realization: &foreign.realization,
            ..accepted_inputs
        },
    );
    rejects(
        "foreign accepted source and resources",
        AuthoredProjectionInputs {
            accepted: &foreign.accepted,
            ..accepted_inputs
        },
    );
    rejects(
        "foreign Run",
        AuthoredProjectionInputs {
            run: &foreign.run,
            ..accepted_inputs
        },
    );
    rejects(
        "foreign snapshot",
        AuthoredProjectionInputs {
            snapshot: &foreign.snapshot,
            ..accepted_inputs
        },
    );
    rejects(
        "foreign block",
        AuthoredProjectionInputs {
            block: &foreign.block,
            ..accepted_inputs
        },
    );

    let outputless_run =
        RunManifestV2::new(&accepted.realization, accepted.run.execution()).unwrap();
    rejects(
        "outputless Run",
        AuthoredProjectionInputs {
            run: &outputless_run,
            ..accepted_inputs
        },
    );
}

fn accepted_authored_field(center: [f64; 2]) -> AcceptedAuthoredField {
    let source = exact_source(center);
    let program = geometry_program(&source);
    let model = ModelEnvelope::from_program(&program).expect("current Model");
    let accepted = AcceptedCircularHoleChordalRealizationV1::from_reference(
        &source,
        2.0e-3,
        16,
        MeshQualityGate::new(1.0e-5).unwrap(),
    )
    .expect("bounded chordal owner");
    let mesh = accepted.mesh().clone();
    let domain = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Domain(definition)
                if matches!(definition.kind(), DomainKind::GeometryRegion { .. }) =>
            {
                Some(definition.id())
            }
            _ => None,
        })
        .expect("one GeometryRegion");
    let field = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Field(definition) => Some(definition.id()),
            _ => None,
        })
        .expect("one scalar Field");
    let plan = fieldwise_plan(domain, field, &mesh);
    let requirements = FieldwiseRealizationRequirements::new(
        domain,
        [field],
        RealizationRequirements::new(
            NonZeroUsize::new(2).unwrap(),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
    )
    .unwrap();
    let resolved = resolve_fieldwise(
        &FieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(224),
            plan,
        ),
        requirements,
        &RealizationCapabilities::symmetric_mixed_simplicial_2d_reference(),
    )
    .expect("P1 field-wise capability");
    let realization =
        RealizationEnvelopeV2::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)
            .expect("field-wise Realization v2");
    let values = mesh
        .mesh()
        .vertices()
        .iter()
        .map(|vertex| vertex[0] + vertex[1])
        .collect();
    let payload = DiscreteFieldPayload::new(
        mesh.mesh(),
        DiscreteFieldAssociation::Vertex,
        DiscreteFieldShape::Scalar,
        values,
    )
    .unwrap();
    let block = DiscreteFieldEnvelopeV1::from_payload(&mesh, &payload).unwrap();
    let snapshot = FieldSnapshotEnvelopeV1::new_authored_fieldwise(
        &model,
        &realization,
        &accepted,
        field,
        std::slice::from_ref(&block),
    )
    .expect("authored P1 snapshot");
    let run = RunManifestV2::new(
        &realization,
        ExecutionProvenanceV1::new(
            "eqiora.host.serial",
            env!("CARGO_PKG_VERSION"),
            "eqiora.reference",
            env!("CARGO_PKG_VERSION"),
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            },
            ReductionPolicy::Reproducible,
        )
        .unwrap(),
    )
    .unwrap()
    .with_output(snapshot.digest().unwrap());
    AcceptedAuthoredField {
        model,
        realization,
        accepted,
        run,
        snapshot,
        block,
    }
}

fn fieldwise_plan(
    domain: eqiora::Id<eqiora::kinds::Domain>,
    field: eqiora::Id<eqiora::kinds::Field>,
    mesh: &SimplicialMeshEnvelopeV1,
) -> FieldwiseRealizationPlan {
    let spatial = FieldwiseSpatialDiscretization::new(
        domain,
        scale(LENGTH),
        [FieldSpaceBinding::new(
            field,
            Space::continuous_lagrange(NonZeroU16::MIN),
        )],
        [],
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::ImportedSimplicial {
                artifact: mesh.artifact_reference().unwrap(),
            },
            QuadraturePolicy::TriangleDuffyGaussLegendre {
                points_per_axis: NonZeroUsize::new(2).unwrap(),
            },
        ),
    )
    .unwrap();
    let scaling = SymmetricCongruenceScaling::new(
        [AlgebraicBlockScale::new(
            AlgebraicBlock::Field(field),
            scale(PRESSURE),
        )],
        scale(PRESSURE),
    )
    .unwrap();
    FieldwiseRealizationPlan::new(
        spatial,
        scaling,
        LinearOperatorProperties::SymmetricIndefinite,
        SolverPlan::new(
            LinearSolver::MinimumResidual,
            1.0e-11,
            1.0e-13,
            NonZeroUsize::new(10_000).unwrap(),
        )
        .unwrap(),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .unwrap()
}

fn scale(dimension: DimExponents) -> PositivePhysicalScale {
    PositivePhysicalScale::new(DynQuantity::new(1.0, dimension)).unwrap()
}

fn exact_source(center: [f64; 2]) -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole(
        [[0.0, 2.2], [0.0, 0.41]],
        center,
        0.05,
        vec![NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0])],
        1.0e-12,
    )
    .expect("exact circular-hole source")
}

fn geometry_program(source: &CanonicalGeometryV1) -> KernelProgram {
    let cartesian = eqiora::api::ModelDocument::compile("authored-p1-projection.eqi", SOURCE)
        .expect("Cartesian scaffold");
    let program = cartesian.program();
    let body = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::CartesianBox { .. }) =>
            {
                Some(domain.id())
            }
            _ => None,
        })
        .expect("one Cartesian body");
    let nodes = program
        .nodes()
        .map(|node| match node {
            KernelNode::Domain(domain) if domain.id() == body => KernelNode::from(
                DomainDef::geometry_region(
                    domain.id(),
                    eqiora::kernel::GeometryDigest::new(source.digest_bytes()),
                    "fluid",
                )
                .unwrap(),
            ),
            _ => node.clone(),
        })
        .collect::<Vec<_>>();
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("authored P1 projection witness");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for node in program.nodes() {
        if let Some(value) = program.value(node.id()) {
            transaction.push(Op::SetValue {
                target: node.id(),
                value,
            });
        }
    }
    for edge in program.edges() {
        transaction.push(Op::Connect {
            from: edge.from(),
            to: edge.to(),
            edge: edge.kind(),
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(program.model(), members, None)
            .unwrap()
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot_with_geometry(&store.snapshot(), program.model(), &[source])
        .expect("geometry-backed Model")
}
