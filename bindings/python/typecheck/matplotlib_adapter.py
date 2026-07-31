from typing import assert_type

from matplotlib.figure import Figure

import eqiora.matplotlib as eqplot
from eqiora.fluid import CircularHoleSteadyStokesResult


def check_matplotlib_adapter(result: CircularHoleSteadyStokesResult) -> None:
    figure = eqplot.plot_pressure(result)
    assert_type(figure, Figure)
