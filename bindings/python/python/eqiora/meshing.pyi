"""Immutable geometry-to-mesh planning owned by the native implementation.

Authority: ``bindings/python/python/eqiora/meshing.py``.
"""

from collections.abc import Collection
from typing import Self, final

import numpy as np
import numpy.typing as npt

from .geometry import Geometry, GeometrySelection

@final
class CartesianMesher:
    """Select deterministic structured Cartesian meshing.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::PyCartesianMesher``.
    """
    def __new__(cls, *, cells: tuple[int, int]) -> Self: ...
    @property
    def cells(self) -> tuple[int, int]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class GmshMesher:
    """Select the exact external Gmsh provider.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::PyGmshMesher``.
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
class ReferenceMesher:
    """Select the deterministic in-process reference provider.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::PyReferenceMesher``.
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
class GmshImport:
    """Policy for one caller-supplied, untracked Gmsh MSH image.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::PyGmshImport``.
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
class MeshRequest:
    """Immutable caller intent for one admitted mesh provider.

    Authority: ``crates/eqiora-python/src/meshing/plan.rs::PyMeshRequest``.
    """

    def __new__(cls, provider: CartesianMesher | GmshMesher | ReferenceMesher, /) -> Self: ...
    @property
    def provider(self) -> CartesianMesher | GmshMesher | ReferenceMesher: ...
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
    def provider(self) -> CartesianMesher | GmshMesher | ReferenceMesher: ...
    @property
    def request(self) -> MeshRequest: ...
    @property
    def production_lineage_bytes(self) -> bytes: ...
    @property
    def production_lineage_digest(self) -> str: ...
    @property
    def boundary_facets(self) -> int: ...
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
    def production_lineage_bytes(self) -> bytes: ...
    @property
    def production_lineage_digest(self) -> str: ...
    @property
    def realization_digest(self) -> str: ...
    @property
    def external_import_manifest_bytes(self) -> bytes | None: ...
    @property
    def external_import_manifest_digest(self) -> str | None: ...
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
    def selection_entity_count(self, name: str | GeometrySelection) -> int: ...
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

def import_gmsh(
    geometry: Geometry,
    source: bytes,
    /,
    *,
    policy: GmshImport,
) -> Mesh:
    """Import one complete Gmsh MSH 4.1 image into the common Mesh.

    The current boundary accepts affine two-dimensional triangles for the
    supplied exact circular-hole Geometry. ``policy`` explicitly owns the
    separately typed boundary-realization and quality policy. External source, adapter,
    normalized-array, and accepted-Mesh identities are retained by
    ``Mesh.external_import_manifest_bytes``.

    Authority: ``crates/eqiora-python/src/meshing/mesh.rs::import_gmsh``.
    """

    ...

__all__ = ["CartesianMesher", "GmshImport", "GmshMesher", "Mesh", "MeshPlan", "MeshRequest", "ReferenceMesher", "generate", "import_gmsh", "resolve"]
