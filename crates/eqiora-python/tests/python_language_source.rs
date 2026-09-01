use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyModule};

const SHIPPED_SOURCE: &str = include_str!("../../../examples/steady-flow-past-cylinder.eqi");

#[test]
fn python_language_source_round_trips_through_the_existing_compiler() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("shipped_source", SHIPPED_SOURCE)?;
        py.run(
            c_str!(
                r#"
import pathlib
import tempfile

q = eqiora.lang
u = q.units

namespace_probe = q.Source()
probe_component = namespace_probe.component("ScalarMath")
probe_body = probe_component.volume("body", dimensions=1)
probe_value = probe_component.field("value", on=probe_body, unit=u.one, initial=0)
probe_component.relation(
    "law", on=probe_body, residual=probe_value - q.math.sin(q.math.pi)
)
assert "math.sin(math.pi)" in namespace_probe.to_eqi()
assert not hasattr(q, "sin")


def cylinder_source(*, doc="Equations-only steady incompressible flow component.", velocity_shape=q.spatial_vector):
    source = q.Source()
    stokes = source.component("SteadyFlowPastCylinder", doc=doc)
    fluid = stokes.volume("fluid", dimensions=2)
    inlet = stokes.boundary("inlet", parent=fluid)
    outlet = stokes.boundary("outlet", parent=fluid)
    walls = stokes.boundary("walls", parent=fluid)
    cylinder = stokes.boundary("cylinder", parent=fluid)

    dynamic_viscosity = stokes.parameter("dynamic_viscosity", unit=u.kg / (u.m * u.s))
    zero_pressure = stokes.parameter("zero_pressure", unit=u.kg / (u.m * u.s**2))
    inlet_speed = stokes.parameter("inlet_speed", unit=u.m / u.s)
    channel_height = stokes.parameter("channel_height", unit=u.m)

    velocity = stokes.field(
        "velocity", on=fluid, unit=u.m / u.s, shape=velocity_shape
    )
    pressure = stokes.field(
        "pressure", on=fluid, unit=u.kg / (u.m * u.s**2), initial=0
    )
    force_potential = stokes.field(
        "force_potential", on=fluid, unit=u.kg / (u.m * u.s**2), initial=0
    )
    inlet_profile = stokes.field(
        "inlet_profile", on=fluid, unit=u.m / u.s, initial=0
    )

    stokes.relation(
        "force_definition", on=fluid, residual=force_potential - zero_pressure
    )
    stokes.relation(
        "inlet_profile_definition",
        on=fluid,
        residual=(
            inlet_profile
            - 4
            * inlet_speed
            * q.coordinate(1)
            * (channel_height - q.coordinate(1))
            / channel_height**2
        ),
    )
    stress = (
        2 * dynamic_viscosity * q.symmetric_part(q.grad(velocity))
        - q.isotropic_lift(pressure)
    )
    stokes.relation(
        "momentum",
        on=fluid,
        residual=-q.div(stress) - q.grad(force_potential),
        doc="Steady Stokes momentum balance.",
    )
    stokes.relation("incompressibility", on=fluid, residual=q.div(velocity))
    stokes.relation(
        "inlet_velocity",
        on=inlet,
        residual=q.trace(velocity) + q.normal(q.isotropic_lift(inlet_profile)),
    )
    stokes.relation(
        "outlet_traction", on=outlet, residual=q.normal(stress)
    )
    stokes.relation("wall_velocity", on=walls, residual=q.trace(velocity))
    stokes.relation(
        "cylinder_velocity", on=cylinder, residual=q.trace(velocity)
    )
    return source


first = cylinder_source()
second = cylinder_source()
assert first.to_eqi() == second.to_eqi()
assert "// Equations-only steady incompressible flow component." in first.to_eqi()
assert "relation momentum continuous on fluid" in first.to_eqi()

graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
geometry = graph.build(fluid, named_topology={
    "fluid": fluid.region,
    "inlet": rectangle.boundaries[0],
    "outlet": rectangle.boundaries[1],
    "walls": rectangle.boundaries[2:],
    "cylinder": circle.boundaries[0],
})
parameters = {
    "dynamic_viscosity": 0.001,
    "zero_pressure": 0.0,
    "inlet_speed": 0.3,
    "channel_height": 0.41,
}

direct_source = cylinder_source()
direct = eqiora.compile(
    source=direct_source,
    geometry=geometry,
    parameters=parameters,
)
with tempfile.TemporaryDirectory() as directory:
    path = pathlib.Path(directory) / "steady-flow-past-cylinder.eqi"
    direct_source.write_eqi(path)
    assert path.read_text(encoding="utf-8") == direct_source.to_eqi()
    emitted = eqiora.compile(path=path, geometry=geometry, parameters=parameters)
assert direct.digest == emitted.digest
shipped = eqiora.compile(
    source=shipped_source,
    filename="steady-flow-past-cylinder.eqi",
    geometry=geometry,
    parameters=parameters,
)
assert direct.structural_fingerprint == shipped.structural_fingerprint

other_comments = eqiora.compile(
    source=cylinder_source(doc="Different presentation-only documentation."),
    geometry=geometry,
    parameters=parameters,
)
assert direct.digest == other_comments.digest

try:
    direct_source.component("Second")
except q.SourceError:
    pass
else:
    raise AssertionError("an emitted Source remained mutable")

left = q.Source()
left_component = left.component("Left")
left_volume = left_component.volume("left", dimensions=2)
left_value = left_component.field("value", on=left_volume, unit=u.m)
right = q.Source()
right_component = right.component("Right")
right_volume = right_component.volume("right", dimensions=2)
right_value = right_component.field("value", on=right_volume, unit=u.m)
for invalid in (
    lambda: left_value + right_value,
    lambda: left_component.boundary("foreign_parent", parent=right_volume),
    lambda: left_component.field("wrong_support", on=right_volume, unit=u.m),
    lambda: left_component.field("value", on=left_volume, unit=u.m),
    lambda: left_component.field("nonfinite", on=left_volume, unit=u.m, initial=float("nan")),
):
    try:
        invalid()
    except q.SourceError:
        pass
    else:
        raise AssertionError("invalid Source authoring input was accepted")

try:
    q.Source().component("not-valid")
except q.SourceError:
    pass
else:
    raise AssertionError("an invalid declaration name was accepted")

deep = q.coordinate(0)
try:
    for _ in range(100):
        deep = q.grad(deep)
except q.SourceError:
    pass
else:
    raise AssertionError("an excessive expression depth was accepted")

with tempfile.TemporaryDirectory() as directory:
    target = pathlib.Path(directory) / "target.eqi"
    target.write_text("preserved", encoding="utf-8")
    link = pathlib.Path(directory) / "link.eqi"
    link.symlink_to(target)
    try:
        cylinder_source().write_eqi(link)
    except ValueError:
        pass
    else:
        raise AssertionError("a symlink output path was accepted")
    assert target.read_text(encoding="utf-8") == "preserved"

try:
    eqiora.compile(
        source=cylinder_source(velocity_shape=None),
        geometry=geometry,
        parameters=parameters,
    )
except eqiora.ValidationError as error:
    assert error.diagnostics
    assert any(
        diagnostic.source_span is not None
        and diagnostic.source_span[0] == "<python-source>"
        for diagnostic in error.diagnostics
    )
else:
    raise AssertionError("a shape-invalid generated source passed the canonical compiler")
"#
            ),
            Some(&locals),
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
