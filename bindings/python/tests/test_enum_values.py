"""Enum values retain exact declaration identity through authoring and execution."""

from fractions import Fraction

import pytest

import eqiora

q = eqiora.lang
MEMBERS = ("Heating", "Cooling", "Fault")


def test_native_enum_values_edit_and_replay_preserve_nominal_identity():
    mode = eqiora.Enum("Mode", members=MEMBERS)
    foreign = eqiora.Enum("Mode", members=MEMBERS)
    heating, cooling = mode.member("Heating"), mode.member("Cooling")
    assert mode != foreign and len({mode, foreign}) == 2
    assert heating == mode.member("Heating") and len({heating, mode.member("Heating")}) == 1
    assert heating != foreign.member("Heating")
    parameter = eqiora.Parameter("mode", value_type=mode.value_type, value=heating)
    observed = eqiora.Field("observed", role=eqiora.FieldRole.Variable)
    model = eqiora.compile(source=eqiora.Module("NativeEnum", mode, parameter, observed,
                               eqiora.Relation("observe", equations=[(observed, 0)])))
    reference = model.parameter("mode")
    assert reference.value == heating and reference.value_type == mode.value_type
    for current in (model, eqiora.Model.from_bytes(model.to_bytes())):
        declaration = current.enum(mode.id)
        assert declaration == mode and declaration.members == MEMBERS
        assert declaration.member("Heating") == heating
        changed = current.commit(current.preview_value_edit(reference.id, cooling))
        assert changed.parameter(reference.id).value == cooling
        assert current.parameter(reference.id).value == heating
        assert eqiora.Model.from_bytes(changed.to_bytes()).parameter(reference.id).value == cooling
    assert eqiora.Model.from_bytes(model.to_bytes()).enum(mode.id).name is None
    for invalid in (0, 1, True, "Heating", foreign.member("Heating")):
        with pytest.raises((TypeError, ValueError)):
            model.preview_value_edit(reference.id, invalid)
    with pytest.raises(TypeError):
        bool(heating)
    with pytest.raises(ValueError):
        mode.member("Missing")
    with pytest.raises(ValueError):
        eqiora.ValueType.array(mode.value_type, 2)
    with pytest.raises(TypeError):
        eqiora.EnumValue()


def enum_source():
    source = eqiora.Module("main")
    mode = source.enum("Mode", members=MEMBERS, doc="Declared operating modes.")
    owner = source.model("Controller")
    tick = owner.clock("tick", period_s=1)
    drive = owner.input("drive", value_type=mode.value_type, at=tick)
    memory = owner.field("memory", role=eqiora.FieldRole.State, value_type=mode.value_type, at=tick)
    output = owner.output("observed", value_type=mode.value_type, at=tick)
    level = owner.output("level", value_type=eqiora.ValueType.real(), at=tick)
    owner.initial((q.pre(memory), mode.member("Fault")))
    owner.relation("remember", eqiora.lang.equation(q.next(memory), drive), at=tick)
    owner.relation("observe", eqiora.lang.equation(output, q.pre(memory)), at=tick)
    owner.relation("classify", eqiora.lang.equation(level, q.case(drive, [
        (mode.member("Heating"), 2), (mode.member("Cooling"), -3), (mode.member("Fault"), 0)])), at=tick)
    return source, mode


def test_source_enum_case_file_execution_and_checkpoint_preserve_values(tmp_path):
    source, authored = enum_source()
    model = eqiora.compile(source=source, entry="Controller")
    declaration = model.enum("Mode")
    assert declaration.members == authored.members
    assert declaration.value_type != authored.value_type  # Compiler canonical IDs have their own owner.
    path = tmp_path / "controller.eqi"
    source.write_eqi(path)
    from_file = eqiora.compile(path=path, entry="Controller")
    assert from_file.to_bytes() == model.to_bytes()
    values = [declaration.member(name) for name in MEMBERS]
    session = model.execution_session(end_time_s=2, max_step_s=0.1, inputs={"drive": ("tick", values)})
    assert session.advance_ticks(1) == 1
    resumed = from_file.resume_execution(session.checkpoint())
    assert resumed.advance_ticks(2) == session.advance_ticks(2) == 2
    for index, (previous, level) in enumerate(zip((values[2], values[0], values[1]), (2.0, -3.0, 0.0))):
        assert resumed.output("observed", index) == (Fraction(index), previous)
        assert resumed.output("level", index) == (Fraction(index), level)
        assert resumed.output("observed", index) == session.output("observed", index)
    assert resumed.field("memory") == values[2]
    for invalid in ("Heating", 0, eqiora.Enum("Mode", members=MEMBERS).member("Heating")):
        with pytest.raises((TypeError, ValueError)):
            model.execution_session(end_time_s=0, max_step_s=0.1, inputs={"drive": ("tick", [invalid])})


