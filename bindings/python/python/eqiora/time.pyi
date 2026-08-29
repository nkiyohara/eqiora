"""Closed temporal policies projected by the native Eqiora resolver.

Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyBackwardEuler``.
"""
from typing import Mapping, Self, final
from . import FieldRef

@final
class OdePlanView:
    """Resolved no-Mesh ODE capability.

    Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyOdePlanView``.
    """
    @property
    def kind(self) -> str: ...
    @property
    def backend(self) -> str: ...
    @property
    def backend_version(self) -> str: ...
    def __repr__(self) -> str: ...

@final
class BackwardEuler:
    """Positive Backward-Euler operator step.

    Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyBackwardEuler``.
    """
    def __new__(cls, step_s: float) -> Self: ...
    @property
    def step_s(self) -> float: ...
    def __repr__(self) -> str: ...

@final
class Tsitouras45:
    """Adaptive explicit ODE integration with exact Field-bound SI tolerances.

    Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyTsitouras45``.
    """
    def __new__(
        cls,
        *,
        initial_step_s: float,
        relative_tolerance: float,
        absolute_tolerances: Mapping[FieldRef, float],
    ) -> Self: ...
    @property
    def initial_step_s(self) -> float: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerances(self) -> dict[FieldRef, float]: ...
    def __repr__(self) -> str: ...

__all__ = ["BackwardEuler", "OdePlanView", "Tsitouras45"]
