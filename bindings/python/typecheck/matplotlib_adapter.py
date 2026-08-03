from typing import assert_type

from matplotlib.figure import Figure

import eqiora.matplotlib as eqplot
from eqiora import FieldRef, Result
from eqiora.trajectory import Trajectory


def check_matplotlib_adapter(result: Result, field: FieldRef) -> None:
    figure = eqplot.plot_scalar_field(result, field=field)
    assert_type(figure, Figure)


def check_structural_adapter(result: Result, displacement: FieldRef) -> None:
    figure = eqplot.plot_deformed_field(result, field=displacement, scale=1.0)
    assert_type(figure, Figure)
    assert_type(eqplot.plot_deformed_field(result, field=displacement), Figure)
    eqplot.plot_deformed_field(  # type: ignore[call-overload]
        result,
        step=2,
        field=displacement,
    )


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
