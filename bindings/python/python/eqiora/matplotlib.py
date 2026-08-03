"""Matplotlib presentation adapters for accepted Eqiora results."""

import math
import warnings

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

from ._eqiora import FieldRef, Result, Trajectory
from .fluid import steady_stokes_evidence
from .solid import linear_elasticity_evidence

__all__ = [
    "plot_deformed_field",
    "plot_displacement",
    "plot_scalar_field",
]

_MISSING = object()


def plot_displacement(
    result: Result,
    /,
    *,
    scale: float = 1.0,
) -> Figure:
    """Deprecated delegation to :func:`plot_deformed_field`."""

    if not isinstance(result, Result):
        raise TypeError("plot_displacement() requires eqiora.Result")
    warnings.warn(
        "plot_displacement() is deprecated; use plot_deformed_field() instead",
        DeprecationWarning,
        stacklevel=2,
    )
    snapshots = result.snapshots
    if len(snapshots) != 1:
        raise ValueError("plot_displacement() requires one static Field snapshot")
    return plot_deformed_field(
        result,
        field=snapshots[0].field,
        scale=scale,
    )


def plot_scalar_field(
    trajectory: Result | Trajectory,
    /,
    *,
    step=_MISSING,
    field: FieldRef,
) -> Figure:
    """Plot one exact invariant vertex scalar from an accepted result or trajectory."""

    if isinstance(trajectory, Result):
        if step is not _MISSING:
            raise TypeError("plot_scalar_field() does not accept step for Result")
        evidence = steady_stokes_evidence(trajectory)
        bounds = evidence.exact_bounds
        minimum = evidence.pressure_minimum
        maximum = evidence.pressure_maximum
        snapshot = trajectory.field(field)
        spatial = trajectory.mesh(field)
        state = None
        scalar_label = _result_scalar_label(snapshot.dimension)
    elif isinstance(trajectory, Trajectory):
        if step is _MISSING:
            raise TypeError("plot_scalar_field() requires step for Trajectory")
        state, snapshot = _trajectory_snapshot(trajectory, step, field)
        spatial = trajectory
        bounds = None
        scalar_label = f"Value [{_coherent_si_unit(snapshot.dimension)}]"
    else:
        raise TypeError(
            "plot_scalar_field() requires eqiora.Result or eqiora.trajectory.Trajectory"
        )

    if snapshot.value_shape != ():
        raise ValueError("plot_scalar_field() requires scalar value shape ()")
    if snapshot.frame != "invariant":
        raise ValueError("plot_scalar_field() requires the invariant frame")
    coordinates, triangles, values, support = _vertex_field_arrays(
        spatial,
        snapshot,
        cell_arity=3,
    )
    restricted = values[support]
    if state is not None:
        minimum = float(restricted.min())
        maximum = float(restricted.max())

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
        vmin=minimum,
        vmax=maximum,
    )
    axes.triplot(
        coordinates[:, 0],
        coordinates[:, 1],
        triangles,
        color="#0f172a",
        linewidth=0.35,
        alpha=0.3,
    )
    if state is not None:
        _set_field_axes_bounds(axes, coordinates[support])
        axes.set_title(f"Scalar field — step {state.step}, t = {state.time_s:g} s")
    else:
        (x_minimum, x_maximum), (y_minimum, y_maximum) = bounds
        axes.set_xlim(x_minimum, x_maximum)
        axes.set_ylim(y_minimum, y_maximum)
        _finish_field_axes(axes)
        axes.set_title("Scalar field")
    divider = make_axes_locatable(axes)
    colorbar_axes = divider.append_axes("right", size="3%", pad=0.16)
    colorbar = figure.colorbar(scalar, cax=colorbar_axes)
    colorbar.set_label(scalar_label)
    return figure


