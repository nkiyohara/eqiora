from matplotlib.figure import Figure

from .fluid import CircularHoleSteadyStokesResult
from .fsi import FixedReferenceFsiResult
from .solid import MixedBoundaryElasticityResult

def plot_displacement(
    result: MixedBoundaryElasticityResult,
    /,
    *,
    scale: float = 1.0,
) -> Figure: ...
def plot_pressure(result: CircularHoleSteadyStokesResult, /) -> Figure: ...
def plot_fixed_reference_fsi(
    result: FixedReferenceFsiResult,
    /,
    *,
    step: int = 2,
    displacement_scale: float = 12.0,
) -> Figure: ...

__all__ = ["plot_displacement", "plot_fixed_reference_fsi", "plot_pressure"]
