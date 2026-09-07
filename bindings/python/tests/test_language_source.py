from pathlib import Path
from fractions import Fraction

import pytest

import eqiora


q = eqiora.lang
u = q.units


def test_source_power_preserves_python_negative_base_grouping():
    source = q.Source()
    component = source.component("Powers")
    body = component.volume("body", dimensions=1)
    x = component.parameter("x", value_type=eqiora.ValueType.real(eqiora.Dimension()))
    cases = [
        ("negative_power", -(x**2), "-x ^ 2 = 0;"),
        ("negative_base", (-x)**2, "(-x) ^ 2 = 0;"),
        ("signed_power", x**-2, "x ^ -2 = 0;"),
        ("quantity_base", q.quantity(-2, u.one)**2, "(-2 [1]) ^ 2 = 0;"),
    ]
    for name, expression, _ in cases:
        component.relation(name, on=body, left=expression, right=0)
    emitted = source.to_eqi()
    for _, _, text in cases:
        assert text in emitted


def test_documentation_emits_attached_paragraphs_with_utf8_byte_bound():
    source = q.Source()
    source.component("Documented", doc="Summary.\n\nFurther **prose**.")
    assert "/// Summary.\n///\n/// Further **prose**.\n" in source.to_eqi()
    bounded = q.Source()
    bounded.component("Bounded", doc="é" * 8192)
    assert "/// " + "é" * 8192 in bounded.to_eqi()
    with pytest.raises(q.SourceError, match="16384-byte limit"):
        q.Source().component("TooLong", doc="é" * 8193)


@pytest.mark.parametrize("value, syntax", [
    (0, "0"),
    (q.quantity(2, u.one / u.m**2), "2 [(1 / (m ^ 2))]"),
    (q.quantity(-2, u.one / u.m**2), "-2 [(1 / (m ^ 2))]"),
])
def test_component_parameter_bindings_emit_explicit_units(value, syntax):
    source = scalar_property_source(binding=value)
    assert f"source_scale = {syntax}" in source.to_eqi()


def test_rational_unit_exponents_preserve_exact_source_spelling():
    source = q.Source()
    component = source.component("Wave")
    component.parameter("amplitude", value_type=eqiora.ValueType.real(eqiora.Dimension(length=Fraction(-1, 2))))
    text = source.to_eqi()
    assert "m ^ (-1 / 2)" in text
    for exponent in [True, 0.5, 1.0]:
        with pytest.raises(TypeError):
            u.m ** exponent
    for exponent in [2147483648, -2147483648, Fraction(1, 2147483648)]:
        with pytest.raises(ValueError):
            u.m ** exponent


def test_input_quantities_compile_from_python_and_emitted_source(tmp_path: Path):
    source = q.Source()
    law = source.component("RootLength")
    region = law.volume("region", dimensions=2)
    length = law.field("length", role=eqiora.FieldRole.Variable, on=region, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))
    law.relation(
        "balance", on=region,
        right=0, left=length - q.math.sqrt(q.quantity(4, u.m.prefixed("m") ** 2)),
    )
    assert "4 [(mm ^ 2)]" in source.to_eqi()
    direct = eqiora.compile(source=source, geometry=rectangle_geometry())
    path = tmp_path / "quantity.eqi"
    source.write_eqi(path)
    emitted = eqiora.compile(path=path, geometry=rectangle_geometry())
    assert direct.digest == emitted.digest
    assert eqiora.Model.from_bytes(direct.to_bytes()).digest == direct.digest
    for unit in [u.kg, u.one, u.s.prefixed("m"), u.m / u.s]:
        with pytest.raises(ValueError):
            unit.prefixed("m")
    for prefix in ["MILLI", "µ", "", "kk"]:
        with pytest.raises(ValueError):
            u.s.prefixed(prefix)
    for value in [True, float("inf"), float("nan")]:
        with pytest.raises((TypeError, ValueError)):
            q.quantity(value, u.s)
    with pytest.raises(TypeError):
        q.quantity(1, "s")


