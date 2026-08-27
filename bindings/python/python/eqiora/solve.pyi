"""Closed algebraic solve policies.

Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyLinear``.
"""
from typing import Self, final

@final
class Linear:
    """Linear-solve controls resolved against Model-owned operator meaning.

    Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyLinear``.
    """
    def __new__(
        cls,
        *,
        relative_tolerance: float,
        absolute_tolerance: float,
        maximum_iterations: int,
    ) -> Self: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

__all__ = ["Linear"]
