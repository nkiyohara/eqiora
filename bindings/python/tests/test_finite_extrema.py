"""Finite extrema preserve typed values through the existing compiled Model lifecycle."""

from fractions import Fraction

import pytest

import eqiora

q = eqiora.lang


@pytest.mark.parametrize("operation", ("min", "max"))
def test_extrema_authoring_calls_once_and_reuses_exact_binder_scope(operation):
    source = q.Source()
    owner = source.model("Extrema")
    rows = owner.index_set("Rows", extent=3)
    seen = []
    def body(i):
        seen.append(i)
        return q.ordinal(i)
    value = getattr(owner, operation)(body, over=rows)
    assert len(seen) == 1
    owner.let_alias("chosen", value)
    with pytest.raises(q.SourceError, match="binder"):
        owner.let_alias("escaped", q.ordinal(seen[0]) + 1)
    foreign = source.component("Other").index_set("Rows", extent=3)
    with pytest.raises(q.SourceError, match="index set"):
        getattr(owner, operation)(body, over=foreign)
    assert len(seen) == 1
    assert f"{operation}(ordinal(i), over = (i in Rows))" in source.to_eqi()


def integer_source(operation):
    source = q.Source()
    owner = source.model("Extrema")
    rows = owner.index_set("Rows", extent=3)
    tick = owner.clock("tick", period_s=1)
    p = owner.parameter("base", value_type=eqiora.ValueType.integer())
    owner.set_default(p, 2**53 + 1)
    out = owner.output("result", value_type=eqiora.ValueType.integer(), at=tick)
    values = q.array((p + 1, p - 2, p + 1))
    owner.relation("emit", at=tick, left=out,
                   right=getattr(owner, operation)(lambda i: values[q.ordinal(i)], over=rows))
    return source


@pytest.mark.parametrize("operation,offset", (("min", -2), ("max", 1)))
def test_extrema_exact_integer_edit_file_replay_and_sampled_resume(operation, offset, tmp_path):
    source = integer_source(operation)
    model = eqiora.compile(source=source, entry="Extrema")
    path = tmp_path / "extrema.eqi"
    source.write_eqi(path)
    from_file = eqiora.compile(path=path, entry="Extrema")
    assert from_file.to_bytes() == model.to_bytes()
    session = model.sampled_session(end_time_s=2, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    resumed = from_file.resume_sampled(session.checkpoint())
    for current in (session, resumed):
        assert current.advance_ticks(2) == 2
        for tick in range(3):
            result = current.output("result", tick)
            assert result == (Fraction(tick), 2**53 + 1 + offset)
            assert type(result[1]) is int
    parameter_id = model.parameter("base").id
    replayed = eqiora.Model.from_bytes(model.to_bytes())
    assert replayed.digest == model.digest
    assert replayed.parameter(parameter_id).value == 2**53 + 1
    for original in (model, replayed):
        changed = original.commit(original.preview_value_edit(parameter_id, 2**53 + 3))
        assert changed.parameter(parameter_id).value == 2**53 + 3
        assert original.parameter(parameter_id).value == 2**53 + 1
        assert eqiora.Model.from_bytes(changed.to_bytes()).digest == changed.digest
    changed = model.commit(model.preview_value_edit(parameter_id, 2**53 + 3))
    updated = changed.sampled_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert updated.advance_ticks(1) == 1
    assert updated.output("result", 0)[1] == 2**53 + 3 + offset


@pytest.mark.parametrize("operation,expected", (("min", -2.0), ("max", 3.0)))
def test_extrema_dimensioned_real_values_keep_their_complete_unit(operation, expected):
    source = q.Source()
    owner = source.model("Lengths")
    rows = owner.index_set("Rows", extent=3)
    tick = owner.clock("tick", period_s=1)
    out = owner.output("result", value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)), at=tick)
    lengths = q.array(tuple(q.quantity(value, eqiora.units.m) for value in (3, -2, 3)))
    owner.relation("emit", at=tick, left=out,
                   right=getattr(owner, operation)(lambda i: lengths[q.ordinal(i)], over=rows))
    model = eqiora.compile(source=source, entry="Lengths")
    session = model.sampled_session(end_time_s=0.1, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    assert session.output("result", 0) == (Fraction(0), expected)


@pytest.mark.parametrize("operation", ("min", "max"))
def test_extrema_evaluate_every_operand_and_do_not_publish_a_failed_tick(operation):
    source = q.Source()
    owner = source.model("Eager")
    rows = owner.index_set("Rows", extent=2)
    tick = owner.clock("tick", period_s=1)
    denominators = owner.input("denominators", value_type=eqiora.ValueType.array(eqiora.ValueType.integer(), 2), at=tick)
    out = owner.output("result", value_type=eqiora.ValueType.integer(), at=tick)
    owner.relation("emit", at=tick, left=out, right=getattr(owner, operation)(
        lambda i: q.quotient(10, denominators[q.ordinal(i)]), over=rows))
    model = eqiora.compile(source=source, entry="Eager")
    session = model.sampled_session(end_time_s=1, max_step_s=0.1,
                                    inputs={"denominators": ("tick", [(1, 2), (1, 0)])})
    assert session.advance_ticks(1) == 1
    assert session.output("result", 0)[1] == (5 if operation == "min" else 10)
    with pytest.raises(eqiora.EqioraError, match="(?i)zero"):
        session.advance_ticks(1)
    assert session.next_tick == Fraction(1)
    assert session.output("result", 1) is None


@pytest.mark.parametrize("operation", ("min", "max"))
@pytest.mark.parametrize("body", ("true", "math.complex(1, 2)", "[1, 2]", "i"))
def test_extrema_reject_values_outside_ordinary_real_integer_scalars(operation, body):
    source = f"""model Invalid() {{
      indexset Rows = range(2);
      variable result: 1;
      relation emit {{ result = {operation}({body}, over = (i in Rows)); }}
    }}"""
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source, entry="Invalid")
    assert any(diagnostic.code == "EQ0603" for diagnostic in error.value.diagnostics)
