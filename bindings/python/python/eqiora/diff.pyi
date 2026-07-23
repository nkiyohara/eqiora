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
    Model,
    ParameterRef as ParameterRef,
    Realization,
)

def compile(
    model: Model,
    realization: Realization,
    *,
    inputs: Sequence[ParameterRef],
    output: FieldRef,
) -> DifferentiableProgram: ...

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
