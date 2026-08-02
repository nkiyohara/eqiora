"""Matplotlib presentation adapters for accepted Eqiora results."""

import math

import numpy as np

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

from ._eqiora import FieldRef, Trajectory
from .fluid import CircularHoleSteadyStokesResult
from .solid import MixedBoundaryElasticityResult

__all__ = [
    "plot_deformed_field",
    "plot_displacement",
    "plot_pressure",
    "plot_scalar_field",
]

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
            "plot_displacement() requires eqiora.solid.MixedBoundaryElasticityResult"
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


def plot_scalar_field(
    trajectory: Trajectory,
    /,
    *,
    step: int,
    field: FieldRef,
) -> Figure:
    """Plot one exact invariant vertex scalar from an accepted trajectory state."""

    state, snapshot = _trajectory_snapshot(trajectory, step, field)
    if snapshot.value_shape != ():
        raise ValueError("plot_scalar_field() requires scalar value shape ()")
    if snapshot.frame != "invariant":
        raise ValueError("plot_scalar_field() requires the invariant frame")
    coordinates, triangles, values, support = _vertex_field_arrays(
        trajectory,
        snapshot,
    )
    restricted = values[support]

    figure = Figure(figsize=(8.0, 5.2), facecolor="#ffffff")
    axes = figure.add_axes((0.09, 0.13, 0.76, 0.76))
    axes.set_facecolor("#f8fafc")
    scalar = axes.tripcolor(
        coordinates[:, 0],
        coordinates[:, 1],
        values,
        triangles=triangles,
        shading="gouraud",
        cmap="viridis",
        vmin=float(restricted.min()),
        vmax=float(restricted.max()),
    )
    axes.triplot(
        coordinates[:, 0],
        coordinates[:, 1],
        triangles,
        color="#0f172a",
        linewidth=0.35,
        alpha=0.3,
    )
    _set_field_axes_bounds(axes, coordinates[support])
    axes.set_title(f"Scalar field — step {state.step}, t = {state.time_s:g} s")
    divider = make_axes_locatable(axes)
    colorbar_axes = divider.append_axes("right", size="3%", pad=0.16)
    colorbar = figure.colorbar(scalar, cax=colorbar_axes)
    colorbar.set_label(f"Value [{_coherent_si_unit(snapshot.dimension)}]")
    return figure


def plot_deformed_field(
    trajectory: Trajectory,
    /,
    *,
    step: int,
    field: FieldRef,
    scale: float = 1.0,
) -> Figure:
    """Compare exact reference and scaled-deformed support geometry."""

    if not isinstance(trajectory, Trajectory):
        raise TypeError("plot_deformed_field() requires eqiora.trajectory.Trajectory")
    scale = float(scale)
    if not math.isfinite(scale) or scale < 0.0:
        raise ValueError("plot_deformed_field() scale must be finite and nonnegative")

    state, snapshot = _trajectory_snapshot(trajectory, step, field)
    if snapshot.value_shape != (trajectory.dimension,):
        raise ValueError(
            "plot_deformed_field() value shape must match the trajectory dimension"
        )
    if snapshot.frame != "spatial-cartesian":
        raise ValueError("plot_deformed_field() requires the spatial-cartesian frame")
    if snapshot.dimension != (0, 1, 0, 0, 0, 0, 0):
        raise ValueError("plot_deformed_field() requires the SI length dimension")

    coordinates, triangles, values, support = _vertex_field_arrays(
        trajectory,
        snapshot,
    )
    edges = _triangle_edges(triangles)
    edge_indices = list(edges)
    deformed = coordinates + scale * values

    figure = Figure(figsize=(7.0, 6.0), facecolor="#ffffff")
    axes = figure.add_axes((0.11, 0.12, 0.84, 0.79))
    axes.set_facecolor("#f8fafc")
    axes.add_collection(
        LineCollection(
            coordinates[edge_indices],
            colors="#64748b",
            linewidths=0.7,
            linestyles="dashed",
            alpha=0.72,
            label="Reference mesh",
        )
    )
    axes.add_collection(
        LineCollection(
            deformed[edge_indices],
            colors="#c2410c",
            linewidths=1.15,
            alpha=0.95,
            label=f"Deformed mesh (scale = {scale:g})",
        )
    )

    _set_field_axes_bounds(
        axes,
        np.concatenate((coordinates[support], deformed[support])),
    )
    axes.set_title(
        f"Deformed field — step {state.step}, t = {state.time_s:g} s, scale {scale:g}"
    )
    axes.legend(loc="upper right")
    return figure


