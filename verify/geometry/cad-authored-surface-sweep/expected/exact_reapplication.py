#!/usr/bin/env python3
"""Clean-room exact reapplication oracle for geometry.cad-authored-surface-sweep.

Independent evidence lane, post-freeze addendum, written from the frozen public
claim only: the admitted box, the six outward face cycles, the sizing predicate,
the exact planar lift, the inward/distance/grading/offset rules, and the
vertex-index, loop-order, sorted-label split, and orientation rules. It imports
nothing from the repository and reads no production output, no fixture, and no
implementation source. Standard library only, run from the repository root::

    python3 verify/geometry/cad-authored-surface-sweep/expected/exact_reapplication.py

Acceptance reinterprets every generated binary64 coordinate exactly as the dyadic
rational it already is (``Fraction.from_float``) and evaluates determinants,
sums, and volumes in exact rational arithmetic. No binary64 reduction is an
acceptance oracle here, and no observed binary64 value is frozen as a constant.
The claim does not fix u-outer versus v-outer square emission, and no value
derived here depends on that choice. Exit 0: all six faces matched. Exit 1:
deviations, all reported. Exit 2: CONTRACT_DEFECT -- the exact determinant sums
do not telescope to the exact box volume, which would make the frozen claim
itself unreachable rather than the implementation wrong.
"""

from __future__ import annotations

import math
import sys
from collections import Counter
from fractions import Fraction
from functools import reduce

Q = Fraction

# --- Frozen public inputs, restated. Nothing below is read from the repository.

# Authored rectangle x [-2,3] m, y [-1,2] m, z0 0.5 m, depth 4 m.
BOX = ((-2.0, 3.0), (-1.0, 2.0), (0.5, 4.5))
AXIS = ("x", "y", "z")
FACES = tuple(f"{a}_{s}" for a in AXIS for s in ("lower", "upper"))

SURFACE_TARGET_EDGE_M = 2.0
LAYERS = 2
GROWTH = 3.0
MAXIMUM_TETRAHEDRA = 144

# Frozen per-pair witness values from case.toml, keyed by sweep axis; both faces
# of a pair carry the same counts through distinct source, inward, and mesh
# identities. Tuple order: surface V, surface T, sweep distance m, volume V, E,
# F, T, boundary facets, interior facets.
FROZEN_PAIR = {
    "x": (16, 18, 5.0, 48, 197, 258, 108, 84, 174),
    "y": (20, 24, 3.0, 60, 255, 340, 144, 104, 236),
    "z": (20, 24, 4.0, 60, 255, 340, 144, 104, 236),
}
FROZEN_OFFSETS = {"x": [0.0, 1.25, 5.0], "y": [0.0, 0.75, 3.0], "z": [0.0, 1.0, 4.0]}
# Boundary facets on x_lower, x_upper, y_lower, y_upper, z_lower, z_upper.
FROZEN_INVENTORY = {
    "x": (18, 18, 12, 12, 12, 12),
    "y": (12, 12, 24, 24, 16, 16),
    "z": (12, 12, 16, 16, 24, 24),
}

# The two named cap witnesses: origin m, u_hat, v_hat, outward_hat, u intervals,
# v intervals, middle plane m, target plane m. Inward is checked against the
# negated frozen outward, which is the frozen rule itself.
FROZEN_FRAME = {
    "z_lower": ((-2.0, -1.0, 0.5), (0, 1, 0), (1, 0, 0), (0, 0, -1), 3, 4, 1.5, 4.5),
    "z_upper": ((-2.0, -1.0, 4.5), (1, 0, 0), (0, 1, 0), (0, 0, 1), 4, 3, 3.5, 0.5),
}

