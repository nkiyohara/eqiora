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
u = eqiora.units

namespace_probe = q.Source()
probe_component = namespace_probe.component("ScalarMath")
probe_body = probe_component.volume("body", dimensions=1)
probe_value = probe_component.field("value", role=eqiora.FieldRole.Variable, on=probe_body, value_type=eqiora.ValueType.real())
probe_component.relation(
    "law", on=probe_body, right=0, left=probe_value - q.math.sin(q.math.pi)
)
assert "math.sin(math.pi)" in namespace_probe.to_eqi()
assert not hasattr(q, "sin")


def cylinder_source(*, doc="Equations-only steady incompressible flow component.", velocity_extent=2):
    source = q.Source()
    stokes = source.component("SteadyFlowPastCylinder", doc=doc)
    fluid = stokes.volume("fluid", dimensions=2)
    inlet = stokes.boundary("inlet", parent=fluid)
    outlet = stokes.boundary("outlet", parent=fluid)
    walls = stokes.boundary("walls", parent=fluid)
    cylinder = stokes.boundary("cylinder", parent=fluid)

    dynamic_viscosity = stokes.parameter("dynamic_viscosity", value_type=eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-1)))
    zero_pressure = stokes.parameter("zero_pressure", value_type=eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-2)))
    inlet_speed = stokes.parameter("inlet_speed", value_type=eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1)))
    channel_height = stokes.parameter("channel_height", value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))

    velocity = stokes.field(
        "velocity", role=eqiora.FieldRole.Variable, on=fluid, value_type=(
            eqiora.ValueType.vector(eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1)), velocity_extent)
            if velocity_extent is not None else eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1))
        )
    )
    pressure = stokes.field(
        "pressure", role=eqiora.FieldRole.Variable, on=fluid, value_type=eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-2))
    )
    force_potential = stokes.field(
        "force_potential", role=eqiora.FieldRole.Variable, on=fluid, value_type=eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-2))
    )
    inlet_profile = stokes.field(
        "inlet_profile", role=eqiora.FieldRole.Variable, on=fluid, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1))
    )

    stokes.relation(
        "force_definition", on=fluid, right=0, left=force_potential - zero_pressure
    )
    stokes.relation(
        "inlet_profile_definition",
        on=fluid,
        right=0, left=(
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
        right=0, left=-q.div(stress) - q.grad(force_potential),
        doc="Steady Stokes momentum balance.",
    )
    stokes.relation("incompressibility", on=fluid, right=0, left=q.div(velocity))
    stokes.relation(
        "inlet_velocity",
        on=inlet,
        right=0, left=q.trace(velocity) + q.normal(q.isotropic_lift(inlet_profile)),
    )
    stokes.relation(
        "outlet_traction", on=outlet, right=0, left=q.normal(stress)
    )
    stokes.relation("wall_velocity", on=walls, right=0, left=q.trace(velocity))
    stokes.relation(
        "cylinder_velocity", on=cylinder, right=0, left=q.trace(velocity)
    )
    return source


first = cylinder_source()
second = cylinder_source()
assert first.to_eqi() == second.to_eqi()
assert "/// Equations-only steady incompressible flow component." in first.to_eqi()
assert "relation momentum on fluid" in first.to_eqi()

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
direct = eqiora.compile(source=direct_source, geometry=geometry, entry='SteadyFlowPastCylinder', bindings={'fluid': geometry.selection('fluid'), 'inlet': (geometry.selection('inlet'), geometry.selection('fluid')), 'outlet': (geometry.selection('outlet'), geometry.selection('fluid')), 'walls': (geometry.selection('walls'), geometry.selection('fluid')), 'cylinder': (geometry.selection('cylinder'), geometry.selection('fluid')), **parameters})
with tempfile.TemporaryDirectory() as directory:
    path = pathlib.Path(directory) / "steady-flow-past-cylinder.eqi"
    direct_source.write_eqi(path)
    assert path.read_text(encoding="utf-8") == direct_source.to_eqi()
    emitted = eqiora.compile(path=path, geometry=geometry, entry='SteadyFlowPastCylinder', bindings={'fluid': geometry.selection('fluid'), 'inlet': (geometry.selection('inlet'), geometry.selection('fluid')), 'outlet': (geometry.selection('outlet'), geometry.selection('fluid')), 'walls': (geometry.selection('walls'), geometry.selection('fluid')), 'cylinder': (geometry.selection('cylinder'), geometry.selection('fluid')), **parameters})
assert direct.digest == emitted.digest
shipped = eqiora.compile(source=shipped_source, filename='steady-flow-past-cylinder.eqi', geometry=geometry, entry='SteadyFlowPastCylinder', bindings={'fluid': geometry.selection('fluid'), 'inlet': (geometry.selection('inlet'), geometry.selection('fluid')), 'outlet': (geometry.selection('outlet'), geometry.selection('fluid')), 'walls': (geometry.selection('walls'), geometry.selection('fluid')), 'cylinder': (geometry.selection('cylinder'), geometry.selection('fluid')), **parameters})
assert direct.structural_fingerprint == shipped.structural_fingerprint

other_comments = eqiora.compile(source=cylinder_source(doc='Different presentation-only documentation.'), geometry=geometry, entry='SteadyFlowPastCylinder', bindings={'fluid': geometry.selection('fluid'), 'inlet': (geometry.selection('inlet'), geometry.selection('fluid')), 'outlet': (geometry.selection('outlet'), geometry.selection('fluid')), 'walls': (geometry.selection('walls'), geometry.selection('fluid')), 'cylinder': (geometry.selection('cylinder'), geometry.selection('fluid')), **parameters})
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
left_value = left_component.field("value", role=eqiora.FieldRole.Variable, on=left_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))
right = q.Source()
right_component = right.component("Right")
right_volume = right_component.volume("right", dimensions=2)
right_value = right_component.field("value", role=eqiora.FieldRole.Variable, on=right_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))
for invalid in (
    lambda: left_value + right_value,
    lambda: left_component.boundary("foreign_parent", parent=right_volume),
    lambda: left_component.field("wrong_support", role=eqiora.FieldRole.Variable, on=right_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1))),
    lambda: left_component.field("value", role=eqiora.FieldRole.Variable, on=left_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1))),
    lambda: left_value + float("nan"),
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
    eqiora.compile(source=cylinder_source(velocity_extent=None), geometry=geometry, entry='SteadyFlowPastCylinder', bindings={'fluid': geometry.selection('fluid'), 'inlet': (geometry.selection('inlet'), geometry.selection('fluid')), 'outlet': (geometry.selection('outlet'), geometry.selection('fluid')), 'walls': (geometry.selection('walls'), geometry.selection('fluid')), 'cylinder': (geometry.selection('cylinder'), geometry.selection('fluid')), **parameters})
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

