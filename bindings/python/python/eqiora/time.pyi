"""Closed temporal policies projected by the native Eqiora resolver.

Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyBackwardEuler``.
"""
from typing import Self, final

@final
class BackwardEuler:
    """Positive Backward-Euler operator step.

    Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyBackwardEuler``.
    """
    def __new__(cls, step_s: float, /) -> Self: ...
    @property
    def step_s(self) -> float: ...
    def __repr__(self) -> str: ...

__all__ = ["BackwardEuler"]
