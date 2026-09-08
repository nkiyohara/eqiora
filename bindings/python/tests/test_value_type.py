from fractions import Fraction

import pytest

import eqiora


def test_value_type_preserves_scalar_domain_dimension_and_axis_roles() -> None:
    dimension = eqiora.Dimension(length=Fraction(-3, 2))
    scalar = eqiora.ValueType.complex(dimension)
    vector = eqiora.ValueType.vector(scalar, 2)
    channels = eqiora.ValueType.array(vector, 3)
    tensor = eqiora.ValueType.tensor(scalar, 3, 2)
    array = eqiora.ValueType.array(eqiora.ValueType.array(scalar, 2), 3)

    assert channels.scalar_domain == "complex"
    assert channels.dimension == dimension
    assert channels.shape == tensor.shape == array.shape == [3, 2]
    assert channels.array_rank == 1
    assert tensor.array_rank == 0
    assert array.array_rank == 2
    assert channels.frame == tensor.frame == "spatial_cartesian"
    assert array.frame == "invariant"
    assert len({channels, tensor, array}) == 3
    assert scalar == eqiora.ValueType.complex(dimension)
    assert scalar != eqiora.ValueType.real(dimension)
    assert hash(scalar) == hash(eqiora.ValueType.complex(dimension))
    with pytest.raises(AttributeError):
        channels.array_rank = 0


@pytest.mark.parametrize("constructor", [
    lambda: eqiora.ValueType.vector(eqiora.ValueType.real(), 0),
    lambda: eqiora.ValueType.tensor(eqiora.ValueType.real(), 2),
    lambda: eqiora.ValueType.array(eqiora.ValueType.real(), 0),
    lambda: eqiora.ValueType.vector(eqiora.ValueType.array(eqiora.ValueType.real(), 2), 2),
    lambda: eqiora.ValueType.tensor(eqiora.ValueType.vector(eqiora.ValueType.real(), 2), 2, 2),
])
def test_invalid_value_type_constructors(constructor) -> None:
    with pytest.raises(ValueError):
        constructor()


@pytest.mark.parametrize("invalid", [True, False, 2.0, Fraction(2), "2"])
def test_extents_require_integers(invalid) -> None:
    scalar = eqiora.ValueType.real()
    for constructor in (
        lambda: eqiora.ValueType.vector(scalar, invalid),
        lambda: eqiora.ValueType.tensor(scalar, 2, invalid),
        lambda: eqiora.ValueType.array(scalar, invalid),
    ):
        with pytest.raises(TypeError):
            constructor()


@pytest.mark.parametrize(("value_type", "syntax"), [
    (eqiora.ValueType.real(eqiora.Dimension(mass=1, length=-1, time=-2)), "kg / (m * s ^ 2)"),
    (eqiora.ValueType.complex(eqiora.Dimension(length=Fraction(-3, 2))), "complex<m ^ (-3 / 2)>"),
    (eqiora.ValueType.vector(eqiora.ValueType.complex(), 2), "vector<complex<1>, 2>"),
    (eqiora.ValueType.tensor(eqiora.ValueType.real(), 2, 2), "tensor<1, 2, 2>"),
    (eqiora.ValueType.array(eqiora.ValueType.vector(eqiora.ValueType.complex(), 2), 3), "array<vector<complex<1>, 2>, 3>"),
])
def test_native_field_type_matches_source_and_replays(value_type, syntax) -> None:
    domain = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))

    field = eqiora.Field("u", role=eqiora.FieldRole.Variable, domain=domain,
                         value_type=value_type)
    balance = eqiora.Relation("balance", domain=domain, equations=[(field - field, 0)])
    native = eqiora.Model.define("typed", domain, field, balance)
    source = eqiora.compile(source=f"""
model typed() {{
  domain body = box(0, 1, 0, 1);

  variable u: {syntax} on body;
  relation balance on body {{ u - u = 0; }}
}}
""")
    assert field.value_type == value_type
    assert field.dimension == value_type.dimension
    assert native.structural_fingerprint == source.structural_fingerprint
    replay = eqiora.Model.from_bytes(native.to_bytes())
    assert replay.to_bytes() == native.to_bytes()
    assert replay.structural_fingerprint == native.structural_fingerprint


def test_spatial_type_requires_matching_support() -> None:
    domain = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))

    field = eqiora.Field("u", role=eqiora.FieldRole.Variable, domain=domain,
                         value_type=eqiora.ValueType.vector(eqiora.ValueType.real(), 3))
    balance = eqiora.Relation("balance", domain=domain, equations=[(field - field, 0)])
    with pytest.raises(eqiora.EqioraError) as caught:
        eqiora.Model.define("typed", domain, field, balance)
    assert caught.value.diagnostics[0].graph_path == ["typed", "u"]

