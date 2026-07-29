#!/usr/bin/env python3
"""Reconstruct the chordal circular-hole reference mesh from RFC 0082.

This rebuilds the 50-chord / 104-vertex / 104-triangle reference topology from
the *public* construction rule in RFC 0082 alone. No Rust source, no production
output and no existing fluid oracle was read while writing it.

RFC 0082 fixes:

- the circular loop is the regular inscribed polygon with phase
  ``theta_i = 2 pi i / n``, ``i = 0 .. n-1``;
- every circular direction is cast from the circle centre to the rectangle, and
  the cast-axis coordinate is set *directly* to the exact rectangle bound rather
  than reconstructed by ``c + ((bound - c)/d) * d``;
- for adjacent ray indices ``i`` and ``j = (i + 1) mod n``, with inner circle
  vertices ``I_i``, ``I_j`` and outer rectangle hits ``O_i``, ``O_j``, **the
  shared quad diagonal is ``O_i--I_j``** and the two cells are
  ``(O_i, O_j, I_j)`` and ``(O_i, I_j, I_i)``, stored in positive orientation;
- rectangle corners crossed between rays are inserted in boundary-angle order
  and a deterministic fan fills the area between the outer ray chord and the
  exact rectangle sides;
- a radial hit within the source classification tolerance of a corner reuses
  that exact corner.

The diagonal sentence is the clarification the earlier return asked for. It
resolves the one predicate the RFC previously left open, and it makes exactly
one mesh admissible: ``mesh.json``.

The opposite split ``I_i--O_j`` is still generated, but only as
``falsifier-wrong-diagonal.json``: an explicit **wrong-contract** artifact used
to show that the accepted probe and reaction observations reject it. It is not a
second admissible reading of RFC 0082 and nothing may consume it as a mesh.

Usage::

    python3 build_mesh.py                       # write both files
    python3 build_mesh.py --diagonal accepted   # write only mesh.json

Standard library only.
"""

from __future__ import annotations

import argparse
import hashlib
import math
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import canonical_json  # noqa: E402

SCHEMA = "eqiora.verify/exact-circular-hole-stokes-2d/mesh/v1"

# ---------------------------------------------------------------------------
# Exact source geometry (RFC 0081 frozen DFG witness)
# ---------------------------------------------------------------------------
X_LO, X_HI = 0.0, 2.2
Y_LO, Y_HI = 0.0, 0.41
CX, CY = 0.2, 0.2
RADIUS = 0.05
CLASSIFICATION_TOLERANCE = 1e-12
SOURCE_SCHEMA = "eqiora.planar-circular-hole-envelope/v1"
SOURCE_KIND = "axis-aligned-rectangle-with-circular-hole-v1"
SOURCE_SHA256 = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
SOURCE_CANONICAL_BYTES = 511

# ---------------------------------------------------------------------------
# Realization policy (RFC 0082 approximation contract; accepted reference witness)
# ---------------------------------------------------------------------------
REQUESTED_MAX_BOUNDARY_ERROR = 1e-4
MIN_MEAN_RATIO_QUALITY = 1e-5
MAX_SEGMENTS = 50
F64_EPSILON = sys.float_info.epsilon
F64_MIN_POSITIVE = sys.float_info.min

# Ideal closed-form values frozen by RFC 0082. They are recorded as decimal
# strings and re-derived to full precision by ``check_mesh.py``; they are never
# used as inputs to the topology below.
IDEAL = {
    "sagitta_n49_m": "1.0273036248318289955797595210037224856637053318839e-4",
    "sagitta_n50_m": "9.8663578586421902383159656827472333154739014922844e-5",
    "area_deficit_n50_m2": "2.0654536205467760336685969666957589060533063430286e-5",
    "perimeter_deficit_n50_m": "2.0666771241244346537321549979462280729278040417922e-4",
}

