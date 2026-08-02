from matplotlib.figure import Figure

from . import FieldRef
from .fluid import CircularHoleSteadyStokesResult
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
def plot_pressure(result: CircularHoleSteadyStokesResult, /) -> Figure: ...
def plot_scalar_field(
    trajectory: Trajectory,
    /,
    *,
    step: int,
    field: FieldRef,
) -> Figure: ...

__all__ = [
    "plot_deformed_field",
    "plot_displacement",
    "plot_pressure",
    "plot_scalar_field",
]