def test_field_has_one_type_input_and_requires_explicit_role() -> None:
    field = eqiora.Field("u", role=eqiora.FieldRole.Variable)
    assert field.value_type == eqiora.ValueType.real()
    assert field.role == eqiora.FieldRole.Variable
    for kwargs in ({}, {"role": "state"}, {"role": eqiora.FieldRole.State, "initial": 0.0},
                   {"role": eqiora.FieldRole.Variable, "representation": None}):
        with pytest.raises(TypeError):
            eqiora.Field("u", **kwargs)
    with pytest.raises(TypeError):
        eqiora.Field("u", role=eqiora.FieldRole.Variable, dimension=eqiora.Dimension())

def test_source_field_uses_the_shared_type_and_native_formatter() -> None:
    value_type = eqiora.ValueType.array(eqiora.ValueType.vector(
        eqiora.ValueType.complex(eqiora.Dimension(length=Fraction(-3, 2))), 2), 3)
    syntax = value_type.to_eqi()
    assert syntax == "array<vector<complex<m ^ (-3 / 2)>, 2>, 3>"
    source = eqiora.lang.Source()
    component = source.component("Typed")
    body = component.volume("body", dimensions=2)
    component.field("channels", role=eqiora.FieldRole.Variable, on=body, value_type=value_type)
    assert f"variable channels: {syntax} on body;" in source.to_eqi()
    with pytest.raises(TypeError):
        component.field("old", role=eqiora.FieldRole.Variable, on=body, unit=eqiora.units.m)

def test_type_emission_obeys_the_native_source_resource_limit() -> None:
    oversized = eqiora.ValueType.array(eqiora.ValueType.real(), 65_537)
    with pytest.raises(ValueError, match="65536"):
        oversized.to_eqi()

def test_source_parameter_uses_the_shared_type_and_native_formatter() -> None:
    value_type = eqiora.ValueType.array(
        eqiora.ValueType.complex(eqiora.Dimension(length=Fraction(-1, 2))), 3
    )
    source = eqiora.lang.Source()
    component = source.component("TypedParameter")
    component.parameter("amplitude", value_type=value_type)
    assert f"parameter amplitude: {value_type.to_eqi()}," in source.to_eqi()
    with pytest.raises(TypeError):
        component.parameter("old", unit=eqiora.units.m)
    with pytest.raises(TypeError, match="eqiora.ValueType"):
        component.parameter("invalid", value_type=eqiora.units.m)


def test_parameter_declaration_retains_its_complete_type() -> None:
    dimension = eqiora.Dimension(time=Fraction(-1, 2))
    value_type = eqiora.ValueType.array(eqiora.ValueType.complex(dimension), 3)
    parameter = eqiora.Parameter("coefficient", value_type=value_type, value=0.0)
    assert parameter.value_type == value_type
    assert parameter.dimension == dimension
    assert parameter.value == (0j, 0j, 0j)
    assert all(type(component) is complex for component in parameter.value)
    assert eqiora.Parameter("scalar", value=2.0).value_type == eqiora.ValueType.real()
    with pytest.raises(TypeError):
        eqiora.Parameter("old", dimension=dimension, value=1.0)


def test_initial_equations_preserve_native_source_identity_and_foreign_ownership() -> None:
    x = eqiora.Field("x", role=eqiora.FieldRole.State)
    rate = eqiora.Parameter("rate", value_type=eqiora.ValueType.real(eqiora.Dimension(time=-1)), value=1.0)
    flow = eqiora.Relation("flow", equations=[(eqiora.derivative(x) + rate * x, 0)])
    initial = eqiora.Initial((x, 2.0))
    native = eqiora.Model.define("decay", x, rate, flow, initial)
    source = eqiora.compile(source="""
model decay() {
  state x: 1;
  parameter rate: 1 / s = 1;
  relation flow { derivative(x) + rate * x = 0; }
  initial { x = 2; }
}
""")
    assert len(initial.equations) == 1
    assert native.structural_fingerprint == source.structural_fingerprint
    assert eqiora.Model.from_bytes(native.to_bytes()).digest == native.digest
    foreign = eqiora.Field("x", role=eqiora.FieldRole.State)
    with pytest.raises(eqiora.ValidationError, match="foreign|omitted"):
        eqiora.Model.define("foreign", x, rate, flow, eqiora.Initial((foreign, 2.0)))


def test_initial_equations_do_not_broadcast_scalars_to_shaped_fields() -> None:
    channels = eqiora.Field("channels", role=eqiora.FieldRole.State,
                            value_type=eqiora.ValueType.array(eqiora.ValueType.real(), 2))
    with pytest.raises(eqiora.ValidationError):
        eqiora.Model.define("no_broadcast", channels, eqiora.Initial((channels, 1.0)))


