use std::fs;
use std::path::Path;

use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyModule};

#[test]
fn python_exact_cylinder_stokes_result_crosses_the_native_boundary() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/steady-flow-past-cylinder.model.json");
        let encoded = fs::read(path)?;
        assert_eq!(encoded.last(), Some(&b'\n'));

        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("model", PyBytes::new(py, &encoded[..encoded.len() - 1]))?;
        py.run(
            c_str!(
                r#"
import hashlib
import json
import sys

MODEL_DIGEST = "8bc5155bc1b64ed37f7a2ac010a966e1619091a118e6cf7806dbdf9621977146"
SOURCE_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
MESH_DIGEST = "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"

def geometry(**overrides):
    arguments = {
        "bounds": ((0.0, 2.2), (0.0, 0.41)),
        "circle_center": (0.2, 0.2),
        "circle_radius": 0.05,
        "region": "fluid",
        "x_lower": "inlet",
        "x_upper": "outlet",
        "y_lower": "walls",
        "y_upper": "walls",
        "hole": "cylinder",
    }
    arguments.update(overrides)
    x_bounds, y_bounds = arguments["bounds"]
    graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=arguments["circle_center"],
        radius=arguments["circle_radius"],
        boolean_tolerance=1e-10,
    )
    lower = graph.face_handle("profile-y-lower")
    upper = graph.face_handle("profile-y-upper")
    sides = (
        {arguments["y_lower"]: (lower, upper)}
        if arguments["y_lower"] == arguments["y_upper"]
        else {arguments["y_lower"]: lower, arguments["y_upper"]: upper}
    )
    return graph.planar_section(named_topology={
        arguments["region"]: graph.face_handle("end-cap"),
        arguments["x_lower"]: graph.face_handle("profile-x-lower"),
        arguments["x_upper"]: graph.face_handle("profile-x-upper"),
        **sides,
        arguments["hole"]: graph.face_handle("cut-wall"),
    })

def mesh(source):
    request = eqiora.meshing.MeshRequest(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    plan = eqiora.meshing.resolve(source, request)
    return plan, eqiora.meshing.generate(source, plan=plan)

source = geometry()
mesh_plan, realized = mesh(source)
assert "numpy" not in sys.modules
assert not hasattr(eqiora.fluid, "solve_exact_cylinder_stokes")
assert "solve_exact_cylinder_stokes" not in eqiora.fluid.__all__
assert not hasattr(eqiora.fluid, "CircularHoleSteadyStokesResult")
assert "CircularHoleSteadyStokesResult" not in eqiora.fluid.__all__
assert not hasattr(sys.modules["eqiora._eqiora"], "CircularHoleSteadyStokesResult")

intent = eqiora.fluid.SteadyStokes(
    length_scale_m=0.41,
    velocity_scale_m_per_s=0.3,
    pressure_scale_pa=0.001 * 0.3 / 0.41,
    relative_tolerance=1e-6,
    absolute_tolerance=1e-13,
    maximum_iterations=10000,
)
assert intent == eqiora.fluid.SteadyStokes(
    length_scale_m=0.41,
    velocity_scale_m_per_s=0.3,
    pressure_scale_pa=0.001 * 0.3 / 0.41,
    relative_tolerance=1e-6,
    absolute_tolerance=1e-13,
    maximum_iterations=10000,
)

current = eqiora.replay(model)
plan = eqiora.fluid.resolve(current, intent, mesh=realized)
assert "numpy" not in sys.modules
assert type(plan).__module__ == "eqiora._eqiora"
assert type(plan).__name__ == "SteadyStokesPlan"
assert plan.model_digest == MODEL_DIGEST == current.digest
assert plan.geometry_digest == SOURCE_DIGEST
assert plan.mesh_digest == MESH_DIGEST
assert plan.semantic_revision == 1
assert plan.realization_revision == 133
assert plan.spatial_dimension == 2
assert plan.length_scale_m == 0.41
assert plan.velocity_scale_m_per_s == 0.3
assert plan.pressure_scale_pa == 0.001 * 0.3 / 0.41
assert plan.solver_algorithm == "sparse-lu"
assert plan.preconditioner == "identity"
assert plan.reduction == "fast"
assert plan.relative_tolerance == 1e-6
assert plan.absolute_tolerance == 1e-13
assert plan.maximum_iterations == 10000
assert plan.solver_backend == "eqiora.faer"
assert plan.workers == 1
assert plan.velocity_space == "simplex-p1-bubble"
assert plan.pressure_space == "continuous-lagrange-1"
envelope = json.loads(plan.canonical_bytes)
assert envelope["model_sha256"] == plan.model_digest
assert envelope["source"]["realization_revision"] == plan.realization_revision
assert hashlib.sha256(
    envelope["schema"].encode() + b"\0" + plan.canonical_bytes
).hexdigest() == plan.realization_digest

run = eqiora.submit(current, plan=plan)
result = run.result()
assert run.result() is result
assert plan.execution_adapter == result.adapter
assert "numpy" not in sys.modules
assert type(result).__module__ == "eqiora._eqiora"
assert type(result).__name__ == "Result"
assert isinstance(result, eqiora.Result)
assert result.model_digest == MODEL_DIGEST
assert result.model_id == current.model_id
assert result.model_revision == 1
assert result.fields == []
assert len(result.snapshots) == 1
snapshot = result.snapshots[0]
assert result.field(snapshot.field) is snapshot
assert result.mesh(snapshot.field) is realized
assert snapshot.mesh_digest == MESH_DIGEST
assert snapshot.dimension == (1, -1, -2, 0, 0, 0, 0)
assert snapshot.value_shape == ()
assert snapshot.frame == "invariant"
assert snapshot.associations == ("vertex",)
evidence = eqiora.fluid.steady_stokes_evidence(result)
assert isinstance(evidence, eqiora.fluid.SteadyStokesEvidence)
assert "numpy" not in sys.modules

binding = json.loads(mesh_plan.canonical_bytes)
assert binding["source_geometry_sha256"] == source.digest
assert binding["realized_geometry_sha256"] == realized.realized_geometry_digest
assert binding["mesh_sha256"] == realized.digest
assert hashlib.sha256(
    binding["schema"].encode() + b"\0" + mesh_plan.canonical_bytes
).hexdigest() == realized.realization_digest

manifest = result.run_manifest()
run_bytes = manifest.to_json()
run = json.loads(run_bytes)
assert run["model_sha256"] == result.model_digest
assert run["realization_sha256"] == plan.realization_digest
assert run["output_sha256"] == [snapshot.digest]
assert hashlib.sha256(
    run["schema"].encode() + b"\0" + run_bytes
).hexdigest() == manifest.digest == evidence.run_digest

coordinates = realized.coordinates
triangles = realized.cells
assert "numpy" in sys.modules
assert coordinates.shape == (662, 2)
assert triangles.shape == (1210, 3)
pressure = snapshot.values("vertex")
assert pressure.shape == (662,)

PRESSURE_TOLERANCE = 3.6587365853658537e-10
PRESSURE_PROBES = (
    ((0.15, 0.2), 0.06959832738138942),
    ((0.25, 0.2), 0.019333181397105),
    ((0.1968604740235343, 0.1500986635785864), 0.04389626088659296),
    ((0.1968604740235343, 0.2499013364214136), 0.045165230577321865),
    ((0.0, 0.2), 0.062148654204247),
    ((2.2, 0.2), 0.0004742049675737538),
)
for position, expected in PRESSURE_PROBES:
    index = min(
        range(len(coordinates)),
        key=lambda candidate: sum(
            (coordinates[candidate, axis] - position[axis]) ** 2 for axis in range(2)
        ),
    )
    assert abs(pressure[index] - expected) <= PRESSURE_TOLERANCE

assert abs(evidence.inlet_flux - (-0.08149573099927537)) <= 6.15002e-8
assert abs(evidence.outlet_flux - 0.08149573099927537) <= 6.15002e-8
assert abs(evidence.net_flux) <= 1e-8
for actual, expected in zip(
    evidence.cylinder_force_on_fluid,
    (-0.006384200476069211, -0.00006344553664047762),
):
    assert abs(actual - expected) <= 1.5002e-10
assert all(abs(component) <= 1e-10 for component in evidence.momentum_closure)
assert evidence.solve.residual_target == 6.138485578780151e-6
assert evidence.solve.true_residual_norm <= evidence.solve.residual_target

try:
    eqiora.replay(b'{"schema":')
except eqiora.CompatibilityError as error:
    assert error.category == "compatibility"
    assert any(item.code == "EQ0901" for item in error.diagnostics)
else:
    raise AssertionError("malformed current Model crossed the native boundary")

_, foreign = mesh(geometry(x_lower="outlet", x_upper="inlet"))
assert foreign.digest == realized.digest
try:
    eqiora.fluid.resolve(current, intent, mesh=foreign)
except eqiora.ValidationError as error:
    assert error.category == "validation"
    assert any(item.code == "EQ0807" for item in error.diagnostics)
else:
    raise AssertionError("foreign exact geometry ownership was admitted")

try:
    eqiora.fluid.resolve(
        current,
        eqiora.fluid.SteadyStokes(
            length_scale_m=0.41,
            velocity_scale_m_per_s=0.3,
            pressure_scale_pa=0.001 * 0.3 / 0.41,
            relative_tolerance=1e-11,
            absolute_tolerance=1e-13,
            maximum_iterations=10000,
        ),
        mesh=realized,
    )
except eqiora.CapabilityError as error:
    assert error.category == "capability"
    assert error.diagnostics
else:
    raise AssertionError("an unsupported intent was resolved into a Plan")
"#
            ),
            Some(&locals),
            Some(&locals),
        )?;
        Ok(())
    })
}

fn public_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
    let package_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
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
assert spec is not None and spec.loader is not None
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
        .expect("the package loader must bind eqiora")
        .cast_into::<PyModule>()?)
}
