"""Conditional authoring preserves lexical ownership without host truth evaluation."""

import pytest

import eqiora

q = eqiora.lang


def test_conditional_authors_both_branches_once_and_keeps_parentheses():
    source = q.Source()
    calls = []
    def body(x):
        calls.append("body")
        def branch(label, value):
            calls.append(label)
            return value
        return 2 * q.if_else(q.less(x, 0), branch("then", -x), branch("else", x))
    operator = source.operator("absolute_twice", inputs={"x": eqiora.ValueType.real()},
                               result_type=eqiora.ValueType.real(), body=body)
    owner = source.model("Conditional")
    owner.let_alias("value", operator(x=-2))
    assert calls == ["body", "then", "else"]
    assert "2 * (if (x) < (0) then -x else x)" in source.to_eqi()


@pytest.mark.parametrize("position", (0, 1, 2))
def test_conditional_checks_all_operand_component_owners(position):
    source = q.Source()
    first = source.model("First")
    second = source.component("Second")
    local = first.parameter("local", value_type=eqiora.ValueType.real())
    foreign = second.parameter("foreign", value_type=eqiora.ValueType.real())
    operands = [local, local, local]
    operands[position] = foreign
    with pytest.raises(q.SourceError, match="different.*owners"):
        q.if_else(*operands)


@pytest.mark.parametrize("position", (0, 1, 2))
def test_conditional_keeps_foreign_constant_call_provenance_in_every_operand(position):
    source = q.Source()
    foreign = q.Source()
    operator = foreign.operator("constant", inputs={}, result_type=eqiora.ValueType.real(), body=lambda: 1)
    operands = [True, 1, 0]
    operands[position] = operator()
    expression = q.if_else(*operands)
    owner = source.model("Local")
    with pytest.raises(q.SourceError, match="Source"):
        owner.let_alias("bad", expression + 1)
    with pytest.raises(q.SourceError, match="foreign Source"):
        source.operator("bad", inputs={}, result_type=eqiora.ValueType.real(), body=lambda: expression)


def test_conditional_does_not_hide_unselected_capture_or_free_binder():
    source = q.Source()
    owner = source.model("Local")
    hidden = owner.parameter("hidden", value_type=eqiora.ValueType.real())
    with pytest.raises(q.SourceError, match="different.*owners"):
        source.operator("bad", inputs={"x": eqiora.ValueType.real()},
                        result_type=eqiora.ValueType.real(),
                        body=lambda x: q.if_else(True, x, hidden))
    rows = owner.index_set("Rows", extent=2)
    escaped = []
    owner.sum(lambda i: escaped.append(i) or 1, over=rows)
    with pytest.raises(q.SourceError, match="binder"):
        owner.let_alias("bad", q.if_else(True, 0, q.ordinal(escaped[0])))


def guarded_sqrt_source(value):
    source = q.Source()
    voltage = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=2, time=-3, current=-1))
    squared_voltage = eqiora.ValueType.real(eqiora.Dimension(mass=2, length=4, time=-6, current=-2))
    operator = source.operator("guarded_root", inputs={"x": squared_voltage}, result_type=voltage,
                               body=lambda x: q.if_else(q.greater_equal(x, q.quantity(0, eqiora.units.V ** 2)),
                                                        q.math.sqrt(x), q.quantity(0, eqiora.units.V)))
    owner = source.model("Guarded")
    tick = owner.clock("tick", period_s=1)
    result = owner.output("result", value_type=voltage, at=tick)
    owner.relation("emit", at=tick, left=result, right=operator(x=q.quantity(value, eqiora.units.V ** 2)))
    return source


def test_guarded_sqrt_authors_portable_call_and_typed_threshold():
    text = guarded_sqrt_source(-1).to_eqi()
    assert "then math.sqrt(x) else 0 [V]" in text
    assert "0 [(V ^ 2)]" in text
    assert "guarded_root(x = -1 [(V ^ 2)])" in text


@pytest.mark.parametrize("value,expected", ((4, 2.0), (-1, 0.0)))
def test_guarded_sqrt_compiles_and_executes_only_selected_branch(value, expected):
    model = eqiora.compile(source=guarded_sqrt_source(value), entry="Guarded")
    session = model.sampled_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    assert session.output("result", 0)[1] == expected


@pytest.mark.parametrize("name,arguments", (("abs", (-1,)), ("min", (2, 3)),
    ("max", (2, 3)), ("clamp", (2, 0, 1)), ("sign", (-1,)), ("step", (0,))))
def test_piecewise_math_helpers_emit_portable_calls(name, arguments):
    source = q.Source()
    owner = source.model("Helpers")
    owner.let_alias("value", getattr(q.math, name)(*arguments))
    assert f"math.{name}({', '.join(str(value) for value in arguments)})" in source.to_eqi()


@pytest.mark.parametrize("name,arity", (("abs", 1), ("min", 2), ("max", 2),
                                         ("clamp", 3), ("sign", 1), ("step", 1)))
def test_piecewise_math_helpers_preserve_foreign_constant_call_ownership(name, arity):
    foreign = q.Source()
    operator = foreign.operator("constant", inputs={}, result_type=eqiora.ValueType.real(), body=lambda: 1)
    for position in range(arity):
        source = q.Source()
        owner = source.model("Local")
        arguments = [0] * arity
        arguments[position] = operator()
        with pytest.raises(q.SourceError, match="Source"):
            owner.let_alias("bad", getattr(q.math, name)(*arguments))


def test_piecewise_operator_body_can_compose_named_helpers():
    source = q.Source()
    operator = source.operator("piecewise", inputs={"x": eqiora.ValueType.real()},
                               result_type=eqiora.ValueType.real(),
                               body=lambda x: q.if_else(q.less(x, 0), q.math.abs(x), q.math.clamp(x, 0, 1)))
    owner = source.model("Piecewise")
    owner.let_alias("result", operator(x=2))
    assert "then math.abs(x) else math.clamp(x, 0, 1)" in source.to_eqi()


def test_conditional_and_clamp_share_the_existing_expression_budget():
    expression = q.quantity(1, eqiora.units.m)
    for _ in range(11):
        expression = expression + expression
    for construct in (lambda: q.if_else(True, expression, 0),
                      lambda: q.math.clamp(expression, 0, 1)):
        with pytest.raises(q.SourceError, match="node limit"):
            construct()


def test_conditional_type_checks_even_an_unselected_branch():
    source = q.Source()
    voltage = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=2, time=-3, current=-1))
    operator = source.operator("invalid", inputs={"x": voltage}, result_type=voltage,
                               body=lambda x: q.if_else(True, x, q.quantity(0, eqiora.units.s)))
    owner = source.model("Invalid")
    observed = owner.field("observed", role=eqiora.FieldRole.Variable, value_type=voltage)
    owner.relation("emit", left=observed, right=operator(x=q.quantity(1, eqiora.units.V)))
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source, entry="Invalid")
    assert any(diagnostic.code == "EQ0603" for diagnostic in error.value.diagnostics)
