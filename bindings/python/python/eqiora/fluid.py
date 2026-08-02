"""Narrow fluid applications composed by Eqiora's shared native layer."""

from ._eqiora import (
    CircularHoleSteadyStokesResult,
    SteadyStokes,
    SteadyStokesPlan,
    resolve_steady_stokes as resolve,
)

__all__ = [
    "CircularHoleSteadyStokesResult",
    "SteadyStokes",
    "SteadyStokesPlan",
    "resolve",
]
