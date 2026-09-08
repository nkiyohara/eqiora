"""Boolean predicates retain exact values and never invoke Python truthiness."""

from fractions import Fraction

import pytest

import eqiora

q = eqiora.lang


@pytest.mark.parametrize("value", (False, True))
def test_boolean_native_creation_edit_and_replay_are_typed(value):
    kind = eqiora.ValueType.boolean()
    assert kind.to_eqi() == "bool"
    assert kind.scalar_domain == "boolean"
    parameter = eqiora.Parameter("enabled", value_type=kind, value=value)
    observed = eqiora.Field("observed", role=eqiora.FieldRole.Variable, value_type=kind)
    relation = eqiora.Relation("observe", equations=[(observed, parameter)])
    model = eqiora.Model.define("Boolean", parameter, observed, relation)
    reference = model.parameter("enabled")
    assert reference.value is value
    assert reference.value_type == kind
    assert reference == model.parameter("enabled")
    assert len({reference, model.parameter("enabled")}) == 1
    assert len(relation.equations) == 1
    for current in (model, eqiora.Model.from_bytes(model.to_bytes())):
        assert current.parameter(reference.id).value is value
        changed = current.commit(current.preview_value_edit(reference.id, not value))
        assert changed.parameter(reference.id).value is not value
        assert eqiora.Model.from_bytes(changed.to_bytes()).parameter(reference.id).value is not value
        assert current.parameter(reference.id).value is value
    for invalid in (0, 1, 1.0, 1j, "true", [True]):
        with pytest.raises((TypeError, ValueError)):
            eqiora.Parameter("invalid", value_type=kind, value=invalid)
        with pytest.raises((TypeError, ValueError)):
            model.preview_value_edit(reference.id, invalid)
    for numeric in (eqiora.ValueType.integer(), eqiora.ValueType.real()):
        with pytest.raises(TypeError):
            eqiora.Parameter("numeric", value_type=numeric, value=value)
    with pytest.raises(ValueError):
        eqiora.ValueType.array(kind, 2)


def test_source_boolean_defaults_and_external_values_preserve_exact_comparisons(tmp_path):
    source = q.Source()
    owner = source.model("Comparisons")
    integer = owner.parameter("n", value_type=eqiora.ValueType.integer())
    owner.set_default(integer, 2**53 + 1)
    expected = {
        "equal": (q.equal(integer, 2**53), False),
        "not_equal": (q.not_equal(integer, 2**53), True),
        "less": (q.less(integer, 2**53 + 2), True),
        "less_equal": (q.less_equal(integer, 2**53), False),
        "greater": (q.greater(integer, 2**53), True),
        "greater_equal": (q.greater_equal(integer, 2**53 + 2), False),
        "combined": (q.logical_and(q.greater(integer, 2**53), q.logical_not(False)), True),
        "alternative": (q.logical_or(False, q.equal(integer, 2**53 + 1)), True),
    }
    for name, (expression, _) in expected.items():
        parameter = owner.parameter(name, value_type=eqiora.ValueType.boolean())
        owner.set_default(parameter, expression)
    enabled = owner.parameter("enabled", value_type=eqiora.ValueType.boolean())
    owner.set_default(enabled, True)
    observed = owner.field("observed", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.boolean())
    owner.relation("observe", left=observed, right=enabled)
    model = eqiora.compile(source=source, entry="Comparisons")
    for name, (_, value) in expected.items():
        assert model.parameter(name).value is value
    assert "true" in source.to_eqi() and "false" in source.to_eqi()
    path = tmp_path / "comparisons.eqi"
    source.write_eqi(path)
    assert eqiora.compile(path=path, entry="Comparisons").to_bytes() == model.to_bytes()
    changed = eqiora.compile(source=source, entry="Comparisons", bindings={"enabled": False})
    assert changed.parameter("enabled").value is False
    for invalid in (0, 1, 1.0):
        with pytest.raises(eqiora.ValidationError):
            eqiora.compile(source=source, entry="Comparisons", bindings={"enabled": invalid})


def test_boolean_sampled_inputs_state_outputs_and_checkpoint_use_bool(tmp_path):
    source = q.Source()
    owner = source.model("Toggle")
    tick = owner.clock("tick", period_s=1)
    kind = eqiora.ValueType.boolean()
    drive = owner.input("drive", value_type=kind, at=tick)
    observed = owner.output("observed", value_type=kind, at=tick)
    memory = owner.field("memory", role=eqiora.FieldRole.State, value_type=kind, at=tick)
    owner.initial((q.pre(memory), False))
    owner.relation("toggle", at=tick, left=q.next(memory), right=q.logical_not(q.pre(memory)))
    owner.relation("observe", at=tick, left=observed, right=q.logical_and(drive, q.logical_not(q.pre(memory))))
    model = eqiora.compile(source=source, entry="Toggle")
    path = tmp_path / "toggle.eqi"
    source.write_eqi(path)
    from_file = eqiora.compile(path=path, entry="Toggle")
    assert from_file.to_bytes() == model.to_bytes()
    session = model.sampled_session(end_time_s=2, max_step_s=0.1,
                                    inputs={"drive": ("tick", [True, False, True])})
    assert session.output("observed", 0) is None
    assert session.advance_ticks(1) == 1
    assert session.field("memory") is True
    assert session.output("observed", 0) == (Fraction(0), True)
    resumed = from_file.resume_sampled(session.checkpoint())
    assert resumed.advance_ticks(2) == session.advance_ticks(2) == 2
    assert resumed.field("memory") is session.field("memory") is True
    for index, expected in enumerate((True, False, True)):
        output = resumed.output("observed", index)
        assert output[0] == Fraction(index)
        assert output[1] is expected
    assert resumed.output("observed", 3) is None
    with pytest.raises(TypeError):
        model.sampled_session(end_time_s=2, max_step_s=0.1,
                              inputs={"drive": ("tick", [True, 0, True])})


