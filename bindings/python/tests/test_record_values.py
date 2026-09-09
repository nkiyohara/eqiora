"""Closed record authoring stays on the ordinary compiler and source path."""
import pytest
import eqiora

q = eqiora.lang


def record_source():
    source = eqiora.Module("main")
    mode = source.enum("Mode", members=("Off", "On"))
    packet = source.record("Packet", members={
        "signal": eqiora.ValueType.real(),
        "valid": eqiora.ValueType.boolean(),
        "mode": mode.value_type,
    }, doc="A closed heterogeneous packet.")
    owner = source.model("Controller")
    bus = owner.field("bus", value_type=packet, role=eqiora.FieldRole.Variable)
    owner.relation("signal", q.equation(bus.member("signal"), 3))
    owner.relation("valid", q.equation(bus.member("valid"), True))
    owner.relation("mode", q.equation(bus.member("mode"), mode.member("On")))
    return source, packet, owner, bus


def test_record_emission_member_selection_and_compile_file_equivalence(tmp_path):
    source, packet, _, bus = record_source()
    assert isinstance(packet, q.Record)
    assert isinstance(bus, q.RecordField)
    assert tuple(packet.members) == ("signal", "valid", "mode")
    assert bus.member("signal")._name == "bus.signal"
    assert bus.member("signal")._owner is bus._owner
    text = source.to_eqi()
    assert "record Packet" in text and "valid: bool" in text
    assert "mode: Mode" in text and "bus.signal" in text
    path = tmp_path / "records.eqi"
    source.write_eqi(path)
    authored = eqiora.compile(source=source, entry="Controller")
    parsed = eqiora.compile(path=path, entry="Controller")
    assert parsed.to_bytes() == authored.to_bytes()
    assert eqiora.Model.from_bytes(authored.to_bytes()).to_bytes() == authored.to_bytes()
    with pytest.raises(q.ModuleError):
        source.record("Later", members={"x": eqiora.ValueType.real()})


def test_record_parameter_constructor_orders_named_members_and_compiles(tmp_path):
    source = eqiora.Module("main")
    pair = source.record("Pair", members={"left": eqiora.ValueType.real(), "ready": eqiora.ValueType.boolean()})
    owner = source.model("Owner")
    parameter = owner.parameter("config", value_type=pair)
    assert isinstance(parameter, q.RecordParameter)
    owner.set_default(parameter, pair(ready=True, left=4))
    result = owner.field("result", value_type=eqiora.ValueType.real(), role=eqiora.FieldRole.Variable)
    owner.relation("observe", q.equation(result, parameter.member("left")))
    text = source.to_eqi()
    assert text.index("left = 4") < text.index("ready = true")
    path = tmp_path / "parameter.eqi"
    source.write_eqi(path)
    assert eqiora.compile(source=source, entry="Owner").to_bytes() == eqiora.compile(path=path, entry="Owner").to_bytes()


def test_record_handles_reject_foreign_modules_and_unknown_members():
    source, packet, owner, bus = record_source()
    foreign = eqiora.Module("foreign")
    other = foreign.record("Packet", members={"signal": eqiora.ValueType.real()})
    for definition in (foreign.model("Other"), foreign.component("Component")):
        with pytest.raises(q.ModuleError, match="Module"):
            definition.field("wrong", value_type=packet, role=eqiora.FieldRole.Variable)
        with pytest.raises(q.ModuleError, match="Module"):
            definition.parameter("wrong", value_type=packet)
    with pytest.raises(q.ModuleError, match="Module"):
        owner.parameter("foreign", value_type=other)
    with pytest.raises(q.ModuleError, match="member"):
        bus.member("missing")
    for members in ({}, {"signal": 1, "valid": True}, {"signal": 1, "valid": True, "mode": 0, "extra": 2}):
        with pytest.raises(q.ModuleError, match="exactly"):
            packet(**members)
    with pytest.raises(TypeError):
        packet.members["extra"] = eqiora.ValueType.real()
    with pytest.raises(AttributeError):
        packet._name = "Changed"
    with pytest.raises(TypeError, match="nested"):
        source.record("Nested", members={"packet": packet})


def test_record_declarations_keep_exact_enum_ownership():
    source = eqiora.Module("main")
    foreign = eqiora.Module("foreign")
    mode = foreign.enum("Mode", members=("Off", "On"))
    with pytest.raises(q.ModuleError):
        source.record("Bad", members={"mode": mode.value_type})
    for members in ({}, {"_": eqiora.ValueType.real()}):
        with pytest.raises(q.ModuleError):
            source.record("Bad", members=members)
    local = source.enum("Local", members=("Off", "On"))
    packet = source.record("Packet", members={"mode": local.value_type})
    with pytest.raises(q.ModuleError, match="Module"):
        packet(mode=mode.member("On"))
