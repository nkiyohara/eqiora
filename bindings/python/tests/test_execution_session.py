"""The reference session exposes accepted boundaries and exact activation identities."""

from fractions import Fraction
import json

import pytest

import eqiora


SOURCE = """
public model Counter(clock tick: periodic, output count: integer at tick) {
  state memory: integer at tick;
  initial { memory = 0; }
  relation step at tick {
    next(memory) = pre(memory) + 1;
    count = next(memory);
  }
}
"""


def test_execution_session_boundary_progress_sequence_and_checkpoint():
    clock = eqiora.ClockDomain(period_s=1)
    model = eqiora.compile(source=SOURCE, entry="Counter", bindings={"tick": clock})
    activations = [node["id"]["ulid"] for node in json.loads(model.to_bytes())["nodes"]
                   if node["definition"]["kind"] == "activation"
                   and node["definition"]["activation"]["kind"] == "periodic"]
    assert len(activations) == 1
    expected_sequence = ((activations[0],),)
    session = model.execution_session(end_time_s=2, max_step_s=1, inputs={})
    assert isinstance(session, eqiora.ExecutionSession)
    assert session.activation_sequence == ()
    assert set(session.progress) == {"model_time", "end_time", "accepted_steps", "maximum_steps"}
    assert session.progress["end_time"] == 2
    assert session.progress["accepted_steps"] == 0
    assert session.advance() is True
    assert session.activation_sequence == expected_sequence
    assert session.output("count", 0) == (Fraction(0), 1)
    observation = session.progress
    assert observation["model_time"] == 0
    assert observation["accepted_steps"] == 1
    observation["model_time"] = -100
    assert session.progress["model_time"] == 0
    checkpoint = session.checkpoint()
    assert isinstance(checkpoint, eqiora.ExecutionCheckpoint)
    resumed = model.resume_execution(checkpoint)
    assert resumed.activation_sequence == session.activation_sequence
    assert resumed.progress == session.progress
    for current in (session, resumed):
        for tick in (1, 2):
            assert current.advance() is True
            assert current.progress["model_time"] == tick
            assert current.activation_sequence == expected_sequence
            assert current.output("count", tick) == (Fraction(tick), tick + 1)
        assert current.advance() is False
        assert current.field("memory") == 3
        assert current.next_tick is None
    other_clock = eqiora.ClockDomain(period_s=1)
    foreign = eqiora.compile(source=SOURCE, entry="Counter", bindings={"tick": other_clock})
    with pytest.raises(eqiora.EqioraError):
        foreign.resume_execution(checkpoint)
