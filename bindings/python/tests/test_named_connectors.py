"""Named physical interfaces use the shared Module graph and compiler laws."""

import pytest

import eqiora


q = eqiora.lang
VOLTAGE = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=2, time=-3, current=-1))
CURRENT = eqiora.ValueType.real(eqiora.Dimension(current=1))
RESISTANCE = eqiora.ValueType.real(eqiora.Dimension(mass=1, length=2, time=-3, current=-2))


def divider(*, across="voltage", through="current", incompatible=False):
    module = eqiora.Module("main")
    pin = module.connector("Pin", across=(across, VOLTAGE), through=(through, CURRENT),
                           doc="An exact nominal electrical terminal.")
    other = module.connector("OtherPin", across=(across, VOLTAGE), through=(through, CURRENT)) if incompatible else pin
    source = module.component("Source")
    supply = source.parameter("supply", value_type=VOLTAGE)
    sp = source.port("positive", connector=pin)
    sn = source.port("negative", connector=pin)
    source.relation("law",
                    q.equation(getattr(sp, across) - getattr(sn, across), supply),
                    q.equation(getattr(sp, through) + getattr(sn, through), 0))
    resistor = module.component("Resistor")
    resistance = resistor.parameter("resistance", value_type=RESISTANCE)
    rp = resistor.port("positive", connector=other)
    rn = resistor.port("negative", connector=other)
    resistor.relation("law", q.equation(
        getattr(rp, across) - getattr(rn, across), resistance * getattr(rp, through)),
        q.equation(getattr(rp, through) + getattr(rn, through), 0))
    ground = module.component("Ground")
    terminal = ground.port("terminal", connector=pin)
    ground.relation("reference", q.equation(getattr(terminal, across), 0))
    root = module.model("Divider")
    supplied = root.instance("source", component=source,
                             bindings={"supply": q.quantity(12, eqiora.units.V)})
    upper = root.instance("upper", component=resistor,
                          bindings={"resistance": q.quantity(1000, eqiora.units.Ohm)})
    lower = root.instance("lower", component=resistor,
                          bindings={"resistance": q.quantity(2000, eqiora.units.Ohm)})
    reference = root.instance("ground", component=ground, bindings={})
    root.connect(supplied["positive"], upper["positive"])
    root.connect(upper["negative"], lower["positive"])
    root.connect(lower["negative"], supplied["negative"], reference["terminal"])
    return module


@pytest.mark.parametrize("names", (("voltage", "current"), ("potential", "flow")))
def test_named_connector_module_and_emitted_source_preserve_meaning(names, tmp_path):
    authored = divider(across=names[0], through=names[1])
    direct = eqiora.compile(source=authored, entry="Divider")
    text = authored.to_eqi()
    assert f"across {names[0]}:" in text
    assert f"through {names[1]}:" in text
    assert "connect source.positive, upper.positive;" in text
    assert "An exact nominal electrical terminal." in text
    assert "across(" not in text and "through(" not in text
    path = tmp_path / "divider.eqi"
    authored.write_eqi(path)
    emitted = eqiora.compile(path=path, entry="Divider")
    assert direct.structural_fingerprint == emitted.structural_fingerprint
    assert direct.to_bytes() == emitted.to_bytes()
    assert eqiora.Model.from_bytes(direct.to_bytes()).digest == direct.digest


@pytest.mark.parametrize("route", ("module", "source"))
def test_equal_quantity_types_do_not_make_distinct_connectors_compatible(route):
    assert eqiora.compile(source=divider(), entry="Divider").digest
    source = divider(incompatible=True)
    if route == "source":
        source = source.to_eqi()
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source, entry="Divider")
    assert any("connector" in item.message.lower() for item in error.value.diagnostics)


def test_named_port_members_are_immutable_and_owned_by_the_exact_component():
    module = eqiora.Module("main")
    pin = module.connector("Pin", across=("voltage", VOLTAGE), through=("current", CURRENT))
    local = module.component("Local")
    foreign = module.component("Foreign")
    port = local.port("terminal", connector=pin)
    other = foreign.port("terminal", connector=pin)
    assert isinstance(port, q.Port)
    assert isinstance(port.voltage, q.Expression)
    assert isinstance(port.current, q.Expression)
    with pytest.raises(AttributeError, match="quantity"):
        _ = port.temperature
    for value, name in ((pin, "_name"), (port, "_name"), (port, "voltage")):
        with pytest.raises(AttributeError):
            setattr(value, name, "changed")
    with pytest.raises(q.ModuleError, match="Component"):
        local.connect(port, other)
    with pytest.raises(q.ModuleError, match="Component"):
        local.relation("bad", q.equation(port.voltage, other.voltage))
    local.let_alias("bad", 1)
    with pytest.raises((TypeError, ValueError)):
        bool(port.voltage)