def cylinder_source(
    *,
    doc: str = "Equations-only steady incompressible flow component.",
    velocity_type=eqiora.ValueType.vector(eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1)), 2),
):
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

    velocity = stokes.field("velocity", role=eqiora.FieldRole.Variable, on=fluid, value_type=velocity_type)
    pressure = stokes.field("pressure", role=eqiora.FieldRole.Variable, on=fluid, value_type=eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-2)))
    force_potential = stokes.field(
        "force_potential", role=eqiora.FieldRole.Variable, on=fluid, value_type=eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-2))
    )
    inlet_profile = stokes.field("inlet_profile", role=eqiora.FieldRole.Variable, on=fluid, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1)))

    stokes.relation(
        "force_definition", on=fluid, left=force_potential, right=zero_pressure
    )
    stokes.relation(
        "inlet_profile_definition",
        on=fluid,
        left=inlet_profile,
        right=(
            4
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
        right=0, left=-q.div(stress) - q.grad(force_potential),
        doc="Steady Stokes momentum balance.",
    )
    stokes.relation("incompressibility", on=fluid, right=0, left=q.div(velocity))
    stokes.relation(
        "inlet_velocity",
        on=inlet,
        right=0, left=q.trace(velocity) + q.normal(q.isotropic_lift(inlet_profile)),
    )
    stokes.relation("outlet_traction", on=outlet, right=0, left=q.normal(stress))
    stokes.relation("wall_velocity", on=walls, right=0, left=q.trace(velocity))
    stokes.relation("cylinder_velocity", on=cylinder, right=0, left=q.trace(velocity))
    return source


def test_relation_requires_ordered_left_and_right_including_zero() -> None:
    source = q.Source()
    component = source.component("NaturalEquation")
    body = component.volume("body", dimensions=2)
    value = component.field("value", role=eqiora.FieldRole.Variable, on=body, value_type=eqiora.ValueType.real())
    source_scale = component.parameter("source_scale", value_type=eqiora.ValueType.real())

    natural = component.relation(
        "natural",
        on=body,
        left=q.div(q.grad(value)),
        right=-source_scale,
        doc="Natural equation.",
    )
    residual = component.relation("residual", on=body, right=0, left=value)
    assert isinstance(natural, q.Relation)
    assert isinstance(residual, q.Relation)
    with pytest.raises(TypeError):
        q.Relation()
    with pytest.raises(AttributeError):
        natural.name = "other"

    text = source.to_eqi()
    assert "/// Natural equation." in text
    assert "div(grad(value)) = -source_scale;" in text
    assert "value = 0;" in text


def test_relation_rejects_mixed_incomplete_foreign_and_nonfinite_equations() -> None:
    source = q.Source()
    component = source.component("Relations")
    body = component.volume("body", dimensions=2)
    value = component.field("value", role=eqiora.FieldRole.Variable, on=body, value_type=eqiora.ValueType.real())
    foreign = q.Source()
    foreign_component = foreign.component("Foreign")
    foreign_body = foreign_component.volume("body", dimensions=2)
    foreign_value = foreign_component.field("value", role=eqiora.FieldRole.Variable, on=foreign_body, value_type=eqiora.ValueType.real())

    invalid = (
        lambda: component.relation("missing", on=body),
        lambda: component.relation("left_only", on=body, left=value),
        lambda: component.relation("right_only", on=body, right=value),
        lambda: component.relation(
            "mixed", on=body, residual=value, left=value, right=0
        ),
        lambda: component.relation(
            "foreign", on=body, left=value, right=foreign_value
        ),
        lambda: component.relation(
            "nonfinite", on=body, left=value, right=float("nan")
        ),
    )
    for operation in invalid:
        with pytest.raises((q.SourceError, TypeError)):
            operation()


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


def scalar_property_source(*, doc: str = "Reference scalar diffusivity release.", binding=None, alias_binding=False):
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
    law_source_scale = law.parameter("source_scale", value_type=eqiora.ValueType.real(eqiora.Dimension(length=-2)))
    diffusivity = law.property("diffusivity", contract=contract)
    potential = law.field("potential", role=eqiora.FieldRole.Variable, on=law_region, value_type=eqiora.ValueType.real())
    law.relation(
        "balance",
        on=law_region,
        right=0, left=(
            -q.div(diffusivity * q.grad(potential))
            - law_source_scale
        ),
    )
    law.relation("left_value", on=law_left, right=0, left=q.trace(potential))
    law.relation("right_value", on=law_right, right=0, left=q.trace(potential))
    law.relation("bottom_value", on=law_bottom, right=0, left=q.trace(potential))
    law.relation("top_value", on=law_top, right=0, left=q.trace(potential))

    root = source.component("PoissonRectangle")
    root_region = root.volume("region", dimensions=2)
    root_left = root.boundary("left", parent=root_region)
    root_right = root.boundary("right", parent=root_region)
    root_bottom = root.boundary("bottom", parent=root_region)
    root_top = root.boundary("top", parent=root_region)
    root_source_scale = root.parameter("source_scale", value_type=eqiora.ValueType.real(eqiora.Dimension(length=-2)))
    if alias_binding:
        binding = root.let_alias("adjusted_source", root_source_scale * 2)
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
        parameters={law_source_scale: root_source_scale if binding is None else binding},
        properties={diffusivity: release},
    )
    return source


