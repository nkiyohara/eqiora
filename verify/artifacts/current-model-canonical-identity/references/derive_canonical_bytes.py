#!/usr/bin/env python3
"""Independent derivation of the current Model and Transaction canonical bytes.

This is the route record for `artifacts.current-model-canonical-identity`. It
re-encodes the frozen public fixture from the wire contract alone and hashes the
result with `hashlib`. It never imports, links, or executes the Rust producer.

Run from the repository root:

    python3 verify/artifacts/current-model-canonical-identity/references/derive_canonical_bytes.py

It exits non-zero if any committed literal disagrees with the derivation.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[4]
EXPECTED = ROOT / "verify/artifacts/current-model-canonical-identity/expected"

CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
FIXED_TIMESTAMP_MILLIS = 1_700_000_008_000

MODEL_SCHEMA = "eqiora.model-envelope/v8"
TRANSACTION_SCHEMA = "eqiora.model-transaction-envelope/v8"
CANONICAL_ENCODING = "eqiora.canonical-json/v1"


def ulid(timestamp_ms: int, random: int) -> str:
    """`Ulid::from_parts`: 48 timestamp bits above 80 randomness bits, base32."""
    value = ((timestamp_ms & ((1 << 48) - 1)) << 80) | (random & ((1 << 80) - 1))
    return "".join(CROCKFORD[(value >> (5 * (25 - i))) & 0x1F] for i in range(26))


PARAMETER = ulid(FIXED_TIMESTAMP_MILLIS, 1)
BODY = ulid(FIXED_TIMESTAMP_MILLIS, 2)
RETAIN = ulid(FIXED_TIMESTAMP_MILLIS, 3)
ACTIVATION = ulid(FIXED_TIMESTAMP_MILLIS, 4)
MODEL = ulid(FIXED_TIMESTAMP_MILLIS, 5)


def wire_id(kind: str, value: str) -> dict:
    return {"kind": kind, "ulid": value}


def dimension(**exponents: int) -> dict:
    base = dict.fromkeys(
        (
            "mass",
            "length",
            "time",
            "current",
            "temperature",
            "amount",
            "luminous_intensity",
        ),
        0,
    )
    base.update(exponents)
    return base


LENGTH = dimension(length=1)


def metres(value: float) -> dict:
    return {"value": value, "dimension": LENGTH}


def fixed(value: float) -> dict:
    return {"source": "fixed", "value": metres(value)}


DRIVEN = {"source": "parameter", "parameter": wire_id("parameter", PARAMETER)}

# Axis order and the lower/upper roles are semantic, so the authored order is
# reproduced literally rather than sorted.
COORDINATES = [
    {"lower": fixed(-1.0), "upper": DRIVEN},
    {"lower": DRIVEN, "upper": fixed(6.0)},
    {"lower": fixed(0.5), "upper": fixed(5.5)},
]

NODE_PARAMETER = {
    "id": wire_id("parameter", PARAMETER),
    "definition": {"kind": "parameter", "value": metres(2.0)},
}
NODE_DOMAIN = {
    "id": wire_id("domain", BODY),
    "definition": {
        "kind": "domain",
        "domain": {"kind": "cartesian-box-sources", "coordinates": COORDINATES},
    },
}
# residual: c = SpatialCoordinate(axis 0); zero = c - c; roots = [zero]
NODE_RELATION = {
    "id": wire_id("relation", RETAIN),
    "definition": {
        "kind": "relation",
        "residuals": {
            "nodes": [
                {"op": "spatial-coordinate", "axis": 0},
                {"op": "sub", "left": 0, "right": 0},
            ],
            "roots": [1],
        },
    },
}
NODE_ACTIVATION = {
    "id": wire_id("activation", ACTIVATION),
    "definition": {"kind": "activation", "activation": {"kind": "continuous"}},
}

# `EntityKind` declaration order fixes the `(kind, ULID)` sort the encoder
# applies: Domain < Representation < Field < Parameter < Port < Relation <
# Activation < Connection < ClockDomain.
NODES = [NODE_DOMAIN, NODE_PARAMETER, NODE_RELATION, NODE_ACTIVATION]

# `WireEdge` sorts on (from, to, kind) under that same order.
EDGES = [
    {
        "from": wire_id("domain", BODY),
        "to": wire_id("parameter", PARAMETER),
        "kind": "depends-on",
    },
    {
        "from": wire_id("relation", RETAIN),
        "to": wire_id("domain", BODY),
        "kind": "applies-on",
    },
    {
        "from": wire_id("activation", ACTIVATION),
        "to": wire_id("relation", RETAIN),
        "kind": "activates",
    },
]

# `KernelNode::Parameter::initial_value()` is `Some(value)`, `Node::kernel`
# stores it, and snapshot admission copies it here. Nothing in the fixture
# issues `SetValue`; this entry is the Parameter's own declared value.
VALUES = [{"target": wire_id("parameter", PARAMETER), "value": metres(2.0)}]

MODEL_ENVELOPE = {
    "schema": MODEL_SCHEMA,
    "encoding": CANONICAL_ENCODING,
    "source_revision": 1,
    "model_ulid": MODEL,
    "nodes": NODES,
    "values": VALUES,
    "edges": EDGES,
    "boundary": [],
}

# View members are canonicalized by the transaction encoder.
VIEW_MEMBERS = [
    wire_id("domain", BODY),
    wire_id("parameter", PARAMETER),
    wire_id("relation", RETAIN),
    wire_id("activation", ACTIVATION),
]

TRANSACTION_ENVELOPE = {
    "schema": TRANSACTION_SCHEMA,
    "encoding": CANONICAL_ENCODING,
    "label": "parameter-driven Cartesian model v8 fixture",
    "ops": [
        {"op": "define-kernel-node", "node": NODE_PARAMETER},
        {"op": "define-kernel-node", "node": NODE_DOMAIN},
        {"op": "define-kernel-node", "node": NODE_RELATION},
        {"op": "define-kernel-node", "node": NODE_ACTIVATION},
        {
            "op": "connect",
            "from": wire_id("domain", BODY),
            "to": wire_id("parameter", PARAMETER),
            "edge": "depends-on",
        },
        {
            "op": "connect",
            "from": wire_id("relation", RETAIN),
            "to": wire_id("domain", BODY),
            "edge": "applies-on",
        },
        {
            "op": "connect",
            "from": wire_id("activation", ACTIVATION),
            "to": wire_id("relation", RETAIN),
            "edge": "activates",
        },
        {
            "op": "define-model-view",
            "view": {"ulid": MODEL, "members": VIEW_MEMBERS, "boundary": []},
        },
    ],
    "preconditions": [],
}

# `WireModelContentV2` omits `source_revision`: a graph revision is provenance,
# not meaning. The Transaction digest covers its complete envelope because the
# operation order is the artifact.
MODEL_DIGEST_FIELDS = (
    "schema",
    "encoding",
    "model_ulid",
    "nodes",
    "values",
    "edges",
    "boundary",
)


def render(value: object) -> bytes:
    """`serde_json::to_vec`: compact, declaration order, shortest round-trip f64."""
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def artifact_digest(schema: str, content: bytes) -> str:
    hasher = hashlib.sha256()
    hasher.update(schema.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(content)
    return hasher.hexdigest()


def model_content(envelope: dict) -> bytes:
    return render({key: envelope[key] for key in MODEL_DIGEST_FIELDS})


def semantic_fingerprint(envelope: dict) -> str:
    """Identity of the Model's meaning, blind to which epoch carried it."""
    payload = {
        key: envelope[key]
        for key in (
            "source_revision",
            "model_ulid",
            "nodes",
            "values",
            "edges",
            "boundary",
        )
    }
    return hashlib.sha256(render(payload)).hexdigest()


