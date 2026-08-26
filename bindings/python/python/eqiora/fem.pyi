"""Closed finite-element spatial policies.

Authority: ``crates/eqiora-python/src/common_plan.rs::PyQ1``.
"""
from typing import Self, final

@final
class Q1:
    """Continuous tensor-product Q1 Galerkin spatial policy.

    Authority: ``crates/eqiora-python/src/common_plan.rs::PyQ1``.
    """
    def __new__(cls) -> Self: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

__all__ = ["Q1"]
