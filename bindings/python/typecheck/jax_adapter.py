from typing import assert_type

import jax

import eqiora
import eqiora.jax as eqjax


def check_jax_adapter(
    program: eqiora.DifferentiableProgram,
    parameters: jax.Array,
) -> None:
    direct = eqjax.JaxProgram(program)
    bound = eqjax.bind(program)
    assert_type(direct, eqjax.JaxProgram)
    assert_type(bound, eqjax.JaxProgram)
    assert_type(bound.program, eqiora.DifferentiableProgram)
    assert_type(bound.input_shape, tuple[int])
    assert_type(bound.output_shape, tuple[int])
    assert_type(bound(parameters), jax.Array)
