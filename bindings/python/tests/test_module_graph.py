"""Python and local source units share one explicit mathematical Module graph."""

from fractions import Fraction

import pytest

import eqiora as q


LOCAL_GAIN = """
public component Gain(clock tick: periodic, parameter gain: 1,
                      input x: 1 at tick, output y: 1 at tick) {
  relation scale at tick { y = gain * x; }
}
"""


def gain_graph(tmp_path):
    path = tmp_path / "local.eqi"
    path.write_text(LOCAL_GAIN, encoding="utf-8")
    parts = q.Module("parts")
    local = parts.import_module("local", path=path)
    block = parts.component("Gain")
    tick = block.clock_requirement("tick")
    gain = block.parameter("gain", value_type=q.ValueType.real())
    x = block.input("x", value_type=q.ValueType.real(), at=tick)
    y = block.output("y", value_type=q.ValueType.real(), at=tick)
    inner = block.instance("inner", component=local.component("Gain"),
                           bindings={"tick": tick, "gain": 2, "x": x})
    block.relation("scale", q.lang.equation(y, gain * inner["y"]), at=tick)

    main = q.Module("main")
    imported = main.import_module("parts", parts)
    body = main.model("Main")
    tick = body.clock("tick", period_s=1)
    x = body.input("x", value_type=q.ValueType.real(), at=tick)
    y = body.output("y", value_type=q.ValueType.real(), at=tick)
    outer = body.instance("outer", component=imported.component("Gain"),
                          bindings={"tick": tick, "gain": 3, "x": x})
    body.relation("observe", q.lang.equation(y, outer["y"]), at=tick)
    return main, parts, path


def test_three_unit_graph_direct_and_emitted_meaning_and_values(tmp_path):
    main, parts, path = gain_graph(tmp_path)
    main_text, parts_text = main.to_eqi(), parts.to_eqi()
    assert main.to_eqi() == main_text
    assert parts.to_eqi() == parts_text
    direct = q.compile(source=main, entry="Main")

    parsed_parts = q.Module.parse("parts", parts_text)
    parsed_parts.import_module("local", path=path)
    parsed_main = q.Module.parse("main", main_text)
    parsed_main.import_module("parts", parsed_parts)
    # Reattaching existing exact imports must not append duplicate declarations.
    assert parsed_parts.to_eqi() == parts_text
    assert parsed_main.to_eqi() == main_text
    replayed = q.compile(source=parsed_main, entry="Main")
    assert direct.structural_fingerprint == replayed.structural_fingerprint
    for model in (direct, replayed):
        session = model.execution_session(end_time_s=1, max_step_s=0.1,
                                          inputs={"x": ("tick", [5.0, -2.0])})
        assert session.advance_ticks(2) == 2
        # The local gain is 2 and the Python gain is 3, so y = 6*x.
        assert session.output("y", 0) == (Fraction(0), 30.0)
        assert session.output("y", 1) == (Fraction(1), -12.0)


def test_direct_compile_does_not_call_public_source_emitter(tmp_path, monkeypatch):
    main, _, _ = gain_graph(tmp_path)

    def forbidden_emission(*args, **kwargs):
        raise AssertionError("direct Module compilation must consume its graph")

    monkeypatch.setattr(q.Module, "to_eqi", forbidden_emission)
    model = q.compile(source=main, entry="Main")
    session = model.execution_session(end_time_s=0.1, max_step_s=0.1,
                                      inputs={"x": ("tick", [5.0])})
    assert session.advance_ticks(1) == 1
    assert session.output("y", 0)[1] == 30.0


def test_explicit_equation_and_import_handles_are_immutable(tmp_path):
    path = tmp_path / "local.eqi"
    path.write_text(LOCAL_GAIN, encoding="utf-8")
    module = q.Module("main")
    imported = module.import_module("local", path=path)
    component = imported.component("Gain")
    body = module.model("Main")
    x = body.field("x", role=q.FieldRole.Variable, value_type=q.ValueType.real())
    equation = q.lang.equation(x, 2)
    for handle in (imported, component, x, equation):
        with pytest.raises((AttributeError, TypeError)):
            handle.injected = object()
    assert component.name == "Gain"
    with pytest.raises((AttributeError, TypeError)):
        component.name = "Replaced"
    assert component.name == "Gain"
    body.relation("balance", equation)
    before = q.compile(source=module, entry="Main").structural_fingerprint
    assert equation.lhs is not None and equation.rhs is not None
    with pytest.raises((AttributeError, TypeError)):
        equation.rhs = 99
    assert q.compile(source=module, entry="Main").structural_fingerprint == before


@pytest.mark.parametrize("coerce", (bool, lambda x: x == x, lambda x: x != x,
                                    lambda x: x < 1, lambda x: 1 <= x))
def test_symbolic_values_never_use_python_truth_equality_or_order(coerce):
    module = q.Module("main")
    body = module.model("Main")
    x = body.field("x", role=q.FieldRole.Variable, value_type=q.ValueType.real())
    with pytest.raises((TypeError, ValueError)):
        coerce(x)


