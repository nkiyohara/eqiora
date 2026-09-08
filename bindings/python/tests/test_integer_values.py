"""Exact integer values use the common typed literal and selected Model owners."""

import pytest

import eqiora


def native_model(name, *declarations):
    observed = eqiora.Field("observed", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    return eqiora.Model.define(name, *declarations, observed,
                               eqiora.Relation("observe", equations=[(observed, 0)]))


VALUES = (-(2**63), -(2**53 + 1), 0, 2**53, 2**53 + 1, 2**53 + 2, 2**63 - 1)


@pytest.mark.parametrize("value", VALUES)
def test_integer_creation_source_read_edit_and_replay_preserve_every_bit(value):
    kind = eqiora.ValueType.integer()
    assert kind.to_eqi() == "integer"
    assert kind.scalar_domain == "integer"
    parameter = eqiora.Parameter("count", value_type=kind, value=value)
    assert type(parameter.value) is int
    assert parameter.value == value
    native = native_model("Integers", parameter)
    source = eqiora.compile(source=f"model Integers() {{ parameter count: integer = {value}; variable observed: 1; relation observe {{ observed = 0; }} }}")
    assert native.structural_fingerprint == source.structural_fingerprint
    for original in (native, source):
        parameter_id = original.parameter("count").id
        replayed = eqiora.Model.from_bytes(original.to_bytes())
        assert replayed.digest == original.digest
        for model in (original, replayed):
            reference = model.parameter(parameter_id)
            assert reference.id == parameter_id
            assert reference.value_type == kind
            assert type(reference.value) is int
            assert reference.value == value
            assert len({reference, model.parameter(parameter_id)}) == 1
            next_value = value - 1 if value == 2**63 - 1 else value + 1
            changed = model.commit(model.preview_value_edit(parameter_id, next_value))
            assert reference.value == value
            assert changed.parameter(parameter_id).value == next_value
            assert changed.digest != model.digest
            restored = eqiora.Model.from_bytes(changed.to_bytes())
            assert restored.digest == changed.digest
            assert restored.parameter(parameter_id).value == next_value


def test_integer_source_builder_and_external_binding_preserve_adjacent_values(tmp_path):
    q = eqiora.lang
    source = q.Source()
    owner = source.model("Selected")
    count = owner.parameter("count", value_type=eqiora.ValueType.integer())
    owner.set_default(count, 2**53 + 1)
    observed = owner.field("observed", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    owner.relation("observe", left=observed, right=q.to_real(count))
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
    model = native_model("Channels", parameter)
    reference = model.parameter("channels")
    replayed = eqiora.Model.from_bytes(model.to_bytes())
    assert replayed.digest == model.digest
    assert replayed.parameter(reference.id).value_type == kind
    assert replayed.parameter(reference.id).value == values
    for value in (1, [[1, 2]], [[1, 2], [3]], [[1, 2], [3, True]]):
        with pytest.raises((TypeError, ValueError)):
            eqiora.Parameter("bad", value_type=kind, value=value)


@pytest.mark.parametrize("value", (True, False, 1.0, 1 + 0j, -(2**63) - 1, 2**63))
def test_integer_native_creation_and_edits_reject_non_integer_or_out_of_range(value):
    kind = eqiora.ValueType.integer()
    with pytest.raises((TypeError, ValueError, OverflowError)):
        eqiora.Parameter("count", value_type=kind, value=value)
    model = native_model("Bounded", eqiora.Parameter("count", value_type=kind, value=1))
    before = model.to_bytes()
    with pytest.raises((TypeError, ValueError, OverflowError)):
        model.preview_value_edit("count", value)
    assert model.to_bytes() == before


@pytest.mark.parametrize("value", (True, -(2**63) - 1, 2**63))
def test_integer_external_bindings_reject_bool_and_overflow(value):
    with pytest.raises((TypeError, ValueError, OverflowError, eqiora.ValidationError)) as error:
        eqiora.compile(source="model Selected(parameter count: integer) { variable observed: 1; relation observe { observed = to_real(count); } }", entry="Selected", bindings={"count": value})
    assert "EQ0604" not in str(error.value)


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
    observed = owner.field("observed", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    owner.relation("observe", left=observed, right=rounded)
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
    for function in (q.to_integer, q.to_real):
        with pytest.raises(q.SourceError, match="owner|Component"):
            right.let_alias("foreign", function(a))


def test_integer_sampled_state_output_and_resume_preserve_adjacent_values(tmp_path):
    from fractions import Fraction

    q = eqiora.lang
    source = q.Source()
    owner = source.model("ExactTicks")
    tick = owner.clock("tick", period_s=1)
    kind = eqiora.ValueType.integer()
    memory = owner.field("memory", value_type=kind, role=eqiora.FieldRole.State, at=tick)
    observed = owner.output("observed", value_type=kind, at=tick)
    initial = 2**53 + 1
    owner.initial(left=q.pre(memory), right=initial)
    owner.relation("increment", at=tick, left=q.next(memory), right=q.pre(memory) + 1)
    owner.relation("observe", at=tick, left=observed, right=q.pre(memory))
    model = eqiora.compile(source=source, entry="ExactTicks")
    path = tmp_path / "exact_ticks.eqi"
    source.write_eqi(path)
    restored = eqiora.compile(path=path, entry="ExactTicks")
    assert restored.to_bytes() == model.to_bytes()
    session = model.execution_session(end_time_s=1, max_step_s=0.1, inputs={})
    assert session.output("observed", 0) is None
    assert session.advance_ticks(1) == 1
    assert type(session.field("memory")) is int
    assert session.field("memory") == initial + 1
    assert session.output("observed", 0) == (Fraction(0), initial)
    resumed = restored.resume_execution(session.checkpoint())
    assert resumed.advance_ticks(1) == session.advance_ticks(1) == 1
    assert resumed.field("memory") == session.field("memory") == initial + 2
    assert resumed.output("observed", 1) == (Fraction(1), initial + 1)
    assert type(resumed.output("observed", 1)[1]) is int
    assert resumed.output("observed", 2) is None


def test_explicit_initial_authoring_rejects_invalid_forms_without_mutation():
    q = eqiora.lang
    source = q.Source()
    owner = source.model("InitialSides")
    other = source.component("Other")
    memory = owner.field("memory", value_type=eqiora.ValueType.integer(), role=eqiora.FieldRole.State)
    foreign = other.field("memory", value_type=eqiora.ValueType.integer(), role=eqiora.FieldRole.State)
    for kwargs in ({"left": memory}, {"right": 1}):
        with pytest.raises(TypeError, match="both"):
            owner.initial(**kwargs)
    with pytest.raises(TypeError, match="combined"):
        owner.initial((memory, 0), left=memory, right=1)
    with pytest.raises(q.SourceError, match="Component"):
        owner.initial(left=memory, right=foreign)
    owner.initial(left=q.pre(memory), right=2**53 + 1, doc="Exact initial assignment.")
    text = source.to_eqi()
    assert text.count("  initial {") == 1
    assert "/// Exact initial assignment.\n  initial {\n    pre(memory) = 9007199254740993;" in text
    with pytest.raises(q.SourceError, match="frozen"):
        owner.initial(left=memory, right=0)
