"""Exact-reference Mesh transport for the private anywidget presentation."""

from __future__ import annotations

import hashlib
import importlib.util
from importlib import resources
from typing import Any, Final

import numpy as np

from eqiora.meshing import Mesh

_PROFILE: Final = "circular-hole-chordal-reference-50/v1"
_SOURCE_DIGEST: Final = (
    "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
)
_RAW_CANONICAL_DIGEST: Final = (
    "d977d9125488fffee72deaf9a0f146bc42dc05a135692919a374d746da0f1079"
)
_MESH_DIGEST: Final = (
    "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a"
)
_MESH_DIGEST_DOMAIN: Final = b"eqiora.simplicial-mesh-envelope/v1\0"
_CANONICAL_BYTES: Final = 4_835
_VERTEX_COUNT: Final = 104
_TRIANGLE_COUNT: Final = 104
_COORDINATE_BYTES: Final = 1_664
_TRIANGLE_BYTES: Final = 1_248
_COORDINATE_SHA256: Final = (
    "2aaf87276bf352faddfadc76e63c1f44340a362047b1399a2e081c798c5921aa"
)
_TRIANGLE_SHA256: Final = (
    "229392dc7faca769c88348cf41a810f29df3a22ad1276cb866783e5e04078a9f"
)
_WIDGET_MIME: Final = "application/vnd.jupyter.widget-view+json"
_PAYLOAD_FIELDS: Final = (
    "profile",
    "mesh_digest",
    "vertex_count",
    "triangle_count",
    "coordinates_f64_le",
    "triangles_u32_le",
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

    class MeshWidget(anywidget.AnyWidget):
        _esm = esm
        _css = css

        profile = traitlets.Unicode().tag(sync=True)
        mesh_digest = traitlets.Unicode().tag(sync=True)
        vertex_count = traitlets.Int().tag(sync=True)
        triangle_count = traitlets.Int().tag(sync=True)
        coordinates_f64_le = traitlets.Bytes().tag(sync=True)
        triangles_u32_le = traitlets.Bytes().tag(sync=True)

        def __init__(self) -> None:
            self._eqiora_sealed = False
            super().__init__(**payload)
            self._eqiora_values = {
                name: getattr(self, name) for name in _PAYLOAD_FIELDS
            }
            self._eqiora_sealed = True

        @traitlets.validate(*_PAYLOAD_FIELDS)
        def _immutable_payload(self, proposal: dict[str, Any]) -> object:
            if getattr(self, "_eqiora_sealed", False):
                raise traitlets.TraitError(
                    f"Eqiora Mesh payload member {proposal['trait'].name!r} is immutable"
                )
            return proposal["value"]

        def set_state(self, sync_data: dict[str, object]) -> None:
            if self._eqiora_sealed and set(sync_data).intersection(_PAYLOAD_FIELDS):
                raise traitlets.TraitError("Eqiora Mesh payload is immutable")
            super().set_state(sync_data)

    return MeshWidget()


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
