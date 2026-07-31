from matplotlib.figure import Figure

from .fluid import CircularHoleSteadyStokesResult
from .solid import MixedBoundaryElasticityResult

def plot_displacement(
    result: MixedBoundaryElasticityResult,
    /,
    *,
    scale: float = 1.0,
) -> Figure: ...
def plot_pressure(result: CircularHoleSteadyStokesResult, /) -> Figure: ...

__all__ = ["plot_displacement", "plot_pressure"]
