"""Installed public-Python execution of the coupled scalar example."""

from pathlib import Path
import runpy

import numpy as np
import pytest


PROGRAM = Path(__file__).resolve().parents[3] / "examples/python/coupled_scalar.py"


@pytest.mark.parametrize("cells", [8, 16])
def test_coupled_scalar_example_preserves_both_fields_and_polynomial_solution(cells):
    solution = runpy.run_path(str(PROGRAM))["solve"](cells)
    model = solution.plan.model
    expected_fields = (model.field("definition.u"), model.field("definition.v"))
    assert len(solution.plan.fields) == 2
    assert all(field in solution.plan.fields for field in expected_fields)
    for field in expected_fields:
        assert solution.result.output(field).field == field

    exact = solution.x * (1.0 - solution.x)
    # On each cell q-I_h q=(x-x_i)(x_{i+1}-x). Its integral against an
    # interior hat is h³/6. The two reaction rows therefore leave defects
    # h³/6 and h³ at the exact nodal values. The reduced matrix is an
    # M-matrix with row sums >=2h, giving ||error||_infinity <= h²/2.
    bound = 0.5 / cells**2 + 2.0e-10
    np.testing.assert_allclose(solution.u, exact, rtol=0.0, atol=bound)
    np.testing.assert_allclose(solution.v, 2.0 * exact, rtol=0.0, atol=bound)
    np.testing.assert_array_equal(solution.u[[0, -1]], [0.0, 0.0])
    np.testing.assert_array_equal(solution.v[[0, -1]], [0.0, 0.0])
