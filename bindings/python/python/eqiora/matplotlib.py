"""Matplotlib presentation adapters for accepted Eqiora results."""

import math

try:
    from matplotlib.collections import LineCollection, PolyCollection
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
from .fsi import FixedReferenceFsiResult
from .solid import MixedBoundaryElasticityResult

__all__ = [
    "plot_displacement",
    "plot_fixed_reference_fsi",
    "plot_pressure",
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


def plot_fixed_reference_fsi(
    result: FixedReferenceFsiResult,
    /,
    *,
    step: int = 2,
    displacement_scale: float = 12.0,
) -> Figure:
    """Plot one solver-owned step from the accepted fixed-reference trajectory."""

    if not isinstance(result, FixedReferenceFsiResult):
        raise TypeError(
            "plot_fixed_reference_fsi() requires "
            "eqiora.fsi.FixedReferenceFsiResult"
        )
    if isinstance(step, bool) or not isinstance(step, int) or step not in (1, 2):
        raise ValueError("plot_fixed_reference_fsi() step must be 1 or 2")
    displacement_scale = float(displacement_scale)
    if not math.isfinite(displacement_scale) or displacement_scale < 0.0:
        raise ValueError(
            "plot_fixed_reference_fsi() displacement_scale must be finite "
            "and nonnegative"
        )

    accepted = result.step(step)
    coordinates = result.coordinates
    cells = result.cells
    fluid_cells = result.fluid_cells
    solid_cells = result.solid_cells
    fluid_triangles = cells[fluid_cells]
    pressure_by_vertex = {
        int(vertex): value
        for vertex, value in zip(
            accepted.pressure_vertices,
            accepted.pressure,
            strict=True,
        )
    }
    fluid_pressure = [
        sum(pressure_by_vertex[int(vertex)] for vertex in triangle) / 3.0
        for triangle in fluid_triangles
    ]
    deformed = coordinates + displacement_scale * accepted.displacement
    solid_edges = _triangle_edges(cells[solid_cells])

    figure = Figure(figsize=(10.0, 5.2), facecolor="#ffffff")
    axes = figure.add_axes((0.07, 0.13, 0.80, 0.76))
    axes.set_facecolor("#f8fafc")
    pressure = PolyCollection(
        coordinates[fluid_triangles],
        array=fluid_pressure,
        cmap="coolwarm",
        edgecolors="#334155",
        linewidths=0.7,
        alpha=0.92,
        label="Fluid pressure",
    )
    axes.add_collection(pressure)
    axes.add_collection(
        LineCollection(
            coordinates[list(solid_edges)],
            colors="#64748b",
            linewidths=0.7,
            linestyles="dashed",
            alpha=0.7,
            label="Solid reference mesh",
        )
    )
    axes.add_collection(
        LineCollection(
            deformed[list(solid_edges)],
            colors="#c2410c",
            linewidths=1.2,
            alpha=0.95,
            label=f"Solid displacement (scale = {displacement_scale:g})",
        )
    )
    axes.add_collection(
        LineCollection(
            coordinates[result.interface_facets],
            colors="#0891b2",
            linewidths=3.0,
            alpha=0.95,
            label="Conforming interface",
        )
    )
    axes.quiver(
        coordinates[:, 0],
        coordinates[:, 1],
        accepted.velocity[:, 0],
        accepted.velocity[:, 1],
        angles="xy",
        scale_units="xy",
        scale=None,
        width=0.004,
        color="#0369a1",
        label="Velocity [m/s] (auto-scaled arrows)",
    )

    x_minimum = min(coordinates[:, 0].min(), deformed[:, 0].min())
    x_maximum = max(coordinates[:, 0].max(), deformed[:, 0].max())
    y_minimum = min(coordinates[:, 1].min(), deformed[:, 1].min())
    y_maximum = max(coordinates[:, 1].max(), deformed[:, 1].max())
    padding = 0.06 * max(x_maximum - x_minimum, y_maximum - y_minimum, 1.0)
    axes.set_xlim(x_minimum - padding, x_maximum + padding)
    axes.set_ylim(y_minimum - padding, y_maximum + padding)
    axes.set_aspect("equal", adjustable="box")
    axes.set_xlabel("x [m]")
    axes.set_ylabel("y [m]")
    axes.set_title(
        f"Fixed-reference FSI — step {accepted.ordinal}, "
        f"t = {accepted.time_s:g} s, displacement scale {displacement_scale:g}"
    )
    axes.legend(loc="upper center", ncols=2)
    divider = make_axes_locatable(axes)
    colorbar_axes = divider.append_axes("right", size="2.5%", pad=0.18)
    colorbar = figure.colorbar(pressure, cax=colorbar_axes)
    colorbar.set_label("Fluid pressure [Pa]")
    return figure


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
