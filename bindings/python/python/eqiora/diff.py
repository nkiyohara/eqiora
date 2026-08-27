"""Accepted implicit differentiation over exact Eqiora programs."""

from ._eqiora import (
    DerivativeImplementation,
    DifferentiableEvaluation,
    DifferentiableJvp,
    DifferentiablePrimal,
    DifferentiableProgram,
    DifferentiableVjp,
    DifferentiationEvidence,
    DifferentiationMode,
    FieldRef,
    LinearizationState,
    ParameterRef,
    _compile_differentiable,
)

__all__ = [
    "DerivativeImplementation",
    "DifferentiableEvaluation",
    "DifferentiableJvp",
    "DifferentiablePrimal",
    "DifferentiableProgram",
    "DifferentiableVjp",
    "DifferentiationEvidence",
    "DifferentiationMode",
    "FieldRef",
    "LinearizationState",
    "ParameterRef",
    "compile",
]


def compile(plan, *, inputs, output):
    """Compile one immutable program over an ordered Parameter coordinate set.

    ``program.evaluate(parameters)`` accepts another complete numerical point
    without mutating the Model or Plan. Parameters, tangents, and
    cotangents are exact CPU ``float64`` arrays.
    """

    return _compile_differentiable(
        plan,
        inputs=inputs,
        output=output,
    )
