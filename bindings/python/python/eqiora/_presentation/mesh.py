"""Exact-reference Mesh transport for the private anywidget presentation."""

from __future__ import annotations

import hashlib
import importlib.util
import uuid
from importlib import resources
from typing import Any, Final

import numpy as np

from eqiora.meshing import Mesh

_PROFILE: Final = "circular-hole-gmsh-4.15.2/v1"
_SOURCE_DIGEST: Final = (
    "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
)
_RAW_CANONICAL_DIGEST: Final = (
    "9d3c6211e6832aa5a5f7e99fa210058ff1b76eab7f1e99aaa7033c282d6e2dd2"
)
_MESH_DIGEST: Final = (
    "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"
)
_MESH_DIGEST_DOMAIN: Final = b"eqiora.simplicial-mesh-envelope/v1\0"
_CANONICAL_BYTES: Final = 42_388
_VERTEX_COUNT: Final = 662
_TRIANGLE_COUNT: Final = 1_210
_COORDINATE_BYTES: Final = 10_592
_TRIANGLE_BYTES: Final = 14_520
_COORDINATE_SHA256: Final = (
    "42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d"
)
_TRIANGLE_SHA256: Final = (
    "05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642"
)
_SELECTION_MEMBERSHIP_DOMAIN: Final = b"eqiora.mesh-selection-membership/v1\0"
_SELECTION_NAMES: Final = ("cylinder", "inlet", "outlet", "walls", "fluid")
_SELECTION_DIMENSIONS: Final = (1, 1, 1, 1, 2)
_WIDGET_MIME: Final = "application/vnd.jupyter.widget-view+json"
_PAYLOAD_FIELDS: Final = (
    "profile",
    "mesh_digest",
    "vertex_count",
    "triangle_count",
    "coordinates_f64_le",
    "triangles_u32_le",
    "correspondence_digest",
    "selection_membership",
    "selection_membership_sha256",
)
_TOKEN_FIELDS: Final = frozenset(
    {
        "source_digest",
        "canonical_bytes",
        "canonical_raw_sha256",
        "mesh_digest",
        "coordinates",
        "triangles",
        "coordinates_sha256",
        "triangles_sha256",
        "correspondence_digest",
        "selection_membership",
        "selection_membership_sha256",
    }
)


class _AdmissionError(Exception):
    pass


def _close(delegate: object | None) -> None:
    if delegate is None:
        return
    try:
        delegate.close()  # type: ignore[attr-defined]
    except Exception:
        pass


def _comm_is_open(delegate: object) -> bool:
    try:
        comm = delegate.comm  # type: ignore[attr-defined]
    except Exception:
        return False
    if comm is None:
        return False
    return not bool(getattr(comm, "_closed", False))


