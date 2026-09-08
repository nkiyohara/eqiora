"""Typed operator handles author closed source functions, never portable callbacks."""

from fractions import Fraction

import pytest

import eqiora

q = eqiora.lang


def conductivity(source, *, body=None):
    temperature = eqiora.ValueType.real(eqiora.Dimension(temperature=1))
    conductivity_type = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=1, time=-3, temperature=-1))
    inverse_temperature = eqiora.ValueType.real(eqiora.Dimension(temperature=-1))
    return source.operator("conductivity", inputs={"x": temperature, "k0": conductivity_type,
                                                  "a": inverse_temperature},
                           result_type=conductivity_type,
                           body=body or (lambda x, k0, a: k0 * (1 + a*x + a*a*x*x)),
                           doc="Quadratic conductivity in temperature.")


def test_operator_callback_once_named_reordering_and_immutable_handle():
    source = q.Source()
    calls = []
    def body(x, k0, a):
        calls.append((x, k0, a))
        return k0 * (1 + a*x + a*a*x*x)
    operator = conductivity(source, body=body)
    first = operator(x=20, k0=10, a=0.01)
    second = operator(a=0.01, x=20, k0=10)
    assert len(calls) == 1
    assert first._text == second._text
    owner = source.model("Conductivity")
    owner.let_alias("first", first)
    owner.let_alias("second", second)
    text = source.to_eqi()
    assert "input x: K" in text
    assert "conductivity(x = 20, k0 = 10, a = 0.01)" in text
    assert "Quadratic conductivity in temperature." in text
    with pytest.raises(AttributeError):
        operator.name = "changed"
    assert len({operator, operator}) == 1


def conductivity_model_source():
    source = q.Source()
    operator = conductivity(source)
    owner = source.model("Conductivity")
    tick = owner.clock("tick", period_s=1)
    kind = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=1, time=-3, temperature=-1))
    out = owner.output("result", value_type=kind, at=tick)
    owner.relation("evaluate", at=tick, left=out, right=operator(
        x=q.quantity(20, eqiora.units.K),
        k0=q.quantity(10, eqiora.units.W / eqiora.units.m / eqiora.units.K),
        a=q.quantity(0.01, eqiora.units.K ** -1)))
    return source


def test_typed_polynomial_operator_source_file_replay_and_execution(tmp_path):
    source = conductivity_model_source()
    model = eqiora.compile(source=source, entry="Conductivity")
    path = tmp_path / "conductivity.eqi"
    source.write_eqi(path)
    from_file = eqiora.compile(path=path, entry="Conductivity")
    assert from_file.to_bytes() == model.to_bytes()
    replayed = eqiora.Model.from_bytes(model.to_bytes())
    assert replayed.to_bytes() == model.to_bytes()
    for current in (model, from_file):
        session = current.execution_session(end_time_s=0.1, max_step_s=0.1, inputs={})
        assert session.advance_ticks(1) == 1
        # 10*(1 + .01*20 + .01^2*20^2) = 10*1.24 = 12.4 W/(m K).
        time, value = session.output("result", 0)
        assert time == Fraction(0)
        assert value == pytest.approx(12.4, rel=1e-14, abs=0)


@pytest.mark.parametrize("arguments", ({"x": 1}, {"x": 1, "k0": 2, "a": 3, "extra": 4}))
def test_operator_rejects_missing_and_unknown_named_arguments(arguments):
    operator = conductivity(q.Source())
    with pytest.raises(q.SourceError, match="named inputs"):
        operator(**arguments)
    with pytest.raises(TypeError):
        operator(1, 2, 3)


def test_operator_rejects_wrong_actual_units_at_shared_compiler():
    source = conductivity_model_source().to_eqi()
    # Change only the temperature argument, preserving a complete valid declaration.
    source = source.replace("x = 20 [K]", "x = 20 [m]")
    assert "x = 20 [m]" in source
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source, entry="Conductivity")
    assert any(diagnostic.code == "EQ0603" for diagnostic in error.value.diagnostics)


@pytest.mark.parametrize("foreign", (False, True))
def test_operator_body_rejects_hidden_model_capture_and_releases_failed_name(foreign):
    source = q.Source()
    hidden_source = q.Source() if foreign else source
    owner = hidden_source.model("Hidden")
    hidden = owner.parameter("hidden", value_type=eqiora.ValueType.real())
    with pytest.raises(q.SourceError, match="formal inputs|different Source|different.*owners"):
        source.operator("bad", inputs={"x": eqiora.ValueType.real()},
                        result_type=eqiora.ValueType.real(), body=lambda x: x + hidden)
    source.operator("bad", inputs={"x": eqiora.ValueType.real()},
                    result_type=eqiora.ValueType.real(), body=lambda x: x + 1)


@pytest.mark.parametrize("wrap", (lambda x: x + 1, lambda x: -x, lambda x: q.array((x,))[0],
                                   lambda x: q.math.sin(x), lambda x: x ** 2))
def test_foreign_constant_operator_calls_cannot_lose_source_ownership(wrap):
    source = q.Source()
    operator = source.operator("offset", inputs={"x": eqiora.ValueType.real()},
                               result_type=eqiora.ValueType.real(), body=lambda x: x + 1)
    foreign = q.Source()
    owner = foreign.model("Foreign")
    with pytest.raises(q.SourceError, match="Source"):
        owner.let_alias("bad", wrap(operator(x=2)))
    with pytest.raises(q.SourceError, match="foreign Source"):
        foreign.operator("bad", inputs={}, result_type=eqiora.ValueType.real(),
                         body=lambda: wrap(operator(x=2)))


def test_operator_composition_accepts_same_source_and_rejects_escaped_formals():
    source = q.Source()
    first = source.operator("increment", inputs={"x": eqiora.ValueType.real()},
                            result_type=eqiora.ValueType.real(), body=lambda x: x + 1)
    escaped = []
    def body(x):
        escaped.append(x)
        return first(x=x) * 2
    second = source.operator("twice", inputs={"x": eqiora.ValueType.real()},
                             result_type=eqiora.ValueType.real(), body=body)
    owner = source.model("Composed")
    owner.let_alias("valid", second(x=3))
    with pytest.raises(q.SourceError, match="Component"):
        owner.let_alias("escaped", escaped[0])
    assert "increment(x = x) * 2" in source.to_eqi()


def test_failed_operator_callback_does_not_make_formals_portable():
    source = q.Source()
    escaped = []
    def fail(x):
        escaped.append(x)
        raise RuntimeError("body failed")
    with pytest.raises(RuntimeError, match="body failed"):
        source.operator("bad", inputs={"x": eqiora.ValueType.real()},
                        result_type=eqiora.ValueType.real(), body=fail)
    with pytest.raises(q.SourceError, match="formal inputs"):
        source.operator("bad", inputs={}, result_type=eqiora.ValueType.real(),
                        body=lambda: escaped[0])
    source.operator("bad", inputs={}, result_type=eqiora.ValueType.real(), body=lambda: 1)


def test_operator_calls_preserve_existing_node_budget():
    source = q.Source()
    operator = source.operator("combine", inputs={"x": eqiora.ValueType.real(), "y": eqiora.ValueType.real()},
                               result_type=eqiora.ValueType.real(), body=lambda x, y: x+y)
    value = q.quantity(1, eqiora.units.m)
    for _ in range(11):
        value = value + value
    with pytest.raises(q.SourceError, match="node limit"):
        operator(x=value, y=value)
