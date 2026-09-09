"""Imported property handles preserve exact module and contract identity."""
import pytest
import eqiora

q = eqiora.lang
u = eqiora.units


def library(name="materials"):
    source = eqiora.Module(name)
    required = source.property_contract("Response", value_type=eqiora.ValueType.real(),
                                       inputs={"x": eqiora.ValueType.real()},
                                       derivatives="first_partials")
    source.property_release("Reference", implements=required, value=required.input("x") * 2,
                            source_unit=u.one, source_scale=1,
                            citation="org.example.reference", license="spdx.CC0_1_0")
    return source


def test_repeated_import_aliases_preserve_contract_identity_and_qualified_emission():
    origin = library()
    source = eqiora.Module("main")
    first = source.import_module("one", origin)
    second = source.import_module("two", origin)
    contract = first.property_contract("Response")
    release = second.property_release("Reference")
    consumer = source.component("Consumer")
    requirement = consumer.property("response", contract=contract)
    x = consumer.parameter("x", value_type=eqiora.ValueType.real())
    consumer.let_alias("result", requirement(x=x))
    root = source.model("Root")
    root.instance("consumer", component=consumer, bindings={"x": 3, "response": release})
    text = source.to_eqi()
    assert "one.Response" in text
    assert "two.Reference" in text
    assert "property contract Response" not in text
    with pytest.raises(AttributeError):
        contract._name = "forged"


def test_local_release_implements_exact_imported_contract_without_copying_it():
    source = eqiora.Module("main")
    imported = source.import_module("materials", library())
    contract = imported.property_contract("Response")
    source.property_release("Local", implements=contract, value=contract.input("x") * 3,
                            source_unit=u.one, source_scale=1,
                            citation="org.example.local", license="spdx.CC0_1_0")
    text = source.to_eqi()
    assert ": materials.Response" in text
    assert "property contract Response" not in text


def test_equal_spelling_from_distinct_modules_is_not_nominal_equality():
    source = eqiora.Module("main")
    first = source.import_module("first", library())
    second = source.import_module("second", library("other_materials"))
    consumer = source.component("Consumer")
    consumer.property("response", contract=first.property_contract("Response"))
    root = source.model("Root")
    with pytest.raises(q.ModuleError, match="exact Module contract release"):
        root.instance("wrong", component=consumer,
                      bindings={"response": second.property_release("Reference")})
    with pytest.raises(q.ModuleError):
        first.property_contract("Reference")
    with pytest.raises(q.ModuleError):
        first.property_release("Response")


def test_dimensioned_imported_analytic_signature_executes_without_copying_release():
    provider = eqiora.Module.parse("motion", """
public dimension Distance = m;
public dimension Duration = s;
public property contract Displacement(input elapsed: Duration): Distance {
  derivatives first_partials;
  branch uniform;
}
public property release Uniform: Displacement {
  analytic { value = 2[m / s] * elapsed; source_unit: m = 1; }
  validity unconditional;
  outside reject;
  branch uniform;
  citation org.example.motion;
  license spdx.CC0_1_0;
}
""", package="org.example.motion")
    source = eqiora.Module("main")
    imported = source.import_module("motion", provider)
    contract = imported.property_contract("Displacement")
    measured = imported.property_release("Uniform")
    length = eqiora.ValueType.real(imported.dimension("Distance"))
    duration = eqiora.ValueType.real(imported.dimension("Duration"))
    consumer = source.component("Travel")
    tick = consumer.clock_requirement("tick")
    displacement = consumer.property("displacement", contract=contract)
    elapsed = consumer.parameter("elapsed", value_type=duration)
    output = consumer.output("distance", value_type=length, at=tick)
    consumer.relation("motion", q.equation(output, displacement(elapsed=elapsed)), at=tick)
    root = source.model("Main")
    clock = root.clock("tick", period_s=1)
    occurrence = root.instance("travel", component=consumer,
                               bindings={"tick": clock, "elapsed": 3, "displacement": measured})
    observed = root.output("distance", value_type=length, at=clock)
    root.relation("observe", q.equation(observed, occurrence["distance"]), at=clock)
    model = eqiora.compile(source=source, entry="Main")
    assert "property release Uniform" not in source.to_eqi()
    assert model.property_bindings[0].inputs == ["elapsed"]
    reopened = eqiora.Model.from_bytes(model.to_bytes())
    assert reopened.to_bytes() == model.to_bytes()
    session = model.execution_session(end_time_s=1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    assert session.output("distance", 0)[1] == 6.0