def test_value_edits_reject_fields_by_alias_and_exact_identity() -> None:
    model = eqiora.compile(source='model m() { variable x: 1; relation law { x = 1; } }')
    for target in ("x", model.field_ids[0]):
        with pytest.raises(eqiora.EqioraError, match="Parameter"):
            model.preview_value_edit(target, 2.0)


def test_source_field_requires_role_and_rejects_embedded_initial_values() -> None:
    source = eqiora.lang.Source()
    component = source.component("Roles")
    body = component.volume("body", dimensions=1)
    for kwargs in ({}, {"role": "state"}, {"role": eqiora.FieldRole.State, "initial": 0.0}):
        with pytest.raises(TypeError):
            component.field("old", on=body, value_type=eqiora.ValueType.real(), **kwargs)
    component.field("stored", on=body, value_type=eqiora.ValueType.real(), role=eqiora.FieldRole.State)
    text = source.to_eqi()
    assert "support body: volume(ambient_dimension = 1)" in text
    assert "state stored: 1 on body;" in text
    assert "representation" not in text


@pytest.mark.parametrize("value, kind, expected", [
    (1 + 2j, eqiora.ValueType.complex(), 1 + 2j),
    (3, eqiora.ValueType.complex(), 3 + 0j),
    ([[1 + 2j, 3], [4, 5 - 6j]],
     eqiora.ValueType.array(eqiora.ValueType.array(eqiora.ValueType.complex(), 2), 2),
     ((1 + 2j, 3 + 0j), (4 + 0j, 5 - 6j))),
    ([1, 2], eqiora.ValueType.array(eqiora.ValueType.real(), 2), (1.0, 2.0)),
])
def test_parameter_preserves_complete_ordered_value(value, kind, expected):
    parameter = eqiora.Parameter("coefficient", value_type=kind, value=value)
    assert parameter.value == expected
    assert parameter.value_type == kind
    if isinstance(expected, complex):
        assert type(parameter.value) is complex
    else:
        assert type(parameter.value) is tuple


def test_parameter_infers_only_scalar_domain_and_rejects_shape_or_imaginary_loss():
    assert eqiora.Parameter("complex_scalar", value=1j).value_type == eqiora.ValueType.complex()
    for value, kind in [
        (1j, eqiora.ValueType.real()),
        ([1, 2], eqiora.ValueType.real()),
        ([1], eqiora.ValueType.array(eqiora.ValueType.real(), 2)),
        ([[1], [2, 3]], eqiora.ValueType.array(eqiora.ValueType.array(eqiora.ValueType.real(), 2), 2)),
        (1, eqiora.ValueType.array(eqiora.ValueType.real(), 2)),
        (complex(1, float("nan")), eqiora.ValueType.complex()),
    ]:
        with pytest.raises((TypeError, ValueError)):
            eqiora.Parameter("invalid", value_type=kind, value=value)
    with pytest.raises(TypeError):
        eqiora.Parameter("bool", value=True)


def test_complete_native_parameter_matches_source_and_retains_typed_edits():
    kind = eqiora.ValueType.array(eqiora.ValueType.complex(), 2)
    coefficient = eqiora.Parameter("coefficient", value_type=kind, value=[1 + 2j, 3 - 4j])
    field = eqiora.Field("x", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.complex())
    native = eqiora.Model.define("typed", coefficient, field,
                                 eqiora.Relation("law", equations=[(field - coefficient[1], 0)]))
    source = eqiora.compile(source="""
model typed() {
  parameter coefficient: array<complex<1>, 2> = [math.complex(1, 2), math.complex(3, -4)];
  variable x: complex<1>;
  relation law { x - coefficient[1] = 0; }
}
""")
    assert native.structural_fingerprint == source.structural_fingerprint
    changed = native.commit(native.preview_value_edit("coefficient", [1 + 7j, 3 - 4j]))
    assert changed.digest != native.digest
    replay = eqiora.Model.from_bytes(changed.to_bytes())
    assert replay.to_bytes() == changed.to_bytes()
    with pytest.raises(eqiora.EqioraError):
        replay.preview_value_edit("coefficient", [1 + 7j, 3 - 4j])


def test_nonzero_spatial_parameter_requires_an_explicit_registered_frame():
    vector = eqiora.ValueType.vector(eqiora.ValueType.real(), 2)
    coefficient = eqiora.Parameter("coefficient", value_type=vector, value=[1, 2])
    assert coefficient.value == (1.0, 2.0)
    body = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))
    observed = eqiora.Field("observed", role=eqiora.FieldRole.Variable, domain=body)
    law = eqiora.Relation("observe", domain=body, equations=[(observed, 0)])
    with pytest.raises(eqiora.ValidationError, match="frame"):
        eqiora.Model.define("missing_frame", body, coefficient, observed, law)