def material_composition_source() -> q.Source:
    source = q.Source()
    conductivity_contract = source.scalar_property_contract("Conductivity", unit=u.one)
    capacity_contract = source.scalar_property_contract("Capacity", unit=u.one)
    conductivity_release = source.scalar_property_release(
        "ConductivityA",
        implements=conductivity_contract,
        value=2,
        source_unit=u.one,
        source_scale=1,
        citation="org.example.a",
        license="spdx.CC0_1_0",
    )
    capacity_release = source.scalar_property_release(
        "CapacityA",
        implements=capacity_contract,
        value=4,
        source_unit=u.one,
        source_scale=1,
        citation="org.example.a",
        license="spdx.CC0_1_0",
    )
    law = source.component("DiffusionLaw")
    region = law.volume("region", dimensions=2)
    conductivity = law.property("conductivity", contract=conductivity_contract)
    capacity = law.property("capacity", contract=capacity_contract)
    law.relation("law", on=region, right=0, left=conductivity / capacity)
    material = source.material_composition(
        "MaterialA",
        properties={
            "conductivity": conductivity_release,
            "capacity": capacity_release,
        },
    )
    root = source.component("UseMaterial")
    root_region = root.volume("region", dimensions=2)
    root.instance(
        "law",
        component=law,
        supports={region: root_region},
        parameters={},
        material=material,
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
        lambda: component.parameter("parameter", value_type=eqiora.ValueType.real(), public=True),
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
    diffusion = law.parameter("diffusion", value_type=eqiora.ValueType.real())
    wave_number = law.parameter("wave_number", value_type=eqiora.ValueType.real(eqiora.Dimension(length=-1)))
    source_scale = law.parameter("source_scale", value_type=eqiora.ValueType.real(eqiora.Dimension(length=-2)))
    potential = law.field("potential", role=eqiora.FieldRole.Variable, on=region, value_type=eqiora.ValueType.real())
    balance = law.relation(
        "balance",
        on=region,
        right=0, left=(
            -q.div(diffusion * q.grad(potential))
            - source_scale * q.math.sin(q.math.pi * wave_number * q.coordinate(0))
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
            * q.math.sin(q.math.pi * wave_number * q.coordinate(0)),
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
    assert "/// Authored scalar primal form." in text
    assert text.count("math.pi") == 2
    assert text.count("math.sin") == 2
    assert not hasattr(q, "sin")

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
    potential = law.field("potential", role=eqiora.FieldRole.Variable, on=region, value_type=eqiora.ValueType.real())
    law.relation("balance", on=region, right=0, left=potential)

    text = source.to_eqi()
    assert "variable potential: 1 on region;" in text
    assert "variable potential: 1 =" not in text

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
    assert "/// Equations-only steady incompressible flow component." in first.to_eqi()

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
        "/// Different release documentation.\n", ""
    ) == first.to_eqi().replace("/// Reference scalar diffusivity release.\n", "")


def test_material_composition_emits_one_ordered_typed_binding_set() -> None:
    text = material_composition_source().to_eqi()
    assert "public material composition MaterialA" in text
    assert text.index("property capacity = CapacityA") < text.index(
        "property conductivity = ConductivityA"
    )
    assert "material = MaterialA" in text
    with pytest.raises(TypeError):
        q.MaterialComposition()


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
    left_value = left_component.field("value", role=eqiora.FieldRole.Variable, on=left_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))
    right = q.Source()
    right_component = right.component("Right")
    right_volume = right_component.volume("right", dimensions=2)
    right_value = right_component.field("value", role=eqiora.FieldRole.Variable, on=right_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))

    invalid = (
        lambda: left_value + right_value,
        lambda: left_component.boundary("foreign_parent", parent=right_volume),
        lambda: left_component.field("wrong_support", role=eqiora.FieldRole.Variable, on=right_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1))),
        lambda: left_component.field("value", role=eqiora.FieldRole.Variable, on=left_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1))),
        lambda: left_value + float("nan"),
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
        left_value + (1 << 1025)

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
            bounded.field(f"value_{index}", role=eqiora.FieldRole.Variable, on=bounded_volume, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))

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
            source=cylinder_source(velocity_type=eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1))),
            geometry=cylinder_geometry(),
            parameters=PARAMETERS,
        )
    assert error.value.diagnostics
    assert any(
        diagnostic.source_span is not None
        and diagnostic.source_span[0] == "<python-source>"
        for diagnostic in error.value.diagnostics
    )