def plot_deformed_field(
    trajectory: Result | Trajectory,
    /,
    *,
    step=_MISSING,
    field: FieldRef,
    scale: float = 1.0,
) -> Figure:
    """Compare exact reference and scaled-deformed support geometry."""

    scale = float(scale)
    if not math.isfinite(scale) or scale < 0.0:
        raise ValueError("plot_deformed_field() scale must be finite and nonnegative")

    if isinstance(trajectory, Result):
        if step is not _MISSING:
            raise TypeError("plot_deformed_field() does not accept step for Result")
        # The current static vector arm is the accepted structural Result. This
        # typed selection rejects scalar and temporal Results before rendering.
        linear_elasticity_evidence(trajectory)
        snapshot = trajectory.field(field)
        spatial = trajectory.mesh(field)
        state = None
        cell_arity = 4
    elif isinstance(trajectory, Trajectory):
        if step is _MISSING:
            raise TypeError("plot_deformed_field() requires step for Trajectory")
        state, snapshot = _trajectory_snapshot(trajectory, step, field)
        spatial = trajectory
        cell_arity = 3
    else:
        raise TypeError(
            "plot_deformed_field() requires eqiora.Result or eqiora.trajectory.Trajectory"
        )

    if snapshot.value_shape != (spatial.dimension,):
        raise ValueError(
            "plot_deformed_field() value shape must match the spatial dimension"
        )
    if snapshot.frame != "spatial-cartesian":
        raise ValueError("plot_deformed_field() requires the spatial-cartesian frame")
    if snapshot.dimension != (0, 1, 0, 0, 0, 0, 0):
        raise ValueError("plot_deformed_field() requires the SI length dimension")

    coordinates, cells, values, support = _vertex_field_arrays(
        spatial,
        snapshot,
        cell_arity=cell_arity,
    )
    edges = _cell_edges(cells)
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
    if state is None:
        axes.set_title(f"Deformed field — scale {scale:g}")
    else:
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


def _vertex_field_arrays(spatial, snapshot, *, cell_arity):
    if snapshot.associations != ("vertex",):
        raise ValueError("field stills require exactly one vertex association")

    coordinates = spatial.coordinates
    cells = spatial.cells
    values = snapshot.values("vertex")
    support = snapshot.support_indices("vertex")
    if spatial.dimension != 2 or coordinates.ndim != 2 or coordinates.shape[1] != 2:
        raise ValueError("field stills require two-dimensional coordinates")
    if cells.ndim != 2 or cells.shape[1] != cell_arity:
        if cell_arity == 3:
            raise ValueError("field stills require affine triangle topology")
        if cell_arity == 4:
            raise ValueError("field stills require quadrilateral topology")
        raise ValueError("field stills received an unsupported topology contract")
    if values.shape[0] != coordinates.shape[0]:
        raise ValueError("field value shape does not match the spatial vertices")
    if support.ndim != 1 or support.size == 0:
        raise ValueError("vertex support must be one-dimensional and nonempty")
    if int(support.max()) >= coordinates.shape[0]:
        raise ValueError("vertex support exceeds the spatial coordinates")
    if not np.array_equal(support, np.unique(support)):
        raise ValueError("vertex support must be sorted and unique")
    if cells.size > 0 and int(cells.max()) >= coordinates.shape[0]:
        raise ValueError("cell topology exceeds the spatial coordinates")

    inside = np.isin(cells, support)
    admitted_cells = cells[np.all(inside, axis=1)]
    if admitted_cells.size == 0:
        raise ValueError("vertex support admits no complete cell")
    if not np.array_equal(np.unique(admitted_cells), support):
        raise ValueError("admitted cell closure differs from vertex support")
    return coordinates, admitted_cells, values, support


def _set_field_axes_bounds(axes, points):
    x_minimum = float(points[:, 0].min())
    x_maximum = float(points[:, 0].max())
    y_minimum = float(points[:, 1].min())
    y_maximum = float(points[:, 1].max())
    padding = 0.05 * max(x_maximum - x_minimum, y_maximum - y_minimum, 1.0)
    axes.set_xlim(x_minimum - padding, x_maximum + padding)
    axes.set_ylim(y_minimum - padding, y_maximum + padding)
    _finish_field_axes(axes)


def _finish_field_axes(axes):
    axes.set_aspect("equal", adjustable="box")
    axes.set_xlabel("x [m]")
    axes.set_ylabel("y [m]")


def _result_scalar_label(dimension):
    if dimension == (1, -1, -2, 0, 0, 0, 0):
        return "Pressure [Pa]"
    return f"Value [{_coherent_si_unit(dimension)}]"


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


def _cell_edges(cells):
    if cells.shape[1] == 3:
        return _triangle_edges(cells)
    if cells.shape[1] == 4:
        return _quadrilateral_edges(cells)
    raise ValueError("field stills require triangle or quadrilateral topology")
