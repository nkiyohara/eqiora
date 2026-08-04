#!/usr/bin/env python3
"""Derive the frozen subprocess frames, occurrence, and two-output Run.

This route uses only the Python standard library, the accepted standalone
prescribed-solid artifacts, and the public protocol contract.  It never reads
or executes a provider or any successor production source.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import struct
from collections.abc import Iterable
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[4]
CASE = ROOT / "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d"
EXPECTED = CASE / "expected"
ACCEPTED = ROOT / "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected"

ENCODING = "eqiora.canonical-json/v1"
OCCURRENCE_SCHEMA = "eqiora.prescribed-dynamic-solid-provider-occurrence-envelope/v1"
PROTOCOL = "eqiora.external-boundary-provider-subprocess/v1"
CONTRACT = "eqiora.prescribed-dynamic-solid-state-boundary/v1"
INPUT_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-input-block/v1"
BINDING_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-binding/v1"
REQUEST_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-request/v1"
CANDIDATE_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-candidate/v1"
TRANSCRIPT_DOMAIN = "eqiora.prescribed-dynamic-solid-provider-transcript/v1"

VERTEX_INDICES = [1, 3, 5, 7]
COMPONENTS = 3
COEFFICIENT_COUNT = len(VERTEX_INDICES) * COMPONENTS
BULK_BYTES = COEFFICIENT_COUNT * 8
PREDICTOR_X_BITS = 0x3F8EB851EB851EB8

PROVIDER = {
    "id": "eqiora.python.prescribed-dynamic-solid-affine",
    "release": "1.0.0",
    "dependencies": [
        {"name": "cpython", "release": "3.12"},
        {"name": "numpy", "release": "2.1.0"},
    ],
}
OUTPUT_REPORT = {
    "status": "success",
    "code": "provider.success",
    "message": "affine predictor completed",
}


def canonical(document: Any) -> bytes:
    return json.dumps(
        document,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
    ).encode("utf-8")


def digest(domain: str, *parts: bytes) -> bytes:
    hasher = hashlib.sha256()
    hasher.update(domain.encode("utf-8"))
    hasher.update(b"\0")
    for index, part in enumerate(parts):
        if index:
            hasher.update(b"\0")
        hasher.update(part)
    return hasher.digest()


def load_fixture(name: str) -> tuple[dict[str, Any], bytes]:
    raw = (ACCEPTED / name).read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise AssertionError(f"{name} must carry one repository newline")
    document = json.loads(raw)
    encoded = canonical(document)
    if raw[:-1].replace(b"\n", b"") != encoded:
        raise AssertionError(f"{name} no longer preserves canonical member bytes")
    return document, encoded


def artifact_identity(document: dict[str, Any], encoded: bytes) -> bytes:
    schema = document["schema"]
    if "source_revision" in document:
        content = canonical(
            {key: value for key, value in document.items() if key != "source_revision"}
        )
    else:
        content = encoded
    return digest(schema, content)


def load_artifact(name: str) -> tuple[dict[str, Any], bytes, bytes]:
    document, encoded = load_fixture(name)
    return document, encoded, artifact_identity(document, encoded)


def require_equal(label: str, actual: Any, expected: Any) -> None:
    if actual != expected:
        raise AssertionError(f"{label}: derived {actual!r}, expected {expected!r}")


def projection_values(block: dict[str, Any]) -> list[float]:
    values = block["values"]
    require_equal("accepted block association", block["association"], "vertex")
    require_equal(
        "accepted block shape",
        block["component_shape"],
        {
            "kind": "vector",
            "components": COMPONENTS,
        },
    )
    require_equal("accepted block entity count", block["entity_count"], 9)
    return [
        values[COMPONENTS * vertex + component]
        for vertex in VERTEX_INDICES
        for component in range(COMPONENTS)
    ]


def bulk(values: Iterable[float]) -> bytes:
    encoded = b"".join(struct.pack("<d", value) for value in values)
    require_equal("bulk byte length", len(encoded), BULK_BYTES)
    return encoded


def input_header(
    *,
    model_identity: str,
    realization_identity: str,
    prior_state_identity: str,
    boundary: str,
    field: str,
    role: str,
    unit: str,
) -> dict[str, Any]:
    return {
        "model_sha256": model_identity,
        "realization_sha256": realization_identity,
        "prior_state_sha256": prior_state_identity,
        "model_time_s": 0.0,
        "boundary_ulid": boundary,
        "field_ulid": field,
        "role": role,
        "unit": unit,
        "value_shape": [COMPONENTS],
        "frame": "spatial-cartesian",
        "representation": "continuous-lagrange-p1-trace",
        "association": "vertex",
        "vertex_indices": VERTEX_INDICES,
        "coefficient_count": COEFFICIENT_COUNT,
        "byte_length": BULK_BYTES,
    }


def input_descriptor(header: dict[str, Any], identity: bytes) -> dict[str, Any]:
    return {
        "role": header["role"],
        "field_ulid": header["field_ulid"],
        "unit": header["unit"],
        "value_shape": header["value_shape"],
        "frame": header["frame"],
        "representation": header["representation"],
        "association": header["association"],
        "coefficient_count": header["coefficient_count"],
        "byte_length": header["byte_length"],
        "block_sha256": identity.hex(),
    }


def output_header(field: str) -> dict[str, Any]:
    return {
        "role": "next-total-displacement",
        "field_ulid": field,
        "unit": "m",
        "value_shape": [COMPONENTS],
        "frame": "spatial-cartesian",
        "representation": "continuous-lagrange-p1-trace",
        "association": "vertex",
        "convention": "total-reference-configuration",
        "coefficient_count": COEFFICIENT_COUNT,
        "byte_length": BULK_BYTES,
    }


def control_frame(document: dict[str, Any]) -> tuple[bytes, bytes]:
    payload = canonical(document)
    if len(payload) > 4096:
        raise AssertionError("control payload exceeds its frozen bound")
    return frame(1, payload), payload


def frame(kind: int, payload: bytes) -> bytes:
    prefix = b"EQP1" + bytes([kind]) + b"\0\0\0" + struct.pack("<Q", len(payload))
    require_equal("frame prefix length", len(prefix), 16)
    return prefix + payload


def build_documents() -> dict[str, bytes]:
    model, _, current_identity = load_artifact("model.json")
    geometry, _, geometry_digest = load_artifact("geometry-identity.json")
    mesh, _, mesh_digest = load_artifact("mesh.json")
    correspondence, _, correspondence_digest = load_artifact("correspondence.json")
    realization, _, realization_digest = load_artifact("realization.json")
    prior_state, _, prior_state_digest = load_artifact("prior-state.json")
    accepted_state, _, accepted_state_digest = load_artifact("accepted-state.json")
    direct_run, _, direct_run_digest = load_artifact("run.json")
    displacement_block, _, displacement_block_digest = load_artifact(
        "prior-displacement-block.json"
    )
    velocity_block, _, velocity_block_digest = load_artifact(
        "prior-velocity-block.json"
    )
    prior_displacement_snapshot, _, prior_displacement_snapshot_identity = (
        load_artifact("prior-displacement-snapshot.json")
    )
    prior_velocity_snapshot, _, prior_velocity_snapshot_identity = load_artifact(
        "prior-velocity-snapshot.json"
    )
    accepted_displacement_block, _, accepted_displacement_block_identity = (
        load_artifact("accepted-displacement-block.json")
    )
    accepted_velocity_block, _, accepted_velocity_block_identity = load_artifact(
        "accepted-velocity-block.json"
    )
    accepted_displacement_snapshot, _, accepted_displacement_snapshot_identity = (
        load_artifact("accepted-displacement-snapshot.json")
    )
    accepted_velocity_snapshot, _, accepted_velocity_snapshot_identity = load_artifact(
        "accepted-velocity-snapshot.json"
    )

    identities = {
        "model": current_identity.hex(),
        "geometry": geometry_digest.hex(),
        "mesh": mesh_digest.hex(),
        "correspondence": correspondence_digest.hex(),
        "realization": realization_digest.hex(),
        "prior_state": prior_state_digest.hex(),
        "accepted_state": accepted_state_digest.hex(),
    }
    for label, document in [
        ("geometry", geometry),
        ("realization", realization),
        ("prior State", prior_state),
        ("accepted State", accepted_state),
        ("direct Run", direct_run),
    ]:
        require_equal(
            f"{label} current lineage",
            document["model_sha256"],
            identities["model"],
        )
    require_equal(
        "realization Geometry lineage",
        realization["geometry_sha256"],
        identities["geometry"],
    )
    require_equal(
        "realization correspondence lineage",
        realization["correspondence_sha256"],
        identities["correspondence"],
    )
    require_equal(
        "realization Mesh lineage",
        realization["spatial"]["discretization"]["mesh"]["artifact_sha256"],
        identities["mesh"],
    )
    require_equal(
        "correspondence Geometry lineage",
        correspondence["geometry_sha256"],
        identities["geometry"],
    )
    require_equal(
        "correspondence Mesh lineage",
        correspondence["mesh_sha256"],
        identities["mesh"],
    )
    require_equal(
        "prior coordinate", prior_state["accepted"], {"step": 0, "time_s": 0.0}
    )
    require_equal(
        "accepted coordinate",
        accepted_state["accepted"],
        {"step": 1, "time_s": 0.25},
    )
    require_equal(
        "direct Run remains singleton accepted State",
        direct_run["output_sha256"],
        [identities["accepted_state"]],
    )
    require_equal(
        "direct Run digest is nonempty",
        len(direct_run_digest),
        hashlib.sha256().digest_size,
    )

    spatial = realization["spatial"]
    displacement_field = spatial["displacement_field_ulid"]
    velocity_field = spatial["velocity_field_ulid"]
    boundary = spatial["driven_boundary_ulid"]
    solid = spatial["solid_domain_ulid"]
    require_equal("semantic revision", realization["semantic_revision"], 1)
    require_equal("time method", realization["time"]["method"], "backward-euler")
    require_equal("time step", realization["time"]["duration_s"], 0.25)
    require_equal(
        "driven vertex order",
        [entry["vertex_index"] for entry in realization["driven_total_displacement"]],
        VERTEX_INDICES,
    )

    def validate_snapshot(
        label: str,
        snapshot: dict[str, Any],
        field: str,
        block_identity: bytes,
    ) -> None:
        for edge, expected in [
            ("model_sha256", identities["model"]),
            ("realization_sha256", identities["realization"]),
            ("geometry_sha256", identities["geometry"]),
            ("correspondence_sha256", identities["correspondence"]),
            ("mesh_sha256", identities["mesh"]),
        ]:
            require_equal(f"{label} {edge}", snapshot[edge], expected)
        require_equal(f"{label} Field", snapshot["field_ulid"], field)
        require_equal(f"{label} support", snapshot["support_domain_ulid"], solid)
        require_equal(
            f"{label} representation",
            snapshot["representation"],
            {
                "scalar": "f64",
                "ordering": "canonical-mesh-entity-major",
                "blocks": [
                    {
                        "association": "vertex",
                        "discrete_field_sha256": block_identity.hex(),
                    }
                ],
            },
        )

    validate_snapshot(
        "prior displacement snapshot",
        prior_displacement_snapshot,
        displacement_field,
        displacement_block_digest,
    )
    validate_snapshot(
        "prior velocity snapshot",
        prior_velocity_snapshot,
        velocity_field,
        velocity_block_digest,
    )
    validate_snapshot(
        "accepted displacement snapshot",
        accepted_displacement_snapshot,
        displacement_field,
        accepted_displacement_block_identity,
    )
    validate_snapshot(
        "accepted velocity snapshot",
        accepted_velocity_snapshot,
        velocity_field,
        accepted_velocity_block_identity,
    )

    def state_fields(
        displacement_snapshot_identity: bytes,
        velocity_snapshot_identity: bytes,
    ) -> list[dict[str, str]]:
        fields = [
            {
                "support_domain_ulid": solid,
                "field_ulid": displacement_field,
                "snapshot_sha256": displacement_snapshot_identity.hex(),
            },
            {
                "support_domain_ulid": solid,
                "field_ulid": velocity_field,
                "snapshot_sha256": velocity_snapshot_identity.hex(),
            },
        ]
        return sorted(fields, key=lambda entry: entry["field_ulid"])

    require_equal(
        "prior State exact Field observations",
        prior_state["fields"],
        state_fields(
            prior_displacement_snapshot_identity, prior_velocity_snapshot_identity
        ),
    )
    require_equal(
        "accepted State exact Field observations",
        accepted_state["fields"],
        state_fields(
            accepted_displacement_snapshot_identity,
            accepted_velocity_snapshot_identity,
        ),
    )

    displacement_values = projection_values(displacement_block)
    velocity_values = projection_values(velocity_block)
    expected_displacement = [0.01, 0.0, 0.0] * len(VERTEX_INDICES)
    expected_velocity = [0.02, 0.0, 0.0] * len(VERTEX_INDICES)
    require_equal("displacement trace bits", displacement_values, expected_displacement)
    require_equal("velocity trace bits", velocity_values, expected_velocity)
    candidate_values = [
        displacement + 0.25 * velocity
        for displacement, velocity in zip(
            displacement_values, velocity_values, strict=True
        )
    ]
    candidate = bulk(candidate_values)
    require_equal(
        "affine predictor x bits",
        struct.unpack("<Q", candidate[:8])[0],
        PREDICTOR_X_BITS,
    )
    require_equal(
        "affine predictor component bits",
        [
            struct.unpack("<Q", candidate[offset : offset + 8])[0]
            for offset in range(0, len(candidate), 8)
        ],
        [PREDICTOR_X_BITS, 0, 0] * len(VERTEX_INDICES),
    )

    displacement_bulk = bulk(displacement_values)
    velocity_bulk = bulk(velocity_values)
    displacement_header = input_header(
        model_identity=identities["model"],
        realization_identity=identities["realization"],
        prior_state_identity=identities["prior_state"],
        boundary=boundary,
        field=displacement_field,
        role="prior-displacement-trace",
        unit="m",
    )
    velocity_header = input_header(
        model_identity=identities["model"],
        realization_identity=identities["realization"],
        prior_state_identity=identities["prior_state"],
        boundary=boundary,
        field=velocity_field,
        role="prior-velocity-trace",
        unit="m/s",
    )
    displacement_identity = digest(
        INPUT_DOMAIN, canonical(displacement_header), displacement_bulk
    )
    velocity_identity = digest(INPUT_DOMAIN, canonical(velocity_header), velocity_bulk)
    inputs = [
        input_descriptor(displacement_header, displacement_identity),
        input_descriptor(velocity_header, velocity_identity),
    ]
    output = output_header(displacement_field)

    hello = {
        "type": "hello",
        "protocol": PROTOCOL,
        "contract": CONTRACT,
        "provider": PROVIDER,
        "capability": {
            "deterministic": True,
            "stateful": False,
            "scalar": "f64",
            "target": "host-cpu",
            "association": "vertex",
            "layout": "entity-major-spatial-cartesian",
            "maximum_input_fields": 2,
            "maximum_output_fields": 1,
            "maximum_coefficients_per_field": COEFFICIENT_COUNT,
            "maximum_aggregate_bulk_bytes": 3 * BULK_BYTES,
        },
    }
    bind = {
        "type": "bind",
        "protocol": PROTOCOL,
        "contract": CONTRACT,
        "model_sha256": identities["model"],
        "semantic_revision": realization["semantic_revision"],
        "realization_sha256": identities["realization"],
        "geometry_sha256": identities["geometry"],
        "correspondence_sha256": identities["correspondence"],
        "mesh_sha256": identities["mesh"],
        "prior_state_sha256": identities["prior_state"],
        "provider": PROVIDER,
        "model_time_s": 0.0,
        "next_time_s": 0.25,
        "delta_time_s": 0.25,
        "solid_domain_ulid": solid,
        "boundary_ulid": boundary,
        "vertex_indices": VERTEX_INDICES,
        "coefficient_order": "vertex-index-ascending-component-x-y-z",
        "inputs": inputs,
        "output": output,
    }
    bind_payload = canonical(bind)
    binding_identity = digest(BINDING_DOMAIN, bind_payload)
    bound = {"type": "bound", "binding_sha256": binding_identity.hex()}
    evaluate = {"type": "evaluate", "binding_sha256": binding_identity.hex()}
    evaluate_payload = canonical(evaluate)
    request_identity = digest(REQUEST_DOMAIN, evaluate_payload)
    candidate_identity = digest(
        CANDIDATE_DOMAIN,
        request_identity,
        canonical(output),
        candidate,
    )
    candidate_control = {
        "type": "candidate",
        "request_sha256": request_identity.hex(),
        "candidate_sha256": candidate_identity.hex(),
        "byte_length": BULK_BYTES,
    }
    report = {
        "type": "report",
        "request_sha256": request_identity.hex(),
        "candidate_sha256": candidate_identity.hex(),
        **OUTPUT_REPORT,
    }
    close = {
        "type": "close",
        "request_sha256": request_identity.hex(),
        "candidate_sha256": candidate_identity.hex(),
        "outcome": "accepted",
    }
    closed = {
        "type": "closed",
        "request_sha256": request_identity.hex(),
        "candidate_sha256": candidate_identity.hex(),
    }

    transcript_parts = []
    control_payloads = []

    def append_control(direction: int, document: dict[str, Any]) -> None:
        framed, payload = control_frame(document)
        transcript_parts.append(bytes([direction]) + framed)
        control_payloads.append(payload)

    def append_bulk(direction: int, payload: bytes) -> None:
        transcript_parts.append(bytes([direction]) + frame(2, payload))

    append_control(1, hello)
    append_control(0, bind)
    append_control(1, bound)
    append_control(0, evaluate)
    append_bulk(0, displacement_bulk)
    append_bulk(0, velocity_bulk)
    append_control(1, candidate_control)
    append_bulk(1, candidate)
    append_control(1, report)
    append_control(0, close)
    append_control(1, closed)
    transcript = b"".join(transcript_parts)
    transcript_identity = digest(TRANSCRIPT_DOMAIN, transcript)
    require_equal("successful frame count", len(transcript_parts), 11)
    require_equal("successful control frame count", len(control_payloads), 8)
    require_equal("aggregate bulk bytes", 3 * BULK_BYTES, 288)
    if len(transcript) > 36864:
        raise AssertionError("successful transcript exceeds its frozen bound")

    occurrence = {
        "schema": OCCURRENCE_SCHEMA,
        "encoding": ENCODING,
        "model_sha256": identities["model"],
        "semantic_revision": 1,
        "realization_sha256": identities["realization"],
        "prior_state_sha256": identities["prior_state"],
        "contract": {
            "generation": 1,
            "approximation": "lagged-accepted-state",
            "statefulness": "stateless",
            "determinism": "required",
            "scalar": "ieee754-binary64",
            "target": "host-cpu",
        },
        "provider": PROVIDER,
        "adapter": {
            "id": "eqiora.subprocess.external-boundary-provider",
            "release": "0.1.0-alpha.1",
            "protocol": PROTOCOL,
        },
        "projection": {
            "geometry_sha256": identities["geometry"],
            "correspondence_sha256": identities["correspondence"],
            "mesh_sha256": identities["mesh"],
            "solid_domain_ulid": solid,
            "boundary_ulid": boundary,
            "model_time_s": 0.0,
            "next_time_s": 0.25,
            "delta_time_s": 0.25,
            "vertex_indices": VERTEX_INDICES,
            "coefficient_order": "vertex-index-ascending-component-x-y-z",
            "inputs": inputs,
            "output": output,
        },
        "request": {
            "binding_sha256": binding_identity.hex(),
            "request_sha256": request_identity.hex(),
        },
        "candidate": {
            "candidate_sha256": candidate_identity.hex(),
            "producer_report": OUTPUT_REPORT,
        },
        "transcript": {
            "transcript_sha256": transcript_identity.hex(),
            "frame_count": 11,
            "control_frame_count": 8,
            "bulk_frame_count": 3,
            "aggregate_bulk_bytes": 288,
        },
        "admission": {
            "status": "accepted",
            "accepted_generation": 1,
            "accepted_state_sha256": identities["accepted_state"],
        },
    }
    occurrence_bytes = canonical(occurrence)
    if len(occurrence_bytes) > 8192:
        raise AssertionError("occurrence exceeds its intrinsic artifact bound")
    occurrence_identity = digest(OCCURRENCE_SCHEMA, occurrence_bytes)

    run = dict(direct_run)
    run["output_sha256"] = sorted(
        [identities["accepted_state"], occurrence_identity.hex()]
    )
    run_bytes = canonical(run)
    require_equal("Run output count", len(run["output_sha256"]), 2)

    require_equal(
        "occurrence transition identity count",
        len(re.findall(rb"[0-9a-f]{64}", occurrence_bytes)),
        13,
    )
    require_equal(
        "Run transition identity count",
        len(re.findall(rb"[0-9a-f]{64}", run_bytes)),
        4,
    )
    for label, binary in [("candidate", candidate), ("transcript", transcript)]:
        try:
            binary.decode("utf-8")
        except UnicodeDecodeError:
            pass
        else:
            raise AssertionError(f"{label} fixture must remain non-UTF-8 binary")

    print(f"predictor x bits: 0x{PREDICTOR_X_BITS:016x}")
    print(f"binding identity: {binding_identity.hex()}")
    print(f"request identity: {request_identity.hex()}")
    print(f"candidate identity: {candidate_identity.hex()}")
    print(f"transcript identity: {transcript_identity.hex()}")
    print(f"occurrence identity: {occurrence_identity.hex()}")
    print(f"transcript bytes: {len(transcript)}")

    return {
        "candidate.bin": candidate,
        "transcript.bin": transcript,
        "provider-occurrence.json": occurrence_bytes + b"\n",
        "run.json": run_bytes + b"\n",
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--write",
        action="store_true",
        help="materialize the initially precommitted oracle fixtures",
    )
    arguments = parser.parse_args()
    documents = build_documents()
    failures = []
    for name, expected in documents.items():
        path = EXPECTED / name
        if arguments.write:
            path.write_bytes(expected)
        elif not path.exists() or path.read_bytes() != expected:
            failures.append(name)
        else:
            print(f"verified {name}: {len(expected)} bytes")
    if failures:
        raise SystemExit("fixture drift: " + ", ".join(failures))


if __name__ == "__main__":
    main()
