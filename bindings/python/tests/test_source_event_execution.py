"""Installed Module events execute and restart through the ordinary session."""

import pytest

import eqiora


def impact_module():
    q = eqiora.lang
    source = eqiora.Module("main")
    owner = source.model("Impact")
    height = owner.field("height", role=eqiora.FieldRole.State,
                         value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))
    velocity = owner.field("velocity", role=eqiora.FieldRole.State,
                           value_type=eqiora.ValueType.real(eqiora.Dimension(length=1, time=-1)))
    owner.initial((height, q.quantity(1, eqiora.units.m)),
                  (velocity, q.quantity(-1, eqiora.units.m / eqiora.units.s)))
    owner.relation("position", q.equation(q.derivative(height), velocity))
    owner.relation("uniform_motion", q.equation(q.derivative(velocity),
                                         q.quantity(0, eqiora.units.m / eqiora.units.s**2)))
    impact = owner.event("impact", height, direction="falling")
    rebound = owner.let_alias("rebound", -0.5 * q.pre(velocity), at=impact)
    owner.relation("reset_height", q.equation(q.next(height), q.quantity(0, eqiora.units.m)),
                   at=impact)
    owner.relation("reset_velocity", q.equation(q.next(velocity), rebound), at=impact)
    return source


@pytest.mark.parametrize("route", ["module", "file", "artifact"])
def test_authored_event_alias_executes_and_restarts_across_impact(tmp_path, route):
    source = impact_module()
    direct = eqiora.compile(source=source, entry="Impact")
    names = {name: direct.field(name).id if route == "artifact" else name
             for name in ("height", "velocity")}
    if route == "file":
        path = tmp_path / "impact.eqi"
        source.write_eqi(path)
        model = eqiora.compile(path=path, entry="Impact")
        assert model.to_bytes() == direct.to_bytes()
    elif route == "artifact":
        model = eqiora.Model.from_bytes(direct.to_bytes())
        assert model.to_bytes() == direct.to_bytes()
    else:
        model = direct

    session = model.execution_session(end_time_s=1.5, max_step_s=0.125, inputs={})
    while session.progress["model_time"] < 0.5:
        assert session.advance()
    resumed = model.resume_execution(session.checkpoint())
    events = 0
    activation_ids = []
    while session.advance():
        assert resumed.advance()
        assert resumed.progress == session.progress
        assert resumed.activation_sequence == session.activation_sequence
        for field in ("height", "velocity"):
            assert resumed.field(names[field]) == session.field(names[field])
        if session.activation_sequence:
            events += 1
            activation_ids.extend(identity for step in session.activation_sequence
                                  for identity in step)
            # h=1-t first reaches zero at t=1. The simultaneous reset reads
            # v_pre=-1 and sets v_next=1/2; the alias is not a solve unknown.
            assert session.progress["model_time"] == pytest.approx(1, abs=1e-8, rel=0)
            assert session.field(names["height"]) == pytest.approx(0, abs=1e-8, rel=0)
            assert session.field(names["velocity"]) == pytest.approx(0.5, abs=1e-8, rel=0)
    assert not resumed.advance()
    assert events == 1
    # After the sole impact h=(t-1)/2, so h(1.5)=1/4. Both flight pieces
    # are affine: no integration truncation error is needed in the bound.
    # 1e-8 allows the reference event localization and residual tolerances.
    assert session.field(names["height"]) == pytest.approx(0.25, abs=1e-8, rel=0)
    assert session.field(names["velocity"]) == pytest.approx(0.5, abs=1e-8, rel=0)
    for invalid in ("rebound", "00000000000000000000000001", *activation_ids):
        with pytest.raises(eqiora.EqioraError):
            session.field(invalid)


@pytest.mark.parametrize("kind", ["algebraic", "clocked"])
def test_module_derivative_retains_compiler_state_and_activation_checks(kind):
    q = eqiora.lang
    source = eqiora.Module("main")
    owner = source.model("Invalid")
    options = {}
    if kind == "clocked":
        options["at"] = owner.clock("tick", period_s=1)
    value = owner.field("value", value_type=eqiora.ValueType.real(),
                        role=(eqiora.FieldRole.Variable if kind == "algebraic"
                              else eqiora.FieldRole.State), **options)
    owner.relation("evolution", q.equation(q.derivative(value),
                                           q.quantity(0, eqiora.units.s**-1)))
    for candidate in (source, source.to_eqi()):
        with pytest.raises(eqiora.ValidationError) as failure:
            eqiora.compile(source=candidate, entry="Invalid")
        assert any(item.code == "EQ0603"
                   and "eligible declared state" in item.message
                   for item in failure.value.diagnostics), failure.value.diagnostics
