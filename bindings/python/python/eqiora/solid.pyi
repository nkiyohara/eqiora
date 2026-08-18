"""Typed structural intents, plans, and scientific evidence.

Authority: ``bindings/python/python/eqiora/solid.py``.
"""

from typing import final

from . import LinearSolveSummary, Model, Result

@final
class LinearElasticity:
    """Complete linear-elasticity request without hidden numerical defaults.

    Authority: ``crates/eqiora-python/src/elasticity.rs::PyLinearElasticity``.
    """

    def __new__(
        cls,
        *,
        cells_per_axis: int,
        relative_tolerance: float,
        absolute_tolerance: float,
        maximum_iterations: int,
    ) -> LinearElasticity: ...
    @property
    def cells_per_axis(self) -> int: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class LinearElasticityPlan:
    """Immutable linear-elasticity plan resolved before submission.

    Authority: ``crates/eqiora-python/src/elasticity.rs::PyLinearElasticityPlan``.
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
    def cells_per_axis(self) -> int: ...
    @property
    def discretization_method(self) -> str: ...
    @property
    def mesh_kind(self) -> str: ...
    @property
    def mesh_policy(self) -> str: ...
    @property
    def field_space(self) -> str: ...
    @property
    def quadrature(self) -> str: ...
    @property
    def quadrature_points_per_axis(self) -> int: ...
    @property
    def scalar_type(self) -> str: ...
    @property
    def vector_layout(self) -> str: ...
    @property
    def coefficient_association(self) -> str: ...
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
class LinearElasticityEvidence:
    """Scientific evidence selected from an accepted structural result.

    Authority: ``crates/eqiora-python/src/elasticity.rs::PyLinearElasticityEvidence``.
    """

    @property
    def run_digest(self) -> str: ...
    @property
    def constrained_reaction(self) -> tuple[float, float]: ...
    @property
    def integrated_body_force(self) -> tuple[float, float]: ...
    @property
    def assembly_packets(self) -> int: ...
    @property
    def assembly_targets(self) -> int: ...
    @property
    def solve(self) -> LinearSolveSummary: ...
    @property
    def exact_bounds(self) -> tuple[tuple[float, float], tuple[float, float]]: ...

def resolve(model: Model, intent: LinearElasticity, /) -> LinearElasticityPlan:
    """Resolve a structural intent without executing it.

    Authority: ``crates/eqiora-python/src/elasticity.rs::resolve``.
    """

    ...

def linear_elasticity_evidence(result: Result, /) -> LinearElasticityEvidence:
    """Select typed linear-elasticity evidence from its result.

    Authority: ``crates/eqiora-python/src/elasticity.rs::linear_elasticity_evidence``.
    """

    ...

#: Deprecated one-prerelease alias for the common :class:`Result` type.
#:
#: Authority: ``bindings/python/python/eqiora/solid.py::__getattr__``.
MixedBoundaryElasticityResult = Result

def solve_mixed_boundary_elasticity(model: Model, /) -> Result:
    """Delegate the legacy solve to the explicit plan and result path.

    Authority: ``bindings/python/python/eqiora/solid.py::solve_mixed_boundary_elasticity``.
    """

    ...

__all__ = [
    "LinearElasticity",
    "LinearElasticityEvidence",
    "LinearElasticityPlan",
    "MixedBoundaryElasticityResult",
    "linear_elasticity_evidence",
    "resolve",
    "solve_mixed_boundary_elasticity",
]
