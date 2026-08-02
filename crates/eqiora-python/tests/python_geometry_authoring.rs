use std::fs;
use std::path::Path;

use eqiora::geometry::{CanonicalGeometryLimits, CanonicalGeometryV1, NamedEntitySet};
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
        let expected_oriented = CanonicalGeometryV1::from_circular_hole(
            [[0.0, 2.2], [0.0, 0.41]],
            [0.2, 0.2],
            0.05,
            vec![
                NamedEntitySet::new("fluid", 2, vec![0]),
                NamedEntitySet::new("floor", 1, vec![2]),
                NamedEntitySet::new("inlet", 1, vec![0]),
                NamedEntitySet::new("ceiling", 1, vec![3]),
                NamedEntitySet::new("cylinder", 1, vec![4]),
                NamedEntitySet::new("outlet", 1, vec![1]),
            ],
            1e-12,
        )
        .expect("the public exact geometry contract must admit the oriented witness");
        let expected_oriented_digest = expected_oriented
            .digest_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            expected_oriented_digest,
            "51ece8fa2d8709d932b0c758d59c187e4fd572f73217c31dcbe407f8d873be7f"
        );
        let expected_off_axis = CanonicalGeometryV1::from_circular_hole(
            [[0.0, 2.2], [0.0, 0.41]],
            [0.3, 0.2],
            0.05,
            vec![
                NamedEntitySet::new("fluid", 2, vec![0]),
                NamedEntitySet::new("walls", 1, vec![3, 2]),
                NamedEntitySet::new("inlet", 1, vec![0]),
                NamedEntitySet::new("cylinder", 1, vec![4]),
                NamedEntitySet::new("outlet", 1, vec![1]),
            ],
            1e-12,
        )
        .expect("the public exact geometry contract must admit the off-axis witness");
        let expected_off_axis_digest = expected_off_axis
            .digest_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            expected_off_axis_digest,
            "552ebf459396ed5bc7f72ab48f34046baa828b6af808794e861bd958dc613881"
        );

        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("expected_json", PyBytes::new(py, expected))?;
        locals.set_item(
            "expected_oriented_json",
            PyBytes::new(py, expected_oriented.canonical_bytes()),
        )?;
        locals.set_item("expected_oriented_digest", expected_oriented_digest)?;
        locals.set_item(
            "expected_off_axis_json",
            PyBytes::new(py, expected_off_axis.canonical_bytes()),
        )?;
        locals.set_item("expected_off_axis_digest", expected_off_axis_digest)?;
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

geometry = make()
assert type(geometry).__module__ == "eqiora._eqiora"
assert type(geometry).__name__ == "Geometry"
assert geometry.canonical_bytes == expected_json
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

oriented = make(y_lower="floor", y_upper="ceiling")
assert oriented.canonical_bytes == expected_oriented_json
assert oriented.digest == expected_oriented_digest
assert oriented.selection_names == (
    "ceiling", "cylinder", "floor", "inlet", "outlet", "fluid"
)

off_axis = make(circle_center=(0.3, 0.2))
assert off_axis.canonical_bytes == expected_off_axis_json
assert off_axis.digest == expected_off_axis_digest

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

canonical_json = geometry.canonical_bytes
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
        let replayed = CanonicalGeometryV1::decode_circular_hole_canonical(
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
