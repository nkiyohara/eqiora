#!/usr/bin/env python3
r"""Independent pre-implementation oracle for the chordal realization envelope.

This oracle is authored by a non-implementing lane from published contracts
only: RFC 0008 (canonical artifact wire and digest framing), RFC 0079
(straight-edged planar region wire, canonical order, binary64 rendering),
RFC 0081 (exact circular-hole wire and DFG witness), and RFC 0082 (chordal
approximation contract, evaluation allowance, segment selection). No Rust was
read to obtain any value below, and no value was copied from production test
output. It is standard library only.

What the oracle derives exactly
-------------------------------
* the canonical binary64 rendering rule of ``eqiora.canonical-json/v1``,
  reconstructed from its branch structure and validated against two frozen
  repository literals it did not author;
* the exact circular-hole source envelope: 511 canonical bytes and its
  domain-separated identity;
* the binary64 boundary-evaluation allowance, as an exact dyadic rational;
* the accepted circular segment count, by two mutually independent routes;
* the ideal sagitta, area-deficit and perimeter-deficit at 80 decimal digits;
* the canonical byte production and digest framing of the new envelope, as a
  total function of its thirteen declared field values; and
* every falsifier whose expected bytes the closed wire determines.

What the oracle deliberately does not freeze
--------------------------------------------
Three of the four bound resource digests are **not derivable** from published
contracts.  They are reported as blocked rather than invented; see
``BLOCKED`` below and the case README.  The encoding witness therefore carries
three explicitly *declared* slot values, computed by :func:`declared_slot`.
They are self-identifying and are **not** predictions of the DFG realization
chain.  Nothing here may be wired as a positive oracle for those three
resources.

Run::

    python3 verify/geometry/circular-hole-chordal-realization-binding/oracle/binding_oracle.py
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
from decimal import Decimal, getcontext

getcontext().prec = 80

# --------------------------------------------------------------------------
# check accounting
# --------------------------------------------------------------------------

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def emit(key: str, value: object) -> None:
    sys.stdout.write(f"{key}={value}\n")


# --------------------------------------------------------------------------
# 1. canonical binary64 rendering of eqiora.canonical-json/v1
# --------------------------------------------------------------------------
#
# RFC 0079 pins identity to "shortest round trip significant digits together
# with its fixed plain-versus-exponent presentation rules".  Those rules are
# reconstructed here from the branch structure of the shortest-decimal printer
# the canonical encoder uses, and are then *validated* against frozen bytes
# this lane did not produce.  Presentation, not digit selection, is where a
# naive `repr` diverges: `repr(1e-05)` is `1e-05`, while the canonical wire
# spells the same value `0.00001`.


def shortest_digits(value: float) -> tuple[str, int]:
    """Return ``(digits, k)`` with ``value == int(digits) * 10 ** k``.

    Two independent routes must agree: widening ``%e`` precision until the
    text round-trips, and the interpreter's own shortest-round-trip ``repr``.
    """
    if value != value or value in (float("inf"), float("-inf")):
        raise ValueError("non-finite value is not encodable")
    if value < 0.0:
        raise ValueError("this witness family has no negative wire value")
    for precision in range(0, 17):
        text = "%.*e" % (precision, value)
        if float(text) == value:
            break
    else:  # pragma: no cover - unreachable for binary64
        raise AssertionError("no round-tripping decimal found")
    significand, _, exponent_text = text.partition("e")
    digits = significand.replace(".", "").rstrip("0") or "0"
    k = int(exponent_text) - (len(digits) - 1)

    # cross-route: repr selects the same digit string.
    repr_text = repr(value)
    repr_digits = (
        repr_text.partition("e")[0].replace(".", "").lstrip("0").rstrip("0") or "0"
    )
    if repr_digits != digits:
        raise AssertionError(f"digit routes disagree for {value!r}")
    return digits, k


def render_f64(value: float) -> str:
    """Canonical ``eqiora.canonical-json/v1`` spelling of a finite binary64."""
    if value == 0.0:
        return "0.0"  # signed zero normalizes to positive zero
    digits, k = shortest_digits(value)
    length = len(digits)
    kk = length + k  # 10 ** (kk - 1) <= value < 10 ** kk
    if 0 <= k and kk <= 16:
        return digits + "0" * k + ".0"
    if 0 < kk <= 16:
        return digits[:kk] + "." + digits[kk:]
    if -5 < kk <= 0:
        return "0." + "0" * (-kk) + digits
    if length == 1:
        return digits + "e" + str(kk - 1)
    return digits[0] + "." + digits[1:] + "e" + str(kk - 1)


def render_string(text: str) -> str:
    if any(ch < " " or ch in '"\\' or ch > "\x7e" for ch in text):
        raise ValueError(f"value needs escaping, outside this witness: {text!r}")
    return '"' + text + '"'


def render_int(value: int) -> str:
    if not isinstance(value, int) or isinstance(value, bool):
        raise TypeError("integer wire fields carry exact integers")
    return str(value)


def obj(pairs: list[tuple[str, str]]) -> str:
    return "{" + ",".join(f"{render_string(k)}:{v}" for k, v in pairs) + "}"


def arr(items: list[str]) -> str:
    return "[" + ",".join(items) + "]"


def digest(domain: str, canonical: bytes) -> str:
    """RFC 0008 identity: sha256(schema-domain || 0x00 || canonical bytes)."""
    return hashlib.sha256(domain.encode("utf-8") + b"\x00" + canonical).hexdigest()


# Rendering rule, checked against the value classes the wire actually carries.
for _value, _want in [
    (0.0, "0.0"),
    (-0.0, "0.0"),
    (1.0, "1.0"),
    (2.2, "2.2"),
    (0.41, "0.41"),
    (0.2, "0.2"),
    (0.05, "0.05"),
    (0.25, "0.25"),
    (0.0625, "0.0625"),
    (1e-12, "1e-12"),
    (1e-4, "0.0001"),
    (1e-5, "0.00001"),
    (1e-6, "1e-6"),
    (1.234567890123e-5, "0.00001234567890123"),
    (100.0, "100.0"),
]:
    check(f"render.{_want}", render_f64(_value) == _want, render_f64(_value))

check(
    "render.roundtrips_every_case",
    all(
        float(render_f64(v)) == (0.0 if v == 0.0 else v)
        for v in (0.0, -0.0, 1.0, 2.2, 0.41, 0.2, 0.05, 1e-12, 1e-4, 1e-5, 1e-6)
    ),
)


# --------------------------------------------------------------------------
# 2. RFC 0079 straight-edged planar region wire
# --------------------------------------------------------------------------

REGION_SCHEMA = "eqiora.geometry-definition-envelope/v1"
ENCODING = "eqiora.canonical-json/v1"
REGION_KIND = "straight-edged-planar-v1"
LENGTH_UNIT = "metre"


def signed_area(loop: list[int], vertices: list[tuple[float, float]]) -> float:
    total = 0.0
    for position, vertex in enumerate(loop):
        x1, y1 = vertices[vertex]
        x2, y2 = vertices[loop[(position + 1) % len(loop)]]
        total += x1 * y2 - x2 * y1
    return total / 2.0


def canonical_loop(
    loop: list[int], vertices: list[tuple[float, float]], want_ccw: bool
) -> list[int]:
    """RFC 0079 steps 3 and 4: orient, then rotate to the smallest index."""
    if (signed_area(loop, vertices) > 0.0) != want_ccw:
        loop = list(reversed(loop))
    start = loop.index(min(loop))
    return loop[start:] + loop[:start]


def canonical_region(
    authored_vertices: list[tuple[float, float]],
    authored_outer: list[int],
    authored_holes: list[list[int]],
    tolerance_m: float,
    authored_entity_sets: list[tuple[str, int, list[int]]],
) -> tuple[bytes, str, dict]:
    """Encode one single-face straight-edged region through RFC 0079 order."""
    normalized = [(x + 0.0, y + 0.0) for x, y in authored_vertices]  # step 1
    order = sorted(range(len(normalized)), key=lambda i: normalized[i])  # step 2
    vertices = [normalized[i] for i in order]
    remap = {old: new for new, old in enumerate(order)}

    outer = canonical_loop([remap[i] for i in authored_outer], vertices, True)
    holes = sorted(
        canonical_loop([remap[i] for i in hole], vertices, False)
        for hole in authored_holes
    )  # step 5

    sets = sorted(  # steps 6 and 7
        (
            (name, dim, sorted(set(members)))
            for name, dim, members in authored_entity_sets
        ),
        key=lambda s: (s[1], s[0].encode("utf-8")),
    )
    if len({name for name, _, _ in sets}) != len(sets):
        raise ValueError("entity-set names are unique across the whole region")

    text = obj(
        [
            ("schema", render_string(REGION_SCHEMA)),
            ("encoding", render_string(ENCODING)),
            ("kind", render_string(REGION_KIND)),
            ("length-unit", render_string(LENGTH_UNIT)),
            ("tolerance-m", render_f64(tolerance_m)),
            (
                "vertices",
                arr([arr([render_f64(x), render_f64(y)]) for x, y in vertices]),
            ),
            (
                "faces",
                arr(
                    [
                        obj(
                            [
                                ("outer", arr([render_int(i) for i in outer])),
                                (
                                    "holes",
                                    arr(
                                        [
                                            arr([render_int(i) for i in hole])
                                            for hole in holes
                                        ]
                                    ),
                                ),
                            ]
                        )
                    ]
                ),
            ),
            (
                "entity-sets",
                arr(
                    [
                        obj(
                            [
                                ("name", render_string(name)),
                                ("dimension", render_int(dim)),
                                ("members", arr([render_int(m) for m in members])),
                            ]
                        )
                        for name, dim, members in sets
                    ]
                ),
            ),
        ]
    )
    data = text.encode("utf-8")
    return (
        data,
        digest(REGION_SCHEMA, data),
        {"vertices": vertices, "outer": outer, "holes": holes},
    )


# Validation route 1: reproduce the frozen RFC 0079 square-with-hole identity.
# These two numbers are published in RFC 0079 and in the registered sibling
# case; reproducing them proves the renderer and the canonical order, and is
# not a source of any value this oracle freezes.
RFC0079_BYTES = 482
RFC0079_SHA256 = "e6f8e17ac215ef37ca3c9de07b9979e34f13412a5de11dc9240ea1def8130030"

_square_bytes, _square_digest, _ = canonical_region(
    [
        (0.0, 0.0),
        (1.0, 0.0),
        (1.0, 1.0),
        (0.0, 1.0),
        (0.25, 0.25),
        (0.75, 0.25),
        (0.75, 0.75),
        (0.25, 0.75),
    ],
    [0, 1, 2, 3],
    [[4, 5, 6, 7]],
    0.0625,
    [("exterior", 1, [0, 1, 2, 3]), ("hole", 1, [4, 5, 6, 7]), ("fluid", 2, [0])],
)
check(
    "rfc0079.byte_length", len(_square_bytes) == RFC0079_BYTES, str(len(_square_bytes))
)
check("rfc0079.digest", _square_digest == RFC0079_SHA256, _square_digest)


# --------------------------------------------------------------------------
# 3. RFC 0081 exact circular-hole source wire, and the DFG source identity
# --------------------------------------------------------------------------

SOURCE_SCHEMA = "eqiora.planar-circular-hole-envelope/v1"
SOURCE_KIND = "axis-aligned-rectangle-with-circular-hole-v1"

BOUNDS = [[0.0, 2.2], [0.0, 0.41]]
CENTER = [0.2, 0.2]
RADIUS_M = 0.05
TOLERANCE_M = 1e-12

# Authored deliberately out of canonical order, with a duplicate member, so the
# canonical `(dimension ascending, name byte order)` rule is exercised.
SOURCE_SETS = [
    ("fluid", 2, [0, 0]),
    ("walls", 1, [3, 2]),
    ("inlet", 1, [0]),
    ("cylinder", 1, [4]),
    ("outlet", 1, [1]),
]


def canonical_source(
    bounds: list[list[float]],
    center: list[float],
    radius_m: float,
    tolerance_m: float,
    entity_sets: list[tuple[str, int, list[int]]],
) -> tuple[bytes, str]:
    """RFC 0081 field order; struct fields keep underscores, values kebab-case."""
    (x_lo, x_hi), (y_lo, y_hi) = bounds
    if not (x_lo < x_hi and y_lo < y_hi):
        raise ValueError("bounds increase strictly")
    if not radius_m > 0.0 or not tolerance_m > 0.0:
        raise ValueError("radius and tolerance are finite and positive")
    clearance = min(
        center[0] - x_lo, x_hi - center[0], center[1] - y_lo, y_hi - center[1]
    )
    if not clearance > radius_m + tolerance_m:
        raise ValueError("the circle is strictly interior beyond the tolerance")

    sets = sorted(
        ((name, dim, sorted(set(members))) for name, dim, members in entity_sets),
        key=lambda s: (s[1], s[0].encode("utf-8")),
    )
    text = obj(
        [
            ("schema", render_string(SOURCE_SCHEMA)),
            ("encoding", render_string(ENCODING)),
            ("kind", render_string(SOURCE_KIND)),
            ("length_unit", render_string(LENGTH_UNIT)),
            ("tolerance_m", render_f64(tolerance_m)),
            (
                "bounds",
                arr([arr([render_f64(lo), render_f64(hi)]) for lo, hi in bounds]),
            ),
            (
                "circle",
                obj(
                    [
                        ("center", arr([render_f64(c) for c in center])),
                        ("radius_m", render_f64(radius_m)),
                    ]
                ),
            ),
            (
                "entity_sets",
                arr(
                    [
                        obj(
                            [
                                ("name", render_string(name)),
                                ("dimension", render_int(dim)),
                                ("members", arr([render_int(m) for m in members])),
                            ]
                        )
                        for name, dim, members in sets
                    ]
                ),
            ),
        ]
    )
    data = text.encode("utf-8")
    return data, digest(SOURCE_SCHEMA, data)


SOURCE_BYTES, SOURCE_SHA256 = canonical_source(
    BOUNDS, CENTER, RADIUS_M, TOLERANCE_M, SOURCE_SETS
)

# Validation route 2: RFC 0081 publishes 511 bytes and this identity.
check("source.byte_length", len(SOURCE_BYTES) == 511, str(len(SOURCE_BYTES)))
check(
    "source.digest",
    SOURCE_SHA256 == "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9",
    SOURCE_SHA256,
)
check(
    "source.digest_framing_is_domain_separated",
    hashlib.sha256(SOURCE_BYTES).hexdigest() != SOURCE_SHA256
    and hashlib.sha256(SOURCE_SCHEMA.encode() + SOURCE_BYTES).hexdigest()
    != SOURCE_SHA256,
)


# --------------------------------------------------------------------------
# 4. high-precision kernel and the RFC 0082 approximation contract
# --------------------------------------------------------------------------


def dpi() -> Decimal:
    getcontext().prec += 12
    lasts, t, s, n, na, d, da = 0, Decimal(3), 3, 1, 0, 0, 24
    while s != lasts:
        lasts = s
        n, na = n + na, na + 8
        d, da = d + da, da + 32
        t = (t * n) / d
        s += t
    getcontext().prec -= 12
    return +s


PI = dpi()
TWO_PI = 2 * PI
check(
    "pi.leading_digits",
    str(PI).startswith("3.14159265358979323846264338327950288419716939937510"),
    str(PI)[:52],
)


def dsin(x: Decimal) -> Decimal:
    getcontext().prec += 12
    i, lasts, s, fact, num, sign = 1, 0, x, 1, x, 1
    while s != lasts:
        lasts = s
        i += 2
        fact *= i * (i - 1)
        num *= x * x
        sign *= -1
        s += num / fact * sign
    getcontext().prec -= 12
    return +s


def dcos(x: Decimal) -> Decimal:
    getcontext().prec += 12
    i, lasts, s, fact, num, sign = 0, 0, Decimal(1), 1, Decimal(1), 1
    while s != lasts:
        lasts = s
        i += 2
        fact *= i * (i - 1)
        num *= x * x
        sign *= -1
        s += num / fact * sign
    getcontext().prec -= 12
    return +s


def dasin(x: Decimal) -> Decimal:
    """Newton on ``sin(t) - x``, seeded from the ordinary binary64 value."""
    import math

    t = Decimal(repr(math.asin(float(x))))
    for _ in range(8):
        t = t - (dsin(t) - x) / dcos(t)
    return +t


check("trig.pythagoras", dsin(PI / 7) ** 2 + dcos(PI / 7) ** 2 == Decimal(1))
check(
    "trig.asin_inverts_sin",
    abs(dsin(dasin(Decimal("0.03"))) - Decimal("0.03")) < Decimal("1e-70"),
)

R = Decimal(repr(RADIUS_M))
MIN_SEGMENTS = 8


def sagitta(n: int) -> Decimal:
    """RFC 0082: ``sagitta(n) = 2 r sin^2(pi / (2 n))``."""
    return 2 * R * dsin(PI / (2 * n)) ** 2


def area_deficit(n: int) -> Decimal:
    """RFC 0082: ``pi r^2 - (n / 2) r^2 sin(2 pi / n)``."""
    return PI * R * R - Decimal(n) / 2 * R * R * dsin(TWO_PI / n)


def perimeter_deficit(n: int) -> Decimal:
    """RFC 0082: ``2 pi r - 2 n r sin(pi / n)``."""
    return 2 * PI * R - 2 * n * R * dsin(PI / n)


# Identity cross-check: the half-angle sagitta form equals the direct form.
# A chord subtending the central angle 2 pi / n has sagitta r (1 - cos(pi / n)),
# and 2 r sin^2(pi / (2 n)) is the same quantity.
check(
    "sagitta.half_angle_identity",
    all(
        abs(sagitta(n) - R * (1 - dcos(PI / n))) < Decimal("1e-70")
        for n in (8, 49, 50, 64)
    ),
)

# --- binary64 evaluation allowance (RFC 0082 floating-point boundary) -------
#
# scale_m = max(|source bounds|, |centre coordinates|, radius, MIN_POSITIVE)
# evaluation_allowance_m = 128 * f64::EPSILON * scale_m

F64_EPSILON = sys.float_info.epsilon
SCALE_M = max(
    abs(0.0),
    abs(2.2),
    abs(0.0),
    abs(0.41),
    abs(0.2),
    abs(0.2),
    RADIUS_M,
    sys.float_info.min,
)
ALLOWANCE_M = 128 * F64_EPSILON * SCALE_M

check("allowance.scale_is_largest_source_length", SCALE_M == 2.2, repr(SCALE_M))
check("allowance.f64_epsilon_is_two_to_minus_52", F64_EPSILON == 2.0**-52)
# 128 * 2**-52 == 2**-45 exactly, and 2.2 has a 52-bit significand, so the
# product is exact in binary64: no rounding enters the allowance.
check(
    "allowance.is_exact_dyadic",
    ALLOWANCE_M.as_integer_ratio() == (2476979795053773, 2**95),
    str(ALLOWANCE_M.as_integer_ratio()),
)
check(
    "allowance.association_order_is_irrelevant",
    128 * (F64_EPSILON * SCALE_M) == ALLOWANCE_M == (128 * F64_EPSILON) * SCALE_M,
)

REQUESTED_MAX_ERROR_M = 1e-4
MIN_MEAN_RATIO = 1e-5
EPSILON_EFFECTIVE_M = REQUESTED_MAX_ERROR_M - ALLOWANCE_M
check(
    "allowance.request_is_strictly_greater",
    REQUESTED_MAX_ERROR_M > ALLOWANCE_M and EPSILON_EFFECTIVE_M > 0.0,
)

# --- segment selection, by two mutually independent routes ------------------

EPS_EFF = Decimal(EPSILON_EFFECTIVE_M)

# Route A: monotone search. sagitta is strictly decreasing in n, so the minimal
# admissible count is found without any inverse function at all.
route_a = None
for candidate in range(MIN_SEGMENTS, 4096):
    if sagitta(candidate) <= EPS_EFF:
        route_a = candidate
        break

# Route B: RFC 0082's stable half-angle inverse.
if EPS_EFF >= 2 * R:
    route_b = MIN_SEGMENTS
else:
    inverse = PI / (2 * dasin((EPS_EFF / (2 * R)).sqrt()))
    route_b = max(MIN_SEGMENTS, int(-(-inverse // 1)))
    while sagitta(route_b) > EPS_EFF:
        route_b += 1
    while route_b > MIN_SEGMENTS and sagitta(route_b - 1) <= EPS_EFF:
        route_b -= 1

CIRCLE_SEGMENTS = route_a
check("segments.two_routes_agree", route_a == route_b, f"{route_a} vs {route_b}")
check("segments.accepted_is_50", CIRCLE_SEGMENTS == 50, str(CIRCLE_SEGMENTS))
check(
    "segments.49_is_insufficient",
    sagitta(49) > EPS_EFF and float(sagitta(49)) > REQUESTED_MAX_ERROR_M,
)
check("segments.50_is_sufficient", sagitta(50) <= EPS_EFF)
check("segments.at_least_eight", CIRCLE_SEGMENTS >= MIN_SEGMENTS)

# --- ideal metrics ---------------------------------------------------------

SAGITTA_50 = sagitta(CIRCLE_SEGMENTS)
AREA_DEFICIT_50 = area_deficit(CIRCLE_SEGMENTS)
PERIMETER_DEFICIT_50 = perimeter_deficit(CIRCLE_SEGMENTS)

IDEAL_BOUNDARY_ERROR_BOUND_M = float(SAGITTA_50) + ALLOWANCE_M
IDEAL_AREA_DEFICIT_M2 = float(AREA_DEFICIT_50)
IDEAL_PERIMETER_DEFICIT_M = float(PERIMETER_DEFICIT_50)

# The one inequality the published contract mandates: the accepted bound must
# not exceed the caller's request.  No tolerance is introduced here.
check(
    "bound.within_requested_maximum",
    IDEAL_BOUNDARY_ERROR_BOUND_M <= REQUESTED_MAX_ERROR_M,
    repr(IDEAL_BOUNDARY_ERROR_BOUND_M),
)
check(
    "deficits.positive", IDEAL_AREA_DEFICIT_M2 > 0.0 and IDEAL_PERIMETER_DEFICIT_M > 0.0
)
check(
    "deficits.second_order_under_doubling",
    all(
        Decimal("3.9") < area_deficit(n) / area_deficit(2 * n) < Decimal("4.1")
        for n in (8, 16, 32)
    ),
)


# --------------------------------------------------------------------------
# 5. blocked derivations
# --------------------------------------------------------------------------

ENVELOPE_SCHEMA = "eqiora.circular-hole-chordal-realization-envelope/v1"

BLOCKED = {
    "realized_geometry_sha256": (
        "The wire is published (RFC 0079), but the binary64 vertex coordinates "
        "are not determined by any published contract. RFC 0082 pins only the "
        "mathematical phase theta_i = 2 pi i / n; it pins neither a binary64 "
        "association order nor a correctly-rounded transcendental, and RFC 0082 "
        "and the capability matrix both explicitly non-claim cross-platform "
        "mesh-byte identity. Every regular inscribed polygon with n >= 8 has "
        "irrational vertices, so no admissible witness avoids this."
    ),
    "mesh_sha256": (
        "eqiora.simplicial-mesh-envelope/v1 has no published canonical field "
        "order. RFC 0013 lists its content only (dimension, affine f64 "
        "coordinates, connectivity, accepted mean-ratio threshold, recomputed "
        "quality evidence); no RFC, schema, or document gives its key spelling, "
        "field order, or vertex/cell numbering rule. It also inherits the "
        "coordinate block above."
    ),
    "correspondence_sha256": (
        "eqiora.geometry-mesh-correspondence-envelope/v1 has no published wire "
        "in any RFC, schema, or document. RFC 0049 additionally closes it over "
        "one exact Model artifact, whose Domain ULIDs are author-chosen and are "
        "not determined by the frozen public claim."
    ),
}


def declared_slot(field: str) -> str:
    """A self-identifying declared value for a blocked slot.

    This is an encoding witness, never a prediction. It exists so that the
    canonical byte production and every single-field mutation are exactly
    checkable today; the implementation must feed its own real digests through
    :func:`canonical_envelope` instead.
    """
    return hashlib.sha256(
        ENVELOPE_SCHEMA.encode("ascii")
        + b"\x00declared-encoding-slot\x00"
        + field.encode("ascii")
    ).hexdigest()


# --------------------------------------------------------------------------
# 6. the new envelope: canonical bytes and identity
# --------------------------------------------------------------------------

FIELD_ORDER = [
    "schema",
    "encoding",
    "source_geometry_sha256",
    "realized_geometry_sha256",
    "mesh_sha256",
    "correspondence_sha256",
    "requested_max_boundary_error_m",
    "boundary_evaluation_allowance_m",
    "boundary_error_bound_m",
    "circle_segments",
    "circle_area_deficit_m2",
    "circle_perimeter_deficit_m",
    "reference_minimum_mean_ratio",
]

DIGEST_FIELDS = (
    "source_geometry_sha256",
    "realized_geometry_sha256",
    "mesh_sha256",
    "correspondence_sha256",
)
REAL_FIELDS = (
    "requested_max_boundary_error_m",
    "boundary_evaluation_allowance_m",
    "boundary_error_bound_m",
    "circle_area_deficit_m2",
    "circle_perimeter_deficit_m",
    "reference_minimum_mean_ratio",
)
HEX = set("0123456789abcdef")


def canonical_envelope(values: dict) -> tuple[bytes, str]:
    """Total function from the thirteen field values to bytes and identity.

    This is the oracle the implementation must agree with byte-for-byte: it
    accepts whatever resource digests and metrics the real chain produced and
    derives the canonical encoding independently.
    """
    if set(values) != set(FIELD_ORDER):
        raise ValueError("exactly the thirteen canonical fields are encoded")
    if values["schema"] != ENVELOPE_SCHEMA or values["encoding"] != ENCODING:
        raise ValueError("closed schema and encoding vocabulary")
    for field in DIGEST_FIELDS:
        text = values[field]
        if len(text) != 64 or not set(text) <= HEX:
            raise ValueError(f"{field} is 64 lowercase hexadecimal characters")
    for field in REAL_FIELDS:
        value = values[field]
        if (
            not isinstance(value, float)
            or value != value
            or value in (float("inf"), float("-inf"))
        ):
            raise ValueError(f"{field} is a finite binary64 value")
        if not value > 0.0:
            raise ValueError(f"{field} is strictly positive")
    if not isinstance(values["circle_segments"], int) or values["circle_segments"] < 8:
        raise ValueError("circle_segments is an integer of at least eight")
    if not values["boundary_error_bound_m"] <= values["requested_max_boundary_error_m"]:
        raise ValueError("the accepted bound never exceeds the request")
    if (
        not values["requested_max_boundary_error_m"]
        > values["boundary_evaluation_allowance_m"]
    ):
        raise ValueError("the request is strictly greater than the allowance")

    pairs: list[tuple[str, str]] = []
    for field in FIELD_ORDER:
        value = values[field]
        if field in ("schema", "encoding") or field in DIGEST_FIELDS:
            pairs.append((field, render_string(value)))
        elif field == "circle_segments":
            pairs.append((field, render_int(value)))
        else:
            pairs.append((field, render_f64(value)))
    data = obj(pairs).encode("utf-8")
    return data, digest(ENVELOPE_SCHEMA, data)


WITNESS = {
    "schema": ENVELOPE_SCHEMA,
    "encoding": ENCODING,
    "source_geometry_sha256": SOURCE_SHA256,  # derived
    "realized_geometry_sha256": declared_slot("realized_geometry_sha256"),
    "mesh_sha256": declared_slot("mesh_sha256"),
    "correspondence_sha256": declared_slot("correspondence_sha256"),
    "requested_max_boundary_error_m": REQUESTED_MAX_ERROR_M,  # derived
    "boundary_evaluation_allowance_m": ALLOWANCE_M,  # derived
    "boundary_error_bound_m": IDEAL_BOUNDARY_ERROR_BOUND_M,  # derived, ideal
    "circle_segments": CIRCLE_SEGMENTS,  # derived
    "circle_area_deficit_m2": IDEAL_AREA_DEFICIT_M2,  # derived, ideal
    "circle_perimeter_deficit_m": IDEAL_PERIMETER_DEFICIT_M,  # derived, ideal
    "reference_minimum_mean_ratio": MIN_MEAN_RATIO,  # derived
}

WITNESS_BYTES, WITNESS_SHA256 = canonical_envelope(WITNESS)

check("envelope.field_count", len(FIELD_ORDER) == 13)
check(
    "envelope.field_order",
    WITNESS_BYTES.decode("utf-8").count('"') >= 2 * len(FIELD_ORDER)
    and all(
        WITNESS_BYTES.decode("utf-8").index(f'"{a}":')
        < WITNESS_BYTES.decode("utf-8").index(f'"{b}":')
        for a, b in zip(FIELD_ORDER, FIELD_ORDER[1:])
    ),
)
check("envelope.no_model_digest", not any("model" in f for f in FIELD_ORDER))
check(
    "envelope.no_extra_wire_field",
    all(
        k not in FIELD_ORDER
        for k in ("kind", "length_unit", "length-unit", "circle_phase", "producer")
    ),
)
check(
    "envelope.digest_framing_is_domain_separated",
    hashlib.sha256(WITNESS_BYTES).hexdigest() != WITNESS_SHA256
    and hashlib.sha256(ENVELOPE_SCHEMA.encode() + WITNESS_BYTES).hexdigest()
    != WITNESS_SHA256,
)
check(
    "envelope.domain_separator_is_the_envelope_schema",
    digest(SOURCE_SCHEMA, WITNESS_BYTES) != WITNESS_SHA256,
)
check("envelope.compact_json", b" " not in WITNESS_BYTES and b"\n" not in WITNESS_BYTES)
check(
    "envelope.encoder_is_deterministic",
    canonical_envelope(dict(WITNESS)) == (WITNESS_BYTES, WITNESS_SHA256),
)


# --------------------------------------------------------------------------
# 7. falsifiers
# --------------------------------------------------------------------------
#
# Failure modes, exactly as the frozen claim names them:
#   canonical-byte/digest    the canonical bytes and envelope identity change
#   replay-mismatch          regeneration from the exact source disagrees
#   semantic-source          the bound source is not this exact circle
#   realized-region          the realized region is not the regenerated region
#   conformance              mesh or correspondence conformance replay fails
#   resource-digest          a supplied resource digest is not the bound one

FALSIFIERS: list[dict] = []


def envelope_mutation(name: str, field: str, mutate, mode: str, note: str) -> None:
    mutated = dict(WITNESS)
    mutated[field] = mutate(WITNESS[field])
    data, sha = canonical_envelope(mutated)
    check(f"falsifier.{name}.changes_identity", sha != WITNESS_SHA256, sha)
    FALSIFIERS.append(
        {
            "id": name,
            "target": field,
            "kind": "envelope-single-field-mutation",
            "failure_mode": mode,
            "expected_sha256": sha,
            "expected_byte_length": len(data),
            "note": note,
        }
    )


def flip_hex(text: str) -> str:
    """Change exactly one nibble, keeping the value a well-formed digest."""
    return text[:-1] + ("0" if text[-1] != "0" else "1")


def next_up(value: float) -> float:
    import struct

    bits = struct.unpack("<Q", struct.pack("<d", value))[0]
    return struct.unpack("<d", struct.pack("<Q", bits + 1))[0]


envelope_mutation(
    "source_geometry_digest",
    "source_geometry_sha256",
    flip_hex,
    "canonical-byte/digest + semantic-source",
    "a different exact source cannot be bound; replay regenerates from the "
    "stored source and its digest must equal the bound one",
)
envelope_mutation(
    "realized_geometry_digest",
    "realized_geometry_sha256",
    flip_hex,
    "canonical-byte/digest + resource-digest",
    "the supplied realized-region artifact no longer matches the bound digest",
)
envelope_mutation(
    "mesh_digest",
    "mesh_sha256",
    flip_hex,
    "canonical-byte/digest + resource-digest",
    "the supplied mesh artifact no longer matches the bound digest",
)
envelope_mutation(
    "correspondence_digest",
    "correspondence_sha256",
    flip_hex,
    "canonical-byte/digest + resource-digest",
    "the supplied correspondence artifact no longer matches the bound digest",
)
envelope_mutation(
    "requested_max_boundary_error",
    "requested_max_boundary_error_m",
    next_up,
    "canonical-byte/digest + replay-mismatch",
    "regeneration uses the stored request, so a perturbed request changes the "
    "effective budget and the regenerated realization",
)
envelope_mutation(
    "boundary_evaluation_allowance",
    "boundary_evaluation_allowance_m",
    next_up,
    "canonical-byte/digest + replay-mismatch",
    "the allowance is derived from the exact source scale, so a stored value "
    "differing from 128 * f64::EPSILON * scale_m cannot survive replay",
)
envelope_mutation(
    "boundary_error_bound",
    "boundary_error_bound_m",
    next_up,
    "canonical-byte/digest + replay-mismatch",
    "the accepted bound is regenerated, not trusted",
)
envelope_mutation(
    "circle_segments",
    "circle_segments",
    lambda n: n + 1,
    "canonical-byte/digest + replay-mismatch",
    "the stored count is replayed as the segment maximum; 51 regenerates 50 "
    "and the stored count no longer equals the regenerated count",
)
envelope_mutation(
    "circle_segments_below_minimal",
    "circle_segments",
    lambda n: n - 1,
    "canonical-byte/digest + replay-mismatch",
    "49 as the segment maximum cannot satisfy the stored request at all",
)
envelope_mutation(
    "circle_area_deficit",
    "circle_area_deficit_m2",
    next_up,
    "canonical-byte/digest + replay-mismatch",
    "the deficit is a regenerated deterministic metric, compared for exact "
    "equality against the regenerated chordal owner",
)
envelope_mutation(
    "circle_perimeter_deficit",
    "circle_perimeter_deficit_m",
    next_up,
    "canonical-byte/digest + replay-mismatch",
    "as for the area deficit",
)
envelope_mutation(
    "reference_minimum_mean_ratio",
    "reference_minimum_mean_ratio",
    next_up,
    "canonical-byte/digest + replay-mismatch",
    "the stored threshold is replayed as the quality gate; a different "
    "threshold is a different regeneration request",
)

check(
    "falsifier.envelope_mutations_are_pairwise_distinct",
    len({f["expected_sha256"] for f in FALSIFIERS}) == len(FALSIFIERS),
)


# --- exact-source falsifiers, frozen through the published RFC 0081 wire ----


def source_mutation(name: str, note: str, **kwargs) -> None:
    fields = {
        "bounds": BOUNDS,
        "center": CENTER,
        "radius_m": RADIUS_M,
        "tolerance_m": TOLERANCE_M,
        "entity_sets": SOURCE_SETS,
    }
    fields.update(kwargs)
    data, sha = canonical_source(
        fields["bounds"],
        fields["center"],
        fields["radius_m"],
        fields["tolerance_m"],
        fields["entity_sets"],
    )
    check(f"falsifier.{name}.changes_source_identity", sha != SOURCE_SHA256, sha)
    FALSIFIERS.append(
        {
            "id": name,
            "target": "exact circular-hole source",
            "kind": "exact-source-mutation",
            "failure_mode": "semantic-source",
            "expected_source_sha256": sha,
            "expected_source_byte_length": len(data),
            "note": note,
        }
    )


source_mutation(
    "source_circle_center",
    "moving the exact circle centre by one ulp is a different exact source; "
    "the bound source_geometry_sha256 no longer resolves",
    center=[next_up(0.2), 0.2],
)
source_mutation(
    "source_circle_radius",
    "a different exact radius is a different exact source",
    radius_m=next_up(0.05),
)
source_mutation(
    "source_boundary_identity",
    "renaming or remembering the circular boundary set differently changes "
    "exact source identity even though the shape is unchanged",
    entity_sets=[
        ("fluid", 2, [0, 0]),
        ("walls", 1, [3, 2]),
        ("inlet", 1, [0]),
        ("cylinder", 1, [4, 3]),
        ("outlet", 1, [1]),
    ],
)

# Signed-zero normalization is identity-preserving, not identity-changing.
_neg_zero_bytes, _neg_zero_sha = canonical_source(
    [[-0.0, 2.2], [-0.0, 0.41]], CENTER, RADIUS_M, TOLERANCE_M, SOURCE_SETS
)
check("source.signed_zero_normalizes", _neg_zero_sha == SOURCE_SHA256, _neg_zero_sha)


# --- a same-named polygonal source substituted for the exact circle ---------
#
# Dyadic coordinates only, so this falsifier is exactly frozen. It carries the
# same five entity-set names as the exact source, and must still be rejected:
# the family, schema domain and identity all differ.

POLY_VERTICES = [
    (0.0, 0.0),
    (2.25, 0.0),
    (2.25, 0.5),
    (0.0, 0.5),
    (0.125, 0.125),
    (0.25, 0.125),
    (0.25, 0.25),
    (0.125, 0.25),
]
POLY_SETS = [
    ("cylinder", 1, [4, 5, 6, 7]),
    ("inlet", 1, [3]),
    ("outlet", 1, [1]),
    ("walls", 1, [0, 2]),
    ("fluid", 2, [0]),
]
POLY_BYTES, POLY_SHA256, POLY_SHAPE = canonical_region(
    POLY_VERTICES, [0, 1, 2, 3], [[4, 5, 6, 7]], 0.0009765625, POLY_SETS
)
check("polygonal_substitute.differs_from_exact_source", POLY_SHA256 != SOURCE_SHA256)
check(
    "polygonal_substitute.same_five_names",
    {n for n, _, _ in POLY_SETS} == {n for n, _, _ in SOURCE_SETS},
)
FALSIFIERS.append(
    {
        "id": "polygonal_source_substituted",
        "target": "source_geometry_sha256",
        "kind": "same-named-polygonal-source",
        "failure_mode": "semantic-source",
        "expected_source_sha256": POLY_SHA256,
        "expected_source_byte_length": len(POLY_BYTES),
        "note": "a straight-edged-planar-v1 region carrying the identical five "
        "entity-set names is not the exact circle: it is a different "
        "family under a different schema domain, so construction must "
        "reject it rather than accept a polygon as the exact source",
    }
)

# --- realized-region falsifiers, frozen through the published RFC 0079 wire -

_moved = list(POLY_VERTICES)
_moved[4] = (next_up(0.125), 0.125)
_MOVED_BYTES, _MOVED_SHA256, _ = canonical_region(
    _moved, [0, 1, 2, 3], [[4, 5, 6, 7]], 0.0009765625, POLY_SETS
)
check("realized_vertex_mutation.changes_identity", _MOVED_SHA256 != POLY_SHA256)
FALSIFIERS.append(
    {
        "id": "realized_boundary_vertex",
        "target": "realized_geometry_sha256",
        "kind": "realized-region-mutation",
        "failure_mode": "realized-region + resource-digest",
        "expected_region_sha256": _MOVED_SHA256,
        "expected_region_byte_length": len(_MOVED_BYTES),
        "note": "moving one realized boundary vertex by one ulp changes the "
        "region identity, so it can no longer equal the regenerated "
        "region nor match the bound realized_geometry_sha256",
    }
)

# Loop rotation and reversal are normalized away by the canonical producer, so
# an *externally supplied* rotated loop is non-canonical and must be rejected
# at admission rather than silently renormalized (RFC 0079).
_rotated_outer = POLY_SHAPE["outer"][1:] + POLY_SHAPE["outer"][:1]
_ROTATED_TEXT = POLY_BYTES.decode("utf-8").replace(
    '"outer":' + arr([render_int(i) for i in POLY_SHAPE["outer"]]),
    '"outer":' + arr([render_int(i) for i in _rotated_outer]),
)
_ROTATED_BYTES = _ROTATED_TEXT.encode("utf-8")
check("realized_order_mutation.is_not_canonical", _ROTATED_BYTES != POLY_BYTES)
check(
    "realized_order_mutation.same_length_different_identity",
    len(_ROTATED_BYTES) == len(POLY_BYTES)
    and digest(REGION_SCHEMA, _ROTATED_BYTES) != POLY_SHA256,
)
FALSIFIERS.append(
    {
        "id": "realized_boundary_order",
        "target": "realized_geometry_sha256",
        "kind": "non-canonical-loop-rotation",
        "failure_mode": "realized-region + canonical-byte/digest",
        "supplied_sha256": digest(REGION_SCHEMA, _ROTATED_BYTES),
        "expected_behaviour": "reject at admission; canonical reconstruction "
        "rotates the loop back to its smallest index and "
        "therefore cannot reproduce the supplied bytes",
        "note": "the same cycle in a different rotation is not a second "
        "identity, and is not silently normalized either",
    }
)

# --- contract-level falsifiers over the two unpublished wires ---------------

for _entry in (
    {
        "id": "mesh_boundary_topology",
        "target": "mesh_sha256",
        "kind": "mesh-topology-mutation",
        "failure_mode": "conformance + resource-digest",
        "expected_behaviour": "removing, adding, or reconnecting one boundary "
        "facet breaks the complete once-only boundary "
        "facet ownership replayed through the "
        "correspondence, and changes the mesh identity "
        "so the bound mesh_sha256 no longer resolves",
        "byte_frozen": False,
        "blocked_by": "mesh_sha256",
    },
    {
        "id": "correspondence_entity_mapping",
        "target": "correspondence_sha256",
        "kind": "correspondence-mapping-mutation",
        "failure_mode": "conformance + resource-digest",
        "expected_behaviour": "remapping one named entity to a different mesh "
        "cell or facet set fails correspondence "
        "conformance replay and changes the "
        "correspondence identity",
        "byte_frozen": False,
        "blocked_by": "correspondence_sha256",
    },
    {
        "id": "conforming_mesh_without_updated_digest",
        "target": "mesh_sha256",
        "kind": "resource-substitution",
        "failure_mode": "resource-digest",
        "expected_behaviour": "a different but individually valid conforming "
        "affine mesh, supplied while the envelope still "
        "carries the previous mesh_sha256, is rejected "
        "on the digest before any conformance work; a "
        "fixed external mesh is admissible only when the "
        "envelope binds that mesh's own digest",
        "byte_frozen": False,
        "blocked_by": "mesh_sha256",
    },
):
    FALSIFIERS.append(dict(_entry))

check("falsifier.count", len(FALSIFIERS) == 21, str(len(FALSIFIERS)))
check(
    "falsifier.covers_every_envelope_field",
    {f["target"] for f in FALSIFIERS} >= set(FIELD_ORDER[2:]),
)
check(
    "falsifier.byte_frozen_count",
    sum(1 for f in FALSIFIERS if f.get("byte_frozen", True) is not False) == 18,
)
check(
    "falsifier.only_blocked_wires_are_unfrozen",
    {f["blocked_by"] for f in FALSIFIERS if f.get("byte_frozen") is False}
    <= set(BLOCKED),
)


# --------------------------------------------------------------------------
# 8. the frozen contract fixture
# --------------------------------------------------------------------------

CONTRACT = {
    "schema": ENVELOPE_SCHEMA,
    "encoding": ENCODING,
    "digest_framing": "sha256(schema-domain UTF-8 || 0x00 || canonical JSON)",
    "canonical_field_order": FIELD_ORDER,
    "has_model_digest": False,
    "derived_from_published_contracts": {
        "exact_source": {
            "schema": SOURCE_SCHEMA,
            "canonical_bytes": len(SOURCE_BYTES),
            "sha256": SOURCE_SHA256,
            "canonical_json": SOURCE_BYTES.decode("utf-8"),
        },
        "boundary_evaluation_allowance_m": {
            "rule": "128 * f64::EPSILON * scale_m",
            "scale_m": render_f64(SCALE_M),
            "exact_integer_ratio": list(ALLOWANCE_M.as_integer_ratio()),
            "canonical_spelling": render_f64(ALLOWANCE_M),
        },
        "epsilon_effective_m": render_f64(EPSILON_EFFECTIVE_M),
        "circle_segments": CIRCLE_SEGMENTS,
        "requested_max_boundary_error_m": render_f64(REQUESTED_MAX_ERROR_M),
        "reference_minimum_mean_ratio": render_f64(MIN_MEAN_RATIO),
        "ideal_high_precision_80_digits": {
            "sagitta_n49_m": str(sagitta(49)),
            "sagitta_n50_m": str(SAGITTA_50),
            "area_deficit_n50_m2": str(AREA_DEFICIT_50),
            "perimeter_deficit_n50_m": str(PERIMETER_DEFICIT_50),
        },
        "ideal_binary64_spelling": {
            "boundary_error_bound_m": render_f64(IDEAL_BOUNDARY_ERROR_BOUND_M),
            "circle_area_deficit_m2": render_f64(IDEAL_AREA_DEFICIT_M2),
            "circle_perimeter_deficit_m": render_f64(IDEAL_PERIMETER_DEFICIT_M),
        },
    },
    "mandated_inequalities": {
        "bound_within_request": "boundary_error_bound_m <= requested_max_boundary_error_m",
        "request_above_allowance": (
            "requested_max_boundary_error_m > boundary_evaluation_allowance_m"
        ),
        "minimum_segments": "circle_segments >= 8",
        "source": "RFC 0082; no tolerance is introduced by this oracle",
    },
    "encoding_witness": {
        "witness_kind": "declared-slot-encoding-witness",
        "is_dfg_realization_prediction": False,
        "warning": (
            "realized_geometry_sha256, mesh_sha256 and correspondence_sha256 "
            "below are declared slots, not derived values. This envelope "
            "digest must never be wired as a positive oracle for the real "
            "DFG chain. The reusable oracle is canonical_envelope(values): "
            "feed the thirteen real field values and require byte-for-byte "
            "and digest equality."
        ),
        "declared_slot_rule": (
            "sha256(envelope schema || 0x00 || 'declared-encoding-slot' || "
            "0x00 || field name)"
        ),
        "values": {
            k: (v if not isinstance(v, float) else render_f64(v))
            for k, v in WITNESS.items()
        },
        "canonical_json": WITNESS_BYTES.decode("utf-8"),
        "canonical_bytes": len(WITNESS_BYTES),
        "sha256": WITNESS_SHA256,
    },
    "not_derivable_from_published_contracts": BLOCKED,
    "falsifiers": FALSIFIERS,
    "checks_total": len(CHECKS),
}

_FIXTURE = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "expected",
    "binding-contract.json",
)
_SERIALIZED = json.dumps(CONTRACT, indent=2, sort_keys=False, ensure_ascii=False) + "\n"

if "--emit" in sys.argv:
    with open(_FIXTURE, "w", encoding="utf-8") as handle:
        handle.write(_SERIALIZED)
    sys.stdout.write(f"emitted {_FIXTURE}\n")

try:
    with open(_FIXTURE, encoding="utf-8") as handle:
        _FROZEN = handle.read()
except FileNotFoundError:
    _FROZEN = None
check(
    "fixture.frozen_file_matches_derivation",
    _FROZEN == _SERIALIZED,
    "missing" if _FROZEN is None else "differs from the derivation",
)


# --------------------------------------------------------------------------
# 9. report
# --------------------------------------------------------------------------

emit("oracle.envelope_schema", ENVELOPE_SCHEMA)
emit("oracle.encoding", ENCODING)
emit("oracle.field_order", ",".join(FIELD_ORDER))
emit("source.canonical_bytes", len(SOURCE_BYTES))
emit("source.sha256", SOURCE_SHA256)
emit("allowance.scale_m", render_f64(SCALE_M))
emit("allowance.evaluation_allowance_m", render_f64(ALLOWANCE_M))
emit("allowance.epsilon_effective_m", render_f64(EPSILON_EFFECTIVE_M))
emit("segments.accepted", CIRCLE_SEGMENTS)
emit("ideal.sagitta_n49_m", str(sagitta(49)))
emit("ideal.sagitta_n50_m", str(SAGITTA_50))
emit("ideal.area_deficit_n50_m2", str(AREA_DEFICIT_50))
emit("ideal.perimeter_deficit_n50_m", str(PERIMETER_DEFICIT_50))
emit("ideal.boundary_error_bound_m.f64", render_f64(IDEAL_BOUNDARY_ERROR_BOUND_M))
emit("ideal.area_deficit_n50_m2.f64", render_f64(IDEAL_AREA_DEFICIT_M2))
emit("ideal.perimeter_deficit_n50_m.f64", render_f64(IDEAL_PERIMETER_DEFICIT_M))
emit("witness.canonical_json", WITNESS_BYTES.decode("utf-8"))
emit("witness.canonical_bytes", len(WITNESS_BYTES))
emit("witness.sha256", WITNESS_SHA256)
for _field in ("realized_geometry_sha256", "mesh_sha256", "correspondence_sha256"):
    emit(f"blocked.{_field}", "not-derivable-from-published-contracts")
emit("falsifiers.total", len(FALSIFIERS))
emit(
    "falsifiers.byte_frozen",
    sum(1 for f in FALSIFIERS if f.get("byte_frozen", True) is not False),
)

failed = [name for name, ok, _ in CHECKS if not ok]
for name, ok, detail in CHECKS:
    if not ok:
        sys.stdout.write(f"FAIL {name} {detail}\n")
emit("checks.total", len(CHECKS))
emit("checks.failed", len(failed))
emit("oracle.result", "pass" if not failed else "fail")
sys.exit(0 if not failed else 1)