def _capture_selection_membership(
    mesh: Mesh,
    token: dict[str, object],
    correspondence_digest: str,
    triangles: np.ndarray[Any, np.dtype[np.uint32]],
) -> tuple[bytes, str]:
    encoded = token["selection_membership"]
    digest = token["selection_membership_sha256"]
    if (
        type(encoded) is not bytes
        or type(digest) is not str
        or len(digest) != 64
        or digest != digest.lower()
        or any(character not in "0123456789abcdef" for character in digest)
        or hashlib.sha256(encoded).hexdigest() != digest
        or not encoded.startswith(_SELECTION_MEMBERSHIP_DOMAIN)
    ):
        raise _AdmissionError

    offset = len(_SELECTION_MEMBERSHIP_DOMAIN)

    def take(length: int) -> bytes:
        nonlocal offset
        end = offset + length
        if length < 0 or end > len(encoded):
            raise _AdmissionError
        value = encoded[offset:end]
        offset = end
        return value

    def u32() -> int:
        return int.from_bytes(take(4), "little")

    if take(64) != correspondence_digest.encode("ascii"):
        raise _AdmissionError
    if u32() != len(_SELECTION_NAMES):
        raise _AdmissionError

    edge_incidence: dict[tuple[int, int], int] = {}
    for triangle in triangles:
        vertices = tuple(int(vertex) for vertex in triangle)
        for first, second in (
            (vertices[0], vertices[1]),
            (vertices[0], vertices[2]),
            (vertices[1], vertices[2]),
        ):
            edge = (first, second) if first < second else (second, first)
            edge_incidence[edge] = edge_incidence.get(edge, 0) + 1
    canonical_edges = tuple(sorted(edge_incidence))
    selected_boundary_entities: set[int] = set()

    observed_names: list[str] = []
    observed_counts: list[int] = []
    for expected_name, expected_dimension in zip(
        _SELECTION_NAMES, _SELECTION_DIMENSIONS, strict=True
    ):
        try:
            name = take(u32()).decode("utf-8", errors="strict")
        except UnicodeError as error:
            raise _AdmissionError from error
        dimension = u32()
        entity_count = u32()
        if name != expected_name or dimension != expected_dimension:
            raise _AdmissionError
        observed_names.append(name)
        observed_counts.append(entity_count)
        previous = -1
        for _ in range(entity_count):
            entity = u32()
            vertex_count = u32()
            if entity <= previous or vertex_count != dimension + 1:
                raise _AdmissionError
            previous = entity
            vertices = tuple(u32() for _ in range(vertex_count))
            if len(set(vertices)) != vertex_count or any(
                vertex >= _VERTEX_COUNT for vertex in vertices
            ):
                raise _AdmissionError
            if dimension == 1:
                if (
                    entity >= len(canonical_edges)
                    or vertices != canonical_edges[entity]
                    or edge_incidence[vertices] != 1
                    or entity in selected_boundary_entities
                ):
                    raise _AdmissionError
                selected_boundary_entities.add(entity)
            elif dimension == 2:
                if entity >= _TRIANGLE_COUNT or vertices != tuple(
                    int(vertex) for vertex in triangles[entity]
                ):
                    raise _AdmissionError
            else:
                raise _AdmissionError
    if offset != len(encoded):
        raise _AdmissionError
    if tuple(observed_names) != mesh.selection_names:
        raise _AdmissionError
    if tuple(observed_counts) != tuple(
        mesh.selection_entity_count(name) for name in observed_names
    ):
        raise _AdmissionError
    if len(selected_boundary_entities) != sum(
        1 for incidence in edge_incidence.values() if incidence == 1
    ):
        raise _AdmissionError
    return (bytes(encoded), digest)


