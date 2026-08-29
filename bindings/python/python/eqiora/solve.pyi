"""Closed algebraic solve policies.

Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyLinear``.
"""
from typing import ClassVar, Final, Self, final

@final
class SolverPlanningObjective:
    """Preference consumed by the versioned host-serial solver planner.

    Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PySolverPlanningObjective``.
    """
    Robust: ClassVar[SolverPlanningObjective]
    Fast: ClassVar[SolverPlanningObjective]
    LowMemory: ClassVar[SolverPlanningObjective]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

#: Prefer the reproducible-reduction catalog member.
#:
#: Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PySolverPlanningObjective``.
Robust: Final[SolverPlanningObjective]
#: Prefer Fast reduction and then the direct catalog member.
#:
#: Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PySolverPlanningObjective``.
Fast: Final[SolverPlanningObjective]
#: Prefer the fixed-vector Krylov catalog member.
#:
#: Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PySolverPlanningObjective``.
LowMemory: Final[SolverPlanningObjective]

@final
class Linear:
    """Linear-solve controls resolved against Model-owned operator meaning.

    Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyLinear``.
    """
    def __new__(
        cls,
        *,
        relative_tolerance: float,
        absolute_tolerance: float,
        maximum_iterations: int,
        objective: SolverPlanningObjective | None = None,
    ) -> Self: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    @property
    def objective(self) -> SolverPlanningObjective | None: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class Newton:
    """Bounded Newton policy owning exact nested linear controls.

    Authority: ``crates/eqiora-python/src/common_plan/policy.rs::PyNewton``.
    """
    def __new__(
        cls,
        *,
        linear: Linear,
        relative_tolerance: float = 1e-9,
        absolute_tolerance: float = 1e-11,
        maximum_iterations: int = 16,
        maximum_line_search_steps: int = 12,
    ) -> Self: ...
    @property
    def linear(self) -> Linear: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    @property
    def maximum_line_search_steps(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class ResolvedLinear:
    """Exact linear algorithm, operator class, and provider selected by resolution.

    Authority: ``crates/eqiora-python/src/common_plan/resolved_solve.rs::PyResolvedLinear``.
    """
    @property
    def algorithm(self) -> str: ...
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
    def operator(self) -> str: ...
    @property
    def backend(self) -> str: ...
    @property
    def backend_version(self) -> str: ...
    @property
    def objective(self) -> SolverPlanningObjective | None: ...
    @property
    def planning_policy_id(self) -> str | None: ...
    @property
    def selected_candidate_id(self) -> str | None: ...
    @property
    def selected_evidence_case(self) -> str | None: ...
    @property
    def planning_reasons(self) -> list[tuple[str, str]]: ...
    def __repr__(self) -> str: ...

@final
class ResolvedNewton:
    """Exact Newton policy and nested resolved linear solver.

    Authority: ``crates/eqiora-python/src/common_plan/resolved_solve.rs::PyResolvedNewton``.
    """
    @property
    def linear(self) -> ResolvedLinear: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    @property
    def maximum_line_search_steps(self) -> int: ...
    def __repr__(self) -> str: ...

__all__ = [
    "SolverPlanningObjective",
    "Robust",
    "Fast",
    "LowMemory",
    "Linear",
    "Newton",
    "ResolvedLinear",
    "ResolvedNewton",
]
