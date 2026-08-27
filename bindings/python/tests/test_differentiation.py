from __future__ import annotations

import numpy as np
import pytest

import eqiora


POISSON = """
model python_differentiated_poisson {
  domain square = box(0, 1, 0, 1);
  domain x_lower = boundary(square, axis = 0, side = lower);
  domain x_upper = boundary(square, axis = 0, side = upper);
  domain y_lower = boundary(square, axis = 1, side = lower);
  domain y_upper = boundary(square, axis = 1, side = upper);
  representation scalar_space = continuum;
  field potential on square as scalar_space: 1 = 0;
  parameter diffusion: 1 = 1;
  parameter wave_number: 1 / m = 3.141592653589793;
  parameter source_scale: 1 / m ^ 2 = 19.739208802178716;
  parameter boundary_offset: 1 = 0;
  relation balance continuous on square {
    -div(diffusion * grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) - boundary_offset = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) - boundary_offset = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) - boundary_offset = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) - boundary_offset = 0; }
}
"""


class DLPackProducer:
    def __init__(self, values: np.ndarray, *, device: tuple[int, int] = (1, 0)):
        self.values = values
        self.device = device
        self.device_queries = 0
        self.exports: list[dict[str, object]] = []

    def __dlpack_device__(self) -> tuple[int, int]:
        self.device_queries += 1
        return self.device

    def __dlpack__(self, **kwargs):
        self.exports.append(dict(kwargs))
        return self.values.__dlpack__(**kwargs)


@pytest.mark.parametrize(
    "method",
    [
        eqiora.ScalarEllipticMethod.FiniteElement,
        eqiora.ScalarEllipticMethod.FiniteVolume,
    ],
)
def test_public_diff_module_exposes_paired_complete_field_actions(method) -> None:
    model = eqiora.compile(source=POISSON)
    realization = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(method=method, cells_per_axis=4),
    )
    inputs = (
        model.parameter("source_scale"),
        model.parameter("diffusion"),
        model.parameter("boundary_offset"),
    )
    program = eqiora.diff.compile(
        model,
        realization,
        inputs=inputs,
        output=model.field("potential"),
    )

    direction = np.array([0.7, -0.2, 0.3], dtype=np.float64)
    primal = program.primal()
    jvp = program.jvp(direction)
    cotangent = np.ones(program.output_shape, dtype=np.float64)
    vjp = program.vjp(cotangent)

    assert isinstance(program, eqiora.DifferentiableProgram)
    assert isinstance(primal.output, eqiora.Array)
    assert primal.output.shape == program.output_shape
    np.testing.assert_array_equal(jvp.output.numpy(), primal.output.numpy())
    np.testing.assert_array_equal(vjp.output.numpy(), primal.output.numpy())
    np.testing.assert_allclose(
        np.dot(jvp.tangent.numpy(), cotangent),
        np.dot(direction, vjp.input_cotangent.numpy()),
        rtol=2.0e-10,
        atol=2.0e-11,
    )
    assert primal.evidence.linearization_state == eqiora.LinearizationState.Established
    assert primal.evidence.derivative_solve is None
    assert jvp.evidence.primal_solve.orientation == "normal"
    assert jvp.evidence.derivative_solve.orientation == "normal"
    assert vjp.evidence.derivative_solve.orientation == "transposed"
    assert program.input_ids == [reference.id for reference in inputs]

    point = np.array([17.0, 1.2, 0.1], dtype=np.float64)
    evaluation = program.evaluate(point)
    point[:] = -1.0
    assert isinstance(evaluation, eqiora.DifferentiableEvaluation)
    np.testing.assert_array_equal(
        evaluation.point.numpy(copy=False),
        np.array([17.0, 1.2, 0.1], dtype=np.float64),
    )
    assert not evaluation.point.numpy(copy=False).flags.writeable
    evaluated_jvp = evaluation.jvp(direction)
    evaluated_vjp = evaluation.vjp(cotangent)
    np.testing.assert_array_equal(
        evaluated_jvp.output.numpy(copy=False),
        evaluation.primal().output.numpy(copy=False),
    )
    np.testing.assert_allclose(
        np.dot(evaluated_jvp.tangent.numpy(copy=False), cotangent),
        np.dot(direction, evaluated_vjp.input_cotangent.numpy(copy=False)),
        rtol=2.0e-10,
        atol=2.0e-11,
    )


