from typing import overload

from matplotlib.figure import Figure

from . import FieldRef, Result
from .solid import MixedBoundaryElasticityResult
from .trajectory import Trajectory

def plot_deformed_field(
    trajectory: Trajectory,
    /,
    *,
    step: int,
    field: FieldRef,
    scale: float = 1.0,
) -> Figure: ...
def plot_displacement(
    result: MixedBoundaryElasticityResult,
    /,
    *,
    scale: float = 1.0,
) -> Figure: ...
@overload
def plot_scalar_field(
    trajectory: Trajectory,
    /,
    *,
    step: int,
    field: FieldRef,
) -> Figure: ...
@overload
def plot_scalar_field(
    result: Result,
    /,
    *,
    field: FieldRef,
) -> Figure: ...

__all__ = [
    "plot_deformed_field",
    "plot_displacement",
    "plot_scalar_field",
]