# Named entity correspondence. RFC 0081 fixes the dimension-1 source entity
# order (x-lower, x-upper, y-lower, y-upper, circular hole) and the DFG witness
# names inlet=[0], outlet=[1], walls=[2,3], cylinder=[4], fluid=[0].
SOURCE_ENTITY_OF_SIDE = {
    "x_low": 0,
    "x_high": 1,
    "y_low": 2,
    "y_high": 3,
    "circle": 4,
}
NAMED_SETS = {
    "inlet": ("x_low",),
    "outlet": ("x_high",),
    "walls": ("y_low", "y_high"),
    "cylinder": ("circle",),
}

# The accepted RFC 0082 split and the wrong-contract split it excludes.
ACCEPTED = "accepted"
FALSIFIER = "wrong-diagonal-falsifier"
SPLITS = {
    ACCEPTED: {
        "file": "mesh.json",
        "role": "accepted",
        "diagonal": "O_i--I_j",
        "diagonal_name": "outer-i-to-inner-j",
        "cells": "(O_i, O_j, I_j) then (O_i, I_j, I_i)",
    },
    FALSIFIER: {
        "file": "falsifier-wrong-diagonal.json",
        "role": "wrong-contract-falsifier",
        "diagonal": "I_i--O_j",
        "diagonal_name": "inner-i-to-outer-j",
        "cells": "(I_i, O_i, O_j) then (I_i, O_j, I_j)",
    },
}

RFC_QUAD_RULE = (
    "for adjacent ray indices i and j = (i + 1) mod n, the shared quad diagonal "
    "is O_i--I_j and the two cells are (O_i, O_j, I_j) and (O_i, I_j, I_i), "
    "stored in positive orientation (RFC 0082, 'Reference topology')"
)


# ---------------------------------------------------------------------------
# Approximation policy, evaluated in binary64 exactly as RFC 0082 states it
# ---------------------------------------------------------------------------
def evaluation_allowance() -> tuple[float, float]:
    scale = max(
        abs(X_LO),
        abs(X_HI),
        abs(Y_LO),
        abs(Y_HI),
        abs(CX),
        abs(CY),
        RADIUS,
        F64_MIN_POSITIVE,
    )
    return 128.0 * F64_EPSILON * scale, scale


def sagitta(n: int) -> float:
    return 2.0 * RADIUS * math.sin(math.pi / (2.0 * n)) ** 2


def select_segments(requested_max_error: float, max_segments: int) -> tuple[int, float]:
    """RFC 0082 stable half-angle inverse plus the direct sagitta correction."""
    allowance, _ = evaluation_allowance()
    if not math.isfinite(requested_max_error) or requested_max_error <= allowance:
        raise ValueError(
            "requested maximum boundary error must exceed the evaluation allowance"
        )
    epsilon_effective = requested_max_error - allowance
    if epsilon_effective >= 2.0 * RADIUS:
        n = 8
    else:
        n = math.ceil(
            math.pi / (2.0 * math.asin(math.sqrt(epsilon_effective / (2.0 * RADIUS))))
        )
        n = max(n, 8)
        while n > 8 and sagitta(n - 1) <= epsilon_effective:
            n -= 1
        while sagitta(n) > epsilon_effective:
            n += 1
    if n > max_segments:
        raise ValueError(
            f"required segment count {n} exceeds the caller limit {max_segments}"
        )
    return n, epsilon_effective


# ---------------------------------------------------------------------------
# Topology
# ---------------------------------------------------------------------------
def cast_ray(theta: float) -> tuple[tuple[float, float], str]:
    """Cast from the circle centre along ``theta`` to the rectangle.

    The hit side's coordinate is assigned the exact bound; only the transverse
    coordinate is reconstructed from the ray parameter.
    """
    dx, dy = math.cos(theta), math.sin(theta)
    candidates: list[tuple[float, str]] = []
    if dx > 0.0:
        candidates.append(((X_HI - CX) / dx, "x_high"))
    if dx < 0.0:
        candidates.append(((X_LO - CX) / dx, "x_low"))
    if dy > 0.0:
        candidates.append(((Y_HI - CY) / dy, "y_high"))
    if dy < 0.0:
        candidates.append(((Y_LO - CY) / dy, "y_low"))
    candidates.sort()
    t, side = candidates[0]
    if len(candidates) > 1 and candidates[1][0] - t <= CLASSIFICATION_TOLERANCE:
        raise ValueError(f"ray at theta={theta!r} hits two sides within tolerance")
    if side == "x_high":
        point = (X_HI, CY + t * dy)
    elif side == "x_low":
        point = (X_LO, CY + t * dy)
    elif side == "y_high":
        point = (CX + t * dx, Y_HI)
    else:
        point = (CX + t * dx, Y_LO)
    return point, side