# The ideal-real minimum determinant of each pair -- what a grid whose every axis
# divides exactly in binary64 would reach -- and the realized exact dyadic
# minimum the generated coordinates attain. Whether the two coincide is derived
# per axis below, never assumed.
IDEAL_REAL_MINIMUM = {"x": Q(5, 3), "y": Q(5, 4), "z": Q(5, 4)}
REALIZED_MINIMUM = {
    "x": Q(5, 3) - Q(5, 3 * 2**54),
    "y": Q(5, 4) - Q(5, 2**56),
    "z": Q(5, 4),
}

# Frozen exact determinant histogram and ordered cells of the primary end cap.
FROZEN_CAP_HISTOGRAM = {Q(5, 4): 72, Q(15, 4): 72}
FROZEN_FIRST = "0,6,1,26 0,1,21,26 0,21,20,26 20,26,21,46 20,21,41,46 20,41,40,46"
FROZEN_LAST = "13,18,19,39 13,38,18,39 13,33,38,39 33,38,39,59 33,58,38,59 33,53,58,59"

# 37 is coprime with 144 and with 108, so i -> 37*i mod N is a bijection.
STRIDE = 37

FAILURES: list[str] = []


def check(label: str, got: object, want: object) -> None:
    """Record a deviation instead of aborting, so one run reports all of them."""
    if got != want:
        FAILURES.append(f"{label}: got {got!r}, want {want!r}")


def exact(value: float) -> Fraction:
    """Reinterpret a generated binary64 value as the dyadic rational it is."""
    if not math.isfinite(value):
        raise AssertionError(f"non-finite generated coordinate {value!r}")
    return Q.from_float(value)


def text(value: Fraction) -> str:
    return f"{value.numerator}/{value.denominator}"


def cells_of(spec: str) -> tuple:
    return tuple(tuple(int(v) for v in c.split(",")) for c in spec.split())


def unit(axis: int, sign: int) -> tuple[float, float, float]:
    return tuple(float(sign) if k == axis else 0.0 for k in range(3))


def face_frame(normal_axis: int, upper: bool) -> dict:
    """Right-handed face frame with u_hat x v_hat = outward_hat, origin at the
    (u, v) = (0, 0) corner, both intrinsic axes positive coordinate axes: the
    only such assignment whose cross product is the outward normal."""
    if upper:
        u_axis, v_axis = (normal_axis + 1) % 3, (normal_axis + 2) % 3
    else:
        u_axis, v_axis = (normal_axis + 2) % 3, (normal_axis + 1) % 3
    outward = unit(normal_axis, 1 if upper else -1)
    u_hat, v_hat = unit(u_axis, 1), unit(v_axis, 1)
    cross = tuple(
        u_hat[(k + 1) % 3] * v_hat[(k + 2) % 3]
        - u_hat[(k + 2) % 3] * v_hat[(k + 1) % 3]
        for k in range(3)
    )
    if cross != outward:
        raise AssertionError("face cycle is not right-handed against outward")
    origin = [BOX[k][0] for k in range(3)]
    origin[normal_axis] = BOX[normal_axis][1 if upper else 0]
    return {
        "name": f"{AXIS[normal_axis]}_{'upper' if upper else 'lower'}",
        "axes": (u_axis, v_axis, normal_axis),
        "origin_m": tuple(origin),
        "u_hat": u_hat,
        "v_hat": v_hat,
        "outward_hat": outward,
        "inward_hat": tuple(-c for c in outward),
        "lengths_m": (
            BOX[u_axis][1] - BOX[u_axis][0],
            BOX[v_axis][1] - BOX[v_axis][0],
        ),
        "opposite_bound_m": BOX[normal_axis][0 if upper else 1],
        "distance_m": BOX[normal_axis][1] - BOX[normal_axis][0],
    }


def coordinates(length: float, n: int) -> list[float]:
    """x_i = i*s for i < n and x_n = L, with s = L/n, all in binary64."""
    s = length / n
    return [i * s for i in range(n)] + [length]


