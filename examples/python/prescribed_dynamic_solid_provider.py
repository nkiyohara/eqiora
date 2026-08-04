"""Frozen NumPy provider for one prescribed dynamic-solid E1 occurrence."""

from __future__ import annotations

import hashlib
import json
import math
import struct
import sys
from typing import BinaryIO

import numpy as np


MAGIC = b"EQP1"
CONTROL = 0x01
BULK = 0x02
PROTOCOL = "eqiora.external-boundary-provider-subprocess/v1"
CONTRACT = "eqiora.prescribed-dynamic-solid-state-boundary/v1"
PROVIDER_ID = "eqiora.python.prescribed-dynamic-solid-affine"
PROVIDER_RELEASE = "1.0.0"
MAX_CONTROL = 4096
BULK_BYTES = 96


def canonical(value: object) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
    ).encode("utf-8")


def digest(domain: bytes, *pieces: bytes) -> str:
    value = hashlib.sha256()
    value.update(domain)
    value.update(b"\x00")
    for index, piece in enumerate(pieces):
        if index:
            value.update(b"\x00")
        value.update(piece)
    return value.hexdigest()


def read_exact(stream: BinaryIO, length: int) -> bytes:
    chunks: list[bytes] = []
    remaining = length
    while remaining:
        chunk = stream.read(remaining)
        if not chunk:
            raise ValueError("truncated provider frame")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def read_frame(stream: BinaryIO, expected_kind: int) -> bytes:
    prefix = read_exact(stream, 16)
    if prefix[:4] != MAGIC or prefix[5:8] != b"\x00\x00\x00":
        raise ValueError("invalid provider frame prefix")
    kind = prefix[4]
    length = struct.unpack("<Q", prefix[8:])[0]
    if kind != expected_kind:
        raise ValueError("provider frame kind is invalid in the active state")
    if kind == CONTROL and length > MAX_CONTROL:
        raise ValueError("control payload exceeds budget")
    if kind == BULK and length != BULK_BYTES:
        raise ValueError("bulk payload has wrong length")
    return read_exact(stream, length)


def write_frame(stream: BinaryIO, kind: int, payload: bytes) -> None:
    if kind == CONTROL and len(payload) > MAX_CONTROL:
        raise ValueError("control payload exceeds budget")
    if kind == BULK and len(payload) != BULK_BYTES:
        raise ValueError("bulk payload has wrong length")
    stream.write(MAGIC + bytes((kind, 0, 0, 0)) + struct.pack("<Q", len(payload)))
    stream.write(payload)
    stream.flush()


def object_without_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate control member")
        value[key] = item
    return value


def decode_control(payload: bytes, keys: list[str]) -> dict[str, object]:
    if len(payload) > MAX_CONTROL or maximum_nesting(payload) > 8:
        raise ValueError("control payload exceeds structural budget")
    value = json.loads(payload, object_pairs_hook=object_without_duplicates)
    if (
        not isinstance(value, dict)
        or list(value) != keys
        or canonical(value) != payload
    ):
        raise ValueError("control payload is not the exact closed canonical object")
    return value


def maximum_nesting(payload: bytes) -> int:
    depth = 0
    maximum = 0
    in_string = False
    escaped = False
    for byte in payload:
        if in_string:
            if escaped:
                escaped = False
            elif byte == 0x5C:
                escaped = True
            elif byte == 0x22:
                in_string = False
            continue
        if byte == 0x22:
            in_string = True
        elif byte in (0x7B, 0x5B):
            depth += 1
            maximum = max(maximum, depth)
        elif byte in (0x7D, 0x5D):
            depth = max(depth - 1, 0)
    return maximum


def exact_provider() -> dict[str, object]:
    return {
        "id": PROVIDER_ID,
        "release": PROVIDER_RELEASE,
        "dependencies": [
            {"name": "cpython", "release": "3.12"},
            {"name": "numpy", "release": "2.1.0"},
        ],
    }


def hello() -> dict[str, object]:
    return {
        "type": "hello",
        "protocol": PROTOCOL,
        "contract": CONTRACT,
        "provider": exact_provider(),
        "capability": {
            "deterministic": True,
            "stateful": False,
            "scalar": "f64",
            "target": "host-cpu",
            "association": "vertex",
            "layout": "entity-major-spatial-cartesian",
            "maximum_input_fields": 2,
            "maximum_output_fields": 1,
            "maximum_coefficients_per_field": 12,
            "maximum_aggregate_bulk_bytes": 288,
        },
    }


def require_digest(value: object) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError("noncanonical digest")
    return value


def require_ulid(value: object) -> str:
    alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
    if (
        not isinstance(value, str)
        or len(value) != 26
        or any(character not in alphabet for character in value)
    ):
        raise ValueError("noncanonical ULID")
    return value