def test_predicate_authoring_preserves_identity_ownership_and_rejects_host_truthiness():
    source = q.Source()
    left, right = source.component("Left"), source.component("Right")
    a = left.parameter("n", value_type=eqiora.ValueType.integer())
    b = right.parameter("n", value_type=eqiora.ValueType.integer())
    assert a == a and a != b and len({a, b}) == 2
    predicates = (q.equal, q.not_equal, q.less, q.less_equal, q.greater, q.greater_equal,
                  q.logical_and, q.logical_or)
    for predicate in predicates:
        with pytest.raises(q.SourceError, match="owners"):
            predicate(a, b)
        with pytest.raises(TypeError, match="truth value"):
            bool(predicate(a, a))
    with pytest.raises(TypeError, match="truth value"):
        bool(a)
    with pytest.raises(q.SourceError, match="Component|owner"):
        right.let_alias("captured", q.logical_not(q.equal(a, 1)))
    native = eqiora.Parameter("n", value_type=eqiora.ValueType.integer(), value=2**53 + 1)
    for predicate in (eqiora.equal, eqiora.not_equal, eqiora.less, eqiora.less_equal,
                      eqiora.greater, eqiora.greater_equal, eqiora.logical_and, eqiora.logical_or):
        with pytest.raises(TypeError, match="truth value"):
            bool(predicate(native, native))
    with pytest.raises(TypeError, match="truth value"):
        bool(eqiora.logical_not(True))


def test_equations_have_no_residual_constructor_or_numeric_boolean_zero_alias():
    flag = eqiora.Field("flag", role=eqiora.FieldRole.State, value_type=eqiora.ValueType.boolean())
    initial = eqiora.Initial((flag, False))
    assert len(initial.equations) == 1
    with pytest.raises(TypeError, match="two explicit sides"):
        eqiora.Initial(flag)
    with pytest.raises(TypeError):
        eqiora.Relation("obsolete", residual=flag)
    with pytest.raises(AttributeError):
        initial.residuals
    source = q.Source()
    owner = source.model("WrongZero")
    field = owner.field("flag", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.boolean())
    owner.relation("invalid", left=field, right=0)
    with pytest.raises(eqiora.ValidationError):
        eqiora.compile(source=source, entry="WrongZero")


@pytest.mark.parametrize("function, operator", (("equal", "=="), ("not_equal", "!="),
    ("less", "<"), ("less_equal", "<="), ("greater", ">"), ("greater_equal", ">=")))
def test_native_named_comparisons_match_exact_source_equation_sides(function, operator):
    integer = eqiora.Parameter("n", value_type=eqiora.ValueType.integer(), value=2**53 + 1)
    flag = eqiora.Field("flag", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.boolean())
    predicate = getattr(eqiora, function)(integer, 2**53)
    native = eqiora.Model.define("Compare", integer, flag,
                                eqiora.Relation("compare", equations=[(flag, predicate)]))
    source = eqiora.compile(source=f"""
model Compare() {{
  parameter n: integer = 9007199254740993;
  variable flag: bool;
  relation compare {{ flag = n {operator} 9007199254740992; }}
}}
""")
    assert native.structural_fingerprint == source.structural_fingerprint


@pytest.mark.parametrize("operation", ("quotient", "remainder", "mixed_comparison", "boolean_add"))
def test_numeric_operators_do_not_coerce_boolean_operands(operation):
    source = q.Source()
    owner = source.model("NoCoercion")
    n = owner.parameter("n", value_type=eqiora.ValueType.integer())
    owner.set_default(n, 1)
    flag = owner.parameter("flag", value_type=eqiora.ValueType.boolean())
    owner.set_default(flag, True)
    if operation in ("quotient", "remainder"):
        expression = getattr(q, operation)(n, flag)
    elif operation == "mixed_comparison":
        expression = q.equal(n, flag)
    else:
        expression = flag + n
    observed = owner.field("observed", role=eqiora.FieldRole.Variable,
                           value_type=eqiora.ValueType.boolean() if operation == "mixed_comparison" else eqiora.ValueType.integer())
    owner.relation("invalid", left=observed, right=expression)
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source, entry="NoCoercion")
    assert error.value.diagnostics[0].code == "EQ0603"


def test_logical_predicates_reuse_expression_depth_and_node_bounds():
    expression = q.logical_not(False)
    for _ in range(62):
        expression = q.logical_not(expression)
    with pytest.raises(q.SourceError, match="depth"):
        q.logical_not(expression)
    expression = q.logical_not(False)
    for _ in range(10):
        expression = q.logical_and(expression, expression)
    with pytest.raises(q.SourceError, match="4096"):
        q.logical_or(expression, expression)
