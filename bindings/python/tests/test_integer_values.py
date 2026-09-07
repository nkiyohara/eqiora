"""Exact integer values use the common typed literal and selected Model owners."""

import pytest

import eqiora


VALUES = (-(2**63), -(2**53 + 1), 0, 2**53, 2**53 + 1, 2**53 + 2, 2**63 - 1)


@pytest.mark.parametrize("value", VALUES)
def test_integer_creation_source_read_edit_and_replay_preserve_every_bit(value):
    kind = eqiora.ValueType.integer()
    assert kind.to_eqi() == "integer"
    assert kind.scalar_domain == "integer"
    parameter = eqiora.Parameter("count", value_type=kind, value=value)
    assert type(parameter.value) is int
    assert parameter.value == value
    native = eqiora.Model.define("Integers", parameter)
    source = eqiora.compile(source=f"model Integers() {{ parameter count: integer = {value}; }}")
    assert native.structural_fingerprint == source.structural_fingerprint
    for model in (native, source, eqiora.Model.from_bytes(native.to_bytes())):
        reference = model.parameter("count")
        assert reference.value_type == kind
        assert type(reference.value) is int
        assert reference.value == value
        assert len({reference, model.parameter("count")}) == 1
        next_value = value - 1 if value == 2**63 - 1 else value + 1
        changed = model.commit(model.preview_value_edit("count", next_value))
        assert reference.value == value
        assert changed.parameter("count").value == next_value
        assert changed.digest != model.digest
        assert eqiora.Model.from_bytes(changed.to_bytes()).parameter("count").value == next_value


def test_integer_source_builder_and_external_binding_preserve_adjacent_values(tmp_path):
    q = eqiora.lang
    source = q.Source()
    owner = source.model("Selected")
    count = owner.parameter("count", value_type=eqiora.ValueType.integer())
    owner.set_default(count, 2**53 + 1)
    text = source.to_eqi()
    assert "9007199254740993" in text
    authored = eqiora.compile(source=source, entry="Selected")
    assert authored.parameter("count").value == 2**53 + 1
    path = tmp_path / "integer.eqi"
    source.write_eqi(path)
    assert eqiora.compile(path=path, entry="Selected").to_bytes() == authored.to_bytes()
    for value in (2**53, 2**53 + 1, 2**53 + 2):
        bound = eqiora.compile(source=source, entry="Selected", bindings={"count": value})
        assert type(bound.parameter("count").value) is int
        assert bound.parameter("count").value == value


def test_integer_channel_values_retain_shape_and_component_bits():
    kind = eqiora.ValueType.array(eqiora.ValueType.array(eqiora.ValueType.integer(), 2), 2)
    values = ((2**53, 2**53 + 1), (2**53 + 2, -(2**63)))
    parameter = eqiora.Parameter("channels", value_type=kind, value=values)
    assert parameter.value == values
    model = eqiora.Model.define("Channels", parameter)
    assert eqiora.Model.from_bytes(model.to_bytes()).parameter("channels").value == values
    for value in (1, [[1, 2]], [[1, 2], [3]], [[1, 2], [3, True]]):
        with pytest.raises((TypeError, ValueError)):
            eqiora.Parameter("bad", value_type=kind, value=value)


@pytest.mark.parametrize("value", (True, False, 1.0, 1 + 0j, -(2**63) - 1, 2**63))
def test_integer_native_creation_and_edits_reject_non_integer_or_out_of_range(value):
    kind = eqiora.ValueType.integer()
    with pytest.raises((TypeError, ValueError, OverflowError)):
        eqiora.Parameter("count", value_type=kind, value=value)
    model = eqiora.Model.define("Bounded", eqiora.Parameter("count", value_type=kind, value=1))
    before = model.to_bytes()
    with pytest.raises((TypeError, ValueError, OverflowError)):
        model.preview_value_edit("count", value)
    assert model.to_bytes() == before


@pytest.mark.parametrize("value", (True, -(2**63) - 1, 2**63))
def test_integer_external_bindings_reject_bool_and_overflow(value):
    with pytest.raises((TypeError, ValueError, OverflowError, eqiora.ValidationError)):
        eqiora.compile(source="model Selected(parameter count: integer) {}", entry="Selected", bindings={"count": value})


def test_untyped_native_parameter_keeps_real_default():
    parameter = eqiora.Parameter("real", value=3)
    assert parameter.value_type == eqiora.ValueType.real()
    assert type(parameter.value) is float


def test_integer_source_functions_retain_exact_values_and_explicit_conversion():
    q = eqiora.lang
    source = q.Source()
    owner = source.model("Arithmetic")
    count = owner.parameter("count", value_type=eqiora.ValueType.integer())
    owner.set_default(count, 2**53 + 1)
    for name, expression in (
        ("adjacent", count + 1),
        ("quotient", q.quotient(-7, 3)),
        ("remainder", q.remainder(-7, 3)),
        ("converted", q.to_integer(4.0)),
    ):
        parameter = owner.parameter(name, value_type=eqiora.ValueType.integer())
        owner.set_default(parameter, expression)
    rounded = owner.parameter("rounded", value_type=eqiora.ValueType.real())
    owner.set_default(rounded, q.to_real(count))
    model = eqiora.compile(source=source, entry="Arithmetic")
    assert model.parameter("adjacent").value == 2**53 + 2
    assert model.parameter("quotient").value == -2
    assert model.parameter("remainder").value == -1
    assert model.parameter("converted").value == 4
    assert model.parameter("rounded").value == float(2**53)


def test_integer_function_authoring_retains_lexical_ownership_and_bounds():
    q = eqiora.lang
    source = q.Source()
    left = source.component("Left")
    right = source.component("Right")
    a = left.parameter("n", value_type=eqiora.ValueType.integer())
    b = right.parameter("n", value_type=eqiora.ValueType.integer())
    for function in (q.quotient, q.remainder):
        with pytest.raises(q.SourceError, match="owners"):
            function(a, b)
        with pytest.raises(TypeError):
            function(a, True)
    for function in (q.to_integer, q.to_real):
        with pytest.raises(q.SourceError, match="owner|Component"):
            right.let_alias("foreign", function(a))
