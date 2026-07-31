"""Matplotlib presentation adapters for accepted Eqiora results."""

import math

try:
    from matplotlib.collections import LineCollection
    from matplotlib.figure import Figure
    from mpl_toolkits.axes_grid1 import make_axes_locatable
except ModuleNotFoundError as error:
    if error.name not in {"matplotlib", "matplotlib.figure"}:
        raise
    raise ImportError(
        "eqiora.matplotlib requires the optional 'matplotlib' dependency; "
        "install eqiora[matplotlib]"
    ) from error

from .fluid import CircularHoleSteadyStokesResult
from .solid import MixedBoundaryElasticityResult

__all__ = ["plot_displacement", "plot_pressure"]

_FIELD_RECT = (0.065, 0.23, 0.82, 0.58)


def plot_pressure(result: CircularHoleSteadyStokesResult, /) -> Figure:
    """Plot the accepted exact-cylinder P1 pressure without changing its meaning."""

    if not isinstance(result, CircularHoleSteadyStokesResult):
        raise TypeError(
            "plot_pressure() requires eqiora.fluid.CircularHoleSteadyStokesResult"
        )

    coordinates = result.coordinates
    triangles = result.triangles
    pressure = result.pressure.numpy(copy=False)

    figure = Figure(
        figsize=(10.0, 3.0),
        facecolor="#ffffff",
    )
    axes = figure.add_axes(_FIELD_RECT)
    axes.set_facecolor("#f8fafc")
    field = axes.tripcolor(
        coordinates[:, 0],
        coordinates[:, 1],
        pressure,
        triangles=triangles,
        shading="gouraud",
        cmap="viridis",
        vmin=result.pressure_minimum,
        vmax=result.pressure_maximum,
    )
    axes.triplot(
        coordinates[:, 0],
        coordinates[:, 1],
        triangles,
        color="#0f172a",
        linewidth=0.25,
        alpha=0.2,
    )
    (x_minimum, x_maximum), (y_minimum, y_maximum) = result.bounds
    axes.set_xlim(x_minimum, x_maximum)
    axes.set_ylim(y_minimum, y_maximum)
    axes.set_aspect("equal", adjustable="box")
    axes.set_xlabel("x [m]")
    axes.set_ylabel("y [m]")
    axes.set_title("Steady Stokes pressure")
    divider = make_axes_locatable(axes)
    colorbar_axes = divider.append_axes("right", size="2.5%", pad=0.18)
    colorbar = figure.colorbar(field, cax=colorbar_axes)
    colorbar.set_label("Pressure [Pa]")
    return figure


def plot_displacement(
    result: MixedBoundaryElasticityResult,
    /,
    *,
    scale: float = 1.0,
) -> Figure:
    """Compare the accepted Q1 mesh with its explicitly scaled displacement."""

    if not isinstance(result, MixedBoundaryElasticityResult):
        raise TypeError(
            "plot_displacement() requires "
            "eqiora.solid.MixedBoundaryElasticityResult"
        )
    scale = float(scale)
    if not math.isfinite(scale) or scale < 0.0:
        raise ValueError("plot_displacement() scale must be finite and nonnegative")

    coordinates = result.coordinates
    deformed = coordinates + scale * result.displacement
    edges = _quadrilateral_edges(result.cells)
    original_segments = coordinates[list(edges)]
    deformed_segments = deformed[list(edges)]

    figure = Figure(figsize=(7.0, 6.0), facecolor="#ffffff")
    axes = figure.add_axes((0.11, 0.12, 0.84, 0.79))
    axes.set_facecolor("#f8fafc")
    axes.add_collection(
        LineCollection(
            original_segments,
            colors="#64748b",
            linewidths=0.55,
            linestyles="dashed",
            alpha=0.7,
            label="Original mesh",
        )
    )
    axes.add_collection(
        LineCollection(
            deformed_segments,
            colors="#0f766e",
            linewidths=0.8,
            alpha=0.95,
            label=f"Displaced mesh (scale = {scale:g})",
        )
    )

    x_minimum = min(coordinates[:, 0].min(), deformed[:, 0].min())
    x_maximum = max(coordinates[:, 0].max(), deformed[:, 0].max())
    y_minimum = min(coordinates[:, 1].min(), deformed[:, 1].min())
    y_maximum = max(coordinates[:, 1].max(), deformed[:, 1].max())
    padding = 0.04 * max(x_maximum - x_minimum, y_maximum - y_minimum, 1.0)
    axes.set_xlim(x_minimum - padding, x_maximum + padding)
    axes.set_ylim(y_minimum - padding, y_maximum + padding)
    axes.set_aspect("equal", adjustable="box")
    axes.set_xlabel("x [m]")
    axes.set_ylabel("y [m]")
    axes.set_title(f"Mixed-boundary displacement — scale {scale:g}")
    axes.legend(loc="upper right")
    return figure


def _quadrilateral_edges(cells):
    edges = {
        tuple(sorted((int(cell[first]), int(cell[second]))))
        for cell in cells
        for first, second in ((0, 1), (1, 3), (3, 2), (2, 0))
    }
    return tuple(sorted(edges))
