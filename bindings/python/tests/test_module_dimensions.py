"""Public structural dimensions retain exact module lookup without nominalizing SI."""
from fractions import Fraction
import pytest
import eqiora


def test_authored_and_imported_dimensions_are_structural_and_compile():
    library = eqiora.Module("dimensions", package="org.example.units")
    expected = eqiora.Dimension(length=Fraction(1, 2), time=-1)
    assert library.dimension("RootSpeed", expected, doc="Structural dimension.") == expected
    text = library.to_eqi()
    assert "public dimension RootSpeed" in text
    assert "Structural dimension." in text
    reopened = eqiora.Module.parse("dimensions", text, package="org.example.units")
    fingerprints = []
    for provider in (library, reopened):
        consumer = eqiora.Module("main")
        imported = consumer.import_module("units", provider)
        dimension = imported.dimension("RootSpeed")
        assert dimension == expected
        model = consumer.model("Main")
        value = model.field("value", value_type=eqiora.ValueType.real(dimension), role=eqiora.FieldRole.Variable)
        model.relation("zero", eqiora.lang.equation(value, 0))
        result = eqiora.compile(source=consumer, entry="Main")
        fingerprints.append(result.structural_fingerprint)
    assert fingerprints[0] == fingerprints[1]


def test_import_resolves_source_forward_aliases_and_rejects_private_wrong_kind():
    provider = eqiora.Module.parse("units", "public dimension Speed = Length / s; dimension Length = m; public component Other() {}")
    consumer = eqiora.Module("main")
    imported = consumer.import_module("u", provider)
    assert imported.dimension("Speed") == eqiora.Dimension(length=1, time=-1)
    for name in ("Length", "Other", "Absent", "other.Speed"):
        with pytest.raises((ValueError, TypeError)):
            imported.dimension(name)
    with pytest.raises(TypeError):
        consumer.dimension("Invalid", eqiora.units.m)


def test_unused_private_dimension_cycle_rejects_descriptor_resolution():
    provider = eqiora.Module.parse("units", "public dimension Length = m; dimension A1 = B1; dimension B1 = A1;")
    with pytest.raises(ValueError, match="cycle"):
        eqiora.Module("main").import_module("u", provider).dimension("Length")


def test_dimension_dependencies_follow_attached_modules_without_transitive_exports():
    base = eqiora.Module("base", package="org.example.base")
    base.dimension("Distance", eqiora.Dimension(length=1))
    provider = eqiora.Module.parse(
        "units",
        "import org.example.base.base as base; "
        "public dimension Speed = base.Distance / s;",
        package="org.example.units",
    )
    consumer = eqiora.Module("main")
    imported = consumer.import_module("u", provider)
    with pytest.raises(eqiora.lang.ModuleError, match="no matching attached source"):
        imported.dimension("Speed")
    provider.import_module("base", base)
    assert imported.dimension("Speed") == eqiora.Dimension(length=1, time=-1)
    for name in ("Distance", "base.Distance", "m", "speed"):
        with pytest.raises(ValueError):
            imported.dimension(name)
    body = consumer.model("Main")
    value = body.field(
        "value", role=eqiora.FieldRole.Variable,
        value_type=eqiora.ValueType.real(imported.dimension("Speed")),
    )
    body.relation("zero", eqiora.lang.equation(value, 0))
    direct = eqiora.compile(source=consumer, entry="Main")
    reopened = eqiora.Module.parse("main", consumer.to_eqi())
    reopened.import_module("u", provider)
    assert eqiora.compile(source=reopened, entry="Main").structural_fingerprint == direct.structural_fingerprint


def test_dimensions_share_module_name_and_freeze_admission():
    module = eqiora.Module("units")
    module.dimension("Distance", eqiora.Dimension(length=1))
    with pytest.raises(eqiora.lang.ModuleError, match="duplicate top-level"):
        module.dimension("Distance", eqiora.Dimension(time=1))
    with pytest.raises(eqiora.lang.ModuleError, match="duplicate top-level"):
        module.component("Distance")
    module.to_eqi()
    with pytest.raises(eqiora.lang.ModuleError, match="frozen"):
        module.dimension("Duration", eqiora.Dimension(time=1))


@pytest.mark.parametrize("type_name, literal, error", (
    ("u.m", "0", "unknown or private dimension alias"),
    ("u.Speed", "1 [u.Speed]", "input-unit"),
))
def test_imports_do_not_export_coherent_si_symbols_or_create_literal_units(type_name, literal, error):
    provider = eqiora.Module("units", package="org.example.units")
    provider.dimension("Speed", eqiora.Dimension(length=1, time=-1))
    consumer = eqiora.Module.parse(
        "main",
        "import org.example.units.units as u; "
        f"model Main() {{ parameter value: {type_name} = {literal}; }}",
    )
    consumer.import_module("u", provider)
    with pytest.raises(eqiora.EqioraError, match=error):
        eqiora.compile(source=consumer, entry="Main")