def test_enum_case_authoring_preserves_foreign_capture_and_closed_patterns():
    source = eqiora.Module("main")
    mode = source.enum("Mode", members=MEMBERS)
    owner = source.model("Owner")
    foreign_source = eqiora.Module("main")
    foreign = foreign_source.enum("Mode", members=MEMBERS)
    expression = q.case(mode.member("Heating"), [(foreign.member("Heating"), 1)])
    with pytest.raises(q.ModuleError, match="(?i)module|foreign"):
        owner.let_alias("foreign", expression)
    with pytest.raises(TypeError, match="patterns"):
        q.case(mode.member("Heating"), [("Mode.Heating", 1)])
    with pytest.raises(TypeError):
        bool(mode.member("Heating"))
    with pytest.raises(ValueError):
        mode.member("Missing")


@pytest.mark.parametrize("labels", [("Heating",), ("Heating", "Heating", "Fault")])
def test_compiler_rejects_nonexhaustive_or_duplicate_case_arms(labels):
    source = eqiora.Module("main")
    mode = source.enum("Mode", members=MEMBERS)
    owner = source.model("InvalidCase")
    observed = owner.field("observed", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    with pytest.raises((q.ModuleError, eqiora.ValidationError), match="(?i)case|arm|exhaustive|duplicate|member"):
        owner.relation("observe", eqiora.lang.equation(observed, q.case(mode.member("Heating"), [(mode.member(label), 1) for label in labels])))
        eqiora.compile(source=source, entry="InvalidCase")


def test_source_enum_parameter_binding_requires_exact_compiled_declaration(tmp_path):
    source = eqiora.Module("main")
    mode = source.enum("Mode", members=MEMBERS)
    owner = source.model("Configured")
    parameter = owner.parameter("mode", value_type=mode.value_type)
    owner.set_default(parameter, mode.member("Heating"))
    observed = owner.field("observed", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    owner.relation("observe", eqiora.lang.equation(observed, q.case(parameter, [
        (mode.member("Heating"), 2), (mode.member("Cooling"), -3), (mode.member("Fault"), 0)])))
    original = eqiora.compile(source=source, entry="Configured")
    declaration = original.enum("Mode")
    cooling = declaration.member("Cooling")
    bound = eqiora.compile(source=source, entry="Configured", bindings={"mode": cooling})
    assert bound.parameter("mode").value == cooling
    assert original.parameter("mode").value == declaration.member("Heating")
    for invalid in ("Cooling", 1, eqiora.Enum("Mode", members=MEMBERS).member("Cooling")):
        with pytest.raises((eqiora.ValidationError, TypeError, ValueError)):
            eqiora.compile(source=source, entry="Configured", bindings={"mode": invalid})
    path = tmp_path / "configured.eqi"
    source.write_eqi(path)
    assert eqiora.compile(path=path, entry="Configured", bindings={"mode": cooling}).to_bytes() == bound.to_bytes()


def test_compiler_rejects_same_source_foreign_enum_case_pattern():
    source = eqiora.Module("main")
    mode = source.enum("Mode", members=MEMBERS)
    foreign = source.enum("Foreign", members=MEMBERS)
    owner = source.model("ForeignCase")
    observed = owner.field("observed", role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
    owner.relation("observe", eqiora.lang.equation(observed, q.case(mode.member("Heating"), [
        (foreign.member(label), 1) for label in MEMBERS])))
    with pytest.raises(eqiora.ValidationError, match="(?i)enum|case|pattern|nominal|member"):
        eqiora.compile(source=source, entry="ForeignCase")