def test_same_named_foreign_owner_cannot_be_captured():
    local, foreign = q.Module("same"), q.Module("same")
    body, other = local.model("Main"), foreign.model("Main")
    x = body.field("x", role=q.FieldRole.Variable, value_type=q.ValueType.real())
    foreign_x = other.field("x", role=q.FieldRole.Variable, value_type=q.ValueType.real())
    body.relation("balance", q.lang.equation(x, 0))
    with pytest.raises((TypeError, ValueError, q.EqioraError)):
        body.relation("capture", q.lang.equation(x, foreign_x))
        q.compile(source=local, entry="Main")


def test_import_namespace_cannot_silently_replace_another_unit():
    main = q.Module("main")
    main.import_module("parts", q.Module("first"))
    with pytest.raises((TypeError, ValueError, q.EqioraError)):
        main.import_module("parts", q.Module("second"))


@pytest.mark.parametrize("dimension,bad_value", (
    (lambda: q.Dimension(length=1), lambda: q.lang.quantity(1, q.units.s)),
    (lambda: q.Dimension(), lambda: True),
))
def test_equation_unit_and_scalar_domain_failures_reach_shared_admission(dimension, bad_value):
    module = q.Module("main")
    body = module.model("Main")
    x = body.field("x", role=q.FieldRole.Variable,
                   value_type=q.ValueType.real(dimension()))
    with pytest.raises((TypeError, ValueError, q.EqioraError)):
        body.relation("bad", q.lang.equation(x, bad_value()))
        q.compile(source=module, entry="Main")


def test_equation_rejects_host_callback_without_executing_it():
    module = q.Module("main")
    body = module.model("Main")
    x = body.field("x", role=q.FieldRole.Variable, value_type=q.ValueType.real())
    body.relation("balance", q.lang.equation(x, 1))
    artifact = q.compile(source=module, entry="Main")
    calls = []

    def callback():
        calls.append(True)
        return 1

    for value in (callback, object(), {"value": 1}, artifact):
        with pytest.raises((TypeError, ValueError)):
            q.lang.equation(x, value)
    assert calls == []


def test_direct_diagnostics_use_ast_owners_and_parsed_diagnostics_keep_real_coordinates():
    module = q.Module("main")
    body = module.model("Main")
    x = body.field("x", role=q.FieldRole.Variable,
                   value_type=q.ValueType.real(q.Dimension(length=1)))
    body.relation("bad", q.lang.equation(x, q.lang.quantity(1, q.units.s)))
    source = module.to_eqi()
    with pytest.raises(q.ValidationError) as direct:
        q.compile(source=module, entry="Main")
    assert any(d.graph_path is not None and d.source_span is None
               for d in direct.value.diagnostics)
    with pytest.raises(q.ValidationError) as parsed:
        q.compile(source=q.Module.parse("main", source), entry="Main")
    assert any(d.source_span is not None and d.source_span[0] == "src/main.eqi"
               and 0 <= d.source_span[1] < d.source_span[2] <= len(source.encode("utf-8"))
               for d in parsed.value.diagnostics)


@pytest.mark.parametrize("compare", (lambda x: bool(x), lambda x: x == x,
                                      lambda x: x != 0, lambda x: x < 1))
def test_native_symbolic_handles_never_produce_host_boolean(compare):
    field = q.Field("x", value_type=q.ValueType.real(), role=q.FieldRole.Variable)
    parameter = q.Parameter("gain", value=2)
    for value in (field, parameter, field + parameter):
        with pytest.raises(TypeError):
            compare(value)


def test_imported_closed_native_module_retains_nominal_identity_and_rejects_mutation():
    mode = q.Enum("Mode", members=("On", "Off"))
    parameter = q.Parameter("mode", value_type=mode.value_type, value=mode.member("On"))
    observed = q.Field("observed", value_type=q.ValueType.real(), role=q.FieldRole.Variable)
    native = q.Module("native", mode, parameter, observed,
                      q.Relation("law", equations=[(observed, 0)]))
    root = q.Module.parse("main", "import eqiora.local_project.native as native; model Main() {}")
    root.import_module("native", native)
    imported = q.compile(source=root, entry="native.native")
    assert imported.enum(mode.id).id == mode.id
    assert imported.parameter("mode").value == mode.member("On")
    assert q.Model.from_bytes(imported.to_bytes()).enum(mode.id).id == mode.id
    with pytest.raises(q.lang.ModuleError, match="frozen"):
        native.model("Mutation")
    with pytest.raises(q.lang.ModuleError, match="frozen"):
        native.import_module("other", q.Module("other"))
    retained = q.compile(source=root, entry="native.native")
    assert retained.to_bytes() == imported.to_bytes()
    foreign = q.Enum("Mode", members=("On", "Off"))
    foreign_parameter = q.Parameter("mode", value_type=foreign.value_type, value=foreign.member("On"))
    with pytest.raises(q.ValidationError, match="(?i)enum|scope|declaration"):
        q.Module("foreign", mode, foreign_parameter)
    with pytest.raises(q.ValidationError, match="(?i)enum|scope|declaration"):
        q.Module("omitted", parameter)
