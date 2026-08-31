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


def scalar_property_source(*, doc: str = "Reference scalar diffusivity release."):
    source = q.Source()
    contract = source.scalar_property_contract("Diffusivity", unit=u.one)
    release = source.scalar_property_release(
        "ReferenceDiffusivity",
        implements=contract,
        value=25,
        source_unit=u.one,
        source_scale=0.001,
        citation="org.example.measurement",
        license="spdx.CC0_1_0",
        doc=doc,
    )

    law = source.component("PoissonLaw")
    law_region = law.volume("region", dimensions=2)
    law_left = law.boundary("left", parent=law_region)
    law_right = law.boundary("right", parent=law_region)
    law_bottom = law.boundary("bottom", parent=law_region)
    law_top = law.boundary("top", parent=law_region)
    law_source_scale = law.parameter("source_scale", unit=u.one / u.m**2)
    diffusivity = law.property("diffusivity", contract=contract)
    potential = law.field("potential", on=law_region, unit=u.one, initial=0)
    law.relation(
        "balance",
        on=law_region,
        residual=(
            -q.div(diffusivity * q.grad(potential))
            - law_source_scale
        ),
    )
    law.relation("left_value", on=law_left, residual=q.trace(potential))
    law.relation("right_value", on=law_right, residual=q.trace(potential))
    law.relation("bottom_value", on=law_bottom, residual=q.trace(potential))
    law.relation("top_value", on=law_top, residual=q.trace(potential))

    root = source.component("PoissonRectangle")
    root_region = root.volume("region", dimensions=2)
    root_left = root.boundary("left", parent=root_region)
    root_right = root.boundary("right", parent=root_region)
    root_bottom = root.boundary("bottom", parent=root_region)
    root_top = root.boundary("top", parent=root_region)
    root_source_scale = root.parameter("source_scale", unit=u.one / u.m**2)
    root.instance(
        "equation",
        component=law,
        supports={
            law_region: root_region,
            law_left: root_left,
            law_right: root_right,
            law_bottom: root_bottom,
            law_top: root_top,
        },
        parameters={law_source_scale: root_source_scale},
        properties={diffusivity: release},
    )
    return source


def test_removed_source_choice_keywords_are_unexpected() -> None:
    source = q.Source()
    with pytest.raises(TypeError, match="unexpected keyword argument 'public'"):
        source.component("Component", public=True)

    contract_source = q.Source()
    with pytest.raises(TypeError, match="unexpected keyword argument 'public'"):
        contract_source.scalar_property_contract("Diffusivity", unit=u.one, public=True)
    contract = contract_source.scalar_property_contract("Diffusivity", unit=u.one)
    release_arguments = {
        "implements": contract,
        "value": 25,
        "source_unit": u.one,
        "source_scale": 0.001,
        "citation": "org.example.measurement",
        "license": "spdx.CC0_1_0",
    }
    with pytest.raises(TypeError, match="unexpected keyword argument 'public'"):
        contract_source.scalar_property_release(
            "ReferenceDiffusivity", **release_arguments, public=True
        )
    with pytest.raises(TypeError, match="unexpected keyword argument 'validity'"):
        contract_source.scalar_property_release(
            "ReferenceDiffusivity",
            **release_arguments,
            validity="unconditional",
        )

    component = contract_source.component("Component")
    volume = component.volume("volume", dimensions=2)
    for operation in (
        lambda: component.volume("other_volume", dimensions=2, public=True),
        lambda: component.boundary("boundary", parent=volume, public=True),
        lambda: component.parameter("parameter", unit=u.one, public=True),
        lambda: component.property("property", contract=contract, public=True),
    ):
        with pytest.raises(TypeError, match="unexpected keyword argument 'public'"):
            operation()


def rectangle_geometry():
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
    return graph.build(
        rectangle,
        named_topology={
            "region": rectangle.region,
            "left": rectangle.boundaries[0],
            "right": rectangle.boundaries[1],
            "bottom": rectangle.boundaries[2],
            "top": rectangle.boundaries[3],
        },
    )


def scalar_primal_source():
    source = q.Source()
    law = source.component("ScalarDiffusion")
    region = law.volume("region", dimensions=2)
    diffusion = law.parameter("diffusion", unit=u.one)
    wave_number = law.parameter("wave_number", unit=u.one / u.m)
    source_scale = law.parameter("source_scale", unit=u.one / u.m**2)
    potential = law.field("potential", on=region, unit=u.one, initial=0)
    balance = law.relation(
        "balance",
        on=region,
        residual=(
            -q.div(diffusion * q.grad(potential))
            - source_scale * q.sin(q.math.pi * wave_number * q.coordinate(0))
        ),
    )
    law.primal_form(
        balance,
        left=q.integrate(
            region,
            q.dot(q.grad(q.test(potential)), diffusion * q.grad(potential)),
        ),
        right=q.integrate(
            region,
            q.test(potential)
            * source_scale
            * q.sin(q.math.pi * wave_number * q.coordinate(0)),
        ),
        doc="Authored scalar primal form.",
    )
    return source


