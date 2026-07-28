#!/usr/bin/env python3
"""Independent identity oracle for Eqiora Issue #120, slice A1.

Freezes the canonical wire bytes, byte length, and framed SHA-256 for the
DFG-shaped witness of the `eqiora.planar-circular-hole-envelope/v1` family.

This file is written by a non-implementing agent from the frozen issue body
alone. It reads no Rust source, no fixture, and no existing artifact. It is
stdlib-only (`hashlib`, `json`, `sys`) and must be committed unchanged.

Number lexical form is fixed by the issue body ("Binary64 lexical spelling is
the exact RFC 0079 `eqiora.canonical-json/v1` profile"):

    `serde_json` shortest round-trip rendering for an f64, including a forced
    ``.0`` on integral finite values and the canonical lowercase exponent
    form -- i.e. Rust `ryu`, which coincides with CPython `repr(float)` on
    every value in this witness. The body names the two spellings this
    witness turns on: ``0.0`` and ``1e-12``. See the REJECTED PROFILES note
    at the bottom of this file.
"""

from __future__ import annotations

import hashlib
import json
import sys

# --- frozen inputs, transcribed from the issue body -------------------------

SCHEMA = "eqiora.planar-circular-hole-envelope/v1"
ENCODING = "eqiora.canonical-json/v1"
KIND = "axis-aligned-rectangle-with-circular-hole-v1"
LENGTH_UNIT = "metre"

TOLERANCE_M = 1e-12
BOUNDS = [[0.0, 2.2], [0.0, 0.41]]
CENTER = [0.2, 0.2]
RADIUS_M = 0.05

# Authored in deliberately non-canonical order, so that the canonical entity-set
# ordering (dimension ascending, then name byte order) is exercised rather than
# assumed. Members are authored with a duplicate and out of order for the same
# reason.
AUTHORED_ENTITY_SETS = [
    ("fluid", 2, [0, 0]),
    ("walls", 1, [3, 2]),
    ("inlet", 1, [0]),
    ("cylinder", 1, [4]),
    ("outlet", 1, [1]),
]

# --- construction 1: explicit one-line literal ------------------------------