def select_intervals(length: float, target: float) -> int:
    """Least n whose maximum realized gap satisfies hypot(D, D) <= h. The libm
    call and an exact rational 2*D^2 <= h^2 test are both taken and required to
    agree, so no sizing decision depends on libm bits."""
    for n in range(1, 50001):
        coords = coordinates(length, n)
        gap = max(b - a for a, b in zip(coords, coords[1:]))
        accepts = math.hypot(gap, gap) <= target
        label = f"sizing predicate agreement L={length} n={n}"
        check(label, accepts, 2 * exact(gap) ** 2 <= exact(target) ** 2)
        if accepts:
            return n
    raise AssertionError(f"no interval count accepted for L={length}")


def divides_exactly(length: float, n: int) -> bool:
    """Do the generated coordinates land on the exact rationals i*L/n? True only
    when L/n is representable. Where it is false the realized cell sizes carry the
    rounding of L/n, so a per-cell ideal-real reference is unreachable in exact
    arithmetic -- while the telescoped sum over the axis still equals L exactly,
    the first coordinate being exactly 0 and the last exactly L."""
    ideal = exact(length)
    return all(exact(x) == ideal * i / n for i, x in enumerate(coordinates(length, n)))


def layer_offsets(distance: float, layers: int, growth: float) -> list[float]:
    """delta(k+1)/delta(k) = growth from the source inward, normalised, with
    offset_0 exactly 0 and the final offset exactly the distance by rule rather
    than by luck; intermediate offsets are generated binary64."""
    weights = [growth**k for k in range(layers)]
    total = math.fsum(weights)
    offsets, running = [0.0], 0.0
    for k in range(layers - 1):
        running += weights[k]
        offsets.append(distance * (running / total))
    offsets.append(distance)
    if any(not (math.isfinite(b) and b > a) for a, b in zip(offsets, offsets[1:])):
        raise AssertionError(f"offsets not strictly increasing: {offsets}")
    return offsets


def determinant(points, cell):
    """Signed xyz determinant: exact over Fraction points, binary64 over float
    points. The expression is identical, only the arithmetic differs."""
    p0, p1, p2, p3 = (points[k] for k in cell)
    a = [p1[k] - p0[k] for k in range(3)]
    b = [p2[k] - p0[k] for k in range(3)]
    c = [p3[k] - p0[k] for k in range(3)]
    return (
        a[0] * (b[1] * c[2] - b[2] * c[1])
        - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0])
    )


def emit_cells(triangles, surface_count, points):
    """Source triangle outer loop, slab inner loop, global-label staircase split.
    Sorting each triangle's global labels before splitting is what makes every
    shared vertical quad take the bottom-min to top-max diagonal, so adjacent
    prism stacks conform. A negative determinant swaps entries 1 and 2."""
    cells, determinants = [], []
    for triangle in triangles:
        sorted_labels = sorted(triangle)
        for layer in range(LAYERS):
            # [b0, b1, b2, t0, t1, t2]; the staircase is (0,1,2,5), (0,1,4,5), (0,3,4,5)
            stack = [
                (layer + d) * surface_count + s for d in (0, 1) for s in sorted_labels
            ]
            for spec in ((0, 1, 2, 5), (0, 1, 4, 5), (0, 3, 4, 5)):
                cell = [stack[k] for k in spec]
                det = determinant(points, cell)
                if det < 0:
                    cell, det = [cell[0], cell[2], cell[1], cell[3]], -det
                if det == 0:
                    raise AssertionError(f"degenerate cell {cell}")
                cells.append(tuple(cell))
                determinants.append(det)
    return cells, determinants


