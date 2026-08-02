from typing import assert_type

from matplotlib.figure import Figure

import eqiora.matplotlib as eqplot
from eqiora import FieldRef
from eqiora.fluid import CircularHoleSteadyStokesResult
from eqiora.solid import MixedBoundaryElasticityResult
from eqiora.trajectory import Trajectory


def check_matplotlib_adapter(result: CircularHoleSteadyStokesResult) -> None:
    figure = eqplot.plot_pressure(result)
    assert_type(figure, Figure)


def check_structural_adapter(result: MixedBoundaryElasticityResult) -> None:
    figure = eqplot.plot_displacement(result, scale=1.0)
    assert_type(figure, Figure)


def check_trajectory_adapters(
    trajectory: Trajectory,
    scalar: FieldRef,
    displacement: FieldRef,
) -> None:
    scalar_figure = eqplot.plot_scalar_field(
        trajectory,
        step=2,
        field=scalar,
    )
    deformed_figure = eqplot.plot_deformed_field(
        trajectory,
        step=2,
        field=displacement,
        scale=12.0,
    )
    assert_type(scalar_figure, Figure)
    assert_type(deformed_figure, Figure)
