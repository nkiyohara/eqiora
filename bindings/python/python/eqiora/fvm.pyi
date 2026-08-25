"""Closed finite-volume spatial policies.

Authority: ``crates/eqiora-python/src/common_plan.rs::PyCellCenteredTpfa``.
"""

from typing import Self, final

@final
class CellCenteredTpfa:
    """Cell-centred orthogonal two-point-flux spatial policy.

    Authority: ``crates/eqiora-python/src/common_plan.rs::PyCellCenteredTpfa``.
    """

    def __new__(cls) -> Self: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

__all__ = ["CellCenteredTpfa"]
