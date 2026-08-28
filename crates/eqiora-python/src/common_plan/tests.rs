use std::collections::BTreeMap;

use eqiora::api::ModelDocument;
use eqiora::geometry::{
    CanonicalGeometryV1, NamedEntitySet, PlanarOperationGraph, PlanarTopologyHandle,
};
use eqiora::{DimExponents, DynQuantity};
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods};

use crate::geometry::PyGeometry;
use crate::model::PyModel;

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
    include_str!("../../../eqiora-api/src/steady_stokes/accepted_component.eqi");
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
    -div(2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))) - grad(load_potential) = 0;
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
const TRANSIENT_SOURCE: &str =
    include_str!("../../../../verify/fluid/cell-centered-navier-stokes-fvm-2d/models/direct.eqi");
const TRANSIENT_CYLINDER_COMPONENT: &str =
    include_str!("../../../../examples/transient-flow-past-cylinder.eqi");

type SupportBinding<'a> = (
    &'a str,
    &'a NamedEntitySet,
    Option<(&'a str, &'a NamedEntitySet)>,
);

fn rectangle_geometry(xmax: f64) -> CanonicalGeometryV1 {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, xmax], [0.0, 1.0]).unwrap();
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

fn scalar_document(geometry: &CanonicalGeometryV1, source_scale: f64) -> ModelDocument {
    let region = geometry.entity_set("region").unwrap();
    let supports: [SupportBinding<'_>; 5] = [
        ("region", region, None),
        (
            "left",
            geometry.entity_set("left").unwrap(),
            Some(("region", region)),
        ),
        (
            "right",
            geometry.entity_set("right").unwrap(),
            Some(("region", region)),
        ),
        (
            "bottom",
            geometry.entity_set("bottom").unwrap(),
            Some(("region", region)),
        ),
        (
            "top",
            geometry.entity_set("top").unwrap(),
            Some(("region", region)),
        ),
    ];
    let parameters = [
        (
            "wave_number",
            DynQuantity::new(
                std::f64::consts::PI,
                DimExponents {
                    length: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "source_scale",
            DynQuantity::new(
                source_scale,
                DimExponents {
                    length: -2,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
    ];
    ModelDocument::compile_external_component(
        "python-common-plan.eqi",
        COMPONENT,
        geometry,
        "PoissonRectangleModel",
        "PoissonRectangle",
        &supports,
        &parameters,
    )
    .unwrap()
}

fn stokes_document(geometry: &CanonicalGeometryV1) -> ModelDocument {
    stokes_document_with_speed(geometry, 0.3)
}

fn stokes_document_with_speed(geometry: &CanonicalGeometryV1, inlet_speed: f64) -> ModelDocument {
    let fluid = geometry.entity_set("fluid").unwrap();
    let supports: [SupportBinding<'_>; 5] = [
        ("fluid", fluid, None),
        (
            "inlet",
            geometry.entity_set("inlet").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "outlet",
            geometry.entity_set("outlet").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "walls",
            geometry.entity_set("walls").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "cylinder",
            geometry.entity_set("cylinder").unwrap(),
            Some(("fluid", fluid)),
        ),
    ];
    let parameters = [
        (
            "dynamic_viscosity",
            DynQuantity::new(
                0.001,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "zero_pressure",
            DynQuantity::new(
                0.0,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -2,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "inlet_speed",
            DynQuantity::new(
                inlet_speed,
                DimExponents {
                    length: 1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "channel_height",
            DynQuantity::new(
                0.41,
                DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
    ];
    ModelDocument::compile_external_component(
        "steady-flow-past-cylinder.eqi",
        STOKES_COMPONENT,
        geometry,
        "SteadyFlowPastCylinderModel",
        "SteadyFlowPastCylinder",
        &supports,
        &parameters,
    )
    .unwrap()
}

fn transient_cylinder_document(geometry: &CanonicalGeometryV1) -> ModelDocument {
    transient_cylinder_document_with_speed(geometry, 0.3)
}

fn transient_cylinder_document_with_speed(
    geometry: &CanonicalGeometryV1,
    inlet_speed: f64,
) -> ModelDocument {
    let fluid = geometry.entity_set("fluid").unwrap();
    let supports: [SupportBinding<'_>; 5] = [
        ("fluid", fluid, None),
        (
            "inlet",
            geometry.entity_set("inlet").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "outlet",
            geometry.entity_set("outlet").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "walls",
            geometry.entity_set("walls").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "cylinder",
            geometry.entity_set("cylinder").unwrap(),
            Some(("fluid", fluid)),
        ),
    ];
    let parameters = [
        (
            "density",
            DynQuantity::new(
                1.0,
                DimExponents {
                    mass: 1,
                    length: -3,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "dynamic_viscosity",
            DynQuantity::new(
                0.001,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "zero_pressure",
            DynQuantity::new(
                0.0,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -2,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "inlet_speed",
            DynQuantity::new(
                inlet_speed,
                DimExponents {
                    length: 1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "channel_height",
            DynQuantity::new(
                0.41,
                DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
    ];
    ModelDocument::compile_external_component(
        "transient-flow-past-cylinder.eqi",
        TRANSIENT_CYLINDER_COMPONENT,
        geometry,
        "TransientFlowPastCylinderModel",
        "TransientFlowPastCylinder",
        &supports,
        &parameters,
    )
    .unwrap()
}

#[test]
fn installed_root_resolve_and_run_retain_exact_python_objects() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(crate::_eqiora)(py);
        let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bindings/python/python/eqiora")
            .canonicalize()?;
        let locals = PyDict::new(py);
        locals.set_item("native", native.bind(py))?;
        locals.set_item("package_directory", package_directory.to_string_lossy())?;
        py.run(
                c_str!(r#"
import importlib.util, pathlib, sys
package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location("eqiora", package_path / "__init__.py", submodule_search_locations=[str(package_path)])
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)

graph = package.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
source = graph.build(rectangle, named_topology={
    "region": rectangle.region,
    "left": rectangle.boundaries[0],
    "right": rectangle.boundaries[1],
    "bottom": rectangle.boundaries[2],
    "top": rectangle.boundaries[3],
})
mesher = package.meshing.CartesianMesher(cells=(2, 3))
mesh_plan = package.meshing.resolve(source, mesher)
mesh = package.meshing.generate(source, plan=mesh_plan)
"#),
                None,
                Some(&locals),
            )?;
        let geometry = locals
            .get_item("source")?
            .unwrap()
            .extract::<PyRef<'_, PyGeometry>>()?
            .geometry()
            .clone();
        let model = PyModel::from_document(
            py,
            scalar_document(&geometry, 2.0 * std::f64::consts::PI.powi(2)),
        )?;
        let fresh_model = PyModel::from_document(
            py,
            scalar_document(&geometry, 3.0 * std::f64::consts::PI.powi(2)),
        )?;
        let foreign_model = PyModel::from_document(
            py,
            scalar_document(&rectangle_geometry(2.0), 2.0 * std::f64::consts::PI.powi(2)),
        )?;
        locals.set_item("model", Py::new(py, model)?)?;
        locals.set_item("fresh_model", Py::new(py, fresh_model)?)?;
        locals.set_item("foreign_model", Py::new(py, foreign_model)?)?;
        py.run(
                c_str!(r#"
linear = package.solve.Linear(relative_tolerance=1e-10, absolute_tolerance=1e-12, maximum_iterations=10000)
q1 = package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
q1_repeat = package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
tpfa = package.resolve(model, mesh=mesh, spatial=package.fvm.CellCenteredTpfa(), solve=linear)
replayed = package.replay(model.to_json())
replayed_plan = package.resolve(replayed, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
fresh_plan = package.resolve(fresh_model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
assert q1.model is model
assert q1.mesh is mesh
assert q1.mesh_digest == mesh.digest
assert q1.identity == q1_repeat.identity
assert replayed_plan.identity == q1.identity
assert fresh_model.digest != model.digest
assert fresh_plan.model_digest == fresh_model.digest
assert fresh_plan.identity != q1.identity
assert q1.identity != tpfa.identity
assert q1.cells == (2, 3)
assert q1.scaling is None
assert q1.scaling_receipt is None
assert q1.solve is linear
q1_result = package.run(q1)
tpfa_result = package.run(tpfa)
assert q1_result.model_digest == q1.model_digest
assert q1_result.plan_key == q1.identity
assert q1_result.mesh(q1.field) is mesh
assert q1_result.logical_shape == (3, 4)
assert len(q1_result.values) == 12
assert not hasattr(q1_result, "run_manifest")
assert tpfa_result.logical_shape == (2, 3)
assert len(tpfa_result.values) == 6
try:
    package.resolve(foreign_model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("foreign Model was admitted against the caller Mesh")
try:
    package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("MINI/P1 selected Stokes physics for a scalar Model")
try:
    package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear,
                    scaling=package.fluid.IncompressibleScaling())
except package.ValidationError:
    pass
else:
    raise AssertionError("flow scaling was admitted for scalar Model mathematics")

foreign_rectangle = graph.rectangle(x_bounds=(0.0, 2.0), y_bounds=(0.0, 1.0))
foreign_source = graph.build(foreign_rectangle, named_topology={
    "region": foreign_rectangle.region,
    "left": foreign_rectangle.boundaries[0],
    "right": foreign_rectangle.boundaries[1],
    "bottom": foreign_rectangle.boundaries[2],
    "top": foreign_rectangle.boundaries[3],
})
foreign_plan = package.meshing.resolve(foreign_source, mesher)
foreign_mesh = package.meshing.generate(foreign_source, plan=foreign_plan)
try:
    package.resolve(model, mesh=foreign_mesh, spatial=package.fem.Q1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("same-cardinality foreign Mesh was admitted")
try:
    package.run(model)
except TypeError:
    pass
else:
    raise AssertionError("root run retained a Model-shaped compatibility path")
"#),
                None,
                Some(&locals),
            )
    })
}

#[test]
fn root_plan_resolves_model_owned_steady_stokes_with_automatic_scaling() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(crate::_eqiora)(py);
        let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bindings/python/python/eqiora")
            .canonicalize()?;
        let locals = PyDict::new(py);
        locals.set_item("native", native.bind(py))?;
        locals.set_item("package_directory", package_directory.to_string_lossy())?;
        py.run(
                c_str!(r#"
import importlib.util, pathlib, sys
package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location("eqiora", package_path / "__init__.py", submodule_search_locations=[str(package_path)])
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)

graph = package.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
source = graph.build(fluid, named_topology={
    "fluid": fluid.region,
    "inlet": rectangle.boundaries[0],
    "outlet": rectangle.boundaries[1],
    "walls": rectangle.boundaries[2:],
    "cylinder": circle.boundaries[0],
})
mesher = package.meshing.GmshMesher(maximum_boundary_error=1e-4, minimum_mean_ratio=1e-5, maximum_boundary_facets=50)
mesh_plan = package.meshing.resolve(source, mesher)
mesh = package.meshing.generate(source, plan=mesh_plan)
"#),
                None,
                Some(&locals),
            )?;
        let geometry = locals
            .get_item("source")?
            .unwrap()
            .extract::<PyRef<'_, PyGeometry>>()?
            .geometry()
            .clone();
        let model = PyModel::from_document(py, stokes_document(&geometry))?;
        let fresh_model = PyModel::from_document(py, stokes_document(&geometry))?;
        let foreign_model = PyModel::from_document(
            py,
            scalar_document(&rectangle_geometry(1.0), 2.0 * std::f64::consts::PI.powi(2)),
        )?;
        locals.set_item("model", Py::new(py, model)?)?;
        locals.set_item("fresh_model", Py::new(py, fresh_model)?)?;
        locals.set_item("foreign_model", Py::new(py, foreign_model)?)?;
        py.run(
                c_str!(r#"
linear = package.solve.Linear(relative_tolerance=1e-6, absolute_tolerance=1e-13, maximum_iterations=10000)
plan = package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
explicit_none = package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear, scaling=None)
all_auto_request = package.fluid.IncompressibleScaling()
all_auto = package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear, scaling=all_auto_request)
partial_equal = package.resolve(
    model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear,
    scaling=package.fluid.IncompressibleScaling(length_m=0.41),
)
partial_changed = package.resolve(
    model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear,
    scaling=package.fluid.IncompressibleScaling(length_m=0.82),
)
manual_equal = package.resolve(
    model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear,
    scaling=package.fluid.IncompressibleScaling(
        length_m=0.41,
        velocity_m_per_s=0.3,
        pressure_pa=0.001 * 0.3 / 0.41,
    ),
)
replayed = package.replay(model.to_json())
replayed_plan = package.resolve(replayed, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
fresh_plan = package.resolve(fresh_model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
assert plan.identity == explicit_none.identity == all_auto.identity == replayed_plan.identity == fresh_plan.identity
assert all_auto_request == package.fluid.IncompressibleScaling()
assert all_auto_request.length_m is None
assert all_auto_request.velocity_m_per_s is None
assert all_auto_request.pressure_pa is None
assert plan.model is model
assert plan.mesh is mesh
assert plan.mesh_digest == mesh.digest
assert plan.realization_digest is not None
assert plan.realization_digest == partial_equal.realization_digest == manual_equal.realization_digest
assert partial_changed.realization_digest != plan.realization_digest
assert partial_equal.identity != plan.identity
assert manual_equal.identity != plan.identity
assert partial_equal.identity != manual_equal.identity
assert plan.scaling.length_m == 0.41
assert plan.scaling.velocity_m_per_s == 0.3
assert plan.scaling.pressure_pa == 0.001 * 0.3 / 0.41
assert partial_changed.scaling.length_m == 0.82
assert partial_changed.scaling.velocity_m_per_s == 0.3
assert partial_changed.scaling.pressure_pa == 0.001 * 0.3 / 0.82
assert partial_equal.scaling.pressure_pa == plan.scaling.pressure_pa
assert manual_equal.scaling.pressure_pa == plan.scaling.pressure_pa
receipt = plan.scaling_receipt
partial_receipt = partial_equal.scaling_receipt
manual_receipt = manual_equal.scaling_receipt
assert receipt.model_digest == plan.model_digest
assert receipt.geometry_digest == plan.geometry_digest
assert receipt.correspondence_digest == plan.correspondence_digest
assert receipt.mesh_digest == plan.mesh_digest
assert len(receipt.components) == 5
component = package.fluid.IncompressibleScalingComponent2d
mode = package.fluid.IncompressibleScalingMode
rule = package.fluid.IncompressibleScalingRule2d
authority = package.fluid.IncompressibleScalingAuthorityKind
assert tuple(record.component for record in receipt.components) == (
    component.Length, component.Velocity, component.Pressure,
    component.Gauge, component.WeakFunctional,
)
assert receipt.length.dimension == (0, 1, 0, 0, 0, 0, 0)
assert receipt.velocity.dimension == (0, 1, -1, 0, 0, 0, 0)
assert receipt.pressure.dimension == (1, -1, -2, 0, 0, 0, 0)
assert receipt.gauge.dimension == (0, 0, -1, 0, 0, 0, 0)
assert receipt.weak_functional.dimension == (1, 1, -3, 0, 0, 0, 0)
assert (receipt.length.mode, receipt.velocity.mode, receipt.pressure.mode,
        receipt.gauge.mode, receipt.weak_functional.mode) == (
    mode.Automatic, mode.Automatic, mode.Derived, mode.Derived, mode.Derived,
)
assert receipt.length.rule == rule.ExactChannelHeightV1
assert receipt.velocity.rule == rule.ExactInletMaximumV1
assert receipt.pressure.rule == rule.ViscousStokesPressureV1
assert receipt.gauge.rule == rule.GaugeRateV1
assert receipt.weak_functional.rule == rule.WeakFunctionalV1
assert receipt.length.dependencies == ()
assert receipt.velocity.dependencies == ()
assert receipt.pressure.dependencies == (component.Length, component.Velocity)
assert receipt.gauge.dependencies == (component.Velocity, component.Length)
assert receipt.weak_functional.dependencies == (component.Pressure, component.Velocity, component.Length)
assert receipt.length.authorities[0].kind == authority.ExactGeometrySpan
assert receipt.length.authorities[0].axis == 1
assert receipt.length.authorities[0].bounds_m == (0.0, 0.41)
assert receipt.velocity.authorities[1].kind == authority.ModelInletMaximum
assert receipt.pressure.authorities[0].kind == authority.ModelDynamicViscosity
assert receipt.gauge.authorities == ()
assert receipt.weak_functional.authorities == ()
assert partial_receipt.length.mode == mode.Manual
assert partial_receipt.velocity.mode == mode.Automatic
assert partial_receipt.pressure.mode == mode.Derived
assert partial_receipt.length.authorities[0].kind == authority.ManualRequest
assert manual_receipt.length.mode == mode.Manual
assert manual_receipt.velocity.mode == mode.Manual
assert manual_receipt.pressure.mode == mode.Manual
assert receipt.provenance_digest != partial_receipt.provenance_digest
assert receipt.provenance_digest != manual_receipt.provenance_digest
assert partial_receipt.provenance_digest != manual_receipt.provenance_digest
try:
    plan.scaling.length_m = 1.0
except AttributeError:
    pass
else:
    raise AssertionError("effective scaling was mutable")
try:
    receipt.components += (receipt.length,)
except AttributeError:
    pass
else:
    raise AssertionError("scaling receipt was mutable")
assert plan.discretization == "mini-p1"
assert plan.mesh_kind == "imported-affine-simplicial"
assert plan.operator_properties == "symmetric-indefinite"
assert plan.solver_algorithm == "sparse-lu"
assert plan.preconditioner == "identity"
assert plan.reduction == "fast"
assert plan.solver_backend == "eqiora.faer"
assert plan.solve is linear
result = package.run(plan)
assert isinstance(result, package.Result)
stokes_evidence = package.fluid.steady_stokes_evidence(result)
assert isinstance(stokes_evidence, package.fluid.SteadyStokesEvidence)
assert stokes_evidence.plan_key == result.plan_key
assert stokes_evidence.exact_bounds == ((0.0, 2.2), (0.0, 0.41))
try:
    package.State.zero(plan)
except ValueError:
    pass
else:
    raise AssertionError("State.zero admitted a steady Plan")
assert result.model_digest == plan.model_digest
assert result.plan_key == plan.identity
assert package.run(partial_equal).plan_key == partial_equal.identity
assert package.run(manual_equal).plan_key == manual_equal.identity
assert len(plan.fields) == 2
assert plan.fields == (plan.velocity_field, plan.pressure_field)
assert plan.field is None
assert not hasattr(result, "outputs")
velocity = result.output(plan.velocity_field)
pressure = result.output(plan.pressure_field)
assert result.mesh(plan.velocity_field) is mesh
assert result.mesh(plan.pressure_field) is mesh
assert velocity.field == plan.velocity_field
assert pressure.field == plan.pressure_field
assert velocity.mesh is pressure.mesh is mesh
assert velocity.dimension == (0, 1, -1, 0, 0, 0, 0)
assert velocity.components == 2
assert velocity.vertex_count == mesh.vertex_count
assert len(velocity.vertex_values) == mesh.vertex_count * 2
assert velocity.cell_bubble_count == mesh.cell_count
assert len(velocity.cell_bubble_values) == mesh.cell_count * 2
assert pressure.dimension == (1, -1, -2, 0, 0, 0, 0)
assert pressure.components == 1
assert pressure.vertex_count == mesh.vertex_count
assert len(pressure.vertex_values) == mesh.vertex_count
assert pressure.cell_bubble_count == 0
assert pressure.cell_bubble_values is None
assert result.solve.algorithm == "sparse-lu"
foreign_field = foreign_model.field(foreign_model.field_ids[0])
try:
    result.output(foreign_field)
except ValueError:
    pass
else:
    raise AssertionError("foreign exact FieldRef selected a common output")
try:
    result.output("pressure")
except TypeError:
    pass
else:
    raise AssertionError("string/name lookup selected a common output")
try:
    package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("Stokes Model selected scalar physics through a spatial policy")
try:
    package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear, scaling=object())
except TypeError:
    pass
else:
    raise AssertionError("untyped scaling reached Plan publication")
try:
    package.resolve(
        model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear,
        temporal=package.time.BackwardEuler(0.01),
    )
except package.ValidationError:
    pass
else:
    raise AssertionError("steady Stokes admitted a temporal policy")
for invalid in (True, False, 1, 0.0, -0.0, -1.0, float("nan"), float("inf"), -float("inf"), "auto", {}):
    try:
        package.fluid.IncompressibleScaling(length_m=invalid)
    except (TypeError, package.ValidationError):
        pass
    else:
        raise AssertionError(f"invalid scaling component was admitted: {invalid!r}")
assert not hasattr(package, "AUTO")
assert not hasattr(package.fluid, "AUTO")
"#),
                None,
                Some(&locals),
            )
    })
}

#[test]
fn root_plan_runs_geometry_backed_transient_cylinder_with_explicit_state() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(crate::_eqiora)(py);
        let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bindings/python/python/eqiora")
            .canonicalize()?;
        let locals = PyDict::new(py);
        locals.set_item("native", native.bind(py))?;
        locals.set_item("package_directory", package_directory.to_string_lossy())?;
        py.run(
            c_str!(r#"
import importlib.util, pathlib, sys
package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location("eqiora", package_path / "__init__.py", submodule_search_locations=[str(package_path)])
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)

graph = package.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
source = graph.build(fluid, named_topology={
    "fluid": fluid.region,
    "inlet": rectangle.boundaries[0],
    "outlet": rectangle.boundaries[1],
    "walls": rectangle.boundaries[2:],
    "cylinder": circle.boundaries[0],
})
"#),
            None,
            Some(&locals),
        )?;
        let geometry = locals
            .get_item("source")?
            .unwrap()
            .extract::<PyRef<'_, PyGeometry>>()?
            .geometry()
            .clone();
        let model = PyModel::from_document(py, transient_cylinder_document(&geometry))?;
        let steady_model = PyModel::from_document(py, stokes_document_with_speed(&geometry, 0.3))?;
        locals.set_item("model", Py::new(py, model)?)?;
        locals.set_item("steady_model", Py::new(py, steady_model)?)?;
        py.run(
            c_str!(r#"
mesher = package.meshing.GmshMesher(maximum_boundary_error=1e-4, minimum_mean_ratio=1e-5, maximum_boundary_facets=50)
mesh = package.meshing.generate(source, plan=package.meshing.resolve(source, mesher))
import numpy as np
linear = package.solve.Linear(relative_tolerance=1e-6, absolute_tolerance=1e-9, maximum_iterations=20000)
steady_plan = package.resolve(
    steady_model,
    mesh=mesh,
    spatial=package.fem.MiniP1(),
    solve=linear,
    scaling=package.fluid.IncompressibleScaling(
        length_m=0.41,
        velocity_m_per_s=0.3,
        pressure_pa=0.001 * 0.3 / 0.41,
    ),
)
steady_result = package.run(steady_plan)
steady_velocity = steady_result.output(steady_plan.velocity_field)
steady_pressure = steady_result.output(steady_plan.pressure_field)
plan = package.resolve(
    model,
    mesh=mesh,
    spatial=package.fem.MiniP1(),
    solve=package.solve.Newton(linear=linear),
    temporal=package.time.BackwardEuler(0.0001),
    scaling=package.fluid.IncompressibleScaling(
        length_m=0.41,
        velocity_m_per_s=0.3,
        pressure_pa=1.0 * 0.3 * 0.3,
    ),
)
state = package.State.initial(
    plan,
    time_s=0.0,
    fields=(
        package.InitialField(
            plan.velocity_field,
            vertex_values=np.asarray(steady_velocity.vertex_values).reshape(mesh.vertex_count, 2),
            cell_values=np.asarray(steady_velocity.cell_bubble_values).reshape(mesh.cell_count, 2),
        ),
        package.InitialField(
            plan.pressure_field,
            vertex_values=np.asarray(steady_pressure.vertex_values),
        ),
    ),
)
assert plan.model is model and plan.mesh is mesh
assert plan.pressure_gauge is package.fluid.PressureGauge2d.BoundaryTraction
assert np.max(np.abs(np.asarray(steady_velocity.vertex_values))) > 0.0
assert state.field(plan.velocity_field).associations == ("vertex", "cell")
result = package.run(plan, state=state, steps=1, output_steps=(1,))
assert len(result.trajectory.states) == 1
wake_state = result.trajectory.states[0]
assert wake_state.time_s == 0.0001
assert np.max(np.abs(wake_state.field(plan.velocity_field).values("vertex"))) > 0.0
vorticity = wake_state.curl(plan.velocity_field)
assert vorticity.operator == "curl"
assert vorticity.source_state_digest == wake_state.digest
assert vorticity.source_field == plan.velocity_field
assert vorticity.mesh_digest == mesh.digest
assert vorticity.support_domain_id == wake_state.field(plan.velocity_field).support_domain_id
assert vorticity.dimension == (0, 0, -1, 0, 0, 0, 0)
assert vorticity.value_shape == ()
assert vorticity.frame == "spatial-axial"
assert vorticity.associations == ("cell",)
assert np.array_equal(vorticity.support_indices("cell"), np.arange(mesh.cell_count, dtype=np.uint32))
assert np.all(np.isfinite(vorticity.values("cell")))
assert np.max(np.abs(vorticity.values("cell"))) > 0.0
assert not vorticity.values("cell").flags.writeable
assert wake_state.curl(plan.velocity_field) == vorticity
front_pressure = wake_state.sample(plan.pressure_field, at=(0.15, 0.2))
rear_pressure = wake_state.sample(plan.pressure_field, at=(0.25, 0.2))
assert front_pressure.source_state_digest == wake_state.digest
assert front_pressure.field == plan.pressure_field
assert front_pressure.mesh_digest == mesh.digest
assert front_pressure.support_domain_id == wake_state.field(plan.pressure_field).support_domain_id
assert front_pressure.point_m == (0.15, 0.2)
assert front_pressure.dimension == (1, -1, -2, 0, 0, 0, 0)
assert front_pressure.frame == "invariant"
assert np.isfinite(front_pressure.value) and np.isfinite(rear_pressure.value)
assert wake_state.sample(plan.pressure_field, at=(0.15, 0.2)) == front_pressure
cylinder = source.selection("cylinder")
cylinder_force = wake_state.boundary_force(cylinder)
assert cylinder_force.source_state_digest == wake_state.digest
assert cylinder_force.selection == cylinder
assert cylinder_force.geometry_digest == source.digest
assert cylinder_force.mesh_digest == mesh.digest
assert cylinder_force.dimension == (1, 0, -2, 0, 0, 0, 0)
assert cylinder_force.frame == "spatial-cartesian"
assert np.all(np.isfinite(cylinder_force.on_domain))
assert cylinder_force.on_selection == tuple(-value for value in cylinder_force.on_domain)
assert wake_state.boundary_force(cylinder) == cylinder_force
try:
    wake_state.curl(plan.pressure_field)
except ValueError as error:
    assert "velocity FieldRef" in str(error)
else:
    raise AssertionError("curl accepted the pressure Field")
for invalid_sample in range(3):
    try:
        if invalid_sample == 0:
            wake_state.sample(plan.velocity_field, at=(0.15, 0.2))
        elif invalid_sample == 1:
            wake_state.sample(plan.pressure_field, at=(3.0, 3.0))
        else:
            wake_state.sample(plan.pressure_field, at=(True, 0.2))
    except (ValueError, package.ValidationError):
        pass
    else:
        raise AssertionError("invalid point sample was admitted")
try:
    state.boundary_force(cylinder)
except KeyError:
    pass
else:
    raise AssertionError("initial State fabricated an accepted boundary force")
try:
    wake_state.boundary_force(source.selection("fluid"))
except ValueError:
    pass
else:
    raise AssertionError("boundary force accepted a region selection")
"#),
            None,
            Some(&locals),
        )
    })
}

#[test]
fn root_plan_resolves_and_runs_transient_flow_through_common_state() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(crate::_eqiora)(py);
        let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bindings/python/python/eqiora")
            .canonicalize()?;
        let locals = PyDict::new(py);
        locals.set_item("native", native.bind(py))?;
        locals.set_item("package_directory", package_directory.to_string_lossy())?;
        let model = PyModel::from_document(
            py,
            ModelDocument::compile("transient-direct.eqi", TRANSIENT_SOURCE).unwrap(),
        )?;
        locals.set_item("model", Py::new(py, model)?)?;
        py.run(
                c_str!(r#"
import importlib.util, pathlib, sys
package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location("eqiora", package_path / "__init__.py", submodule_search_locations=[str(package_path)])
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)

graph = package.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
source = graph.build(rectangle, named_topology={
    "region": rectangle.region,
    "left": rectangle.boundaries[0],
    "right": rectangle.boundaries[1],
    "bottom": rectangle.boundaries[2],
    "top": rectangle.boundaries[3],
})
affine_plan = package.meshing.resolve(source, package.meshing.AffineTriangleMesher(cells=(2, 3)))
affine = package.meshing.generate(source, plan=affine_plan)
cartesian_plan = package.meshing.resolve(source, package.meshing.CartesianMesher(cells=(4, 4)))
cartesian = package.meshing.generate(source, plan=cartesian_plan)
linear = package.solve.Linear(relative_tolerance=1e-10, absolute_tolerance=1e-12, maximum_iterations=2000)
newton = package.solve.Newton(linear=linear)
custom_newton = package.solve.Newton(
    linear=linear,
    relative_tolerance=2e-9,
    absolute_tolerance=3e-11,
    maximum_iterations=19,
    maximum_line_search_steps=7,
)
temporal = package.time.BackwardEuler(0.01)
scaling = package.fluid.IncompressibleScaling(length_m=1.0, velocity_m_per_s=2.0, pressure_pa=3.0)

mini = package.resolve(model, mesh=affine, spatial=package.fem.MiniP1(), solve=newton, scaling=scaling, temporal=temporal)
fvm = package.resolve(model, mesh=cartesian, spatial=package.fvm.CellCentered(), solve=newton, scaling=scaling, temporal=temporal)
replayed = package.resolve(package.replay(model.to_json()), mesh=affine, spatial=package.fem.MiniP1(), solve=newton, scaling=scaling, temporal=temporal)
custom = package.resolve(model, mesh=affine, spatial=package.fem.MiniP1(), solve=custom_newton, scaling=scaling, temporal=temporal)
assert mini.identity == replayed.identity
assert mini.identity != fvm.identity
assert mini.identity != custom.identity
assert mini.model is model and mini.mesh is affine
assert fvm.model is model and fvm.mesh is cartesian
assert mini.fields == (mini.velocity_field, mini.pressure_field)
assert fvm.fields == (fvm.velocity_field, fvm.pressure_field)
assert mini.solve is not newton and mini.solve.linear is not linear
assert mini.solve.relative_tolerance == 1e-9
assert mini.solve.absolute_tolerance == 1e-11
assert mini.solve.maximum_iterations == 16
assert mini.solve.maximum_line_search_steps == 12
assert mini.solve.linear.relative_tolerance == linear.relative_tolerance
assert mini.solve.linear.absolute_tolerance == linear.absolute_tolerance
assert mini.solve.linear.maximum_iterations == linear.maximum_iterations
assert custom.solve.relative_tolerance == 2e-9
assert custom.solve.absolute_tolerance == 3e-11
assert custom.solve.maximum_iterations == 19
assert custom.solve.maximum_line_search_steps == 7
assert mini.temporal is temporal and mini.temporal.step_s == 0.01
assert mini.realization_digest is None and fvm.realization_digest is None
assert mini.space is None and fvm.space is None
assert mini.velocity_space == "simplex-p1-bubble"
assert mini.pressure_space == "continuous-lagrange-p1"
assert fvm.velocity_space == fvm.pressure_space == "cell-constant"
assert mini.pressure_gauge is package.fluid.PressureGauge2d.ZeroIntegral
assert fvm.pressure_gauge is package.fluid.PressureGauge2d.ZeroIntegral
assert mini.mesh_kind == "imported-affine-simplicial"
assert fvm.mesh_kind == "supplied-cartesian"
assert mini.solver_algorithm == "sparse-lu"
assert fvm.solver_algorithm == "bicgstab"
assert mini.reduction == "fast" and fvm.reduction == "reproducible"
assert mini.solver_backend == "eqiora.faer"
assert fvm.solver_backend == "eqiora.reference"
assert mini.scaling.length_m == 1.0 and mini.scaling.velocity_m_per_s == 2.0 and mini.scaling.pressure_pa == 3.0

for kwargs in (
    dict(temporal=None),
    dict(solve=linear, temporal=temporal),
    dict(scaling=None, temporal=temporal),
    dict(scaling=package.fluid.IncompressibleScaling(), temporal=temporal),
    dict(scaling=package.fluid.IncompressibleScaling(length_m=1.0), temporal=temporal),
):
    request = dict(mesh=affine, spatial=package.fem.MiniP1(), solve=newton, scaling=scaling, temporal=temporal)
    request.update(kwargs)
    try:
        package.resolve(model, **request)
    except package.ValidationError:
        pass
    else:
        raise AssertionError(f"invalid transient request admitted: {kwargs}")
for forbidden in ("state", "horizon", "output"):
    request = dict(mesh=affine, spatial=package.fem.MiniP1(), solve=newton, scaling=scaling, temporal=temporal)
    request[forbidden] = object()
    try:
        package.resolve(model, **request)
    except TypeError:
        pass
    else:
        raise AssertionError(f"future execution argument admitted: {forbidden}")
for invalid_step in (True, 1, 0.0, -0.0, -1.0, float("nan"), float("inf"), -float("inf")):
    try:
        package.time.BackwardEuler(invalid_step)
    except (TypeError, package.ValidationError):
        pass
    else:
        raise AssertionError(f"invalid BackwardEuler step admitted: {invalid_step!r}")
for kwargs in (
    dict(relative_tolerance=True),
    dict(relative_tolerance=1),
    dict(relative_tolerance=-1e-9),
    dict(relative_tolerance=1.0),
    dict(relative_tolerance=float("nan")),
    dict(relative_tolerance=float("inf")),
    dict(absolute_tolerance=True),
    dict(absolute_tolerance=1),
    dict(absolute_tolerance=-1e-11),
    dict(absolute_tolerance=float("nan")),
    dict(maximum_iterations=True),
    dict(maximum_iterations=0),
    dict(maximum_iterations=-1),
    dict(maximum_iterations=1.0),
    dict(maximum_line_search_steps=True),
    dict(maximum_line_search_steps=-1),
    dict(maximum_line_search_steps=65),
    dict(maximum_line_search_steps=1.0),
    dict(relative_tolerance=0.0, absolute_tolerance=0.0),
):
    try:
        package.solve.Newton(linear=linear, **kwargs)
    except (TypeError, package.ValidationError):
        pass
    else:
        raise AssertionError(f"invalid Newton controls admitted: {kwargs}")
mini_zero = package.State.zero(mini)
assert mini_zero.time_s == 0.0
assert mini_zero.mesh is affine
assert mini_zero.model is model
assert mini_zero.source_plan_identity == mini.identity
assert mini_zero.source_request_identity is None
assert mini_zero.source_trajectory_identity is None
assert mini_zero.source_kind == "zero"
assert len(mini_zero.fields) == 2
assert mini_zero.field(mini.velocity_field).associations == ("vertex", "cell")
assert mini_zero.field(mini.pressure_field).associations == ("vertex",)

mini_one_sync = package.run(mini, state=mini_zero, steps=1, output_steps=(1,))
mini_one_async = package.submit(mini, state=mini_zero, steps=1, output_steps=(1,)).result()
assert mini_one_sync.trajectory.digest == mini_one_async.trajectory.digest
mini_one_time = package.run(mini, state=mini_zero, until_s=0.01, output_times_s=(0.01,))
assert mini_one_sync.trajectory.digest == mini_one_time.trajectory.digest
mini_two = package.run(mini, state=mini_zero, steps=2, output_steps=(1, 2))
assert tuple(state.step for state in mini_two.trajectory.states) == (1, 2)
assert tuple(state.time_s for state in mini_two.trajectory.states) == (0.01, 0.02)
mini_restart = package.State.from_result(custom, mini_one_sync, time_s=0.01)
assert mini_restart.state_space_identity == mini_zero.state_space_identity
assert mini_restart.source_plan_identity == mini.identity
assert mini_restart.source_request_identity == mini_one_sync.plan_key
assert mini_restart.source_trajectory_identity == mini_one_sync.trajectory.digest
assert mini_restart.source_kind == "result"
assert mini_restart.mesh is affine
assert mini_restart.field(mini.velocity_field).values("vertex").flags.writeable is False

fvm_zero = package.State.zero(fvm)
assert fvm_zero.mesh is cartesian
assert fvm_zero.field(fvm.velocity_field).associations == ("cell",)
assert fvm_zero.field(fvm.pressure_field).associations == ("cell",)
fvm_two = package.run(fvm, state=fvm_zero, steps=2, output_steps=(2,))
assert fvm_two.trajectory.plan_identity == fvm.identity
assert fvm_two.trajectory.realization_digest is None
assert fvm_two.trajectory.request_identity == fvm_two.plan_key
assert fvm_two.trajectory.run_digest == fvm_two.plan_key
fvm_first = package.run(fvm, state=fvm_zero, steps=1, output_steps=(1,))
fvm_restart = package.State.from_result(fvm, fvm_first, time_s=0.01)
fvm_second = package.run(fvm, state=fvm_restart, steps=1, output_steps=(1,))
assert fvm_two.trajectory.states[0] == fvm_second.trajectory.states[0]
alternate_scaling = package.fluid.IncompressibleScaling(length_m=2.0, velocity_m_per_s=4.0, pressure_pa=6.0)
fvm_alternate = package.resolve(
    model, mesh=cartesian, spatial=package.fvm.CellCentered(), solve=newton,
    scaling=alternate_scaling, temporal=temporal,
)
assert fvm_alternate.identity != fvm.identity
fvm_second_alternate = package.run(
    fvm_alternate, state=fvm_restart, steps=1, output_steps=(1,),
)
assert fvm_second_alternate.trajectory.states[0] == fvm_second.trajectory.states[0]

for invalid_time in (float("nan"), float("inf"), -1.0, -0.0):
    try:
        package.State.zero(mini, time_s=invalid_time)
    except (ValueError, package.ValidationError):
        pass
    else:
        raise AssertionError(f"invalid zero-State time was admitted: {invalid_time!r}")
for invalid_operation in range(6):
    try:
        if invalid_operation == 0:
            package.submit(fvm, state=mini_zero, steps=1, output_steps=(1,))
        elif invalid_operation == 1:
            package.State.from_result(fvm, mini_one_sync, time_s=0.01)
        elif invalid_operation == 2:
            package.submit(mini, state=mini_zero, steps=2, output_steps=(0,))
        elif invalid_operation == 3:
            package.submit(mini, state=mini_zero, steps=2, output_steps=(1, 1))
        elif invalid_operation == 4:
            package.submit(mini, state=mini_zero, steps=2, output_steps=(3,))
        else:
            package.submit(mini, state=mini_zero, until_s=0.015, output_times_s=(0.01,))
    except (ValueError, package.ValidationError):
        pass
    else:
        raise AssertionError("foreign State or invalid exact schedule was admitted")

for kwargs in (
    {},
    dict(state=mini_zero),
    dict(state=mini_zero, steps=1),
    dict(state=mini_zero, output_steps=(1,)),
    dict(state=mini_zero, steps=1, output_steps=(1,), until_s=0.01, output_times_s=(0.01,)),
):
    try:
        package.submit(mini, **kwargs)
    except TypeError:
        pass
    else:
        raise AssertionError(f"incomplete or mixed transient Run controls admitted: {kwargs}")
try:
    package.submit(model=model, end_time=0.01, max_step=0.01)
except TypeError:
    pass
else:
    raise AssertionError("legacy root submit form remained callable")
"#),
                None,
                Some(&locals),
            )
    })
}

#[test]
fn installed_root_common_elasticity_uses_exact_caller_mesh_and_field() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(crate::_eqiora)(py);
        let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bindings/python/python/eqiora")
            .canonicalize()?;
        let locals = PyDict::new(py);
        locals.set_item("native", native.bind(py))?;
        locals.set_item("package_directory", package_directory.to_string_lossy())?;
        locals.set_item("elasticity_source", ELASTICITY_COMPONENT)?;
        py.run(
                c_str!(r#"
import importlib.util, pathlib, sys
package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location("eqiora", package_path / "__init__.py", submodule_search_locations=[str(package_path)])
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)

graph = package.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
geometry = graph.build(rectangle, named_topology={
    "region": rectangle.region,
    "left": rectangle.boundaries[0],
    "right": rectangle.boundaries[1],
    "bottom": rectangle.boundaries[2],
    "top": rectangle.boundaries[3],
})
mesher = package.meshing.CartesianMesher(cells=(2, 3))
mesh_plan = package.meshing.resolve(geometry, mesher)
mesh = package.meshing.generate(geometry, plan=mesh_plan)
model = package.compile(
    source=elasticity_source,
    filename="elasticity.eqi",
    geometry=geometry,
    parameters={"mu": 3.0, "lambda": 0.0, "length_scale": 1.0},
)
replayed = package.replay(model.to_json())
alternate = package.compile(
    source=elasticity_source,
    filename="alternate-elasticity.eqi",
    geometry=geometry,
    parameters={"mu": 4.0, "lambda": 0.0, "length_scale": 1.0},
)
linear = package.solve.Linear(relative_tolerance=1e-10, absolute_tolerance=1e-12, maximum_iterations=10000)
plan = package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
replayed_plan = package.resolve(replayed, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
alternate_plan = package.resolve(alternate, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
assert plan.identity == replayed_plan.identity
assert alternate_plan.identity != plan.identity
assert alternate_plan.model_digest == alternate.digest
assert plan.model is model
assert plan.mesh is mesh
assert plan.field == model.field(plan.field.id)
assert plan.fields == (plan.field,)
assert plan.velocity_field is None and plan.pressure_field is None
assert plan.cells == (2, 3)
assert plan.spatial == package.fem.Q1()
assert plan.solve is linear
assert plan.scaling is None and plan.scaling_receipt is None
assert plan.temporal is None
assert plan.solver_algorithm == "conjugate-gradient"
assert plan.preconditioner == "identity"
assert plan.reduction == "reproducible"
assert plan.placement == "host-serial" and plan.workers == 1

result = package.run(plan)
elasticity_evidence = package.solid.linear_elasticity_evidence(result)
assert isinstance(elasticity_evidence, package.solid.LinearElasticityEvidence)
assert elasticity_evidence.plan_key == result.plan_key
assert elasticity_evidence.exact_bounds == ((0.0, 1.0), (0.0, 1.0))
output = result.output(plan.field)
assert output.field == plan.field
assert output.mesh is mesh
assert output.components == 2
assert output.vertex_count == 12
assert len(output.vertex_values) == 24
assert output.dimension == (0, 1, 0, 0, 0, 0, 0)
assert output.cell_bubble_values is None and output.cell_bubble_count == 0
assert result.mesh(plan.field) is mesh
assert package.submit(plan).result().output(plan.field).vertex_count == 12

load_potential_id = model.field_ids[0]
if load_potential_id == plan.field.id:
    load_potential_id = model.field_ids[1]
load_potential = model.field(load_potential_id)
try:
    result.output(load_potential)
except KeyError:
    pass
else:
    raise AssertionError("load potential escaped as an elasticity Result Field")
try:
    result.output("displacement")
except TypeError:
    pass
else:
    raise AssertionError("string Field lookup was admitted")
try:
    result.output(alternate_plan.field)
except ValueError:
    pass
else:
    raise AssertionError("foreign exact FieldRef was admitted")

foreign_rectangle = graph.rectangle(x_bounds=(0.0, 2.0), y_bounds=(0.0, 1.0))
foreign_geometry = graph.build(foreign_rectangle, named_topology={
    "region": foreign_rectangle.region,
    "left": foreign_rectangle.boundaries[0],
    "right": foreign_rectangle.boundaries[1],
    "bottom": foreign_rectangle.boundaries[2],
    "top": foreign_rectangle.boundaries[3],
})
foreign_mesh_plan = package.meshing.resolve(foreign_geometry, mesher)
foreign_mesh = package.meshing.generate(foreign_geometry, plan=foreign_mesh_plan)
triangle_plan = package.meshing.resolve(
    geometry,
    package.meshing.AffineTriangleMesher(cells=(2, 3)),
)
triangle_mesh = package.meshing.generate(geometry, plan=triangle_plan)
for kwargs in (
    dict(mesh=mesh, spatial=package.fvm.CellCenteredTpfa(), solve=linear),
    dict(mesh=triangle_mesh, spatial=package.fem.Q1(), solve=linear),
    dict(mesh=foreign_mesh, spatial=package.fem.Q1(), solve=linear),
    dict(mesh=mesh, spatial=package.fem.Q1(), solve=package.solve.Newton(linear=linear)),
    dict(mesh=mesh, spatial=package.fem.Q1(), solve=linear, temporal=package.time.BackwardEuler(0.1)),
    dict(mesh=mesh, spatial=package.fem.Q1(), solve=linear, scaling=package.fluid.IncompressibleScaling()),
):
    try:
        package.resolve(model, **kwargs)
    except package.ValidationError:
        pass
    else:
        raise AssertionError(f"elasticity cross-wire was admitted: {kwargs}")
"#),
                None,
                Some(&locals),
            )
    })
}
