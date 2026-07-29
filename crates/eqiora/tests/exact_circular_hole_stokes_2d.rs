use std::num::NonZeroUsize;

use eqiora::artifact::{
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, SimplicialMeshEnvelopeV1,
};
use eqiora::compatibility::ExactModelCodec;
use eqiora::geometry::{
    CanonicalCircularHoleGeometryV1, CanonicalGeometryRef, CircularHoleChordalMeshV1,
    EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet,
};
use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora::kernel::{BoundarySide, DomainDef, DomainKind, KernelNode};
use eqiora::meshing::MeshQualityGate;
use eqiora::ontology::ModelView;
use eqiora::realization::{
    FieldwiseRealizationRequest, RealizationCapabilities, RealizationRevision, SemanticRevision,
    resolve_fieldwise,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};
use eqiora::{DimExponents, DynQuantity};
use eqiora_numerics::fluid::{
    IncompressibleFlowScaleProfile2d, SteadyStokesGeometryBinding2d,
    lower_steady_incompressible_stokes_geometry_2d, solve_resolved_steady_stokes_geometry_mini_2d,
    steady_stokes_fieldwise_requirements_2d, steady_stokes_mini_plan_2d,
};

const SOURCE: &str = r#"
model Main {
  domain body = box(0, 2.2, 0, 0.41);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  representation space = continuum;

  field velocity on body as space: m / s shape spatial_vector;
  field pressure on body as space: kg / (m * s ^ 2) = 0;
  field force_potential on body as space: kg / (m * s ^ 2) = 0;
  field inlet_profile on body as space: m / s = 0;
  parameter dynamic_viscosity: kg / (m * s) = 0.001;
  parameter zero_pressure: kg / (m * s ^ 2) = 0;
  parameter inlet_speed: m / s = 0.3;
  parameter channel_height: m = 0.41;

  relation force_definition continuous on body {
    force_potential - zero_pressure = 0;
  }
  relation inlet_profile_definition continuous on body {
    inlet_profile
      - 4 * inlet_speed * coordinate(1) * (channel_height - coordinate(1))
        / channel_height ^ 2 = 0;
  }
  relation momentum continuous on body {
    -div(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) - grad(force_potential) = 0;
  }
  relation incompressibility continuous on body {
    div(velocity) = 0;
  }

  relation inlet_velocity continuous on x_lower {
    trace(velocity) + normal(isotropic_lift(inlet_profile)) = 0;
  }
  relation outlet_traction continuous on x_upper {
    normal(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) = 0;
  }
  relation lower_wall continuous on y_lower { trace(velocity) = 0; }
  relation upper_wall continuous on y_upper { trace(velocity) = 0; }
}
"#;

const LENGTH: DimExponents = DimExponents {
    length: 1,
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

#[test]
fn exact_geometry_model_executes_the_frozen_reference_tuple() {
    let source = exact_source();
    let program = geometry_program(&source);
    let owner = CircularHoleChordalMeshV1::from_exact(
        &source,
        1.0e-4,
        50,
        MeshQualityGate::new(1.0e-5).unwrap(),
    )
    .expect("frozen chordal owner");
    let geometry = GeometryDefinitionV1::from_region(owner.region());
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(owner.mesh()).expect("mesh artifact");
    let mesh_reference = mesh.artifact_reference().expect("mesh identity");
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .expect("correspondence");
    let binding = SteadyStokesGeometryBinding2d::new(source, owner, geometry, mesh, correspondence)
        .expect("source-bound Stokes binding");
    let model = lower_steady_incompressible_stokes_geometry_2d(&program, &exact_source())
        .expect("geometry-backed Stokes lowers");
    let scales = IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(0.41, LENGTH),
        DynQuantity::new(0.3, VELOCITY),
        DynQuantity::new(0.001 * 0.3 / 0.41, PRESSURE),
    )
    .expect("frozen coherent-SI scale profile");
    let solver = SolverPlan::new(
        LinearSolver::MinimumResidual,
        1.0e-11,
        1.0e-13,
        NonZeroUsize::new(10_000).unwrap(),
    )
    .expect("frozen solver tuple");
    let plan = steady_stokes_mini_plan_2d(&model, mesh_reference, scales, solver)
        .expect("method-neutral MINI plan");
    let resolved = resolve_fieldwise(
        &FieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(131),
            plan,
        ),
        steady_stokes_fieldwise_requirements_2d(&model),
        &RealizationCapabilities::symmetric_mixed_simplicial_2d_reference(),
    )
    .expect("reference capability resolves");
    let (_, solution) = solve_resolved_steady_stokes_geometry_mini_2d(
        &program,
        &resolved,
        &binding,
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("frozen reference tuple executes");
    assert!(solution.named_boundary_reaction("cylinder").is_some());
}

fn exact_source() -> CanonicalCircularHoleGeometryV1 {
    CanonicalCircularHoleGeometryV1::new(
        [[0.0, 2.2], [0.0, 0.41]],
        [0.2, 0.2],
        0.05,
        vec![
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, vec![2, 3]),
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![1]),
        ],
        1.0e-12,
    )
    .expect("frozen exact source")
}

fn geometry_program(source: &CanonicalCircularHoleGeometryV1) -> KernelProgram {
    let cartesian = ExactModelCodec::V5
        .compile("exact-circular-hole-stokes-2d.eqi", SOURCE)
        .expect("Cartesian authoring scaffold compiles");
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
        .expect("one body");
    let mut nodes = Vec::new();
    for node in program.nodes() {
        let replacement = match node {
            KernelNode::Domain(domain) if domain.id() == body => KernelNode::from(
                DomainDef::geometry_region(
                    domain.id(),
                    eqiora::kernel::GeometryDigest::new(source.digest_bytes()),
                    "fluid",
                )
                .unwrap(),
            ),
            KernelNode::Domain(domain) => match domain.kind() {
                DomainKind::CartesianBoundary { axis, side } => {
                    let name = match (*axis, *side) {
                        (0, BoundarySide::Lower) => "inlet",
                        (0, BoundarySide::Upper) => "outlet",
                        (1, BoundarySide::Lower) => "walls",
                        (1, BoundarySide::Upper) => "cylinder",
                        _ => panic!("unexpected Cartesian scaffold boundary"),
                    };
                    KernelNode::from(DomainDef::geometry_boundary(domain.id(), name).unwrap())
                }
                _ => node.clone(),
            },
            _ => node.clone(),
        };
        nodes.push(replacement);
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("graph-authored exact circular-hole Stokes witness");
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
            edge: if edge.kind() == EdgeKind::BoundaryOf {
                EdgeKind::BoundaryOf
            } else {
                edge.kind()
            },
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(program.model(), members, None)
            .expect("closed geometry witness")
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("geometry witness commits");
    KernelProgram::from_snapshot_with_geometry(
        &store.snapshot(),
        program.model(),
        &[CanonicalGeometryRef::from(source)],
    )
    .expect("exact geometry admission")
}
