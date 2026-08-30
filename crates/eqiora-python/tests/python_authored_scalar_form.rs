use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyModule};

const SOURCE: &str = r#"
public component AuthoredPoisson {
  public support square: volume(ambient_dimension = 2);
  public support x_lower: boundary(parent = square);
  public support x_upper: boundary(parent = square);
  public support y_lower: boundary(parent = square);
  public support y_upper: boundary(parent = square);
  public parameter diffusion: 1;
  public parameter other_diffusion: 1;
  public parameter source_scale: 1 / m ^ 2;
  public parameter other_source: 1 / m ^ 2;
  representation scalar_space = continuum;
  field potential on square as scalar_space: 1 = 0;
  relation balance continuous on square {
    -div(diffusion * grad(potential)) = source_scale;
  }
  relation x_lower_value continuous on x_lower { trace(potential) = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) = 0; }
  form primal for balance {
    integrate(square, dot(grad(test(potential)), diffusion * grad(potential)))
      = integrate(square, test(potential) * source_scale);
  }
}
"#;

#[test]
fn python_authored_scalar_form_closes_compile_resolve_run_and_plan_replay() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("source", SOURCE)?;
        py.run(
            c_str!(r#"
graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
geometry = graph.build(rectangle, named_topology={
    "square": rectangle.region,
    "x_lower": rectangle.boundaries[0],
    "x_upper": rectangle.boundaries[1],
    "y_lower": rectangle.boundaries[2],
    "y_upper": rectangle.boundaries[3],
})
parameters = {"diffusion": 1.0, "other_diffusion": 2.0, "source_scale": 1.0, "other_source": 3.0}
model = eqiora.compile(source=source, geometry=geometry, parameters=parameters)
mesh_plan = eqiora.meshing.resolve(geometry, eqiora.meshing.CartesianMesher(cells=(3, 3)))
mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)
linear = eqiora.solve.Linear(relative_tolerance=1e-10, absolute_tolerance=1e-12, maximum_iterations=1000)
plan = eqiora.resolve(model, mesh=mesh, spatial=eqiora.fem.Q1(), solve=linear)
assert plan.formulation.requested == eqiora.FormulationSelectionMode.Authored
assert plan.formulation.requested_source_identity == model.authored_formulations[0].source_identity
result = eqiora.run(plan)
assert result.plan_key == plan.identity

plan_bytes = plan.to_bytes()
replayed = eqiora.Plan.from_bytes(plan_bytes)
assert replayed.to_bytes() == plan_bytes
assert replayed.identity == plan.identity
assert replayed.formulation.requested == eqiora.FormulationSelectionMode.Authored
assert replayed.formulation.requested_source_identity == plan.formulation.requested_source_identity
assert eqiora.run(replayed).plan_key == plan.identity
try:
    eqiora.Plan.from_bytes(plan_bytes.replace(b"resolved-common-plan/v2", b"resolved-common-plan/v1"))
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("superseded Plan v1 schema was accepted")

plain = eqiora.Model.from_bytes(model.to_bytes())
plain_plan = eqiora.resolve(plain, mesh=mesh, spatial=eqiora.fem.Q1(), solve=linear)
assert plain_plan.formulation.requested == eqiora.FormulationSelectionMode.Automatic
assert plain_plan.identity != plan.identity

for changed, expected in (
    (source.replace("diffusion * grad(potential)))", "other_diffusion * grad(potential)))"), "coefficient"),
    (source.replace("test(potential) * source_scale", "test(potential) * other_source"), "source"),
    (source.replace("test(potential) * source_scale", "-test(potential) * source_scale"), "source term"),
    (source.replace("trace(potential) = 0;", "trace(potential) = 1;", 1), "homogeneous-essential"),
):
    mismatched = eqiora.compile(source=changed, geometry=geometry, parameters=parameters)
    try:
        eqiora.resolve(mismatched, mesh=mesh, spatial=eqiora.fem.Q1(), solve=linear)
    except eqiora.ValidationError as error:
        assert expected in str(error), str(error)
    else:
        raise AssertionError(f"mismatched authored Formulation was accepted: {expected}")
"#),
            None,
            Some(&locals),
        )
    })
}

fn public_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
    let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/python/python/eqiora")
        .canonicalize()?;
    let locals = PyDict::new(py);
    locals.set_item("native", native.bind(py))?;
    locals.set_item("package_directory", package_directory.to_string_lossy())?;
    py.run(
        c_str!(
            r#"
import importlib.util
import pathlib
import sys

package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location(
    "eqiora",
    package_path / "__init__.py",
    submodule_search_locations=[str(package_path)],
)
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)
"#
        ),
        None,
        Some(&locals),
    )?;
    Ok(locals
        .get_item("package")?
        .expect("public package must load")
        .cast_into::<PyModule>()?)
}
