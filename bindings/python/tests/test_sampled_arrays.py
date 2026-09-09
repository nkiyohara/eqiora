"""Fixed channel-array state updates use one exact sampled-session transaction."""

from fractions import Fraction

import pytest

import eqiora

q = eqiora.lang


def coupled_source(domain, *, reverse=False):
    source = eqiora.Module("main")
    owner = source.model("CoupledArrays")
    tick = owner.clock("tick", period_s=1)
    scalar = eqiora.ValueType.integer() if domain == "integer" else eqiora.ValueType.real()
    kind = eqiora.ValueType.array(scalar, 2)
    drive = owner.input("drive", value_type=kind, at=tick)
    a = owner.field("a", role=eqiora.FieldRole.State, value_type=kind, at=tick)
    b = owner.field("b", role=eqiora.FieldRole.State, value_type=kind, at=tick)
    seen_a = owner.output("seen_a", value_type=kind, at=tick)
    seen_b = owner.output("seen_b", value_type=kind, at=tick)
    seed = 2**53 + 1 if domain == "integer" else 1.0
    initial = [(q.pre(a), q.array((seed, 2))), (q.pre(b), q.array((10, 20)))]
    owner.initial(*(reversed(initial) if reverse else initial))
    equations = [
        ("update_a", q.next(a), q.array((q.pre(a)[0] + drive[0], q.pre(b)[1]))),
        ("update_b", q.next(b), q.array((q.pre(a)[1], q.pre(b)[0] + drive[1]))),
        ("observe_a", seen_a, q.pre(a)),
        ("observe_b", seen_b, q.pre(b)),
    ]
    for name, left, right in reversed(equations) if reverse else equations:
        owner.relation(name, q.equation(left, right), at=tick)
    return source, seed


@pytest.mark.parametrize("domain", ("real", "integer"))
def test_coupled_array_updates_are_simultaneous_and_resume_exactly(domain, tmp_path):
    source, seed = coupled_source(domain)
    model = eqiora.compile(source=source, entry="CoupledArrays")
    path = tmp_path / "coupled_arrays.eqi"
    source.write_eqi(path)
    from_file = eqiora.compile(path=path, entry="CoupledArrays")
    assert from_file.to_bytes() == model.to_bytes()
    permuted, _ = coupled_source(domain, reverse=True)
    reordered = eqiora.compile(source=permuted, entry="CoupledArrays")
    inputs = {"drive": ("tick", [(3, 4), (5, 6), (7, 8)])}
    session = model.execution_session(end_time_s=2, max_step_s=0.1, inputs=inputs)
    other = reordered.execution_session(end_time_s=2, max_step_s=0.1, inputs=inputs)
    # a'=(a[0]+drive[0], b[1]); b'=(a[1], b[0]+drive[1]),
    # with every RHS reading the same pre-tick pair.
    states = (
        ((seed, 2), (10, 20)),
        ((seed + 3, 20), (2, 14)),
        ((seed + 8, 14), (20, 8)),
        ((seed + 15, 8), (14, 28)),
    )
    assert session.output("seen_a", 0) is None
    assert session.advance_ticks(1) == other.advance_ticks(1) == 1
    assert session.field("a") == other.field("a") == states[1][0]
    assert session.field("b") == other.field("b") == states[1][1]
    resumed = from_file.resume_execution(session.checkpoint())
    for index in (1, 2):
        for current in (session, other, resumed):
            assert current.advance_ticks(1) == 1
            assert current.field("a") == states[index + 1][0]
            assert current.field("b") == states[index + 1][1]
    for current in (session, other, resumed):
        for index in range(3):
            for name, expected in zip(("seen_a", "seen_b"), states[index]):
                output = current.output(name, index)
                assert output[0] == Fraction(index)
                assert output[1] == expected
                assert isinstance(output[1], tuple)
                assert all(type(value) is (int if domain == "integer" else float)
                           for value in output[1])
        assert current.output("seen_a", 3) is None
        assert current.next_tick is None


def overflow_source():
    source = eqiora.Module("main")
    owner = source.model("AtomicArrays")
    tick = owner.clock("tick", period_s=1)
    real = eqiora.ValueType.array(eqiora.ValueType.real(), 2)
    integer = eqiora.ValueType.array(eqiora.ValueType.integer(), 2)
    safe = owner.field("safe", role=eqiora.FieldRole.State, value_type=real, at=tick)
    count = owner.field("count", role=eqiora.FieldRole.State, value_type=integer, at=tick)
    seen_safe = owner.output("seen_safe", value_type=real, at=tick)
    seen_count = owner.output("seen_count", value_type=integer, at=tick)
    owner.initial((q.pre(safe), q.array((1.0, 2.0))),
                  (q.pre(count), q.array((4, 2**63 - 2))))
    owner.relation("safe_update", q.equation(q.next(safe), q.array((q.pre(safe)[0] + 0.5, q.pre(safe)[1] - 0.25))), at=tick)
    owner.relation("counter_update", q.equation(q.next(count), q.array((q.pre(count)[0] + 1, q.pre(count)[1] + 1))), at=tick)
    owner.relation("observe_safe", q.equation(seen_safe, q.pre(safe)), at=tick)
    owner.relation("observe_count", q.equation(seen_count, q.pre(count)), at=tick)
    return source


def test_late_integer_array_overflow_keeps_the_entire_previous_tick():
    model = eqiora.compile(source=overflow_source(), entry="AtomicArrays")
    session = model.execution_session(end_time_s=2, max_step_s=0.1, inputs={})
    assert session.advance_ticks(1) == 1
    assert session.field("safe") == (1.5, 1.75)
    assert session.field("count") == (5, 2**63 - 1)
    resumed = model.resume_execution(session.checkpoint())
    for current in (session, resumed):
        for _ in range(2):
            # The second integer element overflows after a valid first element.
            with pytest.raises(eqiora.EqioraError, match="(?i)overflow"):
                current.advance_ticks(1)
            assert current.field("safe") == (1.5, 1.75)
            assert current.field("count") == (5, 2**63 - 1)
            assert current.next_tick == Fraction(1)
            assert current.output("seen_safe", 0) == (Fraction(0), (1.0, 2.0))
            assert current.output("seen_count", 0) == (Fraction(0), (4, 2**63 - 2))
            assert current.output("seen_safe", 1) is None
            assert current.output("seen_count", 1) is None


@pytest.mark.parametrize("sample", (3, (1,), (1, 2, 3), ((1, 2), (3, 4)), (True, 2), (1.0, 2)))
def test_integer_array_input_requires_exact_shape_and_component_types(sample):
    source, _ = coupled_source("integer")
    model = eqiora.compile(source=source, entry="CoupledArrays")
    with pytest.raises((TypeError, ValueError)):
        model.execution_session(end_time_s=2, max_step_s=0.1,
                              inputs={"drive": ("tick", [(3, 4), sample, (7, 8)])})


@pytest.mark.parametrize("right", ("[1]", "1", "tensor_value(frame = body, components = [1, 2])"))
def test_whole_array_assignment_rejects_shape_broadcast_and_spatial_substitution(right):
    source = f"""
model InvalidArray() {{
  domain body = box(0, 1, 0, 1);
  clock tick = periodic(1 [s] / 1, phase = 0 [s] / 1);
  state memory: array<1, 2> at tick;
  initial {{ pre(memory) = [1, 2]; }}
  relation update at tick {{ next(memory) = {right}; }}
}}
"""
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=source)
    assert error.value.diagnostics[0].code == "EQ0603"