def test_invalid_connector_declarations_do_not_reserve_names():
    module = eqiora.Module("main")
    for across, through, error in (
        (("same", VOLTAGE), ("same", CURRENT), q.ModuleError),
        (("voltage", object()), ("current", CURRENT), TypeError),
        (("not valid", VOLTAGE), ("current", CURRENT), q.ModuleError),
        ("voltage", ("current", CURRENT), TypeError),
    ):
        with pytest.raises(error):
            module.connector("Pin", across=across, through=through)
    module.connector("Pin", across=("voltage", VOLTAGE), through=("current", CURRENT))
    with pytest.raises(q.ModuleError):
        module.connector("Pin", across=("voltage", VOLTAGE), through=("current", CURRENT))


def test_foreign_module_connector_and_connection_shape_fail_locally():
    module = eqiora.Module("main")
    other = eqiora.Module("main")
    foreign = other.connector("Pin", across=("voltage", VOLTAGE), through=("current", CURRENT))
    owner = module.component("Owner")
    with pytest.raises(q.ModuleError, match="Module"):
        owner.port("terminal", connector=foreign)
    pin = module.connector("Pin", across=("voltage", VOLTAGE), through=("current", CURRENT))
    port = owner.port("terminal", connector=pin)
    for ports in ((), (port,), (port,) * 257):
        with pytest.raises(q.ModuleError, match="between 2 and 256"):
            owner.connect(*ports)
    with pytest.raises(q.ModuleError, match="physical ports"):
        owner.connect(port, port.voltage)


def test_connector_only_module_emits_and_freezes_its_nominal_declaration():
    module = eqiora.Module("pins")
    module.connector("Pin", across=("voltage", VOLTAGE), through=("current", CURRENT))
    assert "connector Pin" in module.to_eqi()
    with pytest.raises(q.ModuleError, match="frozen"):
        module.connector("Other", across=("voltage", VOLTAGE), through=("current", CURRENT))


@pytest.mark.parametrize("name", ("_name", "_owner", "_connector", "member"))
def test_member_access_preserves_names_that_overlap_python_attributes(name):
    module = eqiora.Module("main")
    pin = module.connector("Pin", across=(name, VOLTAGE), through=("current", CURRENT))
    component = module.component("Ground")
    port = component.port("terminal", connector=pin)
    component.relation("reference", q.equation(port.member(name), 0))
    assert f"terminal.{name} = 0;" in module.to_eqi()


@pytest.mark.parametrize("bad", ("foreign", "not_equation", "oversized"))
def test_later_invalid_equation_never_reserves_a_relation_or_consumes_its_budget(bad):
    module = eqiora.Module("main")
    owner = module.component("Owner")
    foreign = module.component("Foreign")
    x = owner.parameter("x", value_type=eqiora.ValueType.real())
    other = foreign.parameter("x", value_type=eqiora.ValueType.real())
    first = q.equation(x, 0)
    remaining = ((q.equation(other, 0),) if bad == "foreign"
                 else (object(),) if bad == "not_equation" else (first,) * 256)
    with pytest.raises((q.ModuleError, TypeError)):
        owner.relation("balance", first, *remaining)
    owner.relation("balance", first, q.equation(x, 1))
    # One Parameter plus one Relation leaves exactly 254 declaration slots.
    for index in range(254):
        owner.let_alias(f"constant_{index}", 1)
    assert module.to_eqi().count("relation balance") == 1


def test_splitting_one_constitutive_owner_is_rejected_instead_of_duplicating_ports():
    source = divider().to_eqi()
    # Keep the same equations, but give the current law another Relation owner.
    source = source.replace("positive.current + negative.current = 0;",
                            "} relation competing { positive.current + negative.current = 0;", 1)
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source, entry="Divider")
    assert any(item.code == "EQ0603" and "more than one owning Relation" in item.message
               for item in error.value.diagnostics)


@pytest.mark.parametrize("names", (("voltage", "current"), ("potential", "flow")))
def test_authored_and_emitted_divider_execute_the_independent_circuit(names, tmp_path):
    source = divider(across=names[0], through=names[1])
    direct = eqiora.compile(source=source, entry="Divider")
    path = tmp_path / "divider.eqi"
    source.write_eqi(path)
    emitted = eqiora.compile(path=path, entry="Divider")
    # Kirchhoff's laws: I=12/(1000+2000)=0.004 A; midpoint=I*2000=8 V.
    # Positive port current enters each component, so the source supplies -I.
    for model in (direct, emitted):
        session = model.execution_session(end_time_s=1, max_step_s=1, inputs={})
        assert session.through("upper.positive") == pytest.approx(0.004, abs=1e-12, rel=0)
        assert session.through("source.positive") == pytest.approx(-0.004, abs=1e-12, rel=0)
        assert session.across("lower.positive") == pytest.approx(8.0, abs=1e-10, rel=0)
        assert session.across("ground.terminal") == 0.0