def test_static_alias_authoring_emits_private_typed_immutable_expressions():
    source = q.Source()
    component = source.component("Aliases")
    pressure = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-2))
    supplied = component.parameter("supplied", value_type=pressure)
    doubled = component.let_alias("doubled", supplied * 2, value_type=pressure,
                                  doc="Twice the supplied pressure.")
    component.let_alias("opposite", -doubled)
    assert type(doubled) is q.Expression
    with pytest.raises(AttributeError):
        doubled.name = "changed"
    text = source.to_eqi()
    assert f"/// Twice the supplied pressure.\n  let doubled: {pressure.to_eqi()} = supplied * 2;" in text
    assert "let opposite = -doubled;" in text
    assert "public parameter doubled" not in text
    with pytest.raises(q.SourceError, match="frozen"):
        component.let_alias("late", 1)


def test_static_alias_authoring_rejects_foreign_values_and_invalid_assertions():
    component = q.Source().component("Owner")
    foreign = q.Source().component("Foreign").parameter("input", value_type=eqiora.ValueType.real())
    with pytest.raises(q.SourceError, match="this Component"):
        component.let_alias("foreign", foreign + 1)
    with pytest.raises(TypeError, match="eqiora.ValueType"):
        component.let_alias("invalid", 1, value_type=u.one)
    with pytest.raises(q.SourceError, match="finite"):
        component.let_alias("invalid", float("nan"))
    component.let_alias("invalid", 1)
    with pytest.raises(q.SourceError, match="duplicate"):
        component.parameter("invalid", value_type=eqiora.ValueType.real())


def test_static_alias_authoring_bounds_total_expression_nodes():
    component = q.Source().component("BoundedAliases")
    expression = q.math.pi
    for _ in range(11):
        expression = expression + expression
    component.let_alias("large", expression)
    component.let_alias("last", 0)
    with pytest.raises(q.SourceError, match="alias expressions exceed the 4096-node limit"):
        component.let_alias("overflow", 0)