def _capture_exact_payload(mesh: object, token: object) -> dict[str, object]:
    if type(mesh) is not Mesh or type(token) is not dict:
        raise _AdmissionError
    if set(token) != _TOKEN_FIELDS:
        raise _AdmissionError

    source_digest = mesh.source_digest
    realized_geometry_digest = mesh.realized_geometry_digest
    mesh_digest = mesh.digest
    correspondence_digest = mesh.correspondence_digest
    realization_digest = mesh.realization_digest
    canonical = mesh.canonical_bytes
    coordinates = mesh.coordinates
    triangles = mesh.cells
    if (
        source_digest != _SOURCE_DIGEST
        or token["source_digest"] != source_digest
        or mesh_digest != _MESH_DIGEST
        or token["mesh_digest"] != mesh_digest
        or token["correspondence_digest"] != correspondence_digest
        or type(canonical) is not bytes
        or len(canonical) != _CANONICAL_BYTES
        or token["canonical_bytes"] != canonical
        or token["canonical_raw_sha256"] != _RAW_CANONICAL_DIGEST
        or hashlib.sha256(canonical).hexdigest() != _RAW_CANONICAL_DIGEST
        or hashlib.sha256(_MESH_DIGEST_DOMAIN + canonical).hexdigest() != mesh_digest
        or mesh.dimension != 2
        or mesh.vertex_count != _VERTEX_COUNT
        or mesh.cell_count != _TRIANGLE_COUNT
        or token["coordinates"] is not coordinates
        or token["triangles"] is not triangles
    ):
        raise _AdmissionError

    if (
        type(coordinates) is not np.ndarray
        or coordinates.shape != (_VERTEX_COUNT, 2)
        or coordinates.dtype != np.dtype(np.float64)
        or not coordinates.flags.c_contiguous
        or coordinates.flags.writeable
        or type(triangles) is not np.ndarray
        or triangles.shape != (_TRIANGLE_COUNT, 3)
        or triangles.dtype != np.dtype(np.uint32)
        or not triangles.flags.c_contiguous
        or triangles.flags.writeable
    ):
        raise _AdmissionError
    if not np.isfinite(coordinates).all():
        raise _AdmissionError
    if not bool(np.all(triangles < _VERTEX_COUNT)):
        raise _AdmissionError

    selection_membership, selection_membership_sha256 = (
        _capture_selection_membership(mesh, token, correspondence_digest, triangles)
    )

    coordinate_snapshot = coordinates.tobytes(order="C")
    triangle_snapshot = triangles.tobytes(order="C")
    if (
        coordinate_snapshot != token["coordinates"].tobytes(order="C")
        or triangle_snapshot != token["triangles"].tobytes(order="C")
        or token["coordinates_sha256"] != _COORDINATE_SHA256
        or token["triangles_sha256"] != _TRIANGLE_SHA256
        or hashlib.sha256(coordinate_snapshot).hexdigest() != _COORDINATE_SHA256
        or hashlib.sha256(triangle_snapshot).hexdigest() != _TRIANGLE_SHA256
    ):
        raise _AdmissionError

    coordinate_copy = np.array(coordinates, dtype="<f8", order="C", copy=True)
    triangle_copy = np.array(triangles, dtype="<u4", order="C", copy=True)
    coordinate_bytes = coordinate_copy.tobytes(order="C")
    triangle_bytes = triangle_copy.tobytes(order="C")
    if (
        len(coordinate_bytes) != _COORDINATE_BYTES
        or len(triangle_bytes) != _TRIANGLE_BYTES
        or mesh.source_digest != source_digest
        or mesh.realized_geometry_digest != realized_geometry_digest
        or mesh.digest != mesh_digest
        or mesh.correspondence_digest != correspondence_digest
        or mesh.realization_digest != realization_digest
        or mesh.canonical_bytes != canonical
        or mesh.coordinates is not coordinates
        or mesh.cells is not triangles
        or coordinates.shape != (_VERTEX_COUNT, 2)
        or coordinates.dtype != np.dtype(np.float64)
        or not coordinates.flags.c_contiguous
        or coordinates.tobytes(order="C") != coordinate_snapshot
        or triangles.shape != (_TRIANGLE_COUNT, 3)
        or triangles.dtype != np.dtype(np.uint32)
        or not triangles.flags.c_contiguous
        or triangles.tobytes(order="C") != triangle_snapshot
        or coordinates.flags.writeable
        or triangles.flags.writeable
    ):
        raise _AdmissionError
    return {
        "profile": _PROFILE,
        "mesh_digest": _MESH_DIGEST,
        "vertex_count": _VERTEX_COUNT,
        "triangle_count": _TRIANGLE_COUNT,
        "coordinates_f64_le": coordinate_bytes,
        "triangles_u32_le": triangle_bytes,
        "correspondence_digest": correspondence_digest,
        "selection_membership": selection_membership,
        "selection_membership_sha256": selection_membership_sha256,
    }


def _load_assets() -> tuple[str, str]:
    root = resources.files(__package__).joinpath("static")
    esm = root.joinpath("mesh-view.mjs").read_text(encoding="utf-8")
    css = root.joinpath("mesh-view.css").read_text(encoding="utf-8")
    if not esm.strip() or not css.strip():
        raise RuntimeError("empty Notebook presentation asset")
    return esm, css