def build_face(frame: dict) -> dict:
    u_axis, v_axis, n_axis = frame["axes"]
    u_length, v_length = frame["lengths_m"]
    origin = frame["origin_m"]
    nu = select_intervals(u_length, SURFACE_TARGET_EDGE_M)
    nv = select_intervals(v_length, SURFACE_TARGET_EDGE_M)

    # Exact affine lift origin + u*u_hat + v*v_hat, evaluated in binary64.
    surface = []
    for v in coordinates(v_length, nv):
        for u in coordinates(u_length, nu):
            point = list(origin)
            point[u_axis], point[v_axis] = origin[u_axis] + u, origin[v_axis] + v
            surface.append(point)

    # k = j*(nu+1)+i, u-fast; per square emit (b, c, a) then (d, a, c).
    triangles = []
    for j in range(nv):
        for i in range(nu):
            a = j * (nu + 1) + i
            b, c, d = a + 1, a + nu + 2, a + nu + 1
            triangles += [(b, c, a), (d, a, c)]

    offsets = layer_offsets(frame["distance_m"], LAYERS, GROWTH)
    inward_sign = -1.0 if frame["outward_hat"][n_axis] > 0 else 1.0
    planes = [origin[n_axis] + inward_sign * off for off in offsets[:-1]]
    planes.append(frame["opposite_bound_m"])  # snap to the opposite exact bound

    # Volume vertex index = layer * V + source index.
    vertices = [
        tuple(exact(plane if k == n_axis else point[k]) for k in range(3))
        for plane in planes
        for point in surface
    ]
    cells, determinants = emit_cells(triangles, len(surface), vertices)
    return {
        "grid": (nu, nv),
        "surface": (len(surface), len(triangles), 3 * len(triangles) * LAYERS),
        "offsets_m": offsets,
        "planes_m": planes,
        "vertices": vertices,
        "cells": cells,
        "determinants": determinants,
        "exact_grid": divides_exactly(u_length, nu) and divides_exactly(v_length, nv),
    }


# --- Topology and bounded-face boundary inventory

FACET_LOCAL = ((0, 1, 2), (0, 1, 3), (0, 2, 3), (1, 2, 3))
EDGE_LOCAL = ((0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3))


def on_bounded_face(points, facet, name: str) -> bool:
    """Bounded face membership, never infinite plane membership: every vertex
    sits exactly on the face plane AND inside its closed bounded rectangle."""
    axis = AXIS.index(name[0])
    plane = exact(BOX[axis][1 if name.endswith("upper") else 0])
    inside = [(exact(low), exact(high)) for low, high in BOX]
    return all(
        points[i][axis] == plane
        and all(lo <= points[i][k] <= hi for k, (lo, hi) in enumerate(inside))
        for i in facet
    )


def topology(face: dict) -> dict:
    edges, multiplicity = set(), {}
    for cell in face["cells"]:
        for i, j in EDGE_LOCAL:
            edges.add(tuple(sorted((cell[i], cell[j]))))
        for i, j, k in FACET_LOCAL:
            key = tuple(sorted((cell[i], cell[j], cell[k])))
            multiplicity[key] = multiplicity.get(key, 0) + 1

    boundary = [f for f, m in multiplicity.items() if m == 1]
    inventory, unclassified = dict.fromkeys(FACES, 0), 0
    for facet in boundary:
        hits = [n for n in FACES if on_bounded_face(face["vertices"], facet, n)]
        if len(hits) == 1:
            inventory[hits[0]] += 1
        else:
            unclassified += 1
    return {
        "edges": len(edges),
        "facets": len(multiplicity),
        "boundary": len(boundary),
        "interior": sum(1 for m in multiplicity.values() if m == 2),
        "inventory": tuple(inventory.values()),
        # Facets shared by three or more cells, and boundary facets that land on
        # no bounded face or on more than one: both must be zero.
        "defects": (sum(1 for m in multiplicity.values() if m > 2), unclassified),
    }


def permute(items, stride: int):
    if math.gcd(stride, len(items)) != 1:
        raise AssertionError(f"stride {stride} is not a bijection")
    return [items[(i * stride) % len(items)] for i in range(len(items))]