def corner_angle(corner: tuple[float, float]) -> float:
    angle = math.atan2(corner[1] - CY, corner[0] - CX)
    return angle + 2.0 * math.pi if angle < 0.0 else angle


def signed_area2(a, b, c) -> float:
    return (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])


def build(split: str, segments: int) -> dict:
    if split not in SPLITS:
        raise ValueError(f"unknown split {split!r}")

    inner: list[tuple[float, float]] = []
    outer: list[tuple[float, float]] = []
    outer_side: list[str] = []
    for i in range(segments):
        theta = 2.0 * math.pi * i / segments
        inner.append((CX + RADIUS * math.cos(theta), CY + RADIUS * math.sin(theta)))
        point, side = cast_ray(theta)
        outer.append(point)
        outer_side.append(side)

    corners = [(X_LO, Y_LO), (X_LO, Y_HI), (X_HI, Y_LO), (X_HI, Y_HI)]
    ray_angles = [2.0 * math.pi * i / segments for i in range(segments)]
    for corner in corners:
        angle = corner_angle(corner)
        for i, ray in enumerate(ray_angles):
            gap = min(abs(angle - ray), 2.0 * math.pi - abs(angle - ray))
            if gap * RADIUS <= CLASSIFICATION_TOLERANCE:
                raise ValueError(
                    f"corner {corner} coincides with ray {i}; corner reuse required"
                )

    # Outer loop in boundary-angle order, starting at theta = 0.
    outer_loop: list[tuple[tuple[float, float], str | None, int | None]] = []
    corner_after_ray: dict[int, list[tuple[float, float]]] = {}
    for corner in corners:
        angle = corner_angle(corner)
        index = (
            max(i for i, ray in enumerate(ray_angles) if ray < angle)
            if angle > 0.0
            else -1
        )
        if angle > ray_angles[-1]:
            index = segments - 1
        corner_after_ray.setdefault(index, []).append(corner)
    for lst in corner_after_ray.values():
        lst.sort(key=corner_angle)

    for i in range(segments):
        outer_loop.append((outer[i], outer_side[i], i))
        for corner in corner_after_ray.get(i, []):
            outer_loop.append((corner, None, None))

    vertices = list(inner) + [entry[0] for entry in outer_loop]
    outer_index_of_ray = {
        entry[2]: segments + position
        for position, entry in enumerate(outer_loop)
        if entry[2] is not None
    }
    corner_positions = {
        position: entry[0]
        for position, entry in enumerate(outer_loop)
        if entry[2] is None
    }

    cells: list[tuple[int, int, int]] = []
    for i in range(segments):
        j = (i + 1) % segments
        ii, ij = i, j
        oi, oj = outer_index_of_ray[i], outer_index_of_ray[j]
        if split == ACCEPTED:
            # RFC 0082: shared diagonal O_i--I_j, listed in the RFC's own order.
            cells.append((oi, oj, ij))
            cells.append((oi, ij, ii))
        else:
            # Wrong contract: the excluded diagonal I_i--O_j.
            cells.append((ii, oi, oj))
            cells.append((ii, oj, ij))
    for position, corner in sorted(corner_positions.items()):
        prev_position = (position - 1) % len(outer_loop)
        next_position = (position + 1) % len(outer_loop)
        cells.append(
            (segments + prev_position, segments + position, segments + next_position)
        )

    cells = [_rotate_to_min(cell) for cell in cells]
    for cell in cells:
        if signed_area2(*[vertices[k] for k in cell]) <= 0.0:
            raise ValueError(f"cell {cell} is not positively oriented")

    facets = _boundary_facets(vertices, cells, segments, outer_loop)
    return _document(split, segments, vertices, cells, facets)


