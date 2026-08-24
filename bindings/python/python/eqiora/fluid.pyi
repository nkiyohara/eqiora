"""Narrow fluid applications composed by Eqiora's shared native layer.

Authority: ``bindings/python/python/eqiora/fluid.py``.
"""

from typing import final

from . import LinearSolveSummary, Model, Result
from .meshing import Mesh

@final
class SteadyStokes:
    """Compatibility wrapper for the former application-shaped request.

    New code composes :class:`eqiora.IncompressibleFlowScales` and
    :class:`eqiora.LinearSolve` through :func:`eqiora.resolve`; the Model owns
    the governing mathematics.

    Authority: ``crates/eqiora-python/src/steady_stokes.rs::PySteadyStokes``.
    """

    def __new__(
        cls,
        *,
        length_scale_m: float,
        velocity_scale_m_per_s: float,
        pressure_scale_pa: float,
        relative_tolerance: float,
        absolute_tolerance: float,
        maximum_iterations: int,
    ) -> SteadyStokes: ...
    @property
    def length_scale_m(self) -> float: ...
    @property
    def velocity_scale_m_per_s(self) -> float: ...
    @property
    def pressure_scale_pa(self) -> float: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class SteadyStokesPlan:
    """Immutable steady-Stokes plan resolved before submission.

    Authority: ``crates/eqiora-python/src/steady_stokes.rs::PySteadyStokesPlan``.
    """

    @property
    def model_digest(self) -> str: ...
    @property
    def semantic_revision(self) -> int: ...
    @property
    def geometry_digest(self) -> str: ...
    @property
    def correspondence_digest(self) -> str: ...
    @property
    def mesh_digest(self) -> str: ...
    @property
    def realization_digest(self) -> str: ...
    @property
    def realization_revision(self) -> int: ...
    @property
    def spatial_dimension(self) -> int: ...
    @property
    def velocity_space(self) -> str: ...
    @property
    def pressure_space(self) -> str: ...
    @property
    def length_scale_m(self) -> float: ...
    @property
    def velocity_scale_m_per_s(self) -> float: ...
    @property
    def pressure_scale_pa(self) -> float: ...
    @property
    def solver_algorithm(self) -> str: ...
    @property
    def preconditioner(self) -> str: ...
    @property
    def reduction(self) -> str: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    @property
    def solver_backend(self) -> str: ...
    @property
    def execution_adapter(self) -> str: ...
    @property
    def workers(self) -> int: ...
    @property
    def canonical_bytes(self) -> bytes: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class SteadyStokesEvidence:
    """Scientific evidence selected from an accepted steady-Stokes result.

    Authority: ``crates/eqiora-python/src/steady_stokes.rs::PySteadyStokesEvidence``.
    """

    @property
    def run_digest(self) -> str: ...
    @property
    def pressure_minimum(self) -> float: ...
    @property
    def pressure_maximum(self) -> float: ...
    @property
    def exact_bounds(self) -> tuple[tuple[float, float], tuple[float, float]]: ...
    @property
    def cylinder_force_on_fluid(self) -> tuple[float, float]: ...
    @property
    def inlet_flux(self) -> float: ...
    @property
    def outlet_flux(self) -> float: ...
    @property
    def net_flux(self) -> float: ...
    @property
    def constrained_reaction(self) -> tuple[float, float]: ...
    @property
    def integrated_body_force(self) -> tuple[float, float]: ...
    @property
    def integrated_boundary_traction(self) -> tuple[float, float]: ...
    @property
    def momentum_closure(self) -> tuple[float, float]: ...
    @property
    def solve(self) -> LinearSolveSummary: ...
    @property
    def continuity_residual_norm(self) -> float: ...

def resolve(
    model: Model,
    intent: SteadyStokes,
    /,
    *,
    mesh: Mesh,
) -> SteadyStokesPlan:
    """Compatibility resolver for the former application-shaped request.

    New code calls :func:`eqiora.resolve` with explicit scale and solve
    policies.

    Authority: ``crates/eqiora-python/src/steady_stokes.rs::resolve``.
    """

    ...

def steady_stokes_evidence(result: Result, /) -> SteadyStokesEvidence:
    """Select typed steady-Stokes evidence from its accepted result.

    Authority: ``crates/eqiora-python/src/steady_stokes.rs::steady_stokes_evidence``.
    """

    ...

__all__ = [
    "SteadyStokes",
    "SteadyStokesEvidence",
    "SteadyStokesPlan",
    "resolve",
    "steady_stokes_evidence",
]
