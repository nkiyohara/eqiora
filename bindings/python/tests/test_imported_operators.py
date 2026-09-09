"""Qualified calls retain exact providers and use shared polynomial admission."""

import pytest
import eqiora as q


POLYNOMIAL = """
public operator polynomial(input x: 1, input scale: 1): 1 = scale * (1 + x + x*x);
"""


def consumer(provider, *, path=None):
    main = q.Module("main", package="consumer.example")
    imported = (main.import_module("ops", path=path) if path else
                main.import_module("ops", provider))
    op = imported.operator("polynomial")
    model = main.model("Main")
    tick = model.clock("tick", period_s=1)
    x = model.input("x", value_type=q.ValueType.real(), at=tick)
    y = model.output("y", value_type=q.ValueType.real(), at=tick)
    model.relation("evaluate", q.lang.equation(y, op(scale=3, x=x)), at=tick)
    return main, op


def test_imported_operator_direct_emitted_local_and_artifact_replay(tmp_path):
    provider = q.Module("operators", package="provider.example")
    provider.operator("polynomial", inputs={"x": q.ValueType.real(), "scale": q.ValueType.real()},
                      result_type=q.ValueType.real(), body=lambda x, scale: scale * (1+x+x*x))
    direct, op = consumer(provider)
    text = direct.to_eqi()
    assert "ops.polynomial(x = x, scale = 3)" in text
    assert "public operator polynomial" not in text
    with pytest.raises(AttributeError):
        op.name = "replacement"
    parsed = q.Module.parse("main", text, package="consumer.example")
    parsed.import_module("ops", q.Module.parse("operators", provider.to_eqi(), package="provider.example"))
    path = tmp_path / "operators.eqi"
    path.write_text(POLYNOMIAL)
    local, _ = consumer(None, path=path)
    models = [q.compile(source=source, entry="Main") for source in (direct, parsed, local)]
    assert models[0].to_bytes() == models[1].to_bytes()
    for model in models:
        assert q.Model.from_bytes(model.to_bytes()).to_bytes() == model.to_bytes()
        for candidate in (model,):
            session = candidate.execution_session(end_time_s=2, max_step_s=0.1,
                                                  inputs={"x": ("tick", [-2.0, 0.0, 4.0])})
            assert session.advance_ticks(3) == 3
            # Independently: 3*(1+x+x²) gives 9, 3, 63 at -2, 0, 4.
            assert [session.output("y", i)[1] for i in range(3)] == [9.0, 3.0, 63.0]


@pytest.mark.parametrize("source", [POLYNOMIAL.replace("public", "private"),
                                      "public component polynomial() {}", "public component Other() {}"])
def test_descriptor_rejects_private_missing_and_wrong_kind(source):
    main = q.Module("main")
    imported = main.import_module("ops", q.Module.parse("ops", source))
    with pytest.raises((ValueError, q.EqioraError), match="public operator"):
        imported.operator("polynomial")


def test_imported_calls_reject_bad_names_and_foreign_owners():
    main = q.Module("main")
    op = main.import_module("ops", q.Module.parse("ops", POLYNOMIAL)).operator("polynomial")
    with pytest.raises(q.lang.ModuleError, match="named inputs"):
        op(x=1)
    with pytest.raises(q.lang.ModuleError, match="named inputs"):
        op(x=1, scale=2, typo=3)
    other = q.Module("other")
    body = other.model("Other")
    x = body.parameter("x", value_type=q.ValueType.real())
    local = main.model("Main")
    with pytest.raises(q.lang.ModuleError, match="Component"):
        local.let_alias("foreign", op(x=x, scale=2))
    with pytest.raises(q.lang.ModuleError, match="Module"):
        body.let_alias("bad", op(x=1, scale=2))


def test_wrong_units_reach_shared_operator_admission():
    main = q.Module("main")
    op = main.import_module("ops", q.Module.parse("ops", POLYNOMIAL)).operator("polynomial")
    body = main.model("Main")
    tick = body.clock("tick", period_s=1)
    out = body.output("y", value_type=q.ValueType.real(), at=tick)
    body.relation("bad", q.lang.equation(out, op(x=q.lang.quantity(1, q.units.m), scale=3)), at=tick)
    with pytest.raises(q.ValidationError) as error:
        q.compile(source=main, entry="Main")
    assert any(item.code == "EQ0603" for item in error.value.diagnostics)


def test_imported_operator_cycle_reaches_shared_definition_gate():
    source = POLYNOMIAL.replace("scale * (1 + x + x*x)", "polynomial(x=x, scale=scale)")
    main, _ = consumer(q.Module.parse("operators", source, package="provider.example"))
    with pytest.raises(q.ValidationError) as error:
        q.compile(source=main, entry="Main")
    assert any("cycl" in item.message or "recurs" in item.message for item in error.value.diagnostics)
