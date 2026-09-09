"""Closed algebraic solve policies with executable Eqiora consumers."""

from ._eqiora import (
    Linear,
    AlgebraicPlanView,
    LinearSolver,
    Preconditioner,
    Reduction,
    SolverProvider,
    Newton,
    ResolvedLinear,
    ResolvedNewton,
    SolverPlanningObjective,
)

Robust = SolverPlanningObjective.Robust
Fast = SolverPlanningObjective.Fast
LowMemory = SolverPlanningObjective.LowMemory

__all__ = [
    "SolverPlanningObjective",
    "Robust",
    "Fast",
    "LowMemory",
    "Linear",
    "AlgebraicPlanView",
    "LinearSolver",
    "Preconditioner",
    "Reduction",
    "SolverProvider",
    "Newton",
    "ResolvedLinear",
    "ResolvedNewton",
]
