use std::collections::BTreeMap;

use eqiora::api::ModelDocument;
use eqiora::geometry::{CanonicalGeometryV1, GeometryGraph, NamedEntitySet, PlanarTopologyHandle};
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
      - source_scale * math.sin(wave_number * coordinate(0))
        * math.sin(wave_number * coordinate(1)) = 0;
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
    let graph = GeometryGraph::new();
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
    scalar_document_from_source(geometry, source_scale, COMPONENT)
}

fn scalar_document_from_source(
    geometry: &CanonicalGeometryV1,
    source_scale: f64,
    component: &str,
) -> ModelDocument {
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
                DimExponents::from_integers([0, -1, 0, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "source_scale",
            DynQuantity::new(
                source_scale,
                DimExponents::from_integers([0, -2, 0, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
    ];
    ModelDocument::compile_external_component(
        "python-common-plan.eqi",
        component,
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
                DimExponents::from_integers([1, -1, -1, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "zero_pressure",
            DynQuantity::new(
                0.0,
                DimExponents::from_integers([1, -1, -2, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "inlet_speed",
            DynQuantity::new(
                inlet_speed,
                DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "channel_height",
            DynQuantity::new(
                0.41,
                DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension"),
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
                DimExponents::from_integers([1, -3, 0, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "dynamic_viscosity",
            DynQuantity::new(
                0.001,
                DimExponents::from_integers([1, -1, -1, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "zero_pressure",
            DynQuantity::new(
                0.0,
                DimExponents::from_integers([1, -1, -2, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "inlet_speed",
            DynQuantity::new(
                inlet_speed,
                DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
        ),
        (
            "channel_height",
            DynQuantity::new(
                0.41,
                DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension"),
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
import importlib.util, os, pathlib, sys, tempfile
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
mesh = package.meshing.generate(mesh_plan)
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
        let variable_coefficient = "(1 + wave_number * coordinate(0))";
        let variable_component = COMPONENT
            .replace(
                "-div(grad(potential))",
                &format!("-div({variable_coefficient} * grad(potential))"),
            )
            .replace(
                "relation top_value continuous on top { trace(potential) = 0; }",
                &format!(
                    "relation top_value continuous on top {{ normal({variable_coefficient} * grad(potential)) = wave_number; }}"
                ),
            );
        let variable_model = PyModel::from_document(
            py,
            scalar_document_from_source(
                &geometry,
                2.0 * std::f64::consts::PI.powi(2),
                &variable_component,
            ),
        )?;
        locals.set_item("model", Py::new(py, model)?)?;
        locals.set_item("fresh_model", Py::new(py, fresh_model)?)?;
        locals.set_item("foreign_model", Py::new(py, foreign_model)?)?;
        locals.set_item("variable_model", Py::new(py, variable_model)?)?;
        py.run(
                c_str!(r#"
linear = package.solve.Linear(relative_tolerance=1e-10, absolute_tolerance=1e-12, maximum_iterations=10000)
q1 = package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
q1_repeat = package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
q1_exact = package.resolve(
    model, mesh=mesh, spatial=package.fem.Q1(),
    formulation=package.formulation.PrimalGalerkin, solve=linear,
)
tpfa = package.resolve(model, mesh=mesh, spatial=package.fvm.CellCenteredTpfa(), solve=linear)
variable_q1 = package.resolve(variable_model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
variable_tpfa = package.resolve(variable_model, mesh=mesh, spatial=package.fvm.CellCenteredTpfa(), solve=linear)
q1_bytes = q1.to_bytes()
portable_q1 = package.Plan.from_bytes(q1_bytes)
plan_directory_owner = tempfile.TemporaryDirectory()
plan_directory = pathlib.Path(plan_directory_owner.name)
plan_path = plan_directory / "q1.eqplan"
plan_path.write_bytes(b"incomplete previous output")
q1.write(plan_path)
assert plan_path.read_bytes() == q1_bytes
assert not list(plan_directory.glob(".eqiora-plan-*.tmp"))
file_q1 = package.Plan.read(plan_path)
assert file_q1.identity == q1.identity
assert file_q1.to_bytes() == q1_bytes

for rejected_name, rejected_bytes in (
    ("truncated.eqplan", q1_bytes[:-1]),
    ("trailing.eqplan", q1_bytes + b"\n"),
    ("unknown-version.eqplan", q1_bytes.replace(b"resolved-common-plan/v2", b"resolved-common-plan/v9")),
):
    rejected_path = plan_directory / rejected_name
    rejected_path.write_bytes(rejected_bytes)
    try:
        package.Plan.read(rejected_path)
    except package.CompatibilityError:
        pass
    else:
        raise AssertionError(f"hostile Plan file must reject: {rejected_name}")

wrong_suffix = plan_directory / "q1.json"
try:
    q1.write(wrong_suffix)
except package.CompatibilityError:
    pass
else:
    raise AssertionError("Plan file paths require the exact .eqplan suffix")
try:
    package.Plan.read(wrong_suffix)
except package.CompatibilityError:
    pass
else:
    raise AssertionError("Plan file paths require the exact .eqplan suffix")
assert not wrong_suffix.exists()

directory_path = plan_directory / "directory.eqplan"
directory_path.mkdir()
try:
    q1.write(directory_path)
except package.CompatibilityError:
    pass
else:
    raise AssertionError("Plan file I/O must reject non-regular paths")
try:
    package.Plan.read(directory_path)
except package.CompatibilityError:
    pass
else:
    raise AssertionError("Plan file I/O must reject non-regular paths")

if os.name != "nt":
    symlink_target = plan_directory / "target.eqplan"
    symlink_target.write_bytes(q1_bytes)
    symlink_path = plan_directory / "symlink.eqplan"
    symlink_path.symlink_to(symlink_target)
    try:
        q1.write(symlink_path)
    except package.CompatibilityError:
        pass
    else:
        raise AssertionError("Plan file I/O must reject symlinks")
    try:
        package.Plan.read(symlink_path)
    except package.CompatibilityError:
        pass
    else:
        raise AssertionError("Plan file I/O must reject symlinks")
    assert symlink_target.read_bytes() == q1_bytes

oversized_path = plan_directory / "oversized.eqplan"
with oversized_path.open("wb") as oversized:
    oversized.truncate(256 * 1024 * 1024 + 1)
try:
    package.Plan.read(oversized_path)
except package.CompatibilityError:
    pass
else:
    raise AssertionError("Plan file I/O must reject input above the decoder bound")

missing_parent_path = plan_directory / "missing" / "q1.eqplan"
try:
    q1.write(missing_parent_path)
except package.CompatibilityError:
    pass
else:
    raise AssertionError("Plan publication must reject a missing staging directory")
assert not missing_parent_path.exists()
assert not list(plan_directory.glob(".eqiora-plan-*.tmp"))
replayed = package.Model.from_bytes(model.to_bytes())
replayed_plan = package.resolve(replayed, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
mesh_bytes = mesh.to_bytes()
replayed_mesh = package.meshing.Mesh.from_bytes(mesh_bytes)
replayed_mesh_plan = package.resolve(model, mesh=replayed_mesh, spatial=package.fem.Q1(), solve=linear)
fresh_plan = package.resolve(fresh_model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
assert q1.model is model
assert q1.mesh is mesh
assert q1.mesh_digest == mesh.digest
assert q1.identity == q1_repeat.identity
assert portable_q1.identity == q1.identity
assert portable_q1.to_bytes() == q1_bytes
assert portable_q1.model.to_bytes() == model.to_bytes()
assert portable_q1.mesh.to_bytes() == mesh.to_bytes()
assert portable_q1.spatial == package.fem.Q1()
assert portable_q1.requested_solve.relative_tolerance == linear.relative_tolerance
assert portable_q1.requested_solve.absolute_tolerance == linear.absolute_tolerance
assert portable_q1.requested_solve.maximum_iterations == linear.maximum_iterations
assert portable_q1.solve.algorithm == q1.solve.algorithm
assert replayed_plan.identity == q1.identity
assert replayed_mesh.digest == mesh.digest
assert replayed_mesh.correspondence_digest == mesh.correspondence_digest
assert replayed_mesh.production_lineage_digest == mesh.production_lineage_digest
assert replayed_mesh.to_bytes() == mesh_bytes
assert replayed_mesh_plan.identity == q1.identity
try:
    package.meshing.Mesh.from_bytes(mesh_bytes + b"\n")
except package.ValidationError:
    pass
else:
    raise AssertionError("noncanonical Mesh bytes must reject")
try:
    package.Plan.from_bytes(q1_bytes + b"\n")
except package.ValidationError:
    pass
else:
    raise AssertionError("noncanonical Plan bytes must reject")
assert fresh_model.digest != model.digest
assert fresh_plan.model_digest == fresh_model.digest
assert fresh_plan.identity != q1.identity
assert q1.identity != tpfa.identity
assert len(q1.realization_digest) == 64 and len(tpfa.realization_digest) == 64
assert q1.realization_digest == replayed_plan.realization_digest
assert q1.realization_digest != tpfa.realization_digest
assert q1.mesh.cells.shape == (6, 4)
assert isinstance(q1.capability, package.ScalarPlanView)
assert q1.capability.coefficient_sampling == "quadrature-point"
assert q1.capability.face_coefficient_policy == "not-applicable"
assert tpfa.capability.coefficient_sampling == "facet-centroid"
assert tpfa.capability.face_coefficient_policy == "direct-centroid-evaluation"
assert isinstance(q1.formulation, package.FormulationView)
assert q1.formulation.requested is package.FormulationSelectionMode.Automatic
assert q1.formulation.effective is package.formulation.PrimalGalerkin
assert q1.formulation.boundary_treatment == "complete-homogeneous-essential"
assert q1.formulation.rule_ids == [
    "fem.derive.v1.test-pairing",
    "fem.derive.v1.divergence-by-parts",
    "fem.derive.v1.boundary-discharge.essential-homogeneous",
    "fem.derive.v1.source-pairing",
]
assert q1.formulation.selection_reason_codes == [
    "eqiora.formulation.auto.primal-galerkin-for-q1/v1",
]
assert q1_exact.formulation.requested is package.FormulationSelectionMode.Exact
assert q1_exact.formulation.effective is q1.formulation.effective
assert q1_exact.identity != q1.identity
assert q1_exact.realization_digest == q1.realization_digest
assert tpfa.formulation is None
assert variable_q1.formulation is None
for label, wrong_spatial, wrong_formulation in (
    ("Q1 mixed", package.fem.Q1(), package.formulation.MixedGalerkin),
    ("TPFA primal", package.fvm.CellCenteredTpfa(), package.formulation.PrimalGalerkin),
):
    try:
        package.resolve(
            model, mesh=mesh, spatial=wrong_spatial,
            formulation=wrong_formulation, solve=linear,
        )
    except package.ValidationError:
        pass
    else:
        raise AssertionError(f"incompatible scalar Formulation must reject: {label}")
try:
    package.resolve(
        variable_model, mesh=mesh, spatial=package.fem.Q1(),
        formulation=package.formulation.PrimalGalerkin, solve=linear,
    )
except package.ValidationError:
    pass
else:
    raise AssertionError("exact primal Formulation must reject an unproved natural boundary")
assert not hasattr(q1.capability, "scaling")
assert q1.requested_solve is linear
assert q1.solve.algorithm == "conjugate-gradient"
q1_result = package.run(q1)
q1_result_bytes = q1_result.to_bytes()
replayed_q1_result = package.Result.from_bytes(q1, q1_result_bytes)
assert replayed_q1_result.to_bytes() == q1_result_bytes
assert replayed_q1_result.output(q1.capability.field).values("vertex").numpy().tolist() == q1_result.output(q1.capability.field).values("vertex").numpy().tolist()
portable_q1_result = package.run(portable_q1)
file_q1_result = package.run(file_q1)
tpfa_result = package.run(tpfa)
variable_q1_result = package.run(variable_q1)
variable_tpfa_result = package.run(variable_tpfa)
assert variable_q1_result.output(variable_q1.capability.field).coefficient_count("vertex") == mesh.vertex_count
assert variable_tpfa_result.output(variable_tpfa.capability.field).coefficient_count("cell") == mesh.cell_count
assert q1_result.model_digest == q1.model_digest
assert q1_result.plan_key == q1.identity
assert q1_result.mesh(q1.capability.field) is mesh
q1_output = q1_result.output(q1.capability.field)
assert q1_output.associations == ("vertex",)
assert q1_output.logical_shape("vertex") == (3, 4)
assert len(q1_output.values("vertex")) == 12
assert portable_q1_result.output(portable_q1.capability.field).logical_shape("vertex") == (3, 4)
assert file_q1_result.output(file_q1.capability.field).logical_shape("vertex") == (3, 4)
plan_directory_owner.cleanup()
assert not hasattr(q1_result, "run_manifest")
tpfa_output = tpfa_result.output(tpfa.capability.field)
assert tpfa_output.associations == ("cell",)
assert tpfa_output.logical_shape("cell") == (2, 3)
assert len(tpfa_output.values("cell")) == 6
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
foreign_mesh = package.meshing.generate(foreign_plan)
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
mesh = package.meshing.generate(mesh_plan)
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
replayed = package.Model.from_bytes(model.to_bytes())
replayed_plan = package.resolve(replayed, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
replayed_mesh = package.meshing.Mesh.from_bytes(mesh.to_bytes())
replayed_mesh_plan = package.resolve(model, mesh=replayed_mesh, spatial=package.fem.MiniP1(), solve=linear)
fresh_plan = package.resolve(fresh_model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
assert plan.identity == explicit_none.identity == all_auto.identity == replayed_plan.identity == fresh_plan.identity
assert replayed_mesh_plan.identity == plan.identity
assert replayed_mesh.to_bytes() == mesh.to_bytes()
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
assert plan.capability.scaling.length_m == 0.41
assert plan.capability.scaling.velocity_m_per_s == 0.3
assert plan.capability.scaling.pressure_pa == 0.001 * 0.3 / 0.41
assert partial_changed.capability.scaling.length_m == 0.82
assert partial_changed.capability.scaling.velocity_m_per_s == 0.3
assert partial_changed.capability.scaling.pressure_pa == 0.001 * 0.3 / 0.82
assert partial_equal.capability.scaling.pressure_pa == plan.capability.scaling.pressure_pa
assert manual_equal.capability.scaling.pressure_pa == plan.capability.scaling.pressure_pa
receipt = plan.capability.scaling_receipt
partial_receipt = partial_equal.capability.scaling_receipt
manual_receipt = manual_equal.capability.scaling_receipt
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
    plan.capability.scaling.length_m = 1.0
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
assert plan.spatial.method == "mini-p1"
assert plan.solve.backend == "eqiora.faer"
assert plan.requested_solve is linear
assert plan.solve.algorithm == "sparse-lu"
assert plan.solve.operator == "symmetric-indefinite"
assert plan.solve.preconditioner == "identity"
assert plan.solve.reduction == "fast"
assert plan.solve.backend == "eqiora.faer"
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
assert plan.fields == (plan.capability.velocity, plan.capability.pressure)
assert isinstance(plan.capability, package.fluid.IncompressibleFlowPlanView)
assert not hasattr(result, "outputs")
velocity = result.output(plan.capability.velocity)
pressure = result.output(plan.capability.pressure)
assert result.mesh(plan.capability.velocity) is mesh
assert result.mesh(plan.capability.pressure) is mesh
assert velocity.field == plan.capability.velocity
assert pressure.field == plan.capability.pressure
assert velocity.mesh is pressure.mesh is mesh
assert velocity.dimension == (0, 1, -1, 0, 0, 0, 0)
assert velocity.value_shape == (2,)
assert velocity.associations == ("vertex", "cell-bubble")
assert velocity.coefficient_count("vertex") == mesh.vertex_count
assert len(velocity.values("vertex")) == mesh.vertex_count * 2
assert velocity.coefficient_count("cell-bubble") == mesh.cell_count
assert len(velocity.values("cell-bubble")) == mesh.cell_count * 2
assert pressure.dimension == (1, -1, -2, 0, 0, 0, 0)
assert pressure.value_shape == ()
assert pressure.associations == ("vertex",)
assert pressure.coefficient_count("vertex") == mesh.vertex_count
assert len(pressure.values("vertex")) == mesh.vertex_count
cylinder = source.selection("cylinder")
inlet = source.selection("inlet")
outlet = source.selection("outlet")
cylinder_force = result.boundary_force(cylinder)
inlet_flux = result.boundary_flux(inlet)
outlet_flux = result.boundary_flux(outlet)
assert cylinder_force.selection == cylinder
assert cylinder_force.source_digest == result.plan_key
assert cylinder_force.source_kind == "result"
assert cylinder_force.on_domain == stokes_evidence.cylinder_force_on_fluid
assert inlet_flux.selection == inlet and outlet_flux.selection == outlet
assert inlet_flux.value == stokes_evidence.inlet_flux
assert outlet_flux.value == stokes_evidence.outlet_flux
assert inlet_flux.value + outlet_flux.value == stokes_evidence.net_flux
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
mesh = package.meshing.generate(package.meshing.resolve(source, mesher))
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
steady_velocity = steady_result.output(steady_plan.capability.velocity)
steady_pressure = steady_result.output(steady_plan.capability.pressure)
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
            plan.capability.velocity,
            vertex_values=np.asarray(steady_velocity.values("vertex")).reshape(mesh.vertex_count, 2),
            cell_values=np.asarray(steady_velocity.values("cell-bubble")).reshape(mesh.cell_count, 2),
        ),
        package.InitialField(
            plan.capability.pressure,
            vertex_values=np.asarray(steady_pressure.values("vertex")),
        ),
    ),
)
assert plan.model is model and plan.mesh is mesh
assert plan.capability.pressure_gauge is package.fluid.PressureGauge2d.BoundaryTraction
assert np.max(np.abs(np.asarray(steady_velocity.values("vertex")))) > 0.0
assert state.field(plan.capability.velocity).associations == ("vertex", "cell")
result = package.run(plan, state=state, steps=1, output_steps=(1,))
result_bytes = result.to_bytes()
replayed_result = package.Result.from_bytes(plan, result_bytes)
assert replayed_result.to_bytes() == result_bytes
assert replayed_result.trajectory.states[0].digest == result.trajectory.states[0].digest
assert len(result.trajectory.states) == 1
wake_state = result.trajectory.states[0]
assert wake_state.time_s == 0.0001
assert np.max(np.abs(wake_state.field(plan.capability.velocity).values("vertex"))) > 0.0
vorticity = wake_state.curl(plan.capability.velocity)
assert vorticity.operator == "curl"
assert vorticity.source_state_digest == wake_state.digest
assert vorticity.source_field == plan.capability.velocity
assert vorticity.mesh_digest == mesh.digest
assert vorticity.support_domain_id == wake_state.field(plan.capability.velocity).support_domain_id
assert vorticity.dimension == (0, 0, -1, 0, 0, 0, 0)
assert vorticity.value_shape == ()
assert vorticity.frame == "spatial-axial"
assert vorticity.associations == ("cell",)
assert np.array_equal(vorticity.support_indices("cell"), np.arange(mesh.cell_count, dtype=np.uint32))
assert np.all(np.isfinite(vorticity.values("cell")))
assert np.max(np.abs(vorticity.values("cell"))) > 0.0
assert not vorticity.values("cell").flags.writeable
assert wake_state.curl(plan.capability.velocity) == vorticity
front_pressure = wake_state.sample(plan.capability.pressure, at=(0.15, 0.2))
rear_pressure = wake_state.sample(plan.capability.pressure, at=(0.25, 0.2))
assert front_pressure.source_state_digest == wake_state.digest
assert front_pressure.field == plan.capability.pressure
assert front_pressure.mesh_digest == mesh.digest
assert front_pressure.support_domain_id == wake_state.field(plan.capability.pressure).support_domain_id
assert front_pressure.point_m == (0.15, 0.2)
assert front_pressure.dimension == (1, -1, -2, 0, 0, 0, 0)
assert front_pressure.frame == "invariant"
assert np.isfinite(front_pressure.value) and np.isfinite(rear_pressure.value)
assert wake_state.sample(plan.capability.pressure, at=(0.15, 0.2)) == front_pressure
cylinder = source.selection("cylinder")
cylinder_force = wake_state.boundary_force(cylinder)
assert cylinder_force.source_digest == wake_state.digest
assert cylinder_force.source_kind == "state"
assert cylinder_force.selection == cylinder
assert cylinder_force.geometry_digest == source.digest
assert cylinder_force.mesh_digest == mesh.digest
assert cylinder_force.dimension == (1, 0, -2, 0, 0, 0, 0)
assert cylinder_force.frame == "spatial-cartesian"
assert np.all(np.isfinite(cylinder_force.on_domain))
assert cylinder_force.on_selection == tuple(-value for value in cylinder_force.on_domain)
assert wake_state.boundary_force(cylinder) == cylinder_force
try:
    wake_state.curl(plan.capability.pressure)
except ValueError as error:
    assert "velocity FieldRef" in str(error)
else:
    raise AssertionError("curl accepted the pressure Field")
for invalid_sample in range(3):
    try:
        if invalid_sample == 0:
            wake_state.sample(plan.capability.velocity, at=(0.15, 0.2))
        elif invalid_sample == 1:
            wake_state.sample(plan.capability.pressure, at=(3.0, 3.0))
        else:
            wake_state.sample(plan.capability.pressure, at=(True, 0.2))
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
import importlib.util, pathlib, sys, tempfile
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
affine = package.meshing.generate(affine_plan)
cartesian_plan = package.meshing.resolve(source, package.meshing.CartesianMesher(cells=(4, 4)))
cartesian = package.meshing.generate(cartesian_plan)
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
mini_bytes = mini.to_bytes()
portable_mini = package.Plan.from_bytes(mini_bytes)
fvm_bytes = fvm.to_bytes()
portable_fvm = package.Plan.from_bytes(fvm_bytes)
planned = {}
for name, objective in (
    ("robust", package.solve.Robust),
    ("fast", package.solve.Fast),
    ("low-memory", package.solve.LowMemory),
):
    planned_linear = package.solve.Linear(
        relative_tolerance=1e-10,
        absolute_tolerance=1e-12,
        maximum_iterations=2000,
        objective=objective,
    )
    planned[name] = package.resolve(
        model,
        mesh=cartesian,
        spatial=package.fvm.CellCentered(),
        solve=package.solve.Newton(linear=planned_linear),
        scaling=scaling,
        temporal=temporal,
    )
mini_exact = package.resolve(
    model, mesh=affine, spatial=package.fem.MiniP1(),
    formulation=package.formulation.MixedGalerkin,
    solve=newton, scaling=scaling, temporal=temporal,
)
fvm_exact = package.resolve(
    model, mesh=cartesian, spatial=package.fvm.CellCentered(),
    formulation=package.formulation.IntegralConservative,
    solve=newton, scaling=scaling, temporal=temporal,
)
replayed = package.resolve(package.Model.from_bytes(model.to_bytes()), mesh=affine, spatial=package.fem.MiniP1(), solve=newton, scaling=scaling, temporal=temporal)
custom = package.resolve(model, mesh=affine, spatial=package.fem.MiniP1(), solve=custom_newton, scaling=scaling, temporal=temporal)
assert mini.identity == replayed.identity
assert portable_mini.identity == mini.identity
assert portable_fvm.identity == fvm.identity
assert portable_mini.to_bytes() == mini_bytes
assert portable_fvm.to_bytes() == fvm_bytes
assert portable_mini.mesh.to_bytes() == affine.to_bytes()
assert portable_fvm.mesh.to_bytes() == cartesian.to_bytes()
assert portable_mini.spatial == package.fem.MiniP1()
assert portable_fvm.spatial == package.fvm.CellCentered()
assert portable_mini.temporal.step_s == mini.temporal.step_s
assert portable_mini.requested_solve.relative_tolerance == newton.relative_tolerance
assert portable_mini.requested_solve.linear.maximum_iterations == linear.maximum_iterations
assert portable_mini.solve.maximum_iterations == mini.solve.maximum_iterations
assert portable_mini.capability.scaling.length_m == mini.capability.scaling.length_m
assert mini.identity != fvm.identity
assert mini.identity != custom.identity
assert mini.model is model and mini.mesh is affine
assert fvm.model is model and fvm.mesh is cartesian
assert mini.fields == (mini.capability.velocity, mini.capability.pressure)
assert fvm.fields == (fvm.capability.velocity, fvm.capability.pressure)
assert mini.solve is not newton and mini.solve.linear is not linear
assert mini.requested_solve is newton
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
assert len(mini.realization_digest) == 64 and len(fvm.realization_digest) == 64
assert mini.realization_digest != fvm.realization_digest
assert mini.capability.velocity_space == "simplex-p1-bubble"
assert mini.capability.pressure_space == "continuous-lagrange-p1"
assert fvm.capability.velocity_space == fvm.capability.pressure_space == "cell-constant"
assert mini.capability.pressure_gauge is package.fluid.PressureGauge2d.ZeroIntegral
assert fvm.capability.pressure_gauge is package.fluid.PressureGauge2d.ZeroIntegral
assert isinstance(mini.formulation, package.FormulationView)
assert mini.formulation.requested is package.FormulationSelectionMode.Automatic
assert mini.formulation.effective is package.formulation.MixedGalerkin
assert mini.formulation.boundary_treatment == "explicit-trace-flux-laws"
assert len(mini.formulation.rule_ids) == 6
assert mini.formulation.selection_reason_codes == [
    "eqiora.formulation.auto.mixed-galerkin-for-mini-p1/v1",
]
assert fvm.formulation.requested is package.FormulationSelectionMode.Automatic
assert fvm.formulation.effective is package.formulation.IntegralConservative
assert fvm.formulation.boundary_treatment == "explicit-trace-flux-laws"
assert len(fvm.formulation.rule_ids) == 7
assert fvm.formulation.selection_reason_codes == [
    "eqiora.formulation.auto.integral-conservative-for-cell-centered-fvm/v1",
]
assert mini_exact.formulation.requested is package.FormulationSelectionMode.Exact
assert fvm_exact.formulation.requested is package.FormulationSelectionMode.Exact
assert mini_exact.formulation.effective is mini.formulation.effective
assert fvm_exact.formulation.effective is fvm.formulation.effective
assert mini_exact.identity != mini.identity
assert fvm_exact.identity != fvm.identity
for wrong_mesh, wrong_spatial, wrong_formulation in (
    (affine, package.fem.MiniP1(), package.formulation.IntegralConservative),
    (cartesian, package.fvm.CellCentered(), package.formulation.MixedGalerkin),
):
    try:
        package.resolve(
            model, mesh=wrong_mesh, spatial=wrong_spatial,
            formulation=wrong_formulation,
            solve=newton, scaling=scaling, temporal=temporal,
        )
    except package.ValidationError:
        pass
    else:
        raise AssertionError("incompatible exact Formulation was admitted")
assert mini.solve.linear.algorithm == "sparse-lu"
assert fvm.solve.linear.algorithm == "bicgstab"
assert mini.solve.linear.reduction == "fast" and fvm.solve.linear.reduction == "reproducible"
assert mini.solve.linear.backend == "eqiora.faer"
assert fvm.solve.linear.backend == "eqiora.reference"
expected_planning = {
    "robust": (
        package.solve.Robust,
        "eqiora.reference.bicgstab-general-jacobi-reproducible-f64",
        "eqiora.reference",
        "bicgstab",
        "reproducible",
    ),
    "fast": (
        package.solve.Fast,
        "eqiora.faer.sparse-lu-general-identity-fast-f64",
        "eqiora.faer",
        "sparse-lu",
        "fast",
    ),
    "low-memory": (
        package.solve.LowMemory,
        "eqiora.faer.bicgstab-general-jacobi-fast-f64",
        "eqiora.faer",
        "bicgstab",
        "fast",
    ),
}
for name, plan in planned.items():
    objective, candidate, backend, algorithm, reduction = expected_planning[name]
    resolved = plan.solve.linear
    assert resolved.objective is objective
    assert resolved.planning_policy_id == "eqiora.host-serial-solver-planning/v1"
    assert resolved.selected_candidate_id == candidate
    assert resolved.selected_evidence_case is not None
    assert len(resolved.planning_reasons) == 6
    assert resolved.backend == backend
    assert resolved.algorithm == algorithm
    assert resolved.reduction == reduction
    assert plan.identity != fvm.identity
assert len({plan.identity for plan in planned.values()}) == 3
try:
    package.resolve(
        model,
        mesh=affine,
        spatial=package.fem.MiniP1(),
        solve=package.solve.Newton(
            linear=package.solve.Linear(
                relative_tolerance=1e-10,
                absolute_tolerance=1e-12,
                maximum_iterations=2000,
                objective=package.solve.Robust,
            ),
        ),
        scaling=scaling,
        temporal=temporal,
    )
except package.ValidationError:
    pass
else:
    raise AssertionError("program-controlled MINI/P1 request was admitted")
assert mini.capability.scaling.length_m == 1.0 and mini.capability.scaling.velocity_m_per_s == 2.0 and mini.capability.scaling.pressure_pa == 3.0

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
mini_zero_bytes = mini_zero.to_bytes()
mini_zero_replayed = package.State.from_bytes(mini, mini_zero_bytes)
assert mini_zero_replayed == mini_zero
assert mini_zero_replayed.to_bytes() == mini_zero_bytes
assert mini_zero_replayed.source_kind == "artifact"
assert mini_zero.mesh is affine
assert mini_zero.model is model
assert mini_zero.source_plan_identity == mini.identity
assert mini_zero.source_request_identity is None
assert mini_zero.source_trajectory_identity is None
assert mini_zero.source_kind == "zero"
assert len(mini_zero.fields) == 2
assert mini_zero.field(mini.capability.velocity).associations == ("vertex", "cell")
assert mini_zero.field(mini.capability.pressure).associations == ("vertex",)

mini_one_sync = package.run(mini, state=mini_zero, steps=1, output_steps=(1,))
mini_one_async = package.submit(mini, state=mini_zero, steps=1, output_steps=(1,)).result()
assert mini_one_sync.trajectory.digest == mini_one_async.trajectory.digest
mini_one_time = package.run(mini, state=mini_zero, until_s=0.01, output_times_s=(0.01,))
assert mini_one_sync.trajectory.digest == mini_one_time.trajectory.digest
mini_two = package.run(mini, state=mini_zero, steps=2, output_steps=(1, 2))
assert tuple(state.step for state in mini_two.trajectory.states) == (1, 2)
assert tuple(state.time_s for state in mini_two.trajectory.states) == (0.01, 0.02)
mini_trajectory_bytes = mini_two.trajectory.to_bytes()
mini_trajectory_replayed = package.trajectory.Trajectory.from_bytes(mini, mini_trajectory_bytes)
assert mini_trajectory_replayed == mini_two.trajectory
assert mini_trajectory_replayed.to_bytes() == mini_trajectory_bytes
assert tuple(state.digest for state in mini_trajectory_replayed.states) == tuple(
    state.digest for state in mini_two.trajectory.states
)
trajectory_directory_owner = tempfile.TemporaryDirectory()
trajectory_directory = pathlib.Path(trajectory_directory_owner.name)
trajectory_path = trajectory_directory / "run.eqtrajectory"
mini_two.trajectory.write(trajectory_path)
assert trajectory_path.read_bytes() == mini_trajectory_bytes
mini_trajectory_file = package.trajectory.Trajectory.read(mini, trajectory_path)
assert mini_trajectory_file == mini_two.trajectory
assert mini_trajectory_file.to_bytes() == mini_trajectory_bytes
assert tuple(state.digest for state in mini_trajectory_file.states) == tuple(
    state.digest for state in mini_two.trajectory.states
)

mini_result_bytes = mini_two.to_bytes()
mini_result_path = trajectory_directory / "run.eqresult"
mini_two.write(mini_result_path)
mini_result_file = package.Result.read(mini, mini_result_path)
assert mini_result_path.read_bytes() == mini_result_bytes
assert mini_result_file.to_bytes() == mini_result_bytes
assert mini_result_file.trajectory.to_bytes() == mini_trajectory_bytes

for name, rejected in (
    ("truncated.eqtrajectory", mini_trajectory_bytes[:-1]),
    ("trailing.eqtrajectory", mini_trajectory_bytes + b"\n"),
    (
        "unknown-version.eqtrajectory",
        mini_trajectory_bytes.replace(b"common-trajectory/v1", b"common-trajectory/v9"),
    ),
):
    rejected_path = trajectory_directory / name
    rejected_path.write_bytes(rejected)
    try:
        package.trajectory.Trajectory.read(mini, rejected_path)
    except package.CompatibilityError:
        pass
    else:
        raise AssertionError(f"hostile Trajectory file must reject: {name}")

wrong_trajectory_suffix = trajectory_directory / "run.json"
for operation in ("write", "read"):
    try:
        if operation == "write":
            mini_two.trajectory.write(wrong_trajectory_suffix)
        else:
            package.trajectory.Trajectory.read(mini, wrong_trajectory_suffix)
    except package.CompatibilityError:
        pass
    else:
        raise AssertionError("Trajectory file paths require the exact .eqtrajectory suffix")

try:
    package.trajectory.Trajectory.read(fvm, trajectory_path)
except package.CompatibilityError:
    pass
else:
    raise AssertionError("Trajectory file was crossed with a different Plan")
trajectory_directory_owner.cleanup()
mini_restart = package.State.from_result(custom, mini_one_sync, time_s=0.01)
assert mini_restart.state_space_identity == mini_zero.state_space_identity
assert mini_restart.source_plan_identity == mini.identity
assert mini_restart.source_request_identity == mini_one_sync.plan_key
assert mini_restart.source_trajectory_identity == mini_one_sync.trajectory.digest
assert mini_restart.source_kind == "result"
assert mini_restart.mesh is affine
assert mini_restart.field(mini.capability.velocity).values("vertex").flags.writeable is False

fvm_zero = package.State.zero(fvm)
fvm_zero_bytes = fvm_zero.to_bytes()
fvm_zero_replayed = package.State.from_bytes(fvm, fvm_zero_bytes)
assert fvm_zero_replayed == fvm_zero
assert fvm_zero_replayed.to_bytes() == fvm_zero_bytes
assert fvm_zero_replayed.source_kind == "artifact"
assert fvm_zero.mesh is cartesian
assert fvm_zero.field(fvm.capability.velocity).associations == ("cell",)
assert fvm_zero.field(fvm.capability.pressure).associations == ("cell",)
for plan in planned.values():
    result = package.run(
        plan,
        state=package.State.zero(plan),
        steps=1,
        output_steps=(1,),
    )
    assert len(result.trajectory.states) == 1
fvm_two = package.run(fvm, state=fvm_zero, steps=2, output_steps=(2,))
assert fvm_two.trajectory.plan_identity == fvm.identity
assert fvm_two.trajectory.realization_digest == fvm.realization_digest
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
mesh = package.meshing.generate(mesh_plan)
model = package.compile(
    source=elasticity_source,
    filename="elasticity.eqi",
    geometry=geometry,
    parameters={"mu": 3.0, "lambda": 0.0, "length_scale": 1.0},
)
replayed = package.Model.from_bytes(model.to_bytes())
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
assert len(plan.realization_digest) == 64
assert plan.realization_digest == replayed_plan.realization_digest
assert alternate_plan.realization_digest == plan.realization_digest
assert plan.model is model
assert plan.mesh is mesh
assert isinstance(plan.capability, package.solid.ElasticityPlanView)
assert plan.capability.displacement == model.field(plan.capability.displacement.id)
assert plan.fields == (plan.capability.displacement,)
assert plan.mesh.cells.shape == (6, 4)
assert plan.spatial == package.fem.Q1()
assert plan.requested_solve is linear
assert plan.solve.algorithm == "conjugate-gradient"
assert not hasattr(plan.capability, "scaling")
assert plan.temporal is None
assert plan.solve.preconditioner == "identity"
assert plan.solve.reduction == "reproducible"
assert plan.execution.placement == "host-serial" and plan.execution.workers == 1

result = package.run(plan)
result_bytes = result.to_bytes()
replayed_result = package.Result.from_bytes(plan, result_bytes)
assert replayed_result.to_bytes() == result_bytes
assert replayed_result.output(plan.capability.displacement).values("vertex").numpy().tolist() == result.output(plan.capability.displacement).values("vertex").numpy().tolist()
elasticity_evidence = package.solid.linear_elasticity_evidence(result)
assert isinstance(elasticity_evidence, package.solid.LinearElasticityEvidence)
assert elasticity_evidence.plan_key == result.plan_key
assert elasticity_evidence.exact_bounds == ((0.0, 1.0), (0.0, 1.0))
output = result.output(plan.capability.displacement)
assert output.field == plan.capability.displacement
assert output.mesh is mesh
assert output.value_shape == (2,)
assert output.coefficient_count("vertex") == 12
assert len(output.values("vertex")) == 24
assert output.dimension == (0, 1, 0, 0, 0, 0, 0)
assert output.associations == ("vertex",)
assert result.mesh(plan.capability.displacement) is mesh
assert package.submit(plan).result().output(plan.capability.displacement).coefficient_count("vertex") == 12

load_potential_id = model.field_ids[0]
if load_potential_id == plan.capability.displacement.id:
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
    result.output(alternate_plan.capability.displacement)
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
foreign_mesh = package.meshing.generate(foreign_mesh_plan)
triangle_plan = package.meshing.resolve(
    geometry,
    package.meshing.AffineTriangleMesher(cells=(2, 3)),
)
triangle_mesh = package.meshing.generate(triangle_plan)
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
