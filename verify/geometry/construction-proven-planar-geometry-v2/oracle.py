#!/usr/bin/env python3
"""Independent exact-artifact oracle for the ordinary DFG Geometry v2 witness."""

from __future__ import annotations

import hashlib
import json
import sys

SCHEMA = "eqiora.planar-construction-circular-hole-envelope/v2"
ENCODING = "eqiora.canonical-json/v1"
KIND = "construction-proven-rectangle-with-circular-hole-v2"
LENGTH_UNIT = "metre"

BOUNDS = [[-0.0, 2.2], [-0.0, 0.41]]
CENTER = [0.2, 0.2]
RADIUS_M = 0.05

# Deliberately noncanonical set and member order exercises the written ordering
# rule independently of the Rust implementation.
AUTHORED_ENTITY_SETS = [
    ("fluid", 2, [0, 0]),
    ("walls", 1, [3, 2]),
    ("inlet", 1, [0]),
    ("cylinder", 1, [4]),
    ("outlet", 1, [1]),
]

LITERAL = (
    '{"schema":"eqiora.planar-construction-circular-hole-envelope/v2"'
    ',"encoding":"eqiora.canonical-json/v1"'
    ',"kind":"construction-proven-rectangle-with-circular-hole-v2"'
    ',"length_unit":"metre"'
    ',"bounds":[[0.0,2.2],[0.0,0.41]]'
    ',"circle":{"center":[0.2,0.2],"radius_m":0.05}'
    ',"entity_sets":['
    '{"name":"cylinder","dimension":1,"members":[4]}'
    ',{"name":"inlet","dimension":1,"members":[0]}'
    ',{"name":"outlet","dimension":1,"members":[1]}'
    ',{"name":"walls","dimension":1,"members":[2,3]}'
    ',{"name":"fluid","dimension":2,"members":[0]}'
    "]}"
).encode("utf-8")

EXPECTED_LEN = 511
EXPECTED_DIGEST = "1811037532ef5697a2c331d47786d39b2a0d3a64b2f348e7859342e742fecca0"
EXPECTED_PLAIN_SHA256 = (
    "bdcd32d3829ad1bf7b8ef455a09bdbe863db88dc6454584381ef38421ea29ddc"
)


def _number(value: float) -> str:
    """Produce the witness's shortest round-tripping binary64 spelling."""
    if value != value or value in (float("inf"), float("-inf")):
        raise ValueError("non-finite value")
    if value == 0.0:
        value = 0.0
    for precision in range(1, 18):
        text = "%.*g" % (precision, value)
        if float(text) == value:
            break
    mantissa, separator, exponent = text.partition("e")
    if "." not in mantissa and not separator:
        mantissa += ".0"
    return mantissa + ("e" + exponent if separator else "")


def _quoted(text: str) -> str:
    if any(character < " " or character in '"\\' for character in text):
        raise ValueError("this witness accepts only unescaped strings")
    return '"' + text + '"'


def canonical_sets() -> list[tuple[str, int, list[int]]]:
    normalized = [
        (name, dimension, sorted(set(members)))
        for name, dimension, members in AUTHORED_ENTITY_SETS
    ]
    assert len({name for name, _, _ in normalized}) == len(normalized)
    return sorted(normalized, key=lambda item: (item[1], item[0].encode("utf-8")))


def hand_encode() -> bytes:
    sets = ",".join(
        "{"
        + _quoted("name")
        + ":"
        + _quoted(name)
        + ","
        + _quoted("dimension")
        + ":"
        + str(dimension)
        + ","
        + _quoted("members")
        + ":["
        + ",".join(str(member) for member in members)
        + "]}"
        for name, dimension, members in canonical_sets()
    )
    fields = [
        ("schema", _quoted(SCHEMA)),
        ("encoding", _quoted(ENCODING)),
        ("kind", _quoted(KIND)),
        ("length_unit", _quoted(LENGTH_UNIT)),
        (
            "bounds",
            "["
            + ",".join(
                "[" + _number(lower) + "," + _number(upper) + "]"
                for lower, upper in BOUNDS
            )
            + "]",
        ),
        (
            "circle",
            "{"
            + _quoted("center")
            + ":["
            + ",".join(_number(value) for value in CENTER)
            + "],"
            + _quoted("radius_m")
            + ":"
            + _number(RADIUS_M)
            + "}",
        ),
        ("entity_sets", "[" + sets + "]"),
    ]
    return (
        "{" + ",".join(_quoted(name) + ":" + value for name, value in fields) + "}"
    ).encode("utf-8")


def json_encode() -> bytes:
    normalized_bounds = [
        [0.0 if value == 0.0 else value for value in bounds] for bounds in BOUNDS
    ]
    document = {
        "schema": SCHEMA,
        "encoding": ENCODING,
        "kind": KIND,
        "length_unit": LENGTH_UNIT,
        "bounds": normalized_bounds,
        "circle": {"center": CENTER, "radius_m": RADIUS_M},
        "entity_sets": [
            {"name": name, "dimension": dimension, "members": members}
            for name, dimension, members in canonical_sets()
        ],
    }
    return json.dumps(document, separators=(",", ":"), ensure_ascii=False).encode()


def framed_digest(wire: bytes) -> str:
    return hashlib.sha256(SCHEMA.encode("ascii") + b"\x00" + wire).hexdigest()


def main() -> int:
    assert hand_encode() == LITERAL
    assert json_encode() == LITERAL
    assert len(LITERAL) == EXPECTED_LEN
    assert hashlib.sha256(LITERAL).hexdigest() == EXPECTED_PLAIN_SHA256
    assert framed_digest(LITERAL) == EXPECTED_DIGEST
    assert EXPECTED_PLAIN_SHA256 != EXPECTED_DIGEST
    assert json.loads(LITERAL)["circle"]["radius_m"] == RADIUS_M

    sys.stdout.write(LITERAL.decode("utf-8") + "\n")
    sys.stdout.write(f"bytes={EXPECTED_LEN}\n")
    sys.stdout.write(f"schema={SCHEMA}\n")
    sys.stdout.write(f"sha256={EXPECTED_DIGEST}\n")
    sys.stdout.write(f"plain_sha256={EXPECTED_PLAIN_SHA256}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
