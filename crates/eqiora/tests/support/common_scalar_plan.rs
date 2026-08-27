use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora::api::{ModelDocument, ScalarEllipticMethod};
use eqiora::artifact::{
    CartesianMeshCellsV1, GeometryMeshCorrespondenceEnvelopeV1, MeshProductionLineageEnvelopeV1,
    ModelEnvelope,
};
use eqiora::geometry::{PlanarOperationGraph, PlanarTopologyHandle};
use eqiora::solver::{LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};
use eqiora_numerics::{
    AuthenticatedCommonMesh, CommonScalarPlan, CommonSolvePolicy, CommonSpatialPolicy,
    resolve_common_plan,
};

pub(crate) const COMPONENT: &str = r#"
public component DifferentiatedPoisson {
  public support square: volume(ambient_dimension = 2);
  public support x_lower: boundary(parent = square);
  public support x_upper: boundary(parent = square);
  public support y_lower: boundary(parent = square);
  public support y_upper: boundary(parent = square);
  representation scalar_space = continuum;
  field potential on square as scalar_space: 1 = 0;
  public parameter diffusion: 1;
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  public parameter boundary_offset: 1;
  relation balance continuous on square {
    -div(diffusion * grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) - boundary_offset = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) - boundary_offset = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) - boundary_offset = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) - boundary_offset = 0; }
}
"#;

pub(crate) fn document_and_plan(method: ScalarEllipticMethod) -> (ModelDocument, CommonScalarPlan) {
    document_and_plan_with_source(method, COMPONENT)
}

pub(crate) fn document_and_plans() -> (ModelDocument, CommonScalarPlan, CommonScalarPlan) {
    document_and_plans_with_source(COMPONENT)
}

pub(crate) fn document_and_plan_with_source(
    method: ScalarEllipticMethod,
    source: &str,
) -> (ModelDocument, CommonScalarPlan) {
    let (document, q1, tpfa) = document_and_plans_with_source(source);
    let plan = match method {
        ScalarEllipticMethod::FiniteElement => q1,
        ScalarEllipticMethod::FiniteVolume => tpfa,
    };
    (document, plan)
}

fn document_and_plans_with_source(
    source: &str,
) -> (ModelDocument, CommonScalarPlan, CommonScalarPlan) {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let edges = rectangle.boundaries();
    let geometry = graph
        .build(
            &rectangle,
            &BTreeMap::from([
                ("square".to_owned(), vec![rectangle.region().into()]),
                (
                    "x_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[0])],
                ),
                (
                    "x_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[1])],
                ),
                (
                    "y_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[2])],
                ),
                (
                    "y_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[3])],
                ),
            ]),
        )
        .unwrap();
    let document = ModelDocument::compile_with_geometry(
        "differentiated-poisson.eqi",
        source,
        &geometry,
        None,
        &[
            ("diffusion", 1.0),
            ("wave_number", std::f64::consts::PI),
            ("source_scale", 2.0 * std::f64::consts::PI.powi(2)),
            ("boundary_offset", 0.0),
        ],
    )
    .unwrap();
    let cells = CartesianMeshCellsV1::new([12, 12]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
            &geometry,
            cells.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
        cells,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    let owner =
        AuthenticatedCommonMesh::structured_cartesian(geometry, mesh, correspondence, production)
            .unwrap();
    let solver = SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-10,
        1.0e-12,
        NonZeroUsize::new(10_000).unwrap(),
    )
    .unwrap();
    let model = ModelEnvelope::from_program(document.program()).unwrap();
    let resolve = |owner, spatial| {
        resolve_common_plan(
            &model,
            owner,
            spatial,
            CommonSolvePolicy::Linear(solver),
            None,
            None,
            &REFERENCE_LINEAR_SOLVER,
        )
        .unwrap()
        .project(
            |plan| plan,
            |_| panic!("scalar fixture resolved as elasticity"),
            |_| panic!("scalar fixture resolved as Stokes"),
            |_| panic!("scalar fixture resolved as transient flow"),
        )
    };
    let q1 = resolve(owner.clone(), CommonSpatialPolicy::Q1);
    let tpfa = resolve(owner, CommonSpatialPolicy::CellCenteredTpfa);
    (document, q1, tpfa)
}
