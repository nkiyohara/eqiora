"""Closed, read-only transport for one accepted Trajectory presentation."""

from __future__ import annotations

import hashlib
import importlib.util
import math
import uuid
from importlib import resources
from typing import Any, Final

import numpy as np

from eqiora.trajectory import Trajectory

_PROFILE: Final = "fixed-mesh-scalar-trajectory-2d/v1"
_WIDGET_MIME: Final = "application/vnd.jupyter.widget-view+json"
_TOKEN_FIELDS: Final = frozenset(
    {
        "model_digest",
        "geometry_digest",
        "correspondence_digest",
        "mesh_digest",
        "realization_digest",
        "run_digest",
        "trajectory_digest",
        "coordinates",
        "cells",
        "states",
    }
)
_PAYLOAD_FIELDS: Final = (
    "profile",
    "trajectory_digest",
    "mesh_digest",
    "vertex_count",
    "triangle_count",
    "state_count",
    "state_digests",
    "snapshot_digests",
    "field_id",
    "dimension",
    "frame",
    "coordinates_f64_le",
    "triangles_u32_le",
    "support_u32_le",
    "steps_u64_le",
    "times_f64_le",
    "values_f64_le",
    "coordinates_sha256",
    "triangles_sha256",
    "support_sha256",
    "steps_sha256",
    "times_sha256",
    "values_sha256",
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
    return comm is not None and not bool(getattr(comm, "_closed", False))


def _digest(value: object) -> str:
    if (
        type(value) is not str
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise _AdmissionError
    return value


def _bytes(array: np.ndarray, dtype: str) -> bytes:
    return np.array(array, dtype=dtype, order="C", copy=True).tobytes(order="C")


def _capture_exact_payload(trajectory: object, token: object) -> dict[str, object]:
    if type(trajectory) is not Trajectory or type(token) is not dict:
        raise _AdmissionError
    if set(token) != _TOKEN_FIELDS or trajectory.dimension != 2:
        raise _AdmissionError

    for name in (
        "model_digest",
        "geometry_digest",
        "correspondence_digest",
        "mesh_digest",
        "realization_digest",
        "run_digest",
    ):
        if token[name] != _digest(getattr(trajectory, name)):
            raise _AdmissionError
    if token["trajectory_digest"] != _digest(trajectory.digest):
        raise _AdmissionError

    coordinates = trajectory.coordinates
    triangles = trajectory.cells
    states = trajectory.states
    if (
        token["coordinates"] is not coordinates
        or token["cells"] is not triangles
        or type(token["states"]) is not tuple
        or len(token["states"]) != len(states)
        or any(left is not right for left, right in zip(token["states"], states))
        or type(coordinates) is not np.ndarray
        or coordinates.ndim != 2
        or coordinates.shape[1] != 2
        or coordinates.dtype != np.dtype(np.float64)
        or not coordinates.flags.c_contiguous
        or coordinates.flags.writeable
        or not np.isfinite(coordinates).all()
        or type(triangles) is not np.ndarray
        or triangles.ndim != 2
        or triangles.shape[1] != 3
        or triangles.dtype != np.dtype(np.uint32)
        or not triangles.flags.c_contiguous
        or triangles.flags.writeable
        or len(coordinates) != 9
        or len(triangles) != 8
        or len(states) != 2
        or not bool(np.all(triangles < len(coordinates)))
    ):
        raise _AdmissionError

    field_id: str | None = None
    dimension: tuple[int, ...] | None = None
    support_snapshot: bytes | None = None
    steps: list[int] = []
    times: list[float] = []
    value_arrays: list[np.ndarray] = []
    state_digests: list[str] = []
    snapshot_digests: list[str] = []
    for state in states:
        state_digests.append(_digest(state.digest))
        candidates = [
            snapshot
            for snapshot in state.fields
            if snapshot.mesh_digest == trajectory.mesh_digest
            and snapshot.field.model_digest == trajectory.model_digest
            and snapshot.value_shape == ()
            and snapshot.frame == "invariant"
            and snapshot.associations == ("vertex",)
        ]
        if len(candidates) != 1:
            raise _AdmissionError
        snapshot = candidates[0]
        snapshot_digests.append(_digest(snapshot.digest))
        support = snapshot.support_indices("vertex")
        values = snapshot.values("vertex")
        if (
            type(support) is not np.ndarray
            or support.dtype != np.dtype(np.uint32)
            or support.ndim != 1
            or support.flags.writeable
            or len(support) != 6
            or type(values) is not np.ndarray
            or values.dtype != np.dtype(np.float64)
            or values.ndim != 1
            or values.flags.writeable
            or len(values) != len(coordinates)
            or not np.isfinite(values).all()
            or not bool(np.all(support < len(coordinates)))
        ):
            raise _AdmissionError
        current_support = _bytes(support, "<u4")
        current_id = snapshot.field.id
        current_dimension = tuple(snapshot.dimension)
        if field_id is None:
            field_id = current_id
            dimension = current_dimension
            support_snapshot = current_support
        elif (
            current_id != field_id
            or current_dimension != dimension
            or current_support != support_snapshot
        ):
            raise _AdmissionError
        steps.append(state.step)
        times.append(state.time_s)
        value_arrays.append(values[support])

    if (
        field_id is None
        or dimension is None
        or support_snapshot is None
        or any(type(step) is not int or step < 0 for step in steps)
        or len(set(steps)) != len(steps)
        or steps != sorted(steps)
        or any(type(time) is not float or not math.isfinite(time) for time in times)
        or any(later <= earlier for earlier, later in zip(times, times[1:]))
    ):
        raise _AdmissionError

    coordinate_bytes = _bytes(coordinates, "<f8")
    triangle_bytes = _bytes(triangles, "<u4")
    step_bytes = np.asarray(steps, dtype="<u8").tobytes(order="C")
    time_bytes = np.asarray(times, dtype="<f8").tobytes(order="C")
    value_bytes = np.concatenate(value_arrays).astype("<f8", copy=True).tobytes(order="C")
    payload: dict[str, object] = {
        "profile": _PROFILE,
        "trajectory_digest": trajectory.digest,
        "mesh_digest": trajectory.mesh_digest,
        "vertex_count": len(coordinates),
        "triangle_count": len(triangles),
        "state_count": len(states),
        "state_digests": ",".join(state_digests),
        "snapshot_digests": ",".join(snapshot_digests),
        "field_id": field_id,
        "dimension": ",".join(str(exponent) for exponent in dimension),
        "frame": "invariant",
        "coordinates_f64_le": coordinate_bytes,
        "triangles_u32_le": triangle_bytes,
        "support_u32_le": support_snapshot,
        "steps_u64_le": step_bytes,
        "times_f64_le": time_bytes,
        "values_f64_le": value_bytes,
    }
    binary_parts: dict[str, bytes] = {
        "coordinates": coordinate_bytes,
        "triangles": triangle_bytes,
        "support": support_snapshot,
        "steps": step_bytes,
        "times": time_bytes,
        "values": value_bytes,
    }
    for name, encoded in binary_parts.items():
        payload[f"{name}_sha256"] = hashlib.sha256(encoded).hexdigest()
    return payload


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

    class _TrajectoryWidget(anywidget.AnyWidget):
        _esm = esm
        _css = css
        layout = traitlets.Instance(Layout, default_value=None, allow_none=True).tag(
            sync=True, **widget_serialization
        )
        profile = traitlets.Unicode().tag(sync=True)
        trajectory_digest = traitlets.Unicode().tag(sync=True)
        mesh_digest = traitlets.Unicode().tag(sync=True)
        vertex_count = traitlets.Int().tag(sync=True)
        triangle_count = traitlets.Int().tag(sync=True)
        state_count = traitlets.Int().tag(sync=True)
        state_digests = traitlets.Unicode().tag(sync=True)
        snapshot_digests = traitlets.Unicode().tag(sync=True)
        field_id = traitlets.Unicode().tag(sync=True)
        dimension = traitlets.Unicode().tag(sync=True)
        frame = traitlets.Unicode().tag(sync=True)
        coordinates_f64_le = traitlets.Bytes().tag(sync=True)
        triangles_u32_le = traitlets.Bytes().tag(sync=True)
        support_u32_le = traitlets.Bytes().tag(sync=True)
        steps_u64_le = traitlets.Bytes().tag(sync=True)
        times_f64_le = traitlets.Bytes().tag(sync=True)
        values_f64_le = traitlets.Bytes().tag(sync=True)
        coordinates_sha256 = traitlets.Unicode().tag(sync=True)
        triangles_sha256 = traitlets.Unicode().tag(sync=True)
        support_sha256 = traitlets.Unicode().tag(sync=True)
        steps_sha256 = traitlets.Unicode().tag(sync=True)
        times_sha256 = traitlets.Unicode().tag(sync=True)
        values_sha256 = traitlets.Unicode().tag(sync=True)
        _eqiora_n3_model_id = traitlets.Unicode().tag(sync=True)

        def __init__(self) -> None:
            self._eqiora_sealed = False
            model_id = uuid.uuid4().hex
            super().__init__(model_id=model_id, _eqiora_n3_model_id=model_id, **payload)
            self._eqiora_sealed = True

        @traitlets.validate(*_PAYLOAD_FIELDS, "_eqiora_n3_model_id")
        def _immutable_payload(self, proposal: dict[str, Any]) -> object:
            if getattr(self, "_eqiora_sealed", False):
                raise traitlets.TraitError("Eqiora Trajectory payload is immutable")
            return proposal["value"]

        def set_state(self, sync_data: dict[str, object]) -> None:
            if self._eqiora_sealed and set(sync_data).intersection(
                (*_PAYLOAD_FIELDS, "_eqiora_n3_model_id")
            ):
                raise traitlets.TraitError("Eqiora Trajectory payload is immutable")
            super().set_state(sync_data)

        def _repr_mimebundle_(
            self, **kwargs: dict[Any, Any]
        ) -> tuple[dict[Any, Any], dict[Any, Any]] | None:
            hook = super()._repr_mimebundle_(**kwargs)
            if type(hook) is tuple and len(hook) == 2:
                data, metadata = hook
                if type(data) is dict and type(metadata) is dict:
                    view = data.get(_WIDGET_MIME)
                    if type(view) is dict and view.get("version_major") == 2:
                        normalized = dict(view)
                        normalized["version_minor"] = 0
                        copied = dict(data)
                        copied[_WIDGET_MIME] = normalized
                        return copied, metadata
            return hook

    return _TrajectoryWidget()


def trajectory_mimebundle(
    trajectory: object, token: object, current_delegate: object | None
) -> tuple[str, object | None, object | None]:
    try:
        payload = _capture_exact_payload(trajectory, token)
    except Exception:
        _close(current_delegate)
        return "unsupported", None, None
    try:
        if importlib.util.find_spec("anywidget") is None:
            _close(current_delegate)
            return "absent", None, None
        esm, css = _load_assets()
    except Exception:
        _close(current_delegate)
        return "corrupt", None, None
    delegate = current_delegate if current_delegate is not None else None
    if delegate is not None and not _comm_is_open(delegate):
        delegate = None
    try:
        if delegate is None:
            delegate = _new_delegate(payload, esm, css)
        return "rich", delegate, delegate._repr_mimebundle_()  # type: ignore[attr-defined]
    except Exception:
        _close(delegate)
        return "corrupt", None, None
