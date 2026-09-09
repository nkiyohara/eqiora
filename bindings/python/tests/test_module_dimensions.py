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
