from matplotlib.figure import Figure

from .fluid import CircularHoleSteadyStokesResult

def plot_pressure(result: CircularHoleSteadyStokesResult, /) -> Figure: ...

__all__ = ["plot_pressure"]