def test_static_alias_compiles_without_required_binding_or_edit_target(tmp_path):
    source = q.Source()
    component = source.component("StaticAlias")
    region = component.volume("region", dimensions=2)
    supplied = component.parameter("supplied", value_type=eqiora.ValueType.real())
    doubled = component.let_alias("doubled", supplied * 2)
    value = component.field("value", on=region, role=eqiora.FieldRole.Variable,
                            value_type=eqiora.ValueType.real())
    component.relation("balance", on=region, left=value, right=doubled)
    model = eqiora.compile(source=source, geometry=rectangle_geometry(), parameters={"supplied": 3.0})
    assert len(model.parameter_ids) == 1
    assert len(model.field_ids) == 1
    with pytest.raises(eqiora.EqioraError, match="Parameter"):
        model.preview_value_edit("doubled", 9.0)
    path = tmp_path / "static-alias.eqi"
    source.write_eqi(path)
    replay = eqiora.compile(path=path, geometry=rectangle_geometry(), parameters={"supplied": 3.0})
    assert replay.structural_fingerprint == model.structural_fingerprint


def test_static_alias_nested_parameter_binding_uses_expression_rhs():
    source = scalar_property_source(alias_binding=True)
    text = source.to_eqi()
    assert "let adjusted_source = source_scale * 2;" in text
    assert "source_scale = adjusted_source" in text
    model = eqiora.compile(source=source, geometry=rectangle_geometry(), parameters={"source_scale": 3.0})
    with pytest.raises(eqiora.EqioraError, match="Parameter"):
        model.preview_value_edit("adjusted_source", 9.0)


def test_static_alias_authoring_cannot_bind_private_alias_as_parameter():
    source = q.Source()
    source.scalar_property_contract("Scalar", unit=u.one)
    child = source.component("Child")
    required = child.parameter("required", value_type=eqiora.ValueType.real())
    private = child.let_alias("private", required * 2)
    root = source.component("Root")
    rhs = root.let_alias("rhs", 3)
    with pytest.raises(q.SourceError, match="Parameter bindings must be complete and exact"):
        root.instance("child", component=child, supports={}, parameters={required: rhs, private: 1})
    root.instance("child", component=child, supports={}, parameters={required: rhs})


@pytest.mark.parametrize("kind", ["parameter", "alias", "compound", "trace", "property"])
def test_static_alias_authoring_rejects_same_source_sibling_capture(kind):
    source = q.Source()
    contract = source.scalar_property_contract("Scalar", unit=u.one)
    release = source.scalar_property_release(
        "Unit", implements=contract, value=1, source_unit=u.one, source_scale=1,
        citation="org.example.unit", license="spdx.CC0_1_0",
    )
    left = source.component("Left")
    left_region = left.volume("region", dimensions=2)
    left_parameter = left.parameter("supplied", value_type=eqiora.ValueType.real())
    left_alias = left.let_alias("derived", left_parameter * 2)
    left_field = left.field("value", on=left_region, role=eqiora.FieldRole.Variable,
                           value_type=eqiora.ValueType.real())
    left_property = left.property("coefficient", contract=contract)
    right = source.component("Right")
    right_region = right.volume("region", dimensions=2)
    right_parameter = right.parameter("supplied", value_type=eqiora.ValueType.real())
    right.let_alias("derived", right_parameter * 3)
    right.property("coefficient", contract=contract)
    right_relation = right.relation("own", on=right_region, left=right_parameter, right=0)
    foreign = {
        "parameter": left_parameter,
        "alias": left_alias,
        "compound": -(left_alias + 1) * 2,
        "trace": q.trace(left_field),
        "property": left_property,
    }[kind]
    for operation in (
        lambda: right.let_alias("captured", foreign),
        lambda: right.relation("captured", on=right_region, left=foreign, right=0),
        lambda: q.integrate(right_region, foreign),
        lambda: foreign + right_parameter,
        lambda: right.primal_form(right_relation, left=q.integrate(left_region, foreign),
                                  right=q.integrate(right_region, right_parameter)),
        lambda: right.instance("child", component=left, supports={left_region: right_region},
                               parameters={left_parameter: foreign}, properties={left_property: release}),
    ):
        with pytest.raises(q.SourceError, match="Component"):
            operation()
