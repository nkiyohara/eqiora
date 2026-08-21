"""Matplotlib presentation adapters for accepted Eqiora results.

Authority: ``bindings/python/python/eqiora/matplotlib.py``.
"""

from typing import overload

from matplotlib.figure import Figure

from . import FieldRef, Result
from .trajectory import Trajectory

@overload
def plot_deformed_field(
    trajectory: Trajectory,
    /,
    *,
    step: int,
    field: FieldRef,
    scale: float = 1.0,
) -> Figure:
    """Compare exact reference and scaled-deformed support geometry.

    Authority: ``bindings/python/python/eqiora/matplotlib.py::plot_deformed_field``.
    """

    ...

@overload
def plot_deformed_field(
    result: Result,
    /,
    *,
    field: FieldRef,
    scale: float = 1.0,
) -> Figure:
    """Compare exact reference and scaled-deformed support geometry.

    Authority: ``bindings/python/python/eqiora/matplotlib.py::plot_deformed_field``.
    """

    ...

def plot_displacement(
    result: Result,
    /,
    *,
    scale: float = 1.0,
) -> Figure:
    """Deprecated delegation to :func:`plot_deformed_field`.

    Authority: ``bindings/python/python/eqiora/matplotlib.py::plot_displacement``.
    """

    ...

@overload
def plot_scalar_field(
    trajectory: Trajectory,
    /,
    *,
    step: int,
    field: FieldRef,
) -> Figure:
    """Plot one invariant vertex scalar from a result or trajectory.

    Authority: ``bindings/python/python/eqiora/matplotlib.py::plot_scalar_field``.
    """

    ...

@overload
def plot_scalar_field(
    result: Result,
    /,
    *,
    field: FieldRef,
) -> Figure:
    """Plot one invariant vertex scalar from a result or trajectory.

    Authority: ``bindings/python/python/eqiora/matplotlib.py::plot_scalar_field``.
    """

    ...

__all__ = [
    "plot_deformed_field",
    "plot_displacement",
    "plot_scalar_field",
]