def run_face(normal_axis: int, upper: bool, box_volume: Fraction) -> dict:
    frame = face_frame(normal_axis, upper)
    name = frame["name"]
    pair = name[0]
    face = build_face(frame)
    topo = topology(face)
    sv, st, distance, vv, ve, vf, vt, bf, itf = FROZEN_PAIR[pair]
    dets = face["determinants"]
    total, counts, low = sum(dets, Q(0)), Counter(dets), min(dets)
    grid, offsets, planes = face["exact_grid"], face["offsets_m"], face["planes_m"]
    nvert, ncell = len(face["vertices"]), len(face["cells"])

    def ck(label, got, want):
        check(f"{name} {label}", got, want)

    ck("surface V/T and required tetrahedra", face["surface"], (sv, st, vt))
    ck("required within caller maximum", face["surface"][2] <= MAXIMUM_TETRAHEDRA, True)
    ck("layer offsets", offsets, FROZEN_OFFSETS[pair])
    ck("offset endpoints", (offsets[0], offsets[-1]), (0.0, distance))
    ck("target plane snap", planes[-1], frame["opposite_bound_m"])
    ck(
        "volume V/E/F/T",
        (nvert, topo["edges"], topo["facets"], ncell),
        (vv, ve, vf, vt),
    )
    ck("boundary/interior facets", (topo["boundary"], topo["interior"]), (bf, itf))
    ck("boundary inventory", topo["inventory"], FROZEN_INVENTORY[pair])
    ck("over-shared and unclassified facets", topo["defects"], (0, 0))
    ck("Euler characteristic", nvert - topo["edges"] + topo["facets"] - ncell, 1)
    ck("non-positive exact determinants", sum(1 for d in dets if d <= 0), 0)
    ck("exact determinant sum", text(total), text(6 * box_volume))
    ck("exact volume (m^3)", text(total / 6), text(box_volume))
    ck("minimum exact determinant", low, REALIZED_MINIMUM[pair])
    ck("minimum reaches ideal-real", low == IDEAL_REAL_MINIMUM[pair], grid)
    ck("distinct exact determinants", len(counts), 2 if grid else 4)

    if name in FROZEN_FRAME:
        origin, u_hat, v_hat, out, wnu, wnv, middle, target = FROZEN_FRAME[name]
        ck("origin_m", frame["origin_m"], origin)
        inward = tuple(-c for c in out)
        for key, want in (
            ("u_hat", u_hat),
            ("v_hat", v_hat),
            ("outward_hat", out),
            ("inward_hat", inward),
        ):
            ck(key, frame[key], tuple(float(c) for c in want))
        ck("u/v intervals", face["grid"], (wnu, wnv))
        ck("middle and target planes", (planes[1], planes[-1]), (middle, target))

    # The exact coverage is invariant under reassociation of the same cells.
    for label, order in (
        ("reversal", list(reversed(face["cells"]))),
        (f"stride-{STRIDE}", permute(face["cells"], STRIDE)),
    ):
        moved = [abs(determinant(face["vertices"], c)) for c in order]
        ck(f"determinant sum under {label}", text(sum(moved, Q(0))), text(total))
        ck(f"determinant multiset under {label}", Counter(moved), counts)

    return {
        "name": name,
        "frame": frame,
        "face": face,
        "topology": topo,
        "sum": total,
        "minimum": low,
        "histogram": counts,
    }


