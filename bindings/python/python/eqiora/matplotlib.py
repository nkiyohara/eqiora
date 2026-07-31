"""Matplotlib presentation adapters for accepted Eqiora results."""

try:
    from matplotlib.figure import Figure
except ModuleNotFoundError as error:
    if error.name not in {"matplotlib", "matplotlib.figure"}:
        raise
    raise ImportError(
        "eqiora.matplotlib requires the optional 'matplotlib' dependency; "
        "install eqiora[matplotlib]"
    ) from error

from .fluid import CircularHoleSteadyStokesResult

__all__ = ["plot_pressure"]

_FIELD_RECT = (0.065, 0.23, 0.82, 0.58)
_COLORBAR_RECT = (0.91, 0.14, 0.025, 0.75)


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
    colorbar_axes = figure.add_axes(_COLORBAR_RECT)
    colorbar = figure.colorbar(field, cax=colorbar_axes)
    colorbar.set_label("Pressure [Pa]")
    return figure