LITERAL = (
    '{"schema":"eqiora.planar-circular-hole-envelope/v1"'
    ',"encoding":"eqiora.canonical-json/v1"'
    ',"kind":"axis-aligned-rectangle-with-circular-hole-v1"'
    ',"length_unit":"metre"'
    ',"tolerance_m":1e-12'
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


# --- construction 2: hand-rolled ordered encoder ----------------------------


def _num(value: float) -> str:
    """Shortest round-tripping decimal for an f64, `.0`-suffixed if integral.

    Derived independently of `repr`/`json`: widen `%g` precision until the
    string parses back to the identical float.
    """
    if value != value or value in (float("inf"), float("-inf")):
        raise ValueError("non-finite value is not encodable")
    if value == 0.0:
        value = 0.0  # normalize negative zero to positive zero
    for precision in range(1, 18):
        text = "%.*g" % (precision, value)
        if float(text) == value:
            break
    else:  # pragma: no cover - unreachable for f64
        raise AssertionError("no round-tripping decimal found")
    mantissa, _, exponent = text.partition("e")
    if "." not in mantissa:
        mantissa += ".0" if not exponent else ""
    return mantissa + ("e" + exponent if exponent else "")


def _string(text: str) -> str:
    if any(ch < " " or ch in '"\\' or ch > "\x7e" for ch in text):
        raise ValueError(f"name needs escaping, outside this witness: {text!r}")
    return '"' + text + '"'


def _canonical_entity_sets() -> list[tuple[str, int, list[int]]]:
    normalized = [
        (name, dim, sorted(set(members))) for name, dim, members in AUTHORED_ENTITY_SETS
    ]
    names = [name for name, _, _ in normalized]
    assert len(set(names)) == len(names), "duplicate entity-set name"
    # canonical order: dimension ascending, then name byte order
    return sorted(normalized, key=lambda s: (s[1], s[0].encode("utf-8")))


def encode() -> bytes:
    sets = ",".join(
        "{"
        + _string("name")
        + ":"
        + _string(name)
        + ","
        + _string("dimension")
        + ":"
        + str(dimension)
        + ","
        + _string("members")
        + ":["
        + ",".join(str(m) for m in members)
        + "]}"
        for name, dimension, members in _canonical_entity_sets()
    )
    fields = [
        (_string("schema"), _string(SCHEMA)),
        (_string("encoding"), _string(ENCODING)),
        (_string("kind"), _string(KIND)),
        (_string("length_unit"), _string(LENGTH_UNIT)),
        (_string("tolerance_m"), _num(TOLERANCE_M)),
        (
            _string("bounds"),
            "["
            + ",".join("[" + _num(lo) + "," + _num(hi) + "]" for lo, hi in BOUNDS)
            + "]",
        ),
        (
            _string("circle"),
            "{"
            + _string("center")
            + ":["
            + ",".join(_num(c) for c in CENTER)
            + "],"
            + _string("radius_m")
            + ":"
            + _num(RADIUS_M)
            + "}",
        ),
        (_string("entity_sets"), "[" + sets + "]"),
    ]
    return ("{" + ",".join(k + ":" + v for k, v in fields) + "}").encode("utf-8")


# --- construction 3: stdlib json with an ordered dict ------------------------


def encode_via_json() -> bytes:
    document = {
        "schema": SCHEMA,
        "encoding": ENCODING,
        "kind": KIND,
        "length_unit": LENGTH_UNIT,
        "tolerance_m": TOLERANCE_M,
        "bounds": BOUNDS,
        "circle": {"center": CENTER, "radius_m": RADIUS_M},
        "entity_sets": [
            {"name": name, "dimension": dimension, "members": members}
            for name, dimension, members in _canonical_entity_sets()
        ],
    }
    return json.dumps(
        document, separators=(",", ":"), ensure_ascii=False, sort_keys=False
    ).encode("utf-8")


# --- frozen expectations -----------------------------------------------------

EXPECTED_BYTES = LITERAL
EXPECTED_LEN = 511
EXPECTED_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"


def digest(canonical_json: bytes) -> str:
    return hashlib.sha256(SCHEMA.encode("ascii") + b"\x00" + canonical_json).hexdigest()


def _check_witness_geometry() -> None:
    """The witness must satisfy the issue's admission predicates."""
    (x_lo, x_hi), (y_lo, y_hi) = BOUNDS
    cx, cy = CENTER
    assert x_lo < x_hi and y_lo < y_hi, "bounds must be strictly increasing"
    assert TOLERANCE_M > 0.0 and RADIUS_M > 0.0, "tolerance and radius must be positive"
    clearances = (
        cx - RADIUS_M - x_lo,
        x_hi - (cx + RADIUS_M),
        cy - RADIUS_M - y_lo,
        y_hi - (cy + RADIUS_M),
    )
    assert all(c > TOLERANCE_M for c in clearances), (
        f"circle not strictly interior: {clearances}"
    )


def main() -> int:
    _check_witness_geometry()

    literal = LITERAL
    encoded = encode()
    via_json = encode_via_json()
    assert encoded == literal, (
        f"encoder disagrees with literal:\n{encoded!r}\n{literal!r}"
    )
    assert via_json == literal, (
        f"json route disagrees with literal:\n{via_json!r}\n{literal!r}"
    )

    assert literal == EXPECTED_BYTES
    assert len(literal) == EXPECTED_LEN, f"length {len(literal)} != {EXPECTED_LEN}"
    actual = digest(literal)
    assert actual == EXPECTED_DIGEST, f"digest {actual} != {EXPECTED_DIGEST}"

    # Round-trip: the bytes must parse and re-encode identically.
    assert encode_via_json() == literal
    assert json.loads(literal.decode("utf-8"))["circle"]["radius_m"] == RADIUS_M

    sys.stdout.write(literal.decode("utf-8") + "\n")
    sys.stdout.write(f"bytes={EXPECTED_LEN}\n")
    sys.stdout.write(f"schema={SCHEMA}\n")
    sys.stdout.write(f"sha256={actual}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

# --- REJECTED PROFILES ------------------------------------------------------
#
# The issue body now states the binary64 lexical profile explicitly: the RFC
# 0079 `eqiora.canonical-json/v1` profile is `serde_json` shortest round-trip
# f64 rendering, with ".0" on integral finite values and the canonical
# lowercase exponent form, and it names "0.0" and "1e-12" as required for this
# witness. Profile A below is therefore the contract, not a choice made here.
#
# The other profiles are retained as falsifier evidence: an implementation that
# reproduces one of these digests has used the named wrong number form, and the
# digest says which one.
#
#   A "0.0" + "1e-12"     (FROZEN)  511 B  b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9
#   B "0"   + "1e-12"     rejected  507 B  c7de52162e0e0c38104e607aea681b968a946f9673a9532e271649f9cd0f4e59
#   C "0.0" + fixed dec.  rejected  520 B  8b2a81724bbd6246451d6bc942e4a093e0ec00a5bfeff8bd9bc458c6d54d2ae1
#   D "0"   + fixed dec.  rejected  516 B  9ded0cf87f4509a17647692e924c5dabb0fa2eebbf32a7307d38bfb37e4a5f83
#
# (B/D are RFC 8785 JCS / ECMAScript Number::toString integral spelling; C/D
# are the exponent-free fixed-decimal spelling "0.000000000001".)
#
# --- STATED DEPENDENCY, NOT VERIFIED HERE -----------------------------------
#
# The issue delegates entity-set sorting to RFC 0079 rather than restating the
# rule. This oracle assumes canonical order is dimension ascending, then set
# name in UTF-8 byte order, which is what the issue's own witness listing
# shows (cylinder, inlet, outlet, walls at dimension 1; then fluid at
# dimension 2). A non-implementing author cannot read RFC 0079 to confirm it.
# If RFC 0079 sorts by name alone, this witness reorders to
# cylinder, fluid, inlet, outlet, walls and the frozen digest is wrong. That
# correction belongs to the contract owner, not to the implementing lane.
