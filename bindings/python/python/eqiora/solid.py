"""Typed structural intents, Plans, and scientific evidence."""

from __future__ import annotations

import warnings
from typing import TYPE_CHECKING

from ._eqiora import (
    LinearElasticity,
    LinearElasticityEvidence,
    LinearElasticityPlan,
    linear_elasticity_evidence,
    resolve_linear_elasticity as resolve,
)

if TYPE_CHECKING:
    from . import Model, Result


def solve_mixed_boundary_elasticity(model: Model, /) -> Result:
    """Compatibility delegation to the explicit Plan and common Result path."""

    warnings.warn(
        "solve_mixed_boundary_elasticity() is deprecated; use "
        "eqiora.solid.resolve() and eqiora.run(..., plan=...) instead",
        DeprecationWarning,
        stacklevel=2,
    )
    from . import run

    intent = LinearElasticity(
        cells_per_axis=16,
        relative_tolerance=1.0e-12,
        absolute_tolerance=1.0e-14,
        maximum_iterations=10_000,
    )
    return run(model, plan=resolve(model, intent))


def __getattr__(name: str):
    if name == "MixedBoundaryElasticityResult":
        warnings.warn(
            "MixedBoundaryElasticityResult is deprecated; use eqiora.Result instead",
            DeprecationWarning,
            stacklevel=2,
        )
        from . import Result

        return Result
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = [
    "LinearElasticity",
    "LinearElasticityEvidence",
    "LinearElasticityPlan",
    "MixedBoundaryElasticityResult",  # noqa: F822 - supplied lazily by __getattr__
    "linear_elasticity_evidence",
    "resolve",
    "solve_mixed_boundary_elasticity",
]
