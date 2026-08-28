use super::*;

use eqiora_artifact::{
    AffineTriangleMeshCellsV1, CartesianMeshCellsV1, GeometryDecoderLimits,
    GeometryMeshCorrespondenceEnvelopeV1, MeshProductionLineageEnvelopeV1, ModelDecoderLimits,
};
use eqiora_core::{DimExponents, DynQuantity};
use eqiora_geometry::{
    CadAuthoredGraph, CanonicalGeometryV1, ConstrainedRectangleV1, NamedEntitySet,
    PlanarOperationGraph, PlanarTopologyHandle,
};
use eqiora_meshing::CartesianMesh;
use eqiora_solver::{
    BackendId, LinearProblem, LinearSolution, REFERENCE_LINEAR_SOLVER, ReplicatedLinearExecution,
    SolverPlan,
};

use eqiora_compiler::CompiledModel;

const COMPONENT: &str = r#"
public component PoissonRectangle {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  representation space = continuum;
  field potential on region as space: 1 = 0;
  relation balance continuous on region {
    -div(grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}
"#;

const STOKES_COMPONENT: &str =
    include_str!("../../../../eqiora-api/src/steady_stokes/accepted_component.eqi");
const ELASTICITY_COMPONENT: &str = r#"
public component MixedBoundaryElasticity {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter mu: kg / (m * s ^ 2);
  public parameter lambda: kg / (m * s ^ 2);
  public parameter length_scale: m;
  representation space = continuum;
  field displacement on region as space: m shape spatial_vector;
  field load_potential on region as space: kg / (m * s ^ 2) = 0;
  relation load continuous on region {
    load_potential - 2 * mu * coordinate(0) / length_scale = 0;
  }
  relation balance continuous on region {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) - grad(load_potential) = 0;
  }
  relation left_fixed continuous on left { trace(displacement) = 0; }
  relation right_free continuous on right {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
  relation bottom_free continuous on bottom {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
  relation top_free continuous on top {
    normal(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) = 0;
  }
}
"#;
const TRANSIENT_SOURCE: &str = include_str!(
    "../../../../../verify/fluid/cell-centered-navier-stokes-fvm-2d/models/direct.eqi"
);
const FSI_COMPONENT: &str = include_str!("../../../../../examples/fixed-reference-fsi.eqi");

type SupportBinding<'a> = (
    &'a str,
    &'a NamedEntitySet,
    Option<(&'a str, &'a NamedEntitySet)>,
);

fn compile_model(
    filename: &str,
    source: &str,
    geometry: &CanonicalGeometryV1,
    model: &str,
    component: &str,
    supports: &[SupportBinding<'_>],
    parameters: &[(&str, DynQuantity)],
) -> ModelEnvelope {
    let compiled = CompiledModel::compile_external_component(
        filename, source, model, component, geometry, supports, parameters,
    )
    .unwrap();
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program =
        KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model, &[geometry]).unwrap();
    ModelEnvelope::from_program(&program).unwrap()
}

#[derive(Debug)]
struct ResolveOnlyBackend;

impl LinearSolverBackend for ResolveOnlyBackend {
    fn provider(&self) -> SolverProvider {
        SolverProvider::new(BackendId::new("eqiora.test-resolve-only"), "1", &[])
    }

    fn capabilities(&self) -> SolverCapabilities {
        SolverCapabilities::exact([
            SolverCapability {
                algorithm: LinearSolver::SparseLu,
                operator_properties: LinearOperatorProperties::SymmetricIndefinite,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            },
            SolverCapability {
                algorithm: LinearSolver::BiConjugateGradientStabilized,
                operator_properties: LinearOperatorProperties::General,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            },
            SolverCapability {
                algorithm: LinearSolver::BiConjugateGradientStabilized,
                operator_properties: LinearOperatorProperties::General,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Reproducible,
                scalar_type: ScalarType::F64,
            },
        ])
        .unwrap()
    }

    fn solve_with_execution(
        &self,
        _problem: &LinearProblem<'_>,
        _plan: SolverPlan,
        _execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, Diagnostic> {
        unreachable!("resolution test must not execute")
    }
}

#[derive(Debug)]
struct AlternateScalarBackend;

impl LinearSolverBackend for AlternateScalarBackend {
    fn provider(&self) -> SolverProvider {
        SolverProvider::new(BackendId::new("eqiora.test-alternate-scalar"), "1", &[])
    }

    fn capabilities(&self) -> SolverCapabilities {
        REFERENCE_LINEAR_SOLVER.capabilities()
    }

    fn solve_with_execution(
        &self,
        _problem: &LinearProblem<'_>,
        _plan: SolverPlan,
        _execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, Diagnostic> {
        unreachable!("provider mismatch must reject before execution")
    }
}

fn rectangle() -> CanonicalGeometryV1 {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let edges = rectangle.boundaries();
    graph
        .build(
            &rectangle,
            &BTreeMap::from([
                ("region".to_owned(), vec![rectangle.region().into()]),
                (
                    "left".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[0])],
                ),
                (
                    "right".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[1])],
                ),
                (
                    "bottom".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[2])],
                ),
                ("top".to_owned(), vec![PlanarTopologyHandle::from(edges[3])]),
            ]),
        )
        .unwrap()
}

