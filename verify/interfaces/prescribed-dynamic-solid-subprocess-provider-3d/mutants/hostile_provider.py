#!/usr/bin/env python3
"""Deterministic hostile peer for the frozen connected-child protocol tests."""

from __future__ import annotations

import hashlib
import json
import math
import struct
import sys
import time
from typing import Any


PROTOCOL = "eqiora.external-boundary-provider-subprocess/v1"
CONTRACT = "eqiora.prescribed-dynamic-solid-state-boundary/v1"
BINDING_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-binding/v1"
REQUEST_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-request/v1"
CANDIDATE_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-candidate/v1"
MODE = sys.argv[1] if len(sys.argv) == 2 else "honest"
IN = sys.stdin.buffer
OUT = sys.stdout.buffer

BIND_MUTATION_MODES = {
    "stale-model-binding",
    "stale-semantic-revision-binding",
    "stale-realization-binding",
    "stale-geometry-binding",
    "stale-correspondence-binding",
    "stale-mesh-binding",
    "stale-prior-state-binding",
    "stale-model-time-binding",
    "stale-next-time-binding",
    "stale-delta-time-binding",
    "stale-solid-body-binding",
    "stale-boundary-binding",
    "stale-displacement-field-binding",
    "stale-velocity-field-binding",
    "stale-output-field-binding",
    "wrong-displacement-role-binding",
    "wrong-velocity-role-binding",
    "wrong-output-role-binding",
    "swapped-input-roles-binding",
    "caller-order-vertices-binding",
    "missing-vertex-binding",
    "duplicate-vertex-binding",
    "reordered-vertices-binding",
    "foreign-vertex-binding",
    "missing-input-descriptor-binding",
    "additional-input-descriptor-binding",
    "wrong-unit-binding",
    "wrong-shape-binding",
    "wrong-frame-binding",
    "wrong-representation-binding",
    "wrong-input-association-binding",
    "wrong-coefficient-count-binding",
    "wrong-byte-length-binding",
    "wrong-coefficient-order-binding",
    "stale-displacement-block-binding",
    "stale-velocity-block-binding",
    "changed-bound-provider-binding",
    "changed-bound-dependency-binding",
}


def canonical(document: Any) -> bytes:
    return json.dumps(
        document,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
    ).encode("utf-8")


def identity(domain: str, *parts: bytes) -> bytes:
    hasher = hashlib.sha256()
    hasher.update(domain.encode())
    hasher.update(b"\0")
    for index, part in enumerate(parts):
        if index:
            hasher.update(b"\0")
        hasher.update(part)
    return hasher.digest()


def prefix(kind: int, length: int) -> bytes:
    return b"EQP1" + bytes([kind]) + b"\0\0\0" + struct.pack("<Q", length)


def framed(kind: int, payload: bytes) -> bytes:
    return prefix(kind, len(payload)) + payload


def write_raw(raw: bytes) -> None:
    OUT.write(raw)
    OUT.flush()


def write_control(document: dict[str, Any]) -> None:
    write_raw(framed(1, canonical(document)))


def write_bulk(payload: bytes) -> None:
    write_raw(framed(2, payload))


def read_exact(length: int) -> bytes:
    output = bytearray()
    while len(output) < length:
        chunk = IN.read(length - len(output))
        if not chunk:
            raise EOFError("peer closed before a complete frame")
        output.extend(chunk)
    return bytes(output)


def read_frame(expected_kind: int) -> bytes:
    header = read_exact(16)
    if header[:4] != b"EQP1" or header[4] != expected_kind or header[5:8] != b"\0\0\0":
        raise AssertionError("Eqiora sent an invalid frame prefix")
    length = struct.unpack("<Q", header[8:])[0]
    return read_exact(length)


def read_control(expected_type: str) -> tuple[dict[str, Any], bytes]:
    payload = read_frame(1)
    document = json.loads(payload)
    if canonical(document) != payload or document.get("type") != expected_type:
        raise AssertionError(f"expected canonical {expected_type} control")
    return document, payload


def provider() -> dict[str, Any]:
    return {
        "id": "eqiora.python.prescribed-dynamic-solid-affine",
        "release": "1.0.0",
        "dependencies": [
            {"name": "cpython", "release": "3.12"},
            {"name": "numpy", "release": "2.1.0"},
        ],
    }


