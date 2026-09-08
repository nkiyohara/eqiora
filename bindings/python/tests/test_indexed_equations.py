"""Installed compile routes consume indexed equations without a second Python DSL."""

from fractions import Fraction

import pytest

import eqiora


def sampled_family_source(kind="model", *, selector="index(Stages, ordinal(j))"):
    return f"""
component Drive(clock tick: periodic, output y: integer at tick) {{}}
component Accumulator(clock tick: periodic, input u: integer at tick,
                      output y: integer at tick) {{
  state memory: integer at tick;
  initial {{ memory = 0; }}
  relation step at tick {{
    next(memory) = pre(memory) + u;
    y = next(memory);
  }}
}}
public {kind} Network(output values: array<integer, 3> at tick) {{
  clock tick = periodic(1[s]);
  indexset Stages = range(3);
  indexset Other = range(3);
  instance driver[k in Stages]: Drive(tick = tick);
  instance cell[k in Stages]: Accumulator(tick = tick);
  relation drive[i in Stages] at tick {{
    driver[index(Stages, ordinal(i))].y = ordinal(i) + 1;
  }}
  connect [j in Stages] driver[{selector}].y -> cell[index(Stages, ordinal(j))].u;
  relation expose at tick {{
    values = [cell[index(Stages, 0)].y, cell[index(Stages, 1)].y,
              cell[index(Stages, 2)].y];
  }}
}}
"""


@pytest.mark.parametrize("kind", ("model", "component"))
def test_indexed_sampled_equations_compile_from_text_and_file_and_resume(kind, tmp_path):
    source = sampled_family_source(kind)
    model = eqiora.compile(source=source, entry="Network")
    path = tmp_path / "indexed_equations.eqi"
    path.write_text(source, encoding="utf-8")
    from_file = eqiora.compile(path=path, entry="Network")
    assert from_file.to_bytes() == model.to_bytes()
    output_name = "definition.values" if kind == "component" else "values"
    session = model.execution_session(end_time_s=2, max_step_s=0.1, inputs={})
    assert session.output(output_name, 0) is None
    assert session.advance_ticks(1) == 1
    resumed = from_file.resume_execution(session.checkpoint())
    # Cell i starts at zero and adds i+1 per phase-zero tick. Publication reads
    # next(memory), so the first published value already includes one update.
    expected = ((1, 2, 3), (2, 4, 6), (3, 6, 9))
    for current in (session, resumed):
        assert current.advance_ticks(2) == 2
        for tick, values in enumerate(expected):
            output = current.output(output_name, tick)
            assert output == (Fraction(tick), values)
            assert isinstance(output[1], tuple)
            assert all(type(value) is int for value in output[1])
        assert current.output(output_name, 3) is None
        assert current.next_tick is None


@pytest.mark.parametrize("kind", ("model", "component"))
@pytest.mark.parametrize("selector", (
    "index(Other, ordinal(j))",
    "index(Stages, ordinal(j) + 1)",
    "index(Stages, ordinal(j) - 1)",
))
def test_indexed_connections_reject_foreign_sets_and_invalid_neighbors(kind, selector):
    with pytest.raises(eqiora.ValidationError) as error:
        eqiora.compile(source=sampled_family_source(kind, selector=selector), entry="Network")
    # Both sets have extent three: equality of extent cannot substitute ownership.
    # Neighbor expressions hit the upper/lower bound during fixed elaboration.
    assert any("index" in diagnostic.message.lower() for diagnostic in error.value.diagnostics)
