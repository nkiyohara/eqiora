"""Narrow fluid applications composed by Eqiora's shared native layer."""

from ._eqiora import (
    CircularHoleSteadyStokesResult,
    solve_exact_cylinder_stokes,
)

__all__ = [
    "CircularHoleSteadyStokesResult",
    "solve_exact_cylinder_stokes",
]