def _rotate_to_min(cell: tuple[int, int, int]) -> tuple[int, int, int]:
    k = cell.index(min(cell))
    return (cell[k], cell[(k + 1) % 3], cell[(k + 2) % 3])


def _boundary_facets(vertices, cells, segments, outer_loop) -> list[dict]:
    seen: dict[frozenset[int], list[tuple[int, int, int]]] = {}
    for index, cell in enumerate(cells):
        for k in range(3):
            a, b = cell[k], cell[(k + 1) % 3]
            seen.setdefault(frozenset((a, b)), []).append((a, b, index))
    facets = []
    for key, uses in seen.items():
        if len(uses) != 1:
            continue
        a, b, cell_index = uses[0]
        facets.append(
            {
                "vertices": [a, b],
                "cell": cell_index,
                "entity": _side_of(vertices, a, b, segments),
            }
        )
    facets.sort(key=lambda f: (f["entity"], f["vertices"]))
    return facets


def _side_of(vertices, a: int, b: int, segments: int) -> str:
    pa, pb = vertices[a], vertices[b]
    if a < segments and b < segments:
        return "circle"
    if pa[0] == X_LO and pb[0] == X_LO:
        return "x_low"
    if pa[0] == X_HI and pb[0] == X_HI:
        return "x_high"
    if pa[1] == Y_LO and pb[1] == Y_LO:
        return "y_low"
    if pa[1] == Y_HI and pb[1] == Y_HI:
        return "y_high"
    raise ValueError(f"boundary facet ({a}, {b}) lies on no exact source entity")