def require_keys(value: object, keys: list[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict) or list(value) != keys:
        raise ValueError(f"{label} members are reordered or incomplete")
    return value


def require_int(value: object, expected: int, label: str) -> None:
    if type(value) is not int or value != expected:
        raise ValueError(f"{label} is not the exact JSON integer")


def require_float(value: object, expected: float, label: str) -> None:
    if (
        type(value) is not float
        or value != expected
        or not math.isfinite(value)
        or (value == 0.0 and math.copysign(1.0, value) < 0.0)
    ):
        raise ValueError(f"{label} is not the exact finite JSON binary64 value")


def require_descriptor(
    value: object,
    *,
    role: str,
    field: str,
    unit: str,
    output: bool,
) -> dict[str, object]:
    expected_keys = [
        "role",
        "field_ulid",
        "unit",
        "value_shape",
        "frame",
        "representation",
        "association",
    ]
    if output:
        expected_keys.append("convention")
    expected_keys.extend(("coefficient_count", "byte_length"))
    if not output:
        expected_keys.append("block_sha256")
    value = require_keys(value, expected_keys, "descriptor")
    shape = value["value_shape"]
    if not isinstance(shape, list) or len(shape) != 1:
        raise ValueError("descriptor value shape is not the exact array")
    require_int(shape[0], 3, "descriptor value shape")
    require_int(value["coefficient_count"], 12, "descriptor coefficient count")
    require_int(value["byte_length"], 96, "descriptor byte length")
    expected: dict[str, object] = {
        "role": role,
        "field_ulid": field,
        "unit": unit,
        "value_shape": [3],
        "frame": "spatial-cartesian",
        "representation": "continuous-lagrange-p1-trace",
        "association": "vertex",
    }
    if output:
        expected["convention"] = "total-reference-configuration"
    expected.update({"coefficient_count": 12, "byte_length": 96})
    if not output:
        require_digest(value["block_sha256"])
        expected["block_sha256"] = value["block_sha256"]
    if value != expected:
        raise ValueError("descriptor differs from the fixed projection")
    return value


def validate_bind(bind: dict[str, object]) -> None:
    require_int(bind["semantic_revision"], 1, "semantic revision")
    require_float(bind["model_time_s"], 0.0, "model time")
    require_float(bind["next_time_s"], 0.25, "next time")
    require_float(bind["delta_time_s"], 0.25, "delta time")
    vertices = bind["vertex_indices"]
    if not isinstance(vertices, list) or len(vertices) != 4:
        raise ValueError("vertex inventory is not the exact array")
    for value, expected in zip(vertices, (1, 3, 5, 7), strict=True):
        require_int(value, expected, "vertex index")
    provider = require_keys(
        bind["provider"], ["id", "release", "dependencies"], "provider"
    )
    dependencies = provider["dependencies"]
    if not isinstance(dependencies, list) or len(dependencies) != 2:
        raise ValueError("provider dependency inventory is not the exact array")
    for dependency in dependencies:
        require_keys(dependency, ["name", "release"], "provider dependency")
    if (
        bind["type"] != "bind"
        or bind["protocol"] != PROTOCOL
        or bind["contract"] != CONTRACT
        or provider != exact_provider()
        or bind["coefficient_order"] != "vertex-index-ascending-component-x-y-z"
    ):
        raise ValueError("bind differs from fixed policy")
    for name in (
        "model_sha256",
        "realization_sha256",
        "geometry_sha256",
        "correspondence_sha256",
        "mesh_sha256",
        "prior_state_sha256",
    ):
        require_digest(bind[name])
    require_ulid(bind["solid_domain_ulid"])
    require_ulid(bind["boundary_ulid"])
    inputs = bind["inputs"]
    output = bind["output"]
    if not isinstance(inputs, list) or len(inputs) != 2 or not isinstance(output, dict):
        raise ValueError("bind projection inventory differs")
    displacement = require_descriptor(
        inputs[0],
        role="prior-displacement-trace",
        field=require_ulid(inputs[0].get("field_ulid")),
        unit="m",
        output=False,
    )
    velocity = require_descriptor(
        inputs[1],
        role="prior-velocity-trace",
        field=require_ulid(inputs[1].get("field_ulid")),
        unit="m/s",
        output=False,
    )
    require_descriptor(
        output,
        role="next-total-displacement",
        field=displacement["field_ulid"],
        unit="m",
        output=True,
    )
    if displacement["field_ulid"] == velocity["field_ulid"]:
        raise ValueError("input Field roles are not distinct")


def input_header(bind: dict[str, object], descriptor: dict[str, object]) -> bytes:
    return canonical(
        {
            "model_sha256": bind["model_sha256"],
            "realization_sha256": bind["realization_sha256"],
            "prior_state_sha256": bind["prior_state_sha256"],
            "model_time_s": bind["model_time_s"],
            "boundary_ulid": bind["boundary_ulid"],
            "field_ulid": descriptor["field_ulid"],
            "role": descriptor["role"],
            "unit": descriptor["unit"],
            "value_shape": descriptor["value_shape"],
            "frame": descriptor["frame"],
            "representation": descriptor["representation"],
            "association": descriptor["association"],
            "vertex_indices": bind["vertex_indices"],
            "coefficient_count": descriptor["coefficient_count"],
            "byte_length": descriptor["byte_length"],
        }
    )


def owned_f64(payload: bytes) -> np.ndarray:
    values = np.frombuffer(payload, dtype="<f8").copy()
    if values.shape != (12,) or not np.isfinite(values).all():
        raise ValueError("bulk is not twelve finite little-endian binary64 values")
    if np.signbit(values[values == 0.0]).any():
        raise ValueError("bulk contains negative zero")
    return values


def send_error(output: BinaryIO, phase: str) -> None:
    payload = canonical(
        {
            "type": "error",
            "phase": phase,
            "code": "provider.invalid-request",
            "message": "provider request rejected",
        }
    )
    try:
        write_frame(output, CONTROL, payload)
    except (BrokenPipeError, OSError):
        pass


def run(input_stream: BinaryIO, output_stream: BinaryIO) -> int:
    if (
        sys.implementation.name != "cpython"
        or sys.version_info[:2] != (3, 12)
        or np.__version__ != "2.1.0"
    ):
        raise RuntimeError("the frozen provider requires CPython 3.12 and NumPy 2.1.0")
    write_frame(output_stream, CONTROL, canonical(hello()))
    phase = "bind"
    try:
        bind_payload = read_frame(input_stream, CONTROL)
        bind_keys = [
            "type",
            "protocol",
            "contract",
            "model_sha256",
            "semantic_revision",
            "realization_sha256",
            "geometry_sha256",
            "correspondence_sha256",
            "mesh_sha256",
            "prior_state_sha256",
            "provider",
            "model_time_s",
            "next_time_s",
            "delta_time_s",
            "solid_domain_ulid",
            "boundary_ulid",
            "vertex_indices",
            "coefficient_order",
            "inputs",
            "output",
        ]
        bind = decode_control(bind_payload, bind_keys)
        validate_bind(bind)
        binding_identity = digest(
            b"eqiora.prescribed-dynamic-solid-provider-binding/v1", bind_payload
        )
        write_frame(
            output_stream,
            CONTROL,
            canonical({"type": "bound", "binding_sha256": binding_identity}),
        )

        phase = "evaluate"
        evaluate_payload = read_frame(input_stream, CONTROL)
        evaluate = decode_control(evaluate_payload, ["type", "binding_sha256"])
        if evaluate != {"type": "evaluate", "binding_sha256": binding_identity}:
            raise ValueError("evaluate differs from active binding")
        request_identity = digest(
            b"eqiora.prescribed-dynamic-solid-provider-request/v1", evaluate_payload
        )
        displacement_payload = read_frame(input_stream, BULK)
        velocity_payload = read_frame(input_stream, BULK)
        inputs = bind["inputs"]
        for descriptor, payload in zip(
            inputs, (displacement_payload, velocity_payload), strict=True
        ):
            identity = digest(
                b"eqiora.prescribed-dynamic-solid-provider-input-block/v1",
                input_header(bind, descriptor),
                payload,
            )
            if identity != descriptor["block_sha256"]:
                raise ValueError("input block identity differs")
        displacement = owned_f64(displacement_payload)
        velocity = owned_f64(velocity_payload)
        delta_time = np.array(bind["delta_time_s"], dtype="<f8")
        candidate = np.add(displacement, delta_time * velocity, dtype="<f8").copy()
        candidate_payload = candidate.tobytes(order="C")
        output_header = canonical(bind["output"])
        candidate_identity = digest(
            b"eqiora.prescribed-dynamic-solid-provider-candidate/v1",
            bytes.fromhex(request_identity),
            output_header,
            candidate_payload,
        )
        write_frame(
            output_stream,
            CONTROL,
            canonical(
                {
                    "type": "candidate",
                    "request_sha256": request_identity,
                    "candidate_sha256": candidate_identity,
                    "byte_length": 96,
                }
            ),
        )
        write_frame(output_stream, BULK, candidate_payload)
        write_frame(
            output_stream,
            CONTROL,
            canonical(
                {
                    "type": "report",
                    "request_sha256": request_identity,
                    "candidate_sha256": candidate_identity,
                    "status": "success",
                    "code": "provider.success",
                    "message": "affine predictor completed",
                }
            ),
        )

        phase = "close"
        close = decode_control(
            read_frame(input_stream, CONTROL),
            ["type", "request_sha256", "candidate_sha256", "outcome"],
        )
        if close != {
            "type": "close",
            "request_sha256": request_identity,
            "candidate_sha256": candidate_identity,
            "outcome": "accepted",
        }:
            raise ValueError("close differs from the admitted occurrence")
        write_frame(
            output_stream,
            CONTROL,
            canonical(
                {
                    "type": "closed",
                    "request_sha256": request_identity,
                    "candidate_sha256": candidate_identity,
                }
            ),
        )
        return 0
    except (KeyError, TypeError, ValueError, json.JSONDecodeError):
        send_error(output_stream, phase)
        return 2


if __name__ == "__main__":
    raise SystemExit(run(sys.stdin.buffer, sys.stdout.buffer))
