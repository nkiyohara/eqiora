from typing import assert_type

from matplotlib.figure import Figure

import eqiora.matplotlib as eqplot
from eqiora.fluid import CircularHoleSteadyStokesResult
from eqiora.solid import MixedBoundaryElasticityResult


def check_matplotlib_adapter(result: CircularHoleSteadyStokesResult) -> None:
    figure = eqplot.plot_pressure(result)
    assert_type(figure, Figure)


def check_structural_adapter(result: MixedBoundaryElasticityResult) -> None:
    figure = eqplot.plot_displacement(result, scale=1.0)
    assert_type(figure, Figure)
