from pathlib import Path

import pytest

import eqiora


q = eqiora.lang
u = q.units


def cylinder_source(
    *,
    doc: str = "Equations-only steady incompressible flow component.",
    velocity_shape=q.spatial_vector,
):
    source = q.Source()
    stokes = source.component("SteadyFlowPastCylinder", public=True, doc=doc)
    fluid = stokes.volume("fluid", dimensions=2, public=True)
    inlet = stokes.boundary("inlet", parent=fluid, public=True)
    outlet = stokes.boundary("outlet", parent=fluid, public=True)
    walls = stokes.boundary("walls", parent=fluid, public=True)
    cylinder = stokes.boundary("cylinder", parent=fluid, public=True)

    dynamic_viscosity = stokes.parameter("dynamic_viscosity", unit=u.kg / (u.m * u.s))
    zero_pressure = stokes.parameter("zero_pressure", unit=u.kg / (u.m * u.s**2))
    inlet_speed = stokes.parameter("inlet_speed", unit=u.m / u.s)
    channel_height = stokes.parameter("channel_height", unit=u.m)

    velocity = stokes.field("velocity", on=fluid, unit=u.m / u.s, shape=velocity_shape)
    pressure = stokes.field("pressure", on=fluid, unit=u.kg / (u.m * u.s**2), initial=0)
    force_potential = stokes.field(
        "force_potential", on=fluid, unit=u.kg / (u.m * u.s**2), initial=0
    )
    inlet_profile = stokes.field("inlet_profile", on=fluid, unit=u.m / u.s, initial=0)

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
    stress = 2 * dynamic_viscosity * q.symmetric_part(
        q.grad(velocity)
    ) - q.isotropic_lift(pressure)
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
    stokes.relation("outlet_traction", on=outlet, residual=q.normal(stress))
    stokes.relation("wall_velocity", on=walls, residual=q.trace(velocity))
    stokes.relation("cylinder_velocity", on=cylinder, residual=q.trace(velocity))
    return source


def cylinder_geometry():
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=(0.2, 0.2), radius=0.05)
    fluid = graph.subtract(rectangle, circle)
    geometry = graph.build(
        fluid,
        named_topology={
            "fluid": fluid.region,
            "inlet": rectangle.boundaries[0],
            "outlet": rectangle.boundaries[1],
            "walls": rectangle.boundaries[2:],
            "cylinder": circle.boundaries[0],
        },
    )
    return geometry


PARAMETERS = {
    "dynamic_viscosity": 0.001,
    "zero_pressure": 0.0,
    "inlet_speed": 0.3,
    "channel_height": 0.41,
}


def test_source_is_deterministic_and_direct_file_compilation_has_one_identity(
    tmp_path: Path,
) -> None:
    first = cylinder_source()
    second = cylinder_source()
    assert first.to_eqi() == second.to_eqi()
    assert "// Equations-only steady incompressible flow component." in first.to_eqi()

    geometry = cylinder_geometry()
    direct = eqiora.compile(
        source=first,
        geometry=geometry,
        parameters=PARAMETERS,
    )
    path = tmp_path / "steady-flow-past-cylinder.eqi"
    first.write_eqi(path)
    assert path.read_text(encoding="utf-8") == first.to_eqi()
    emitted = eqiora.compile(path=path, geometry=geometry, parameters=PARAMETERS)
    other_comments = eqiora.compile(
        source=cylinder_source(doc="Different presentation-only documentation."),
        geometry=geometry,
        parameters=PARAMETERS,
    )
    assert direct.digest == emitted.digest == other_comments.digest


def test_source_owns_handles_limits_and_atomic_output(tmp_path: Path) -> None:
    left = q.Source()
    left_component = left.component("Left")
    left_volume = left_component.volume("left", dimensions=2)
    left_value = left_component.field("value", on=left_volume, unit=u.m)
    right = q.Source()
    right_component = right.component("Right")
    right_volume = right_component.volume("right", dimensions=2)
    right_value = right_component.field("value", on=right_volume, unit=u.m)

    invalid = (
        lambda: left_value + right_value,
        lambda: left_component.boundary("foreign_parent", parent=right_volume),
        lambda: left_component.field("wrong_support", on=right_volume, unit=u.m),
        lambda: left_component.field("value", on=left_volume, unit=u.m),
        lambda: left_component.field(
            "nonfinite", on=left_volume, unit=u.m, initial=float("nan")
        ),
    )
    for operation in invalid:
        with pytest.raises(q.SourceError):
            operation()

    with pytest.raises(q.SourceError):
        q.Source().component("not-valid")

    deep = q.coordinate(0)
    with pytest.raises(q.SourceError):
        for _ in range(100):
            deep = q.grad(deep)

    wide = q.coordinate(0)
    with pytest.raises(q.SourceError):
        for _ in range(20):
            wide = wide + wide

    bounded = q.Source().component("Bounded")
    bounded_volume = bounded.volume("volume", dimensions=2)
    with pytest.raises(q.SourceError):
        for index in range(300):
            bounded.field(f"value_{index}", on=bounded_volume, unit=u.m)

    target = tmp_path / "target.eqi"
    target.write_text("preserved", encoding="utf-8")
    link = tmp_path / "link.eqi"
    link.symlink_to(target)
    with pytest.raises(ValueError):
        cylinder_source().write_eqi(link)
    assert target.read_text(encoding="utf-8") == "preserved"


def test_canonical_compiler_owns_expression_shape_diagnostics() -> None:
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(
            source=cylinder_source(velocity_shape=None),
            geometry=cylinder_geometry(),
            parameters=PARAMETERS,
        )
    assert error.value.diagnostics