def print_face(result: dict) -> None:
    frame, face, topo = result["frame"], result["face"], result["topology"]
    inv, ideal = topo["inventory"], IDEAL_REAL_MINIMUM[result["name"][0]]
    n_axis = frame["axes"][2]
    inward = f"{'+' if frame['inward_hat'][n_axis] > 0 else '-'}{AXIS[n_axis]}"
    minimum = text(result["minimum"])
    if not face["exact_grid"]:
        minimum += f" = {ideal} - {text(ideal - result['minimum'])}, below ideal-real"
    print(
        f"{result['name']}  origin {frame['origin_m']} inward {inward} "
        f"grid {face['grid'][0]}x{face['grid'][1]} planes {face['planes_m']}  V/E/F/T "
        f"{len(face['vertices'])}/{topo['edges']}/{topo['facets']}/{len(face['cells'])}"
        f"  euler 1  boundary/interior {topo['boundary']}/{topo['interior']}"
    )
    print(
        f"  inventory x {inv[0]}/{inv[1]} y {inv[2]}/{inv[3]} z {inv[4]}/{inv[5]}"
        f"  det sum {text(result['sum'])}  volume {text(result['sum'] / 6)} m^3"
        f"  {len(face['cells'])} positive dets, {len(result['histogram'])} distinct"
        f"  minimum {minimum}"
    )


def print_fold_note(face: dict) -> None:
    """Short NON-GATING explanation of the naive binary64 quotient fold: reported
    so the observation is not rediscovered as a bug, never asserted, never
    compared against a frozen constant, never used to accept or reject."""
    points = [tuple(float(c) for c in v) for v in face["vertices"]]
    terms = [abs(determinant(points, cell)) / 6.0 for cell in face["cells"]]
    fold = [
        reduce(lambda a, b: a + b, order, 0.0)
        for order in (terms, sorted(terms), sorted(terms, reverse=True))
    ]
    print("NON-GATING binary64 note (reported, never asserted, never a constant)")
    print(
        f"  naive left fold of fl(det/6) over the same {len(terms)} terms: emission "
        f"{fold[0]!r}, ascending {fold[1]!r}, descending {fold[2]!r}; compensated "
        f"{math.fsum(terms)!r}. A quantity that moves under reassociation of the "
        "same terms cannot be an acceptance oracle; the exact rational volume above "
        "is the order-independent one, and no fold value is frozen anywhere."
    )


def main() -> int:
    box_volume = Q(1)
    for low, high in BOX:
        box_volume *= exact(high) - exact(low)
    print(
        f"clean-room exact reapplication -- geometry.cad-authored-surface-sweep\n"
        f"box x {BOX[0]} y {BOX[1]} z {BOX[2]}; exact volume {text(box_volume)} m^3; "
        f"edge {SURFACE_TARGET_EDGE_M} m, layers {LAYERS}, growth {GROWTH}, maximum "
        f"{MAXIMUM_TETRAHEDRA}; binary64 coordinates read exactly as dyadic rationals\n"
    )

    results = []
    for normal_axis in range(3):
        for upper in (False, True):
            results.append(run_face(normal_axis, upper, box_volume))
            print_face(results[-1])

    if any(r["sum"] != 6 * box_volume for r in results):
        sums = " ".join(f"{r['name']}={text(r['sum'])}" for r in results)
        print(f"\nCONTRACT_DEFECT: sums do not telescope to the box: {sums}")
        return 2

    cap = next(r for r in results if r["name"] == "z_upper")
    check("z_upper determinant histogram", cap["histogram"], FROZEN_CAP_HISTOGRAM)
    check("z_upper first six", tuple(cap["face"]["cells"][:6]), cells_of(FROZEN_FIRST))
    check("z_upper last six", tuple(cap["face"]["cells"][-6:]), cells_of(FROZEN_LAST))
    print(
        f"\nexact coverage: determinant sum {text(6 * box_volume)} and volume "
        f"{text(box_volume)} m^3 on all six faces, laterals included. z_upper "
        "primary: histogram 5/4 x72 and 15/4 x72; frozen first and last six "
        "oriented cells reproduced.\n"
    )
    print_fold_note(cap["face"])

    if FAILURES:
        print(f"\nFAIL: {len(FAILURES)} deviation(s)")
        for failure in FAILURES:
            print(f"  {failure}")
        return 1
    print("\nPASS: every derived value matched every frozen value on all six faces.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