fn fsi_geometry() -> CanonicalGeometryV1 {
    let graph = PlanarOperationGraph::new();
    let fluid = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let solid = graph.rectangle([1.0, 2.0], [0.0, 1.0]).unwrap();
    let fluid_edges = fluid.boundaries();
    let solid_edges = solid.boundaries();
    let partition = graph
        .partition(&fluid, &solid, [fluid_edges[1], solid_edges[0]])
        .unwrap();
    graph
        .build(
            &partition,
            &BTreeMap::from([
                ("fluid".to_owned(), vec![fluid.region().into()]),
                ("fluid_x_lower".to_owned(), vec![fluid_edges[0].into()]),
                ("fluid_x_upper".to_owned(), vec![fluid_edges[1].into()]),
                ("fluid_y_lower".to_owned(), vec![fluid_edges[2].into()]),
                ("fluid_y_upper".to_owned(), vec![fluid_edges[3].into()]),
                ("solid".to_owned(), vec![solid.region().into()]),
                ("solid_x_lower".to_owned(), vec![solid_edges[0].into()]),
                ("solid_x_upper".to_owned(), vec![solid_edges[1].into()]),
                ("solid_y_lower".to_owned(), vec![solid_edges[2].into()]),
                ("solid_y_upper".to_owned(), vec![solid_edges[3].into()]),
            ]),
        )
        .unwrap()
}

fn fsi_model(geometry: &CanonicalGeometryV1) -> ModelEnvelope {
    let fluid = geometry.entity_set("fluid").unwrap();
    let solid = geometry.entity_set("solid").unwrap();
    let supports = [
        ("fluid", fluid, None),
        (
            "fluid_x_lower",
            geometry.entity_set("fluid_x_lower").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "fluid_x_upper",
            geometry.entity_set("fluid_x_upper").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "fluid_y_lower",
            geometry.entity_set("fluid_y_lower").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "fluid_y_upper",
            geometry.entity_set("fluid_y_upper").unwrap(),
            Some(("fluid", fluid)),
        ),
        ("solid", solid, None),
        (
            "solid_x_lower",
            geometry.entity_set("solid_x_lower").unwrap(),
            Some(("solid", solid)),
        ),
        (
            "solid_x_upper",
            geometry.entity_set("solid_x_upper").unwrap(),
            Some(("solid", solid)),
        ),
        (
            "solid_y_lower",
            geometry.entity_set("solid_y_lower").unwrap(),
            Some(("solid", solid)),
        ),
        (
            "solid_y_upper",
            geometry.entity_set("solid_y_upper").unwrap(),
            Some(("solid", solid)),
        ),
    ];
    let density = DimExponents {
        mass: 1,
        length: -3,
        ..DimExponents::DIMENSIONLESS
    };
    let viscosity = DimExponents {
        mass: 1,
        length: -1,
        time: -1,
        ..DimExponents::DIMENSIONLESS
    };
    let pressure = DimExponents {
        mass: 1,
        length: -1,
        time: -2,
        ..DimExponents::DIMENSIONLESS
    };
    compile_model(
        "fixed-reference-fsi.eqi",
        FSI_COMPONENT,
        geometry,
        "FixedReferenceFsiModel",
        "FixedReferenceFsi2d",
        &supports,
        &[
            ("fluid_density", DynQuantity::new(2.0, density)),
            ("fluid_viscosity", DynQuantity::new(0.5, viscosity)),
            ("solid_density", DynQuantity::new(3.0, density)),
            ("solid_mu", DynQuantity::new(4.0, pressure)),
            ("solid_lambda", DynQuantity::new(2.0, pressure)),
            ("zero_pressure", DynQuantity::new(0.0, pressure)),
        ],
    )
}

fn fsi_resources(geometry: &CanonicalGeometryV1) -> AuthenticatedCommonMesh {
    let policy = AffineTriangleMeshCellsV1::new([2, 2]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_adjacent_rectangle_partition_affine_triangles(
            geometry,
            policy.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
        policy,
        geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    AuthenticatedCommonMesh::adjacent_partition(geometry.clone(), mesh, correspondence, production)
        .unwrap()
}

mod fsi;

#[test]
fn registered_model_driven_common_mesh_admission_evidence() {
    fsi::exercise_model_driven_common_mesh_admission_evidence();
}
mod mesh;
mod plans;
mod transient;

use mesh::*;
use plans::*;
use transient::*;