def test_diff_input_admission_is_explicit_and_model_bound() -> None:
    model = eqiora.compile(source=POISSON)
    realization = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(
            method=eqiora.ScalarEllipticMethod.FiniteElement,
            cells_per_axis=4,
        ),
    )
    program = eqiora.diff.compile(
        model,
        realization,
        inputs=(model.parameter("source_scale"),),
        output=model.field("potential"),
    )

    direction = np.array([1.0], dtype=np.float64)
    producer = DLPackProducer(direction)
    protocol_result = program.jvp(producer)
    numpy_result = program.jvp(direction)
    np.testing.assert_array_equal(
        protocol_result.tangent.numpy(copy=False),
        numpy_result.tangent.numpy(copy=False),
    )
    assert producer.device_queries == 1
    assert len(producer.exports) == 1
    export = producer.exports[0]
    assert export["dl_device"] == (1, 0)
    assert export["copy"] is False
    assert export["max_version"][0] == 1
    assert export.get("stream") is None

    cotangent = np.arange(program.output_shape[0], dtype=np.float64)
    cotangent_producer = DLPackProducer(cotangent)
    protocol_vjp = program.vjp(cotangent_producer)
    numpy_vjp = program.vjp(cotangent)
    np.testing.assert_array_equal(
        protocol_vjp.input_cotangent.numpy(copy=False),
        numpy_vjp.input_cotangent.numpy(copy=False),
    )
    assert cotangent_producer.device_queries == 1
    assert len(cotangent_producer.exports) == 1
    assert cotangent_producer.exports[0]["copy"] is False

    foreign_device = DLPackProducer(direction, device=(2, 0))
    with pytest.raises(BufferError, match="already reside on CPU device 0"):
        program.jvp(foreign_device)
    assert foreign_device.exports == []

    foreign_ordinal = DLPackProducer(direction, device=(1, 1))
    with pytest.raises(BufferError, match="already reside on CPU device 0"):
        program.jvp(foreign_ordinal)
    assert foreign_ordinal.exports == []

    for rejected in (
        DLPackProducer(np.array([1.0], dtype=np.float32)),
        DLPackProducer(np.array([[1.0]], dtype=np.float64)),
        DLPackProducer(np.array([1.0, 2.0], dtype=np.float64)),
        DLPackProducer(np.array([np.nan], dtype=np.float64)),
    ):
        with pytest.raises(BufferError):
            program.jvp(rejected)

    strided_cotangent = DLPackProducer(
        np.arange(2 * program.output_shape[0], dtype=np.float64)[::2]
    )
    with pytest.raises(BufferError, match="C-contiguous"):
        program.vjp(strided_cotangent)

    class IncompleteDLPackProducer:
        def __dlpack__(self, **kwargs):
            return direction.__dlpack__(**kwargs)

    with pytest.raises(BufferError, match="complete DLPack producer"):
        program.jvp(IncompleteDLPackProducer())

    with pytest.raises(BufferError):
        program.jvp(np.array([1.0], dtype=np.float32))
    with pytest.raises(BufferError):
        program.jvp(np.array([np.nan], dtype=np.float64))
    with pytest.raises(BufferError):
        program.jvp(
            np.array(
                [1.0],
                dtype=np.dtype(">f8" if np.little_endian else "<f8"),
            )
        )
    with pytest.raises(BufferError):
        program.vjp(np.zeros(program.output_shape[0] - 1, dtype=np.float64))
    with pytest.raises(BufferError):
        program.evaluate(np.zeros(2, dtype=np.float64))
    with pytest.raises(eqiora.ValidationError):
        eqiora.diff.compile(
            model,
            realization,
            inputs=iter((model.parameter("source_scale"),)),
            output=model.field("potential"),
        )

    foreign = eqiora.compile(
        source=POISSON.replace("diffusion: 1 = 1", "diffusion: 1 = 2")
    )
    with pytest.raises(eqiora.ValidationError):
        eqiora.diff.compile(
            foreign,
            realization,
            inputs=(model.parameter("source_scale"),),
            output=model.field("potential"),
        )
