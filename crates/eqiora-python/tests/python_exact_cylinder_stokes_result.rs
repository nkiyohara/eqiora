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
MESH_DIGEST = "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a"

def geometry(**overrides):
    arguments = {
        "bounds": ((0.0, 2.2), (0.0, 0.41)),
        "circle_center": (0.2, 0.2),
        "circle_radius": 0.05,
        "tolerance": 1e-12,
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
    return graph.planar_circular_section(
        classification_tolerance=arguments["tolerance"],
        region=arguments["region"],
        x_lower=arguments["x_lower"],
        x_upper=arguments["x_upper"],
        y_lower=arguments["y_lower"],
        y_upper=arguments["y_upper"],
        hole=arguments["hole"],
    )

def mesh(source):
    request = eqiora.meshing.MeshRequest(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    plan = eqiora.meshing.resolve(source, request)
    return eqiora.meshing.generate(source, plan=plan)

source = geometry()
realized = mesh(source)
assert "numpy" not in sys.modules
result = eqiora.fluid.solve_exact_cylinder_stokes(
    model=model,
    geometry=source,
    mesh=realized,
)
assert "numpy" not in sys.modules
assert type(result).__module__ == "eqiora._eqiora"
assert type(result).__name__ == "CircularHoleSteadyStokesResult"
assert isinstance(result, eqiora.fluid.CircularHoleSteadyStokesResult)
assert result.model_digest == MODEL_DIGEST
assert result.exact_source_digest == SOURCE_DIGEST
assert result.mesh_digest == MESH_DIGEST
assert result.semantic_revision == 1
assert result.realization_revision == 133
assert result.pressure.shape == (104,)
assert result.pressure[0] == result.pressure[0]
assert "numpy" not in sys.modules

binding = json.loads(result.chordal_realization_json)
assert binding["source_geometry_sha256"] == result.exact_source_digest
assert binding["mesh_sha256"] == result.mesh_digest
assert hashlib.sha256(
    binding["schema"].encode() + b"\0" + result.chordal_realization_json
).hexdigest() == result.chordal_realization_digest

run = json.loads(result.run_manifest_json)
assert run["model_sha256"] == result.model_digest
assert run["realization_sha256"] == result.realization_digest
assert run["output_sha256"] == [result.snapshot_digest]
assert hashlib.sha256(
    run["schema"].encode() + b"\0" + result.run_manifest_json
).hexdigest() == result.run_digest

coordinates = result.coordinates
triangles = result.triangles
assert "numpy" in sys.modules
assert coordinates.shape == (104, 2)
assert triangles.shape == (104, 3)
assert result.pressure.numpy(copy=False).shape == (104,)

try:
    eqiora.fluid.solve_exact_cylinder_stokes(
        model=b'{"schema":',
        geometry=source,
        mesh=realized,
    )
except eqiora.CompatibilityError as error:
    assert error.category == "compatibility"
    assert any(item.code == "EQ0901" for item in error.diagnostics)
else:
    raise AssertionError("malformed current Model crossed the native boundary")

foreign = geometry(tolerance=1e-10)
try:
    eqiora.fluid.solve_exact_cylinder_stokes(
        model=model,
        geometry=foreign,
        mesh=realized,
    )
except eqiora.ValidationError as error:
    assert error.category == "validation"
    assert any(item.code == "EQ0807" for item in error.diagnostics)
else:
    raise AssertionError("foreign exact geometry ownership was admitted")
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
