"""Accepted implicit differentiation over exact Eqiora programs.

Authority: ``bindings/python/python/eqiora/diff.py``.
"""

from collections.abc import Sequence

from . import (
    DerivativeImplementation as DerivativeImplementation,
    DifferentiableEvaluation as DifferentiableEvaluation,
    DifferentiableJvp as DifferentiableJvp,
    DifferentiablePrimal as DifferentiablePrimal,
    DifferentiableProgram as DifferentiableProgram,
    DifferentiableVjp as DifferentiableVjp,
    DifferentiationEvidence as DifferentiationEvidence,
    DifferentiationMode as DifferentiationMode,
    FieldRef as FieldRef,
    LinearizationState as LinearizationState,
    Plan,
    ParameterRef as ParameterRef,
)

def compile(
    plan: Plan,
    *,
    inputs: Sequence[ParameterRef],
    output: FieldRef,
) -> DifferentiableProgram:
    """Compile a program over an ordered parameter-coordinate set.

    Authority: ``bindings/python/python/eqiora/diff.py::compile``.
    """

    ...

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