def committed(path: pathlib.Path) -> bytes:
    return path.read_bytes().rstrip(b"\n")


def check(label: str, derived: object, frozen: object, failures: list[str]) -> None:
    equal = derived == frozen
    shown = f"{len(derived)} bytes" if isinstance(derived, bytes) else derived
    print(f"  [{'ok ' if equal else 'DIFF'}] {label}: {shown}")
    if not equal:
        failures.append(f"{label}: derived {derived!r} != committed {frozen!r}")


def main() -> int:
    failures: list[str] = []

    print("Model")
    model_bytes = render(MODEL_ENVELOPE)
    check("bytes", len(model_bytes), 2347, failures)
    check(
        "raw sha256",
        hashlib.sha256(model_bytes).hexdigest(),
        "7e179d0d90f8789b9818eae7b5696e10c33a9350a34205d2e7cfd56b938aa427",
        failures,
    )
    check(
        "artifact digest",
        artifact_digest(MODEL_SCHEMA, model_content(MODEL_ENVELOPE)),
        "e410295337a3a51a271f272e03ae7d7a4b8e7df1b04faf76645bb1e18567e4b3",
        failures,
    )
    check(
        "committed literal",
        model_bytes,
        committed(EXPECTED / "model-v8.json"),
        failures,
    )

    print("Model Transaction")
    transaction_bytes = render(TRANSACTION_ENVELOPE)
    check("bytes", len(transaction_bytes), 2646, failures)
    check(
        "raw sha256",
        hashlib.sha256(transaction_bytes).hexdigest(),
        "5ceeef06b286e3edba6f7978c42c227a3c78db2f1d80de7d9b917ec23f8afc47",
        failures,
    )
    check(
        "artifact digest",
        artifact_digest(TRANSACTION_SCHEMA, transaction_bytes),
        "132168803ac8882f0f35187215d3f2ce44817d03921d6ad95b73a9cac62aa102",
        failures,
    )
    check(
        "committed literal",
        transaction_bytes,
        committed(EXPECTED / "model-transaction-v8.json"),
        failures,
    )

    print("Cylinder resource")
    historical = committed(
        ROOT
        / "verify/artifacts/current-model-canonical-identity/expected/historical"
        / "steady-flow-past-cylinder.model-v7.json"
    )
    current = committed(ROOT / "examples/steady-flow-past-cylinder.model.json")
    old, new = '"schema":"eqiora.model-envelope/v7"', f'"schema":"{MODEL_SCHEMA}"'
    if historical.count(old.encode()) != 1:
        failures.append(
            "historical cylinder resource does not carry exactly one schema field"
        )
        return report(failures)
    check(
        "re-encoded from v7",
        historical.replace(old.encode(), new.encode()),
        current,
        failures,
    )
    check("bytes", len(current), 16797, failures)
    check(
        "raw sha256",
        hashlib.sha256(current).hexdigest(),
        "672016cb80683fb1448adab79d7c8f6a2fdda22f92c6df2d82b684bd5e65e099",
        failures,
    )

    historical_envelope = json.loads(historical)
    current_envelope = json.loads(current)
    check(
        "artifact digest",
        artifact_digest(MODEL_SCHEMA, model_content(current_envelope)),
        "8bc5155bc1b64ed37f7a2ac010a966e1619091a118e6cf7806dbdf9621977146",
        failures,
    )
    check(
        "superseded v7 digest",
        artifact_digest("eqiora.model-envelope/v7", model_content(historical_envelope)),
        "668fa55e5ab1a46d0b7523e4e3162442ccd7698697c4308604cf4fe9269249de",
        failures,
    )
    check(
        "semantic fingerprint preserved",
        semantic_fingerprint(current_envelope),
        semantic_fingerprint(historical_envelope),
        failures,
    )
    check(
        "model ULID",
        current_envelope["model_ulid"],
        "01KYQFNFX85DKM2SE5FR6H4WPJ",
        failures,
    )
    check("source revision", current_envelope["source_revision"], 1, failures)

    print("Historical negative corpus")
    for family, schema in (
        ("model", "eqiora.model-envelope"),
        ("model-transaction", "eqiora.model-transaction-envelope"),
    ):
        for version in range(1, 8):
            specimen = json.loads(
                committed(EXPECTED / f"historical/{family}-v{version}.json")
            )
            check(
                f"{family} v{version} schema",
                specimen["schema"],
                f"{schema}/v{version}",
                failures,
            )

    return report(failures)


def report(failures: list[str]) -> int:
    if failures:
        print("\nFAILED:")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print("\nEvery committed literal agrees with the independent derivation.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