def _document(split, segments, vertices, cells, facets) -> dict:
    spec = SPLITS[split]
    accepted_split = split == ACCEPTED
    allowance, scale = evaluation_allowance()
    accepted, epsilon_effective = select_segments(
        REQUESTED_MAX_BOUNDARY_ERROR, MAX_SEGMENTS
    )
    if accepted != segments:
        raise ValueError(f"policy selected {accepted} segments, not {segments}")
    entity_facets: dict[str, list[int]] = {name: [] for name in SOURCE_ENTITY_OF_SIDE}
    for index, facet in enumerate(facets):
        entity_facets[facet["entity"]].append(index)
    named = {}
    for name, sides in NAMED_SETS.items():
        members = sorted(i for side in sides for i in entity_facets[side])
        named[name] = {
            "dimension": 1,
            "source_entities": [SOURCE_ENTITY_OF_SIDE[s] for s in sides],
            "source_sides": list(sides),
            "facets": members,
        }
    named["fluid"] = {
        "dimension": 2,
        "source_entities": [0],
        "source_sides": ["face"],
        "cells": list(range(len(cells))),
    }
    if accepted_split:
        purpose = (
            "the one immutable source-bound copy of the accepted RFC 0082 chordal "
            "circular-hole reference mesh, reconstructed from the public RFC 0082 "
            "construction including its frozen O_i--I_j quad diagonal"
        )
        role_detail = (
            "authoritative: this is the single admissible reading of RFC 0082 and the "
            "only mesh the contract oracle routes may consume"
        )
    else:
        purpose = (
            "WRONG-CONTRACT FALSIFIER, not a mesh: the excluded I_i--O_j quad diagonal, "
            "retained only so the accepted probe and reaction observations can be shown "
            "to reject it"
        )
        role_detail = (
            "not admissible under RFC 0082 and not a second reading of it: the RFC fixes "
            "the shared diagonal as O_i--I_j. No oracle route, production path or "
            "consumer may treat this file as the RFC 0082 mesh. It exists only as the "
            "negative input of the wrong_quad_diagonal falsifier"
        )
    return {
        "schema": SCHEMA,
        "role": spec["role"],
        "role_detail": role_detail,
        "purpose": purpose,
        "construction": {
            "rfc": "0082",
            "segments": segments,
            "phase_rule": "theta_i = 2 * pi * i / n for i = 0 .. n-1",
            "ray_rule": (
                "cast from the circle centre along theta_i to the rectangle; the hit "
                "side's coordinate is assigned the exact bound and only the transverse "
                "coordinate is reconstructed as c + t * d"
            ),
            "quad_rule": RFC_QUAD_RULE,
            "quad_diagonal": spec["diagonal"],
            "quad_diagonal_name": spec["diagonal_name"],
            "quad_cells": spec["cells"],
            "quad_diagonal_status": (
                "frozen by RFC 0082 and satisfied by this file"
                if accepted_split
                else "violates the RFC 0082 frozen diagonal; this file is a falsifier"
            ),
            "corner_rule": (
                "rectangle corners crossed between rays are inserted in boundary-angle "
                "order; one fan triangle per crossed corner fills the area between the "
                "outer ray chord and the exact rectangle sides"
            ),
            "vertex_order": "inner circle vertices 0..n-1, then the outer loop in boundary-angle order from theta = 0",
            "cell_order": "the two triangles of each ray pair in the RFC's listing order, in ray order, then the crossed-corner fans in outer-loop order",
            "cell_rotation": "each triangle is rotated to start at its smallest vertex index, preserving the positive orientation",
            "facet_order": "sorted by (entity, vertex pair); the pair is the adjacent cell's directed edge, so the fluid lies to its left",
        },
        "source": {
            "schema": SOURCE_SCHEMA,
            "kind": SOURCE_KIND,
            "length_unit": "metre",
            "bounds_m": [[X_LO, X_HI], [Y_LO, Y_HI]],
            "circle_center_m": [CX, CY],
            "circle_radius_m": RADIUS,
            "classification_tolerance_m": CLASSIFICATION_TOLERANCE,
            "canonical_bytes": SOURCE_CANONICAL_BYTES,
            "sha256": SOURCE_SHA256,
            "sha256_provenance": (
                "cited from the accepted RFC 0081 frozen witness, not re-derived here: "
                "RFC 0079 does not publish the entity-set member spelling needed to "
                "rebuild the 511 canonical bytes without guessing"
            ),
        },
        "policy": {
            "requested_max_boundary_error_m": REQUESTED_MAX_BOUNDARY_ERROR,
            "min_mean_ratio_quality": MIN_MEAN_RATIO_QUALITY,
            "max_segments": MAX_SEGMENTS,
            "accepted_segments": accepted,
            "allowance_scale_m": scale,
            "evaluation_allowance_m": allowance,
            "epsilon_effective_m": epsilon_effective,
            "ideal_m": dict(IDEAL),
        },
        "counts": {
            "vertices": len(vertices),
            "cells": len(cells),
            "boundary_facets": len(facets),
            "interior_edges": (3 * len(cells) - len(facets)) // 2,
            "edges": (3 * len(cells) + len(facets)) // 2,
            "euler_characteristic": (
                len(vertices) - (3 * len(cells) + len(facets)) // 2 + len(cells)
            ),
        },
        "vertices_m": [list(v) for v in vertices],
        "cells": [list(c) for c in cells],
        "boundary_facets": [
            {"vertices": f["vertices"], "cell": f["cell"], "entity": f["entity"]}
            for f in facets
        ],
        "entity_sets": named,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--diagonal", choices=sorted(SPLITS), default=None)
    parser.add_argument("--out", type=pathlib.Path, default=None)
    args = parser.parse_args()

    here = pathlib.Path(__file__).resolve().parent
    wanted = [args.diagonal] if args.diagonal else [ACCEPTED, FALSIFIER]
    segments, _ = select_segments(REQUESTED_MAX_BOUNDARY_ERROR, MAX_SEGMENTS)
    for split in wanted:
        document = build(split, segments)
        payload = canonical_json.dump_bytes(document)
        target = args.out if args.out else here / SPLITS[split]["file"]
        target.write_bytes(payload)
        print(
            f"{target.name}  role={SPLITS[split]['role']}  "
            f"sha256={hashlib.sha256(payload).hexdigest()}  bytes={len(payload)}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