def hello() -> dict[str, Any]:
    return {
        "type": "hello",
        "protocol": PROTOCOL,
        "contract": CONTRACT,
        "provider": provider(),
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


def changed_bind(document: dict[str, Any]) -> dict[str, Any]:
    """Mutate one named binding role without spelling transition-owned keys."""
    changed = json.loads(canonical(document))
    top = list(changed)
    inputs = changed[top[18]]
    output = changed[top[19]]
    zeros = "0" * 64
    foreign = "0" * 26
    if MODE == "stale-model-binding":
        changed[top[3]] = zeros
    elif MODE == "stale-semantic-revision-binding":
        changed[top[4]] = 2
    elif MODE == "stale-realization-binding":
        changed[top[5]] = zeros
    elif MODE == "stale-geometry-binding":
        changed[top[6]] = zeros
    elif MODE == "stale-correspondence-binding":
        changed[top[7]] = zeros
    elif MODE == "stale-mesh-binding":
        changed[top[8]] = zeros
    elif MODE == "stale-prior-state-binding":
        changed[top[9]] = zeros
    elif MODE == "stale-model-time-binding":
        changed[top[11]] = 0.125
    elif MODE == "stale-next-time-binding":
        changed[top[12]] = 0.5
    elif MODE == "stale-delta-time-binding":
        changed[top[13]] = 0.5
    elif MODE == "stale-solid-body-binding":
        changed[top[14]] = foreign
    elif MODE == "stale-boundary-binding":
        changed[top[15]] = foreign
    elif MODE == "stale-displacement-field-binding":
        inputs[0]["field_ulid"] = foreign
    elif MODE == "stale-velocity-field-binding":
        inputs[1]["field_ulid"] = foreign
    elif MODE == "stale-output-field-binding":
        output["field_ulid"] = foreign
    elif MODE == "wrong-displacement-role-binding":
        inputs[0]["role"] = "foreign-displacement"
    elif MODE == "wrong-velocity-role-binding":
        inputs[1]["role"] = "foreign-velocity"
    elif MODE == "wrong-output-role-binding":
        output["role"] = "foreign-output"
    elif MODE == "swapped-input-roles-binding":
        inputs[0]["role"], inputs[1]["role"] = inputs[1]["role"], inputs[0]["role"]
    elif MODE == "caller-order-vertices-binding":
        changed[top[16]] = [7, 5, 3, 1]
    elif MODE == "missing-vertex-binding":
        changed[top[16]] = [1, 3, 5]
    elif MODE == "duplicate-vertex-binding":
        changed[top[16]] = [1, 3, 3, 7]
    elif MODE == "reordered-vertices-binding":
        changed[top[16]] = [1, 5, 3, 7]
    elif MODE == "foreign-vertex-binding":
        changed[top[16]] = [1, 3, 5, 8]
    elif MODE == "missing-input-descriptor-binding":
        inputs.pop()
    elif MODE == "additional-input-descriptor-binding":
        inputs.append(dict(inputs[-1]))
    elif MODE == "wrong-unit-binding":
        inputs[0]["unit"] = "mm"
    elif MODE == "wrong-shape-binding":
        inputs[0]["value_shape"] = [2]
    elif MODE == "wrong-frame-binding":
        inputs[0]["frame"] = "material"
    elif MODE == "wrong-representation-binding":
        inputs[0]["representation"] = "discontinuous"
    elif MODE == "wrong-input-association-binding":
        inputs[0]["association"] = "cell"
    elif MODE == "wrong-coefficient-count-binding":
        inputs[0]["coefficient_count"] = 11
    elif MODE == "wrong-byte-length-binding":
        inputs[0]["byte_length"] = 88
    elif MODE == "wrong-coefficient-order-binding":
        changed[top[17]] = "caller-order"
    elif MODE == "stale-displacement-block-binding":
        inputs[0]["block_sha256"] = zeros
    elif MODE == "stale-velocity-block-binding":
        inputs[1]["block_sha256"] = zeros
    elif MODE == "changed-bound-provider-binding":
        changed[top[10]]["id"] = "eqiora.python.foreign"
    elif MODE == "changed-bound-dependency-binding":
        changed[top[10]]["dependencies"][1]["release"] = "2.1.1"
    return changed


def changed_hello() -> tuple[dict[str, Any], bytes | None]:
    document = hello()
    payload = None
    capability = document["capability"]
    if MODE == "malformed-json":
        payload = b"{"
    elif MODE == "malformed-utf8":
        payload = b"\xff"
    elif MODE == "duplicate-field":
        valid = canonical(document)
        payload = valid.replace(b'{"type":', b'{"type":"hello","type":', 1)
    elif MODE == "missing-field":
        del document["provider"]
    elif MODE == "unknown-field":
        document["unexpected"] = True
    elif MODE == "reordered-field":
        document = {key: document[key] for key in reversed(document)}
    elif MODE == "whitespace-drift":
        payload = b" " + canonical(document)
    elif MODE == "number-spelling-drift":
        payload = canonical(document).replace(
            b'"maximum_input_fields":2', b'"maximum_input_fields":2.0'
        )
    elif MODE == "excessive-nesting":
        document["unexpected"] = [[[[[[[[[True]]]]]]]]]
    elif MODE == "dependency-omission":
        document["provider"]["dependencies"].pop()
    elif MODE == "dependency-duplication":
        document["provider"]["dependencies"].append(
            dict(document["provider"]["dependencies"][0])
        )
    elif MODE == "dependency-reordering":
        document["provider"]["dependencies"].reverse()
    elif MODE == "wrong-protocol":
        document["protocol"] = "foreign"
    elif MODE == "wrong-contract":
        document["contract"] = "foreign"
    elif MODE == "wrong-determinism":
        capability["deterministic"] = False
    elif MODE == "wrong-statefulness":
        capability["stateful"] = True
    elif MODE == "wrong-scalar":
        capability["scalar"] = "f32"
    elif MODE == "wrong-target":
        capability["target"] = "accelerator"
    elif MODE == "wrong-association":
        capability["association"] = "cell"
    elif MODE == "wrong-layout":
        capability["layout"] = "component-major"
    elif MODE == "wrong-input-count":
        capability["maximum_input_fields"] = 3
    elif MODE == "wrong-output-count":
        capability["maximum_output_fields"] = 2
    elif MODE == "wrong-coefficient-limit":
        capability["maximum_coefficients_per_field"] = 11
    elif MODE == "wrong-aggregate-limit":
        capability["maximum_aggregate_bulk_bytes"] = 287
    elif MODE == "wrong-provider-id":
        document["provider"]["id"] = "eqiora.python.foreign"
    elif MODE == "wrong-provider-release":
        document["provider"]["release"] = "1.0.1"
    elif MODE == "wrong-python-policy":
        document["provider"]["dependencies"][0]["release"] = "3.13"
    elif MODE == "wrong-numpy-release":
        document["provider"]["dependencies"][1]["release"] = "2.1.1"
    return document, payload


def send_hello() -> None:
    document, payload = changed_hello()
    payload = canonical(document) if payload is None else payload
    if MODE == "wrong-magic":
        write_raw(b"NOPE" + framed(1, payload)[4:])
    elif MODE == "wrong-magic-cleanup-noise":
        write_raw(b"NOPE" + framed(1, payload)[4:])
        sys.stderr.write("cleanup exit after initiating bad magic\n")
        raise SystemExit(7)
    elif MODE == "wrong-frame-kind":
        write_raw(framed(2, payload))
    elif MODE == "nonzero-reserved":
        raw = bytearray(framed(1, payload))
        raw[5] = 1
        write_raw(bytes(raw))
    elif MODE == "big-endian-length":
        write_raw(
            b"EQP1" + bytes([1]) + b"\0\0\0" + struct.pack(">Q", len(payload)) + payload
        )
    elif MODE == "nonportable-length":
        write_raw(prefix(1, (1 << 64) - 1))
    elif MODE == "truncated-prefix":
        write_raw(prefix(1, len(payload))[:11])
    elif MODE == "truncated-control":
        write_raw(prefix(1, len(payload) + 1) + payload)
    elif MODE == "declared-length-mismatch":
        write_raw(prefix(1, len(payload) - 1) + payload)
    elif MODE == "control-budget-breach":
        write_raw(prefix(1, 4097) + b"x" * 4097)
    else:
        write_raw(framed(1, payload))


def error(phase: str) -> dict[str, Any]:
    return {
        "type": "error",
        "phase": phase,
        "code": "provider.rejected",
        "message": "hostile provider rejection",
    }


def placeholder(kind: str) -> dict[str, Any]:
    zeros = "0" * 64
    if kind == "bound":
        return {"type": "bound", "binding_sha256": zeros}
    return {
        "type": "candidate",
        "request_sha256": zeros,
        "candidate_sha256": zeros,
        "byte_length": 96,
    }


def mutate_candidate(payload: bytes) -> bytes:
    values = list(struct.unpack("<12d", payload))
    if MODE == "negative-zero":
        values[1] = -0.0
    elif MODE == "nan":
        values[0] = math.nan
    elif MODE == "infinity":
        values[0] = math.inf
    elif MODE == "candidate-as-increment":
        values[0::3] = [0.005] * 4
    elif MODE == "stale-prior-velocity":
        values[0::3] = [0.01] * 4
    elif MODE == "changed-time-step":
        values[0::3] = [0.02] * 4
    elif MODE == "ignored-input":
        values[0::3] = [0.25 * 0.02] * 4
    elif MODE == "wrong-candidate-bits":
        bits = struct.unpack("<Q", struct.pack("<d", values[0]))[0] + 1
        values[0] = struct.unpack("<d", struct.pack("<Q", bits))[0]
    return struct.pack("<12d", *values)


def main() -> None:
    if MODE == "stderr-overflow":
        sys.stderr.buffer.write(b"x" * 4097)
        sys.stderr.buffer.flush()
    if MODE == "exit-before-hello":
        return
    if MODE == "timeout-before-hello":
        time.sleep(10)
        return
    if MODE == "timeout-late-cleanup-noise":
        sys.stderr.write("bounded noise before structural timeout\n")
        sys.stderr.flush()
        time.sleep(10)
        return
    if MODE == "late-response-after-cancellation":
        time.sleep(1)
        send_hello()
        sys.stderr.write("late response after cancellation\n")
        raise SystemExit(7)
    if MODE == "cancel-before-hello":
        time.sleep(2)
        return

    if MODE == "bound-before-bind":
        document, raw = changed_hello()
        hello_payload = canonical(document) if raw is None else raw
        write_raw(framed(1, hello_payload) + framed(1, canonical(placeholder("bound"))))
    else:
        send_hello()
    if MODE == "close-input-after-hello":
        IN.close()
        time.sleep(10)
        return
    if MODE in {
        "wrong-magic",
        "wrong-magic-cleanup-noise",
        "wrong-frame-kind",
        "nonzero-reserved",
        "big-endian-length",
        "nonportable-length",
        "truncated-prefix",
        "truncated-control",
        "declared-length-mismatch",
        "control-budget-breach",
        "malformed-json",
        "malformed-utf8",
        "duplicate-field",
        "missing-field",
        "unknown-field",
        "reordered-field",
        "whitespace-drift",
        "number-spelling-drift",
        "excessive-nesting",
        "dependency-omission",
        "dependency-duplication",
        "dependency-reordering",
        "wrong-protocol",
        "wrong-contract",
        "wrong-determinism",
        "wrong-statefulness",
        "wrong-scalar",
        "wrong-target",
        "wrong-association",
        "wrong-layout",
        "wrong-input-count",
        "wrong-output-count",
        "wrong-coefficient-limit",
        "wrong-aggregate-limit",
        "wrong-provider-id",
        "wrong-provider-release",
        "wrong-python-policy",
        "wrong-numpy-release",
    }:
        return

    bind, bind_payload = read_control("bind")
    binding = identity(BINDING_DOMAIN, bind_payload)
    if MODE == "error-bind":
        write_control(error("bind"))
        return
    if MODE == "exit-before-bound":
        return
    if MODE == "timeout-before-bound":
        time.sleep(10)
        return
    bound_identity = binding.hex()
    if MODE == "wrong-binding-identity":
        bound_identity = "0" * 64
    elif MODE in BIND_MUTATION_MODES:
        bound_identity = identity(BINDING_DOMAIN, canonical(changed_bind(bind))).hex()
    bound = {"type": "bound", "binding_sha256": bound_identity}
    write_control(bound)
    if MODE == "duplicate-bound":
        write_control(bound)
    elif MODE == "candidate-before-evaluate":
        write_control(placeholder("candidate"))

    evaluate, evaluate_payload = read_control("evaluate")
    request = identity(REQUEST_DOMAIN, evaluate_payload)
    if MODE == "error-evaluate":
        write_control(error("evaluate"))
        return
    displacement = read_frame(2)
    velocity = read_frame(2)
    if len(displacement) != 96 or len(velocity) != 96:
        raise AssertionError("expected two 96-byte input blocks")
    if MODE == "exit-before-candidate":
        return
    if MODE == "timeout-before-candidate":
        time.sleep(10)
        return
    if MODE == "cancel-before-candidate":
        time.sleep(2)
        return

    first = struct.unpack("<12d", displacement)
    second = struct.unpack("<12d", velocity)
    if MODE == "swapped-bulk-frames":
        first, second = second, first
    candidate_values = [
        left + 0.25 * right for left, right in zip(first, second, strict=True)
    ]
    candidate_bulk = struct.pack("<12d", *candidate_values)
    candidate_bulk = mutate_candidate(candidate_bulk)
    output = list(bind.values())[-1]
    candidate_identity = identity(
        CANDIDATE_DOMAIN,
        request,
        canonical(output),
        candidate_bulk,
    )
    request_text = request.hex()
    candidate_text = candidate_identity.hex()
    if MODE == "wrong-request-identity":
        request_text = "0" * 64
    if MODE == "wrong-candidate-identity":
        candidate_text = "0" * 64
    candidate_control = {
        "type": "candidate",
        "request_sha256": request_text,
        "candidate_sha256": candidate_text,
        "byte_length": 96,
    }
    write_control(candidate_control)
    if MODE == "report-before-candidate-bulk":
        write_control(
            {
                "type": "report",
                "request_sha256": request.hex(),
                "candidate_sha256": candidate_identity.hex(),
                "status": "success",
                "code": "provider.success",
                "message": "affine predictor completed",
            }
        )
    elif MODE == "truncated-bulk":
        write_raw(prefix(2, 96) + candidate_bulk[:-8])
        return
    elif MODE == "bulk-length-mismatch":
        write_raw(prefix(2, 95) + candidate_bulk)
    elif MODE == "bulk-budget-breach":
        write_bulk(candidate_bulk + b"\0")
    elif MODE == "wrong-bulk-kind":
        write_raw(framed(1, candidate_bulk))
    elif MODE == "wrong-endian-binary64":
        write_bulk(
            b"".join(
                struct.pack(">d", value)
                for value in struct.unpack("<12d", candidate_bulk)
            )
        )
    else:
        write_bulk(candidate_bulk)
    if MODE in {
        "report-before-candidate-bulk",
        "truncated-bulk",
        "bulk-length-mismatch",
        "bulk-budget-breach",
        "wrong-bulk-kind",
        "wrong-endian-binary64",
        "negative-zero",
        "nan",
        "infinity",
        "candidate-as-increment",
        "stale-prior-velocity",
        "changed-time-step",
        "ignored-input",
        "wrong-candidate-bits",
        "wrong-request-identity",
        "wrong-candidate-identity",
        "swapped-bulk-frames",
    }:
        return
    if MODE == "exit-before-report":
        return
    if MODE == "timeout-before-report":
        time.sleep(10)
        return

    report = {
        "type": "report",
        "request_sha256": request.hex(),
        "candidate_sha256": candidate_identity.hex(),
        "status": "success",
        "code": "provider.success",
        "message": "affine predictor completed",
    }
    if MODE == "wrong-success-code":
        report["code"] = "provider.other"
    elif MODE == "wrong-success-message":
        report["message"] = "different report"
    elif MODE == "wrong-report-request":
        report["request_sha256"] = "0" * 64
    elif MODE == "wrong-report-candidate":
        report["candidate_sha256"] = "0" * 64
    write_control(report)
    if MODE in {
        "wrong-success-code",
        "wrong-success-message",
        "wrong-report-request",
        "wrong-report-candidate",
    }:
        return
    if MODE == "duplicate-report" or MODE == "extra-response-before-close":
        write_control(report)

    if MODE == "cleanup-noise-after-local-failure":
        try:
            close, _ = read_control("close")
        except EOFError:
            sys.stderr.write("bounded cleanup stderr after local failure\n")
            raise SystemExit(7) from None
    else:
        close, _ = read_control("close")
    if MODE == "error-close":
        write_control(error("close"))
        return
    if MODE == "exit-before-closed":
        return
    if MODE == "timeout-before-closed":
        time.sleep(10)
        return
    closed = {
        "type": "closed",
        "request_sha256": request.hex(),
        "candidate_sha256": candidate_identity.hex(),
    }
    write_control(closed)
    if MODE == "duplicate-closed" or MODE == "response-after-close":
        write_control(closed)
    elif MODE == "extra-bytes-after-closed":
        write_raw(b"x")
    elif MODE == "nonzero-exit":
        raise SystemExit(7)
    elif MODE == "dirty-eof-delay":
        time.sleep(10)
    require_equal = close["outcome"] == "accepted"
    if not require_equal:
        raise AssertionError("Eqiora did not report accepted close")


if __name__ == "__main__":
    main()
