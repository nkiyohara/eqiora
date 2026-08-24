use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyModule};

const SOURCE: &str = include_str!("../../../examples/steady-flow-past-cylinder.eqi");

#[test]
fn python_binds_component_to_python_owned_geometry_without_shape_duplication() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("source", SOURCE)?;
        py.run(
            c_str!(
                r#"
graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=1e-10,
).circular_through_cut(
    center=(0.2, 0.2),
    radius=0.05,
    boolean_tolerance=1e-10,
)
geometry = graph.planar_circular_section(
    classification_tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)
supports = {
    name: geometry.selection(name)
    for name in ("fluid", "inlet", "outlet", "walls", "cylinder")
}
height = geometry.bounds[1][1] - geometry.bounds[1][0]
parameters = {
    "dynamic_viscosity": 0.001,
    "zero_pressure": 0.0,
    "inlet_speed": 0.3,
    "channel_height": height,
}
model = eqiora.bind_component(
    source,
    component="SteadyFlowPastCylinder",
    geometry=geometry,
    supports=supports,
    parameters=parameters,
    filename="steady-flow-past-cylinder.eqi",
)
assert type(model).__name__ == "Model"
assert b'"geometry-region"' in model.to_json()
assert b'"geometry-boundary"' in model.to_json()

try:
    eqiora.bind_component(
        source,
        component="SteadyFlowPastCylinder",
        geometry=geometry,
        supports={name: value for name, value in supports.items() if name != "cylinder"},
        parameters=parameters,
    )
except eqiora.ValidationError as error:
    assert any("cylinder" in diagnostic.message for diagnostic in error.diagnostics)
else:
    raise AssertionError("missing support returned a Model")

foreign = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(0.0, 2.3),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=1e-10,
).circular_through_cut(
    center=(0.2, 0.2),
    radius=0.05,
    boolean_tolerance=1e-10,
).planar_circular_section(
    classification_tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)
try:
    eqiora.bind_component(
        source,
        component="SteadyFlowPastCylinder",
        geometry=geometry,
        supports={**supports, "inlet": foreign.selection("inlet")},
        parameters=parameters,
    )
except eqiora.ValidationError as error:
    assert "foreign or stale" in error.diagnostics[0].message
else:
    raise AssertionError("foreign selection returned a Model")

try:
    eqiora.bind_component(
        source,
        component="SteadyFlowPastCylinder",
        geometry=geometry,
        supports={**supports, "inlet": supports["fluid"], "fluid": supports["inlet"]},
        parameters=parameters,
    )
except eqiora.ValidationError as error:
    assert any("support" in diagnostic.message for diagnostic in error.diagnostics)
else:
    raise AssertionError("kind-swapped selections returned a Model")

try:
    eqiora.bind_component(
        source,
        component="SteadyFlowPastCylinder",
        geometry=geometry,
        supports={**supports, "inlet": "inlet"},
        parameters=parameters,
    )
except TypeError:
    pass
else:
    raise AssertionError("a raw selection name returned a Model")

try:
    eqiora.bind_component(
        source,
        component="SteadyFlowPastCylinder",
        geometry=geometry,
        supports=supports,
        parameters={**parameters, "extra": 1.0},
    )
except eqiora.ValidationError as error:
    assert any("extra" in diagnostic.message for diagnostic in error.diagnostics)
else:
    raise AssertionError("an extra parameter returned a Model")

try:
    eqiora.bind_component(
        source,
        component="SteadyFlowPastCylinder",
        geometry=geometry,
        supports=supports,
        parameters={**parameters, "inlet_speed": float("nan")},
    )
except eqiora.ValidationError as error:
    assert any("finite" in diagnostic.message for diagnostic in error.diagnostics)
else:
    raise AssertionError("a non-finite parameter returned a Model")
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
    "eqiora", package_path / "__init__.py",
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
