"""Immutable geometry-to-mesh planning owned by the native implementation.

Authority: ``bindings/python/python/eqiora/meshing.py``.
"""

from collections.abc import Collection
from typing import Self, final

import numpy as np
import numpy.typing as npt

from .geometry import Geometry

@final
class MeshRequest:
    """Immutable caller intent for the admitted planar mesh provider.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::PyMeshRequest``.
    """

    def __new__(
        cls,
        *,
        maximum_boundary_error: float = ...,
        minimum_mean_ratio: float = ...,
        maximum_boundary_facets: int = ...,
    ) -> Self: ...
    @property
    def maximum_boundary_error(self) -> float: ...
    @property
    def minimum_mean_ratio(self) -> float: ...
    @property
    def maximum_boundary_facets(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class MeshPlan:
    """Complete provider choice bound to one exact geometry.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::PyMeshPlan``.
    """

    @property
    def source_digest(self) -> str: ...
    @property
    def provider(self) -> str: ...
    @property
    def request(self) -> MeshRequest: ...
    @property
    def boundary_facets(self) -> int: ...
    @property
    def boundary_error_bound(self) -> float: ...
    @property
    def boundary_evaluation_allowance(self) -> float: ...
    @property
    def canonical_bytes(self) -> bytes: ...
    @property
    def achieved_minimum_mean_ratio(self) -> float: ...
    def __repr__(self) -> str: ...

@final
class Mesh:
    """Immutable source-bound accepted mesh.

    Authority: ``crates/eqiora-python/src/meshing/mesh.rs::PyMesh``.
    """

    @property
    def source_digest(self) -> str: ...
    @property
    def realized_geometry_digest(self) -> str: ...
    @property
    def digest(self) -> str: ...
    @property
    def correspondence_digest(self) -> str: ...
    @property
    def realization_digest(self) -> str: ...
    @property
    def canonical_bytes(self) -> bytes: ...
    @property
    def dimension(self) -> int: ...
    @property
    def vertex_count(self) -> int: ...
    @property
    def cell_count(self) -> int: ...
    @property
    def coordinates(self) -> npt.NDArray[np.float64]: ...
    @property
    def cells(self) -> npt.NDArray[np.uint32]: ...
    @property
    def minimum_mean_ratio(self) -> float: ...
    @property
    def selection_names(self) -> tuple[str, ...]: ...
    def selection_entity_count(self, name: str) -> int: ...
    def _repr_mimebundle_(
        self,
        include: Collection[str] | None = None,
        exclude: Collection[str] | None = None,
    ) -> dict[str, object]: ...
    def __repr__(self) -> str: ...

def resolve(geometry: Geometry, request: MeshRequest, /) -> MeshPlan:
    """Resolve a provider plan for the exact supplied geometry.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::resolve``.
    """

    ...

def generate(geometry: Geometry, /, *, plan: MeshPlan) -> Mesh:
    """Publish the accepted mesh owned by a resolved plan.

    Authority: ``crates/eqiora-python/src/meshing/mesh.rs::generate``.
    """

    ...

__all__ = ["Mesh", "MeshPlan", "MeshRequest", "generate", "resolve"]