def _new_delegate(payload: dict[str, object], esm: str, css: str) -> object:
    import anywidget
    import traitlets
    from ipywidgets import Layout, widget_serialization  # type: ignore[import-untyped]

    class _MeshWidget(anywidget.AnyWidget):
        _esm = esm
        _css = css

        layout = traitlets.Instance(
            Layout, default_value=None, allow_none=True
        ).tag(sync=True, **widget_serialization)
        profile = traitlets.Unicode().tag(sync=True)
        mesh_digest = traitlets.Unicode().tag(sync=True)
        vertex_count = traitlets.Int().tag(sync=True)
        triangle_count = traitlets.Int().tag(sync=True)
        coordinates_f64_le = traitlets.Bytes().tag(sync=True)
        triangles_u32_le = traitlets.Bytes().tag(sync=True)
        correspondence_digest = traitlets.Unicode().tag(sync=True)
        selection_membership = traitlets.Bytes().tag(sync=True)
        selection_membership_sha256 = traitlets.Unicode().tag(sync=True)
        _eqiora_n1_model_id = traitlets.Unicode().tag(sync=True)

        def __init__(self) -> None:
            self._eqiora_sealed = False
            model_id = uuid.uuid4().hex
            super().__init__(model_id=model_id, _eqiora_n1_model_id=model_id, **payload)
            self._eqiora_values = {
                name: getattr(self, name) for name in _PAYLOAD_FIELDS
            }
            self._eqiora_sealed = True

        @traitlets.validate(*_PAYLOAD_FIELDS, "_eqiora_n1_model_id")
        def _immutable_payload(self, proposal: dict[str, Any]) -> object:
            if getattr(self, "_eqiora_sealed", False):
                if proposal["trait"].name == "_eqiora_n1_model_id":
                    raise traitlets.TraitError(
                        "Eqiora Mesh model identity is immutable"
                    )
                raise traitlets.TraitError(
                    f"Eqiora Mesh payload member {proposal['trait'].name!r} is immutable"
                )
            return proposal["value"]

        def set_state(self, sync_data: dict[str, object]) -> None:
            if self._eqiora_sealed:
                if set(sync_data).intersection(_PAYLOAD_FIELDS):
                    raise traitlets.TraitError("Eqiora Mesh payload is immutable")
                if "_eqiora_n1_model_id" in sync_data:
                    raise traitlets.TraitError(
                        "Eqiora Mesh model identity is immutable"
                    )
            super().set_state(sync_data)

        def _repr_mimebundle_(
            self, **kwargs: dict[Any, Any]
        ) -> tuple[dict[Any, Any], dict[Any, Any]] | None:
            hook = super()._repr_mimebundle_(**kwargs)
            if type(hook) is tuple and len(hook) == 2:
                data, metadata = hook
                if type(data) is dict and type(metadata) is dict:
                    widget_view = data.get(_WIDGET_MIME)
                    if (
                        type(widget_view) is dict
                        and set(widget_view) == {
                            "version_major",
                            "version_minor",
                            "model_id",
                        }
                        and type(widget_view["version_major"]) is int
                        and widget_view["version_major"] == 2
                        and type(widget_view["version_minor"]) is int
                        and widget_view["version_minor"] == 1
                        and type(widget_view["model_id"]) is str
                        and bool(widget_view["model_id"])
                    ):
                        # AnyWidget 0.11 emits protocol 2.1 while this closed
                        # presentation contract deliberately publishes 2.0.
                        normalized_view = dict(widget_view)
                        normalized_view["version_minor"] = 0
                        normalized_data = dict(data)
                        normalized_data[_WIDGET_MIME] = normalized_view
                        return (normalized_data, metadata)
            return hook

    return _MeshWidget()


def mesh_mimebundle(
    mesh: object, token: object, current_delegate: object | None
) -> tuple[str, object | None, object | None]:
    """Return a private native-adapter outcome for one exact Mesh."""

    try:
        payload = _capture_exact_payload(mesh, token)
    except Exception:
        _close(current_delegate)
        return ("unsupported", None, None)

    try:
        if importlib.util.find_spec("anywidget") is None:
            _close(current_delegate)
            return ("absent", None, None)
        esm, css = _load_assets()
    except Exception:
        _close(current_delegate)
        return ("corrupt", None, None)

    delegate = current_delegate if current_delegate is not None else None
    if delegate is not None and not _comm_is_open(delegate):
        delegate = None
    try:
        if delegate is None:
            delegate = _new_delegate(payload, esm, css)
        hook = delegate._repr_mimebundle_()  # type: ignore[attr-defined]
        return ("rich", delegate, hook)
    except Exception:
        _close(delegate)
        return ("corrupt", None, None)