def test_python_source_emits_and_fresh_compile_inspects_scalar_primal_form(
    tmp_path: Path,
) -> None:
    source = scalar_primal_source()
    text = source.to_eqi()
    assert "form primal for balance" in text
    assert "// Authored scalar primal form." in text
    assert text.count("math.pi") == 2

    model = eqiora.compile(
        source=source,
        geometry=rectangle_geometry(),
        parameters={"diffusion": 1.0, "wave_number": 2.0, "source_scale": 2.0},
    )
    assert len(model.authored_formulations) == 1
    form = model.authored_formulations[0]
    assert form.kind == "primal"
    assert len(form.source_identity) == 64
    assert form.filename == "<python-source>"
    assert form.trial_field_id in model.field_ids

    path = tmp_path / "scalar-primal.eqi"
    source.write_eqi(path)
    emitted = eqiora.compile(
        path=path,
        geometry=rectangle_geometry(),
        parameters={"diffusion": 1.0, "wave_number": 2.0, "source_scale": 2.0},
    )
    assert emitted.digest == model.digest

    replayed = eqiora.Model.from_bytes(model.to_bytes())
    assert replayed.authored_formulations == ()


def test_uninitialized_scalar_field_compiles_from_source_and_emitted_file(
    tmp_path: Path,
) -> None:
    source = q.Source()
    law = source.component("AlgebraicField")
    region = law.volume("region", dimensions=2)
    potential = law.field("potential", on=region, unit=u.one)
    law.relation("balance", on=region, residual=potential)

    text = source.to_eqi()
    assert "field potential on region as space: 1;" in text
    assert "field potential on region as space: 1 =" not in text

    direct = eqiora.compile(source=source, geometry=rectangle_geometry())
    path = tmp_path / "uninitialized-scalar.eqi"
    source.write_eqi(path)
    emitted = eqiora.compile(path=path, geometry=rectangle_geometry())
    assert direct.digest == emitted.digest


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


def test_scalar_property_source_emits_for_the_exact_package_path(
    tmp_path: Path,
) -> None:
    first = scalar_property_source()
    second = scalar_property_source()
    assert first.to_eqi() == second.to_eqi()
    assert "public property contract Diffusivity" in first.to_eqi()
    assert "property diffusivity = ReferenceDiffusivity" in first.to_eqi()

    with pytest.raises(q.SourceError, match="requires an exact Model Package"):
        eqiora.compile(
            source=first,
            geometry=rectangle_geometry(),
            component="PoissonRectangle",
            parameters={"source_scale": 1.0},
        )
    path = tmp_path / "property-poisson.eqi"
    first.write_eqi(path)
    assert path.read_text(encoding="utf-8") == first.to_eqi()
    assert scalar_property_source(doc="Different release documentation.").to_eqi().replace(
        "// Different release documentation.\n", ""
    ) == first.to_eqi().replace("// Reference scalar diffusivity release.\n", "")


def test_scalar_property_source_owns_exact_handles_and_complete_binding() -> None:
    source = q.Source()
    contract = source.scalar_property_contract("Diffusivity", unit=u.one)
    with pytest.raises(TypeError):
        q.PropertyContract()
    with pytest.raises(TypeError):
        q.PropertyRelease()
    with pytest.raises(q.SourceError, match="strictly positive"):
        source.scalar_property_release(
            "ReferenceDiffusivity",
            implements=contract,
            value=25,
            source_unit=u.one,
            source_scale=0,
            citation="org.example.measurement",
            license="spdx.CC0_1_0",
        )
    with pytest.raises(q.SourceError, match="citation identity"):
        source.scalar_property_release(
            "ReferenceDiffusivity",
            implements=contract,
            value=25,
            source_unit=u.one,
            source_scale=0.001,
            citation="not/a/name/path",
            license="spdx.CC0_1_0",
        )
    release = source.scalar_property_release(
        "ReferenceDiffusivity",
        implements=contract,
        value=25,
        source_unit=u.one,
        source_scale=0.001,
        citation="org.example.measurement",
        license="spdx.CC0_1_0",
    )
    with pytest.raises(AttributeError):
        contract.name = "Other"
    with pytest.raises(AttributeError):
        release.value = 1

    foreign = q.Source()
    foreign_component = foreign.component("Foreign")
    with pytest.raises(q.SourceError, match="belong to this Source"):
        foreign_component.property("diffusivity", contract=contract)

    consumer = source.component("Consumer")
    requirement = consumer.property("diffusivity", contract=contract)
    root = source.component("Root")
    with pytest.raises(q.SourceError, match="property bindings must be complete"):
        root.instance(
            "equation",
            component=consumer,
            supports={},
            parameters={},
            properties={},
        )
    root.instance(
        "equation",
        component=consumer,
        supports={},
        parameters={},
        properties={requirement: release},
    )
    assert "property diffusivity = ReferenceDiffusivity" in source.to_eqi()
    with pytest.raises(q.SourceError):
        source.component("Third")


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

    assert isinstance(q.math.pi, q.Expression)
    assert q.math.pi is q.math.pi
    with pytest.raises(TypeError):
        float(q.math.pi)
    with pytest.raises(AttributeError):
        q.math.pi = left_value
    with pytest.raises(q.SourceError, match="different Source"):
        q.math.pi + left_value + right_value

    with pytest.raises(q.SourceError):
        q.Source().component("not-valid")
    with pytest.raises(q.SourceError):
        q.Source().component("a" * 1025)
    with pytest.raises(q.SourceError):
        left_component.field("huge", on=left_volume, unit=u.m, initial=1 << 1025)

    deep = q.math.pi
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
    assert any(
        diagnostic.source_span is not None
        and diagnostic.source_span[0] == "<python-source>"
        for diagnostic in error.value.diagnostics
    )