def _trajectory_snapshot(trajectory, step, field):
    if not isinstance(trajectory, Trajectory):
        raise TypeError("field stills require eqiora.trajectory.Trajectory")
    state = trajectory.state(step)
    return state, state.field(field)


def _vertex_field_arrays(trajectory, snapshot):
    if snapshot.associations != ("vertex",):
        raise ValueError("field stills require exactly one vertex association")

    coordinates = trajectory.coordinates
    cells = trajectory.cells
    values = snapshot.values("vertex")
    support = snapshot.support_indices("vertex")
    if trajectory.dimension != 2 or coordinates.ndim != 2 or coordinates.shape[1] != 2:
        raise ValueError("field stills require two-dimensional coordinates")
    if cells.ndim != 2 or cells.shape[1] != 3:
        raise ValueError("field stills require affine triangle topology")
    if values.shape[0] != coordinates.shape[0]:
        raise ValueError("field value shape does not match the trajectory vertices")
    if support.ndim != 1 or support.size == 0:
        raise ValueError("vertex support must be one-dimensional and nonempty")
    if int(support.max()) >= coordinates.shape[0]:
        raise ValueError("vertex support exceeds the trajectory coordinates")
    if not np.array_equal(support, np.unique(support)):
        raise ValueError("vertex support must be sorted and unique")
    if cells.size > 0 and int(cells.max()) >= coordinates.shape[0]:
        raise ValueError("triangle topology exceeds the trajectory coordinates")

    inside = np.isin(cells, support)
    triangles = cells[np.all(inside, axis=1)]
    if triangles.size == 0:
        raise ValueError("vertex support admits no complete triangle")
    if not np.array_equal(np.unique(triangles), support):
        raise ValueError("admitted triangle closure differs from vertex support")
    return coordinates, triangles, values, support


def _set_field_axes_bounds(axes, points):
    x_minimum = float(points[:, 0].min())
    x_maximum = float(points[:, 0].max())
    y_minimum = float(points[:, 1].min())
    y_maximum = float(points[:, 1].max())
    padding = 0.05 * max(x_maximum - x_minimum, y_maximum - y_minimum, 1.0)
    axes.set_xlim(x_minimum - padding, x_maximum + padding)
    axes.set_ylim(y_minimum - padding, y_maximum + padding)
    axes.set_aspect("equal", adjustable="box")
    axes.set_xlabel("x [m]")
    axes.set_ylabel("y [m]")


def _coherent_si_unit(dimension):
    terms = [
        base if exponent == 1 else f"{base}^{exponent}"
        for base, exponent in zip(
            ("kg", "m", "s", "A", "K", "mol", "cd"),
            dimension,
            strict=True,
        )
        if exponent != 0
    ]
    return "·".join(terms) if terms else "1"


def _quadrilateral_edges(cells):
    edges = {
        tuple(sorted((int(cell[first]), int(cell[second]))))
        for cell in cells
        for first, second in ((0, 1), (1, 3), (3, 2), (2, 0))
    }
    return tuple(sorted(edges))


def _triangle_edges(cells):
    edges = {
        tuple(sorted((int(cell[first]), int(cell[second]))))
        for cell in cells
        for first, second in ((0, 1), (1, 2), (2, 0))
    }
    return tuple(sorted(edges))
