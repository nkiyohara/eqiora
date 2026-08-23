#!/usr/bin/env python3
"""Derive the exact standalone prescribed-solid artifact graph."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
from itertools import combinations
from pathlib import Path
from typing import Any, Iterable


CASE = Path(__file__).resolve().parents[1]
EXPECTED = CASE / "expected"
MODEL_PATH = EXPECTED / "model.json"
MODEL_SCHEMA = "eqiora.model-envelope/v8"
ENCODING = "eqiora.canonical-json/v1"

REALIZATION_SCHEMA = "eqiora.prescribed-dynamic-solid-realization-envelope/v1"
GEOMETRY_SCHEMA = "eqiora.geometry-identity-envelope/v1"
MESH_SCHEMA = "eqiora.simplicial-mesh-envelope/v1"
CORRESPONDENCE_SCHEMA = "eqiora.geometry-mesh-correspondence-envelope/v1"
BLOCK_SCHEMA = "eqiora.discrete-field-envelope/v1"
SNAPSHOT_SCHEMA = "eqiora.field-snapshot-envelope/v1"
STATE_SCHEMA = "eqiora.spatial-state-envelope/v1"
RUN_SCHEMA = "eqiora.run-manifest/v2"
SIGNAL_FIXTURES = {
    "geometry-identity.json",
    "realization.json",
    "prior-displacement-snapshot.json",
    "prior-velocity-snapshot.json",
    "accepted-displacement-snapshot.json",
    "accepted-velocity-snapshot.json",
    "prior-state.json",
    "accepted-state.json",
    "run.json",
}

VERTICES = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
    [0.5, 0.5, 0.5],
]

CELLS = [
    [8, 0, 6, 2],
    [8, 0, 4, 6],
    [8, 1, 7, 5],
    [8, 1, 3, 7],
    [8, 0, 5, 4],
    [8, 0, 1, 5],
    [8, 2, 7, 3],
    [8, 2, 6, 7],
    [8, 0, 3, 1],
    [8, 0, 2, 3],
    [8, 4, 7, 6],
    [8, 4, 5, 7],
]

ZERO = 0x0000000000000000
PRIOR_DISPLACEMENT_BITS = [
    [ZERO, ZERO, ZERO],
    [0x3F847AE147AE147B, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F847AE147AE147B, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F847AE147AE147B, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F847AE147AE147B, ZERO, ZERO],
    [0x3F747AE147AE147B, ZERO, ZERO],
]
PRIOR_VELOCITY_BITS = [
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147B, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147B, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147B, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147B, ZERO, ZERO],
    [0x3F847AE147AE147B, ZERO, ZERO],
]
ACCEPTED_DISPLACEMENT_BITS = [
    [ZERO, ZERO, ZERO],
    [0x3F8EB851EB851EB8, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F8EB851EB851EB8, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F8EB851EB851EB8, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F8EB851EB851EB8, ZERO, ZERO],
    [0x3F7EB851EB851EB9, ZERO, ZERO],
]
ACCEPTED_VELOCITY_BITS = [
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147A, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147A, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147A, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3F947AE147AE147A, ZERO, ZERO],
    [0x3F847AE147AE147C, ZERO, ZERO],
]

DIMENSION_LENGTH = {
    "mass": 0,
    "length": 1,
    "time": 0,
    "current": 0,
    "temperature": 0,
    "amount": 0,
    "luminous_intensity": 0,
}
DIMENSION_VELOCITY = {**DIMENSION_LENGTH, "time": -1}


def canonical(document: Any) -> bytes:
    return json.dumps(
        document,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
    ).encode("utf-8")


def fixture_bytes(encoded: bytes) -> bytes:
    """Keep canonical member bytes while isolating each top-level member by line."""
    if not encoded.startswith(b"{") or not encoded.endswith(b"}"):
        raise AssertionError("an artifact fixture must be one canonical JSON object")
    output = bytearray()
    depth = 0
    quoted = False
    escaped = False
    for byte in encoded:
        output.append(byte)
        if quoted:
            if escaped:
                escaped = False
            elif byte == ord("\\"):
                escaped = True
            elif byte == ord('"'):
                quoted = False
            continue
        if byte == ord('"'):
            quoted = True
        elif byte in (ord("{"), ord("[")):
            depth += 1
        elif byte in (ord("}"), ord("]")):
            depth -= 1
        elif byte == ord(",") and depth == 1:
            output.append(ord("\n"))
    if quoted or escaped or depth != 0:
        raise AssertionError("canonical fixture scanning lost JSON structure")
    return bytes(output) + b"\n"


def artifact_digest(schema: str, encoded: bytes) -> str:
    return hashlib.sha256(schema.encode("utf-8") + b"\0" + encoded).hexdigest()


def model_content(encoded: bytes) -> bytes:
    marker = b'"source_revision":'
    start = encoded.find(marker)
    if start < 0 or encoded.find(marker, start + 1) >= 0:
        raise AssertionError("the canonical Model must carry one source revision")
    end = start + len(marker)
    while encoded[end : end + 1].isdigit():
        end += 1
    if encoded[end : end + 1] != b",":
        raise AssertionError("the source revision must precede another member")
    return encoded[:start] + encoded[end + 1 :]


def bits(value: int) -> float:
    return struct.unpack(">d", struct.pack(">Q", value))[0]


def values(sequence: Iterable[Iterable[int]]) -> list[list[float]]:
    return [[bits(component) for component in vector] for vector in sequence]


def exactly_one(items: Iterable[Any], label: str) -> Any:
    found = list(items)
    if len(found) != 1:
        raise AssertionError(f"expected one {label}, found {len(found)}")
    return found[0]


def node_ulid(node: dict[str, Any]) -> str:
    return node["id"]["ulid"]


def load_model() -> tuple[dict[str, Any], bytes]:
    raw = MODEL_PATH.read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise AssertionError("the frozen Model must have one repository newline")
    encoded = raw[:-1]
    document = json.loads(encoded)
    if canonical(document) != encoded:
        raise AssertionError("the frozen Model is not canonical compact JSON")
    if document["schema"] != MODEL_SCHEMA or document["encoding"] != ENCODING:
        raise AssertionError("the frozen Model schema or encoding changed")
    return document, encoded


def structural_roles(model: dict[str, Any]) -> dict[str, str]:
    nodes = model["nodes"]
    body = exactly_one(
        (
            node
            for node in nodes
            if node["id"]["kind"] == "domain"
            and node["definition"].get("kind") == "domain"
            and node["definition"]["domain"].get("kind") == "cartesian-box-sources"
            and [
                (
                    coordinate["lower"]["value"]["value"],
                    coordinate["upper"]["value"]["value"],
                )
                for coordinate in node["definition"]["domain"]["coordinates"]
            ]
            == [(0.0, 1.0)] * 3
        ),
        "exact unit-cube body Domain",
    )

    def boundary(axis: int, side: str) -> dict[str, Any]:
        return exactly_one(
            (
                node
                for node in nodes
                if node["id"]["kind"] == "domain"
                and node["definition"].get("kind") == "domain"
                and node["definition"]["domain"]
                == {"kind": "cartesian-boundary", "axis": axis, "side": side}
            ),
            f"Cartesian axis {axis} {side} boundary Domain",
        )

    def field(dimension: dict[str, int]) -> dict[str, Any]:
        return exactly_one(
            (
                node
                for node in nodes
                if node["id"]["kind"] == "field"
                and node["definition"].get("kind") == "shaped-field"
                and node["definition"].get("dimension") == dimension
                and node["definition"].get("shape") == [3]
                and node["definition"].get("frame") == "spatial-cartesian"
            ),
            f"vector Field with dimension {dimension}",
        )

    roles = {
        "body": node_ulid(body),
        "displacement": node_ulid(field(DIMENSION_LENGTH)),
        "velocity": node_ulid(field(DIMENSION_VELOCITY)),
        "fixed": node_ulid(boundary(0, "lower")),
        "driven": node_ulid(boundary(0, "upper")),
    }
    if len(set(roles.values())) != len(roles):
        raise AssertionError("the structurally derived roles are not distinct")
    roles["boundaries"] = {
        f"{axis}-{side}": node_ulid(boundary(axis, side))
        for axis in range(3)
        for side in ("lower", "upper")
    }
    return roles


def build_documents() -> dict[str, bytes]:
    model, model_bytes = load_model()
    roles = structural_roles(model)
    model_identity = artifact_digest(MODEL_SCHEMA, model_content(model_bytes))

    geometry = {
        "schema": GEOMETRY_SCHEMA,
        "encoding": ENCODING,
        "model_sha256": model_identity,
        "model_ulid": model["model_ulid"],
        "semantic_revision": model["source_revision"],
        "producer": "semantic-cartesian-v1",
        "length_unit": "metre",
        "tolerance_m": 1e-12,
        "bodies": [
            {
                "domain_ulid": roles["body"],
                "entity": {"dimension": 3, "index": 0},
                "bounds_m": [
                    {"lower_m": 0.0, "upper_m": 1.0},
                    {"lower_m": 0.0, "upper_m": 1.0},
                    {"lower_m": 0.0, "upper_m": 1.0},
                ],
                "boundaries": [
                    {
                        "domain_ulid": roles["boundaries"][f"{axis}-{side}"],
                        "entity": {"dimension": 2, "index": 2 * axis + side_index},
                        "parent_entity": {"dimension": 3, "index": 0},
                        "axis": axis,
                        "side": side,
                        "orientation": "parent-outward",
                    }
                    for axis in range(3)
                    for side_index, side in enumerate(("lower", "upper"))
                ],
            }
        ],
    }
    geometry_bytes = canonical(geometry)
    geometry_identity = artifact_digest(GEOMETRY_SCHEMA, geometry_bytes)

    mesh = {
        "schema": MESH_SCHEMA,
        "encoding": ENCODING,
        "topology": {"dimension": 3, "cell_family": "simplex"},
        "geometry": {"coordinate_scalar": "f64", "mapping": "affine"},
        "vertices": VERTICES,
        "cells": CELLS,
        "acceptance": {"minimum_mean_ratio": 0.1},
        "evidence": {
            "minimum_mean_ratio": bits(0x3FEAE0D94CBC98B9),
            "minimum_signed_measure_scale": 0.5,
        },
    }
    mesh_bytes = canonical(mesh)
    mesh_identity = artifact_digest(MESH_SCHEMA, mesh_bytes)

    facets = sorted(
        {
            tuple(sorted(face))
            for cell in CELLS
            for face in combinations(cell, 3)
        }
    )
    boundaries = []
    for axis in range(3):
        for side_index, side in enumerate(("lower", "upper")):
            coordinate = float(side_index)
            selected = [
                index
                for index, face in enumerate(facets)
                if all(VERTICES[vertex][axis] == coordinate for vertex in face)
            ]
            if len(selected) != 2:
                raise AssertionError("each cube side must contain two exact facets")
            boundaries.append(
                {
                    "domain_ulid": roles["boundaries"][f"{axis}-{side}"],
                    "parent_ulid": roles["body"],
                    "geometry_entity": {
                        "dimension": 2,
                        "index": 2 * axis + side_index,
                    },
                    "axis": axis,
                    "side": side,
                    "orientation": "parent-outward",
                    "facet_indices": selected,
                }
            )
    correspondence = {
        "schema": CORRESPONDENCE_SCHEMA,
        "encoding": ENCODING,
        "geometry_sha256": geometry_identity,
        "mesh_sha256": mesh_identity,
        "dimension": 3,
        "bodies": [
            {
                "domain_ulid": roles["body"],
                "geometry_entity": {"dimension": 3, "index": 0},
                "cell_indices": list(range(len(CELLS))),
            }
        ],
        "boundaries": boundaries,
    }
    correspondence_bytes = canonical(correspondence)
    correspondence_identity = artifact_digest(
        CORRESPONDENCE_SCHEMA, correspondence_bytes
    )

    driven = [
        {"vertex_index": vertex, "value_m": [bits(0x3F8EB851EB851EB8), 0.0, 0.0]}
        for vertex in (1, 3, 5, 7)
    ]
    realization = {
        "schema": REALIZATION_SCHEMA,
        "encoding": ENCODING,
        "model_sha256": model_identity,
        "model_ulid": model["model_ulid"],
        "semantic_revision": model["source_revision"],
        "source": {"kind": "explicit", "realization_revision": 1},
        "geometry_sha256": geometry_identity,
        "correspondence_sha256": correspondence_identity,
        "spatial": {
            "spatial_dimension": 3,
            "scalar": "f64",
            "vector_layout": "replicated",
            "solid_domain_ulid": roles["body"],
            "displacement_field_ulid": roles["displacement"],
            "velocity_field_ulid": roles["velocity"],
            "fixed_boundary_ulid": roles["fixed"],
            "driven_boundary_ulid": roles["driven"],
            "space": {"kind": "continuous-lagrange", "order": 1},
            "discretization": {
                "method": "continuous-galerkin",
                "mesh": {
                    "kind": "imported-simplicial",
                    "artifact_sha256": mesh_identity,
                },
                "quadrature": "exact-affine-p1-tetrahedron-mass-and-stiffness",
            },
        },
        "time": {"method": "backward-euler", "duration_s": 0.25},
        "driven_total_displacement": driven,
        "solver": {
            "operator_properties": "symmetric-positive-definite",
            "algorithm": "conjugate-gradient",
            "preconditioner": "identity",
            "reduction": "reproducible",
            "relative_tolerance": 1e-13,
            "absolute_tolerance": 1e-15,
            "maximum_iterations": 500,
        },
        "placement": {
            "target": {"kind": "host-cpu", "threads": 1},
            "schedule": {"kind": "offline"},
            "assembly_execution": "host-serial",
            "solve_execution": "host-serial",
            "verification_execution": "host-serial",
            "layout_artifacts": {"kind": "replicated"},
        },
    }
    realization_bytes = canonical(realization)
    realization_identity = artifact_digest(REALIZATION_SCHEMA, realization_bytes)

    block_values = {
        "prior-displacement": values(PRIOR_DISPLACEMENT_BITS),
        "prior-velocity": values(PRIOR_VELOCITY_BITS),
        "accepted-displacement": values(ACCEPTED_DISPLACEMENT_BITS),
        "accepted-velocity": values(ACCEPTED_VELOCITY_BITS),
    }
    blocks: dict[str, tuple[bytes, str]] = {}
    for name, vectors in block_values.items():
        document = {
            "schema": BLOCK_SCHEMA,
            "encoding": ENCODING,
            "mesh_sha256": mesh_identity,
            "association": "vertex",
            "component_shape": {"kind": "vector", "components": 3},
            "entity_count": len(VERTICES),
            "values": [component for vector in vectors for component in vector],
        }
        encoded = canonical(document)
        blocks[name] = (encoded, artifact_digest(BLOCK_SCHEMA, encoded))

    snapshot_specs = {
        "prior-displacement": (roles["displacement"], DIMENSION_LENGTH),
        "prior-velocity": (roles["velocity"], DIMENSION_VELOCITY),
        "accepted-displacement": (roles["displacement"], DIMENSION_LENGTH),
        "accepted-velocity": (roles["velocity"], DIMENSION_VELOCITY),
    }
    snapshots: dict[str, tuple[bytes, str]] = {}
    for name, (field, dimension) in snapshot_specs.items():
        document = {
            "schema": SNAPSHOT_SCHEMA,
            "encoding": ENCODING,
            "model_sha256": model_identity,
            "semantic_revision": model["source_revision"],
            "realization_sha256": realization_identity,
            "geometry_sha256": geometry_identity,
            "correspondence_sha256": correspondence_identity,
            "mesh_sha256": mesh_identity,
            "field_ulid": field,
            "support_domain_ulid": roles["body"],
            "physical": {
                "unit_system": "coherent-si",
                "dimension": dimension,
                "value_shape": {"extents": [3]},
                "frame": "spatial-cartesian",
            },
            "representation": {
                "scalar": "f64",
                "ordering": "canonical-mesh-entity-major",
                "blocks": [
                    {
                        "association": "vertex",
                        "discrete_field_sha256": blocks[name][1],
                    }
                ],
            },
        }
        encoded = canonical(document)
        snapshots[name] = (encoded, artifact_digest(SNAPSHOT_SCHEMA, encoded))

    def state(name: str, step: int, time_s: float) -> tuple[bytes, str]:
        entries = [
            {
                "support_domain_ulid": roles["body"],
                "field_ulid": field,
                "snapshot_sha256": snapshots[f"{name}-{role}"][1],
            }
            for role, field in (
                ("displacement", roles["displacement"]),
                ("velocity", roles["velocity"]),
            )
        ]
        entries.sort(key=lambda entry: entry["field_ulid"])
        document = {
            "schema": STATE_SCHEMA,
            "encoding": ENCODING,
            "model_sha256": model_identity,
            "semantic_revision": model["source_revision"],
            "realization_sha256": realization_identity,
            "geometry_sha256": geometry_identity,
            "correspondence_sha256": correspondence_identity,
            "mesh_sha256": mesh_identity,
            "accepted": {"step": step, "time_s": time_s},
            "fields": entries,
        }
        encoded = canonical(document)
        return encoded, artifact_digest(STATE_SCHEMA, encoded)

    prior_state = state("prior", 0, 0.0)
    accepted_state = state("accepted", 1, 0.25)
    run = {
        "schema": RUN_SCHEMA,
        "encoding": ENCODING,
        "model_sha256": model_identity,
        "semantic_revision": model["source_revision"],
        "realization_sha256": realization_identity,
        "execution": {
            "adapter": "eqiora.host.serial",
            "adapter_version": "0.1.0-alpha.3",
            "solver_backend": "eqiora.reference",
            "solver_backend_version": "0.1.0-alpha.3",
            "libraries": {},
            "topology": {"kind": "host", "workers": 1},
            "reduction": "reproducible",
        },
        "output_sha256": [accepted_state[1]],
    }
    run_bytes = canonical(run)

    scientific_center = 0.0075
    persisted_center = values(ACCEPTED_DISPLACEMENT_BITS)[8][0]
    if persisted_center.hex() != bits(0x3F7EB851EB851EB9).hex():
        raise AssertionError("the persisted center bit pattern changed")
    if abs(persisted_center - scientific_center) > 1e-13:
        raise AssertionError("the live center no longer satisfies the accepted tolerance")

    return {
        "geometry-identity.json": geometry_bytes,
        "mesh.json": mesh_bytes,
        "correspondence.json": correspondence_bytes,
        "realization.json": realization_bytes,
        **{f"{name}-block.json": encoded for name, (encoded, _) in blocks.items()},
        **{
            f"{name}-snapshot.json": encoded
            for name, (encoded, _) in snapshots.items()
        },
        "prior-state.json": prior_state[0],
        "accepted-state.json": accepted_state[0],
        "run.json": run_bytes,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    arguments = parser.parse_args()
    documents = build_documents()
    failures = []
    for name, encoded in documents.items():
        path = EXPECTED / name
        expected = fixture_bytes(encoded) if name in SIGNAL_FIXTURES else encoded + b"\n"
        if arguments.write:
            path.write_bytes(expected)
        elif not path.exists() or path.read_bytes() != expected:
            failures.append(name)
    if failures:
        raise SystemExit("fixture drift: " + ", ".join(failures))
    for name, encoded in sorted(documents.items()):
        schema = json.loads(encoded)["schema"]
        print(f"{name}: {len(encoded)} bytes {artifact_digest(schema, encoded)}")


if __name__ == "__main__":
    main()
