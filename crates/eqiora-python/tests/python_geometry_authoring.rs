use std::fs;
use std::path::Path;

use eqiora::geometry::{CanonicalCircularHoleGeometryV1, CanonicalGeometryLimits};
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBytes, PyDict, PyDictMethods, PyModule};

#[test]
fn python_exact_circular_hole_geometry_replays_rust_owned_identity() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/steady-flow-past-cylinder.geometry.json");
        let reference = fs::read(fixture)?;
        assert_eq!(reference.last(), Some(&b'\n'));
        let expected = &reference[..reference.len() - 1];

        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("expected_json", PyBytes::new(py, expected))?;
        py.run(
            c_str!(
                r#"
def make(**overrides):
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
    return eqiora.geometry.RectangleWithCircularHole(**arguments)

geometry = make()
assert type(geometry).__module__ == "eqiora._eqiora"
assert geometry.canonical_json == expected_json
assert geometry.selection_names == (
    "cylinder", "inlet", "outlet", "walls", "fluid"
)
assert tuple(geometry.selection_dimension(name) for name in geometry.selection_names) == (
    1, 1, 1, 1, 2
)

same = make()
signed_zero = make(bounds=((-0.0, 2.2), (-0.0, 0.41)))
swapped = make(x_lower="outlet", x_upper="inlet")
assert geometry == same == signed_zero
assert hash(geometry) == hash(same) == hash(signed_zero)
assert geometry != swapped
assert geometry.digest != swapped.digest

try:
    geometry.selection_dimension("missing")
except eqiora.ValidationError as error:
    assert error.category == "validation"
    assert error.diagnostics
else:
    raise AssertionError("an unknown exact selection returned a value")

try:
    make(
        bounds=((0.0, 1.0), (0.0, 1.0)),
        circle_center=(0.1875, 0.5),
        circle_radius=0.125,
        tolerance=0.0625,
    )
except eqiora.ValidationError as error:
    assert error.category == "validation"
    assert error.diagnostics
else:
    raise AssertionError("a circle at tolerance clearance was admitted")

canonical_json = geometry.canonical_json
python_digest = geometry.digest
"#
            ),
            Some(&locals),
            Some(&locals),
        )?;

        let canonical_json = locals
            .get_item("canonical_json")?
            .expect("Python geometry must expose canonical bytes")
            .extract::<Vec<u8>>()?;
        let python_digest = locals
            .get_item("python_digest")?
            .expect("Python geometry must expose its digest")
            .extract::<String>()?;
        let replayed = CanonicalCircularHoleGeometryV1::decode_canonical(
            &canonical_json,
            CanonicalGeometryLimits::default(),
        )
        .expect("Python bytes must replay through the public Rust geometry contract");
        let rust_digest = replayed
            .digest_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(python_digest, rust_digest);
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
