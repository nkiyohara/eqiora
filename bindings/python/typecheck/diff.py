from typing import assert_type

import eqiora
from eqiora import diff
from eqiora.diff import (
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
)


def check_diff_exports(
    plan: eqiora.Plan,
    parameter: ParameterRef,
    field: FieldRef,
    evaluation: DifferentiableEvaluation,
    primal: DifferentiablePrimal,
    jvp: DifferentiableJvp,
    vjp: DifferentiableVjp,
    evidence: DifferentiationEvidence,
) -> None:
    program = diff.compile(
        plan,
        inputs=(parameter,),
        output=field,
    )
    assert_type(program, DifferentiableProgram)
    assert_type(evaluation.primal(), DifferentiablePrimal)
    assert_type(primal.evidence, DifferentiationEvidence)
    assert_type(jvp.evidence, DifferentiationEvidence)
    assert_type(vjp.evidence, DifferentiationEvidence)
    assert_type(evidence.mode, DifferentiationMode)
    assert_type(evidence.implementation, DerivativeImplementation)
    assert_type(evidence.linearization_state, LinearizationState)
    assert_type(evidence.plan_identity, str)
    assert_type(program.plan_identity, str)
