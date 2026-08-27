from __future__ import annotations

import os
import subprocess
import sys

import numpy as np
import pytest

torch = pytest.importorskip("torch")

import eqiora
import eqiora.torch as eqtorch


POISSON = """
model pytorch_differentiated_poisson {
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


def test_base_import_remains_torch_free() -> None:
    script = """
import sys
import eqiora

assert "torch" not in sys.modules
"""
    subprocess.run(
        [sys.executable, "-I", "-c", script],
        check=True,
        text=True,
        capture_output=True,
    )


def differentiable_program(method) -> eqiora.DifferentiableProgram:
    model = eqiora.compile(source=POISSON)
    realization = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(method=method, cells_per_axis=4),
    )
    return eqiora.diff.compile(
        model,
        realization,
        inputs=(
            model.parameter("source_scale"),
            model.parameter("diffusion"),
            model.parameter("boundary_offset"),
        ),
        output=model.field("potential"),
    )


@pytest.mark.parametrize(
    "method",
    [
        eqiora.ScalarEllipticMethod.FiniteElement,
        eqiora.ScalarEllipticMethod.FiniteVolume,
    ],
)
def test_functional_operator_matches_native_primal_and_vjp(method) -> None:
    program = differentiable_program(method)
    operator = eqtorch.bind(program)
    parameters = torch.tensor(
        [17.0, 1.2, 0.1],
        dtype=torch.float64,
        requires_grad=True,
    )
    cotangent = torch.linspace(
        0.25,
        1.25,
        operator.output_shape[0],
        dtype=torch.float64,
    )

    output = operator(parameters)
    native = program.evaluate(parameters.detach())
    np.testing.assert_allclose(
        output.detach().numpy(),
        native.primal().output.numpy(copy=False),
        rtol=0.0,
        atol=0.0,
    )
    assert output.dtype is torch.float64
    assert output.device == torch.device("cpu")
    assert output.shape == operator.output_shape
    assert output.data_ptr() != parameters.data_ptr()

    output.backward(cotangent)
    np.testing.assert_allclose(
        parameters.grad.numpy(),
        native.vjp(cotangent).input_cotangent.numpy(copy=False),
        rtol=2.0e-11,
        atol=2.0e-12,
    )

    with torch.no_grad():
        output[0] = -123.0
    assert operator(parameters.detach())[0].item() != -123.0


def test_autograd_is_repeatable_and_zero_cotangent_is_exact() -> None:
    operator = eqtorch.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    parameters = torch.tensor(
        [17.0, 1.2, 0.1],
        dtype=torch.float64,
        requires_grad=True,
    )

    output = operator(parameters)
    output.sum().backward(retain_graph=True)
    first = parameters.grad.detach().clone()
    parameters.grad = None
    output.sum().backward()
    torch.testing.assert_close(parameters.grad, first, rtol=0.0, atol=0.0)

    zero = torch.autograd.grad(
        operator(parameters),
        parameters,
        grad_outputs=torch.zeros(operator.output_shape, dtype=torch.float64),
    )[0]
    torch.testing.assert_close(zero, torch.zeros_like(parameters), rtol=0.0, atol=0.0)
    assert not operator(parameters.detach()).requires_grad


def test_double_backward_is_an_explicit_nonclaim() -> None:
    operator = eqtorch.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    parameters = torch.tensor(
        [17.0, 1.2, 0.1],
        dtype=torch.float64,
        requires_grad=True,
    )
    first = torch.autograd.grad(
        operator(parameters).sum(),
        parameters,
        create_graph=True,
    )[0]
    with pytest.raises(RuntimeError):
        torch.autograd.grad(first.sum(), parameters)


@pytest.mark.parametrize(
    "method",
    [
        eqiora.ScalarEllipticMethod.FiniteElement,
        eqiora.ScalarEllipticMethod.FiniteVolume,
    ],
)
def test_registration_passes_opcheck_gradcheck_and_fullgraph_compile(method) -> None:
    operator = eqtorch.bind(differentiable_program(method))
    parameters = torch.tensor(
        [17.0, 1.2, 0.1],
        dtype=torch.float64,
        requires_grad=True,
    )

    torch.library.opcheck(
        eqtorch._solve,
        (
            parameters,
            operator._token,
            operator.input_shape[0],
            operator.output_shape[0],
        ),
    )
    assert torch.autograd.gradcheck(
        operator,
        (parameters,),
        eps=1.0e-6,
        atol=2.0e-6,
        rtol=2.0e-5,
    )

    def objective(values):
        return operator(values).square().sum()

    eager_value = objective(parameters)
    eager = torch.autograd.grad(eager_value, parameters)[0]
    compiled = torch.compile(objective, fullgraph=True)
    compiled_parameters = parameters.detach().clone().requires_grad_(True)
    compiled_value = compiled(compiled_parameters)
    torch.testing.assert_close(
        compiled_value.detach(),
        eager_value.detach(),
        rtol=2.0e-13,
        atol=2.0e-14,
    )
    actual = torch.autograd.grad(compiled_value, compiled_parameters)[0]
    torch.testing.assert_close(actual, eager, rtol=2.0e-11, atol=2.0e-12)


@pytest.mark.parametrize(
    ("parameters", "error"),
    [
        (torch.ones(3, dtype=torch.float32), TypeError),
        (torch.ones((1, 3), dtype=torch.float64), ValueError),
        (torch.arange(6, dtype=torch.float64)[::2], ValueError),
        (
            torch.sparse_coo_tensor(
                torch.tensor([[0, 2]]),
                torch.ones(2, dtype=torch.float64),
                (3,),
                check_invariants=False,
            ),
            TypeError,
        ),
        (torch.ones(2, dtype=torch.float64), ValueError),
        (torch.tensor([17.0, float("nan"), 0.1], dtype=torch.float64), BufferError),
    ],
)
def test_inputs_fail_closed_before_or_at_native_admission(parameters, error) -> None:
    operator = eqtorch.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    with pytest.raises(error):
        operator(parameters)


def test_registry_is_identity_deduplicated_and_retained_for_backward() -> None:
    program = differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    first = eqtorch.bind(program)
    second = eqtorch.bind(program)
    assert first._token == second._token

    parameters = torch.tensor(
        [17.0, 1.2, 0.1],
        dtype=torch.float64,
        requires_grad=True,
    )
    output = eqtorch.bind(program)(parameters)
    output.sum().backward()
    assert parameters.grad is not None


def test_unknown_or_mismatched_tokens_fail_closed() -> None:
    parameters = torch.tensor([17.0, 1.2, 0.1], dtype=torch.float64)
    with pytest.raises(RuntimeError, match="not available"):
        eqtorch._solve(parameters, "not-a-token", 3, 25)

    operator = eqtorch.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    with pytest.raises(RuntimeError, match="metadata"):
        eqtorch._solve(parameters, operator._token, 3, operator.output_shape[0] + 1)


def test_supported_torch_series_is_exact() -> None:
    assert eqtorch._torch_series(torch.__version__) == (2, 13)
    if expected := os.environ.get("EQIORA_TEST_TORCH_VERSION"):
        assert torch.__version__.split("+", maxsplit=1)[0] == expected