#[test]
fn python_sampled_signature_round_trips_through_the_actual_parser() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let locals = PyDict::new(py);
        locals.set_item("eqiora", public_module(py)?)?;
        py.run(c_str!(r#"
q = eqiora.lang
source = q.Source()
model = source.model("Sampled")
tick = model.clock_requirement("tick")
drive = model.input("drive", value_type=eqiora.ValueType.real(), at=tick)
observed = model.output("observed", value_type=eqiora.ValueType.real(), at=tick)
memory = model.field("memory", value_type=eqiora.ValueType.real(), role=eqiora.FieldRole.State, at=tick)
model.initial(q.pre(memory))
model.relation("update", at=tick, left=q.next(memory), right=q.pre(memory) + drive)
model.relation("observe", at=tick, left=observed, right=q.pre(memory))
text = source.to_eqi()
"#), Some(&locals), Some(&locals))?;
        let text: String = locals
            .get_item("text")?
            .expect("authored Source")
            .extract()?;
        let parsed = eqiora::language::parse("sampled.eqi", &text);
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let formatted = eqiora::language::format(parsed.document().expect("parsed Source"));
        let reparsed = eqiora::language::parse("formatted.eqi", &formatted);
        assert!(
            reparsed.diagnostics().is_empty(),
            "{:?}",
            reparsed.diagnostics()
        );
        assert_eq!(
            formatted,
            eqiora::language::format(reparsed.document().unwrap())
        );
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
