use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

#[test]
fn python_result_observations_retain_types_rules_and_exact_state_lineage() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let locals = PyDict::new(py);
        locals.set_item("eqiora", public_module(py)?)?;
        py.run(c_str!(r#"
import math
linear = eqiora.solve.Linear(algorithm=eqiora.solve.LinearSolver.BiConjugateGradientStabilized, preconditioner=eqiora.solve.Preconditioner.Identity, reduction=eqiora.solve.Reduction.Reproducible, provider=eqiora.solve.SolverProvider.reference(), relative_tolerance=1e-12, absolute_tolerance=1e-14, maximum_iterations=100)
finite = eqiora.compile(source="model M() { domain P = scalar_physical(across voltage: 1, through current: 1); port a: P; port b: P; connect a, b; relation voltage { a.voltage=2; } relation ground { b.current=0; } observable twice: 1=a.voltage+a.voltage; }")
output = finite.observable("twice")
assert isinstance(output, eqiora.ObservableRef)
assert finite.observable(output.id) == output
assert eqiora.Model.from_bytes(finite.to_bytes()).observable(output.id) == output
finite_linear = eqiora.solve.Linear(algorithm=eqiora.solve.LinearSolver.SparseLu, preconditioner=eqiora.solve.Preconditioner.Identity, reduction=eqiora.solve.Reduction.Fast, provider=eqiora.solve.SolverProvider.faer(), relative_tolerance=1e-12, absolute_tolerance=1e-14, maximum_iterations=100)
plan = eqiora.resolve(finite, solve=finite_linear)
assert isinstance(plan.capability, eqiora.solve.AlgebraicPlanView)
assert plan.capability.unknown_count == 4
result = eqiora.run(plan, state=eqiora.State.initial(plan))
observed = result.observe(output)
assert math.isclose(observed.value, 4.0, abs_tol=1e-12)
assert observed.value_type == eqiora.ValueType.real(eqiora.Dimension())
assert observed.evaluation_kind == "value"
assert observed.observable_id == output.id
assert observed.quadrature is observed.quadrature_points is None
assert eqiora.Result.from_bytes(plan, result.to_bytes()).observe(output).result_identity == observed.result_identity
for invalid in (lambda: finite.observable("a"), lambda: result.observe("twice"),
                lambda: result.observe(output, quadrature_points=2)):
    try:
        invalid()
    except (ValueError, TypeError):
        pass
    else:
        raise AssertionError("invalid Observable selection or rule was admitted")
foreign = eqiora.compile(source="model M() { domain P = scalar_physical(across voltage: 1, through current: 1); port a: P; port b: P; connect a, b; relation voltage { a.voltage=3; } relation ground { b.current=0; } observable twice: 1=a.voltage+a.voltage; }")
try:
    result.observe(foreign.observable("twice"))
except ValueError:
    pass
else:
    raise AssertionError("foreign ObservableRef was admitted")

graph = eqiora.geometry.GeometryGraph()
interval = graph.interval(bounds=(0.0, 1.0))
geometry = graph.build(interval, named_topology={"body": interval.region, "left": interval.boundaries[0], "right": interval.boundaries[1]})
source = """
public component Heat(support body: volume(ambient_dimension=1), support left: boundary(parent=body), support right: boundary(parent=body)) {
  variable temperature: K on body;
  parameter capacity: J/(K*m)=3;
  relation balance on body { -div(grad(temperature))=12[K/m^2]; }
  relation left_value on left { trace(temperature)=300[K]; }
  relation right_value on right { trace(temperature)=300[K]; }
  observable energy: J=integral(capacity*(temperature-300[K]), measure(body));
  observable endpoint: K=integral(trace(temperature), measure(left));
}
"""
model = eqiora.compile(source=source, geometry=geometry, entry="Heat", bindings={"body": geometry.selection("body"), "left": (geometry.selection("left"), geometry.selection("body")), "right": (geometry.selection("right"), geometry.selection("body"))})
mesh = eqiora.meshing.generate(eqiora.meshing.resolve(geometry, eqiora.meshing.CartesianMesher(cells=(4,))))
plan = eqiora.resolve(model, mesh=mesh, spatial=eqiora.fem.Q1(), solve=linear)
result = eqiora.run(plan)
energy, endpoint = model.observable("definition.energy"), model.observable("definition.endpoint")
value = result.observe(energy, quadrature_points=2)
# Q1 hats integrate nodal T-300=(0,9/8,3/2,9/8,0) to 15/16; capacity is 3.
assert math.isclose(value.value, 45/16, abs_tol=1e-9)
assert value.quadrature == "GaussLegendre" and value.quadrature_dimension == 1
assert value.quadrature_points == 2
boundary = result.observe(endpoint, quadrature_points=1)
assert math.isclose(boundary.value, 300, abs_tol=1e-10)
assert boundary.quadrature == "Point" and boundary.quadrature_dimension == 0
field = model.field("definition.temperature")
tangent = result.observable_state_tangent({field: (eqiora.Dimension(temperature=1), [2.0]*5)})
jvp = result.observe_state_jvp(energy, tangent, quadrature_points=2)
assert math.isclose(jvp.value, 6.0, abs_tol=1e-10)
assert jvp.evaluation_kind == "state-jvp" and jvp.value_type == value.value_type
assert jvp.result_identity == tangent.result_identity == value.result_identity
for invalid in (lambda: result.observe(energy), lambda: result.observe(endpoint, quadrature_points=2),
                lambda: result.observable_state_tangent({field: (eqiora.Dimension(), [2.0]*5)}),
                lambda: result.observable_state_tangent({field: (eqiora.Dimension(temperature=1), [2.0]*4)}),
                lambda: result.observable_state_tangent({field: (eqiora.Dimension(temperature=1), [float("nan")]*5)})):
    try:
        invalid()
    except (ValueError, eqiora.ValidationError):
        pass
    else:
        raise AssertionError("invalid spatial observation policy or tangent was admitted")
other_plan = eqiora.resolve(model, mesh=mesh, spatial=eqiora.fem.Q1(), solve=eqiora.solve.Linear(algorithm=eqiora.solve.LinearSolver.BiConjugateGradientStabilized, preconditioner=eqiora.solve.Preconditioner.Identity, reduction=eqiora.solve.Reduction.Reproducible, provider=eqiora.solve.SolverProvider.reference(), relative_tolerance=1e-10, absolute_tolerance=1e-12, maximum_iterations=100))
other_result = eqiora.run(other_plan)
try:
    other_result.observe_state_jvp(energy, tangent, quadrature_points=2)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("stale Result tangent was admitted")
"#), Some(&locals), Some(&locals))
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
    py.import("eqiora")
}
