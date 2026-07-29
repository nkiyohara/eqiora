#!/usr/bin/env python3
"""Independently re-derive and check the immutable chordal mesh copy.

The checker never imports ``build_mesh``. It reads the stored JSON and proves
its content from the RFC 0081 source facts and the RFC 0082 approximation and
topology contract:

- canonical serialization round-trip and SHA-256;
- every count, the Euler characteristic and the edge incidence;
- strict positive orientation of every triangle;
- consistent edge orientation (orientable manifold) and the boundary facet set;
- the circle predicate on the 50 chord vertices and the exact side predicate on
  the 54 outer vertices;
- **the frozen RFC 0082 quad diagonal**: each ray hit ``O_i`` is re-identified
  from the published cast rule, and the two cells of every adjacent ray pair are
  required to be ``(O_i, O_j, I_j)`` and ``(O_i, I_j, I_i)``, with ``O_i--I_j``
  an interior edge and ``I_i--O_j`` absent from the complex;
- the declared ``role``, so an accepted mesh and a wrong-contract falsifier can
  never be confused for one another;
- simplicity, strict convexity and centre containment of the chord loop;
- the measured symmetric Hausdorff bound against the requested error;
- the measured area and perimeter deficits against the frozen closed forms;
- the complete named partition and its RFC 0081 source-entity correspondence;
- the binary64 evaluation allowance, effective epsilon and segment minimality;
- the frozen 50-digit ideal values, re-derived at 60 digits.

Two files live here and they are **not** two admissible meshes.
``mesh.json`` is the one accepted mesh. ``falsifier-wrong-diagonal.json`` is the
excluded ``I_i--O_j`` split, kept only as the negative input of the
``wrong_quad_diagonal`` falsifier; the checker proves it violates the frozen
diagonal rather than merely differing from it.

Requires ``mpmath`` (the high-precision closed forms only); everything else is
standard library.

Usage::

    python3 check_mesh.py                       # both files, with their roles
    python3 check_mesh.py mesh.json             # one file, role read from it
"""

from __future__ import annotations

import hashlib
import json
import math
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import canonical_json  # noqa: E402

try:
    import mpmath
except ImportError:  # pragma: no cover - dependency declaration
    print("FATAL: check_mesh.py requires mpmath (pip install mpmath)", file=sys.stderr)
    raise SystemExit(2)

mpmath.mp.dps = 60

X_LO, X_HI, Y_LO, Y_HI = 0.0, 2.2, 0.0, 0.41
CX, CY, RADIUS = 0.2, 0.2, 0.05
TOL = 1e-12
SEGMENTS = 50
SOURCE_SHA256 = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"

ULP_ALLOWANCE = (
    1e-14  # generous binary64 drift allowance for libm-evaluated coordinates
)

ACCEPTED_ROLE = "accepted"
FALSIFIER_ROLE = "wrong-contract-falsifier"
EXPECTED_FILES = (
    ("mesh.json", ACCEPTED_ROLE),
    ("falsifier-wrong-diagonal.json", FALSIFIER_ROLE),
)
EXPECTED_DIAGONAL = {ACCEPTED_ROLE: "O_i--I_j", FALSIFIER_ROLE: "I_i--O_j"}
EXPECTED_DIAGONAL_NAME = {
    ACCEPTED_ROLE: "outer-i-to-inner-j",
    FALSIFIER_ROLE: "inner-i-to-outer-j",
}


class Checker:
    def __init__(self) -> None:
        self.passed = 0
        self.failures: list[str] = []

    def check(self, name: str, ok: bool, detail: str = "") -> bool:
        if ok:
            self.passed += 1
        else:
            self.failures.append(f"{name}: {detail}")
        return ok

    def close(self, name: str, got, want, allowance: float, detail: str = "") -> bool:
        delta = abs(mpmath.mpf(got) - mpmath.mpf(want))
        return self.check(
            name,
            delta <= allowance,
            f"|{got} - {want}| = {mpmath.nstr(delta, 6)} > {allowance} {detail}".strip(),
        )

    def close_rel(self, name: str, got, want, relative: float) -> bool:
        got, want = mpmath.mpf(got), mpmath.mpf(want)
        delta = abs(got - want) / abs(want)
        return self.check(
            name,
            delta <= relative,
            f"relative |{mpmath.nstr(got, 20)} - {mpmath.nstr(want, 20)}| = "
            f"{mpmath.nstr(delta, 6)} > {relative}",
        )


# The RFC 0082 closed forms are statements about the *exact* source circle, so
# they take the exact decimal radius 1/20. Using the binary64 spelling of 0.05
# instead shifts every ideal value by one relative half-ulp (2.78e-17 absolute
# on the radius), which is 1x for the r-linear sagitta and perimeter deficits
# and 2x for the r-quadratic area deficit. That is exactly the gap against the
# frozen RFC 0082 constants, and it is far below any mesh predicate here.
R_EXACT = mpmath.mpf(1) / 20


def sagitta(n) -> mpmath.mpf:
    return 2 * R_EXACT * mpmath.sin(mpmath.pi / (2 * n)) ** 2


def ideal_area_deficit(n) -> mpmath.mpf:
    return mpmath.pi * R_EXACT**2 - (mpmath.mpf(n) / 2) * R_EXACT**2 * mpmath.sin(
        2 * mpmath.pi / n
    )


def ideal_perimeter_deficit(n) -> mpmath.mpf:
    return 2 * mpmath.pi * R_EXACT - 2 * n * R_EXACT * mpmath.sin(mpmath.pi / n)


def cross(o, a, b) -> float:
    return (a[0] - o[0]) * (b[1] - o[1]) - (b[0] - o[0]) * (a[1] - o[1])


def point_segment_distance(p, a, b) -> float:
    vx, vy = b[0] - a[0], b[1] - a[1]
    wx, wy = p[0] - a[0], p[1] - a[1]
    denom = vx * vx + vy * vy
    t = 0.0 if denom == 0.0 else max(0.0, min(1.0, (wx * vx + wy * vy) / denom))
    return math.hypot(wx - t * vx, wy - t * vy)


def mean_ratio_quality(a, b, c) -> float:
    """4*sqrt(3)*area / sum of squared edge lengths; 1 for the equilateral cell."""
    area2 = cross(a, b, c)
    lsq = (
        (b[0] - a[0]) ** 2
        + (b[1] - a[1]) ** 2
        + (c[0] - b[0]) ** 2
        + (c[1] - b[1]) ** 2
        + (a[0] - c[0]) ** 2
        + (a[1] - c[1]) ** 2
    )
    return 2.0 * math.sqrt(3.0) * area2 / lsq


# ---------------------------------------------------------------------------
# The frozen RFC 0082 quad diagonal
# ---------------------------------------------------------------------------
def ray_cast(theta: float) -> tuple[tuple[float, float], str]:
    """Re-derive the RFC 0082 cast independently of the builder.

    The cast-axis coordinate is the exact rectangle bound; only the transverse
    coordinate is reconstructed as ``c + t * d``.
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
    t, side = min(candidates)
    if side == "x_high":
        return (X_HI, CY + t * dy), side
    if side == "x_low":
        return (X_LO, CY + t * dy), side
    if side == "y_high":
        return (CX + t * dx, Y_HI), side
    return (CX + t * dx, Y_LO), side


def canonical_cell(cell) -> tuple[int, int, int]:
    k = cell.index(min(cell))
    return (cell[k], cell[(k + 1) % 3], cell[(k + 2) % 3])


def identify_rays(vertices, ck: Checker) -> list[int] | None:
    """Map ray index -> stored outer vertex index, from the published cast rule.

    ``I_i`` is vertex ``i``: the chord phase rule checked above pins the inner
    loop to ``theta_i = 2 pi i / n`` in stored order. ``O_i`` is recovered by
    re-casting the ray and matching the unique outer vertex it lands on.
    """
    outer_of_ray: list[int] = []
    for i in range(SEGMENTS):
        want, _ = ray_cast(2.0 * math.pi * i / SEGMENTS)
        matches = [
            k
            for k in range(SEGMENTS, len(vertices))
            if math.dist(vertices[k], want) <= ULP_ALLOWANCE
        ]
        if len(matches) != 1:
            ck.check(
                "quad_ray_hits_identified",
                False,
                f"ray {i} matched {len(matches)} stored outer vertices",
            )
            return None
        outer_of_ray.append(matches[0])
    if not ck.check(
        "quad_ray_hits_identified",
        len(set(outer_of_ray)) == SEGMENTS,
        "two rays matched the same stored outer vertex",
    ):
        return None
    remaining = sorted(set(range(SEGMENTS, len(vertices))) - set(outer_of_ray))
    corners = {(X_LO, Y_LO), (X_LO, Y_HI), (X_HI, Y_LO), (X_HI, Y_HI)}
    ck.check(
        "quad_non_ray_outer_vertices_are_corners",
        {tuple(vertices[k]) for k in remaining} == corners,
        f"{[vertices[k] for k in remaining]}",
    )
    return outer_of_ray


def check_quad_diagonal(  # noqa: C901 - one linear predicate over the ray pairs
    doc, vertices, cells, undirected, ck: Checker, role: str
) -> None:
    """The RFC 0082 diagonal is now a checkable predicate; check it as one."""
    outer_of_ray = identify_rays(vertices, ck)
    if outer_of_ray is None:
        return
    present = {canonical_cell(c) for c in cells}

    accepted_hits = 0
    excluded_hits = 0
    diagonal_interior = 0
    excluded_absent = 0
    order_matches = 0
    for i in range(SEGMENTS):
        j = (i + 1) % SEGMENTS
        o_i, o_j, i_i, i_j = outer_of_ray[i], outer_of_ray[j], i, j
        accepted = (
            canonical_cell([o_i, o_j, i_j]),
            canonical_cell([o_i, i_j, i_i]),
        )
        excluded = (
            canonical_cell([i_i, o_i, o_j]),
            canonical_cell([i_i, o_j, i_j]),
        )
        if all(c in present for c in accepted):
            accepted_hits += 1
        if all(c in present for c in excluded):
            excluded_hits += 1
        if undirected.get(frozenset((o_i, i_j)), 0) == 2:
            diagonal_interior += 1
        if frozenset((i_i, o_j)) not in undirected:
            excluded_absent += 1
        if tuple(canonical_cell(c) for c in cells[2 * i : 2 * i + 2]) == accepted:
            order_matches += 1

    if role == ACCEPTED_ROLE:
        ck.check(
            "quad_diagonal_is_frozen_O_i_to_I_j",
            accepted_hits == SEGMENTS,
            f"{accepted_hits}/{SEGMENTS} ray pairs carry (O_i,O_j,I_j) and (O_i,I_j,I_i)",
        )
        ck.check(
            "quad_diagonal_edge_is_interior",
            diagonal_interior == SEGMENTS,
            f"{diagonal_interior}/{SEGMENTS} O_i--I_j edges are shared by two cells",
        )
        ck.check(
            "quad_excluded_diagonal_absent",
            excluded_absent == SEGMENTS,
            f"{SEGMENTS - excluded_absent} I_i--O_j edges are present in the complex",
        )
        ck.check(
            "quad_excluded_split_not_present",
            excluded_hits == 0,
            f"{excluded_hits} ray pairs carry the excluded split",
        )
        ck.check(
            "quad_cell_order_matches_rfc_listing",
            order_matches == SEGMENTS,
            f"{order_matches}/{SEGMENTS} ray pairs store (O_i,O_j,I_j) before (O_i,I_j,I_i)",
        )
    else:
        # The falsifier must *violate* the frozen rule, not merely differ from
        # the accepted file. Both directions are asserted.
        ck.check(
            "falsifier_violates_frozen_diagonal",
            accepted_hits == 0,
            f"{accepted_hits} ray pairs still satisfy the frozen RFC 0082 split",
        )
        ck.check(
            "falsifier_carries_excluded_split",
            excluded_hits == SEGMENTS,
            f"{excluded_hits}/{SEGMENTS} ray pairs carry the excluded I_i--O_j split",
        )
        ck.check(
            "falsifier_frozen_diagonal_edge_absent",
            diagonal_interior == 0,
            f"{diagonal_interior} O_i--I_j edges are interior in the falsifier",
        )

    declared = doc["construction"]["quad_diagonal"]
    ck.check(
        "quad_diagonal_declaration_matches_geometry",
        declared == EXPECTED_DIAGONAL[role],
        f"declared {declared!r} for role {role!r}",
    )
    declared_name = doc["construction"]["quad_diagonal_name"]
    ck.check(
        "quad_diagonal_name_matches_geometry",
        declared_name == EXPECTED_DIAGONAL_NAME[role],
        f"declared {declared_name!r} for role {role!r}",
    )


def check_mesh(  # noqa: C901 - one linear audit
    path: pathlib.Path, ck: Checker, expected_role: str | None
) -> dict:
    payload = path.read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    doc = json.loads(payload.decode("utf-8"))

    ck.check(
        "canonical_round_trip",
        canonical_json.dump_bytes(doc) == payload,
        "re-serializing the parsed document does not reproduce the stored bytes",
    )
    ck.check(
        "schema",
        doc["schema"] == "eqiora.verify/exact-circular-hole-stokes-2d/mesh/v1",
        repr(doc.get("schema")),
    )

    role = doc["role"]
    ck.check(
        "role_is_known",
        role in (ACCEPTED_ROLE, FALSIFIER_ROLE),
        repr(role),
    )
    if expected_role is not None:
        ck.check(
            "role_matches_filename",
            role == expected_role,
            f"{role!r} != {expected_role!r} for {path.name}",
        )

    vertices = [tuple(v) for v in doc["vertices_m"]]
    cells = [tuple(c) for c in doc["cells"]]
    facets = doc["boundary_facets"]
    counts = doc["counts"]
    policy = doc["policy"]
    source = doc["source"]

    # ---- counts -----------------------------------------------------------
    ck.check("count_vertices", len(vertices) == 104, str(len(vertices)))
    ck.check("count_cells", len(cells) == 104, str(len(cells)))
    ck.check("count_boundary_facets", len(facets) == 104, str(len(facets)))
    ck.check(
        "counts_match_arrays",
        counts["vertices"] == len(vertices)
        and counts["cells"] == len(cells)
        and counts["boundary_facets"] == len(facets),
        str(counts),
    )

    # ---- cell validity and orientation ------------------------------------
    bad_index = [
        c
        for c in cells
        if len(set(c)) != 3 or any(not 0 <= k < len(vertices) for k in c)
    ]
    ck.check("cell_indices_valid", not bad_index, str(bad_index[:3]))
    areas = [0.5 * cross(vertices[c[0]], vertices[c[1]], vertices[c[2]]) for c in cells]
    ck.check(
        "cells_positively_oriented",
        all(a > 0.0 for a in areas),
        f"min signed area {min(areas)!r}",
    )
    ck.check(
        "cell_rotation_canonical",
        all(c[0] == min(c) for c in cells),
        "a cell does not start at its smallest index",
    )
    ck.check(
        "all_vertices_used",
        len({k for c in cells for k in c}) == len(vertices),
        "orphan vertex present",
    )

    # ---- edge incidence and orientability ---------------------------------
    directed: dict[tuple[int, int], int] = {}
    undirected: dict[frozenset[int], int] = {}
    for index, c in enumerate(cells):
        for k in range(3):
            a, b = c[k], c[(k + 1) % 3]
            directed[(a, b)] = directed.get((a, b), 0) + 1
            undirected[frozenset((a, b))] = undirected.get(frozenset((a, b)), 0) + 1
    ck.check(
        "directed_edges_unique",
        all(v == 1 for v in directed.values()),
        "a directed edge is used twice (non-orientable or duplicated cell)",
    )
    ck.check(
        "edge_incidence",
        all(v in (1, 2) for v in undirected.values()),
        "an edge is shared by more than two cells",
    )
    boundary_edges = {k for k, v in undirected.items() if v == 1}
    interior = sum(1 for v in undirected.values() if v == 2)
    ck.check("count_interior_edges", interior == 104, str(interior))
    ck.check("count_edges_total", len(undirected) == 208, str(len(undirected)))
    ck.check(
        "boundary_edges_match_facets",
        boundary_edges == {frozenset(f["vertices"]) for f in facets},
        "facet inventory differs from the once-used edge set",
    )
    euler = len(vertices) - len(undirected) + len(cells)
    ck.check("euler_characteristic", euler == 0, str(euler))
    ck.check(
        "stored_euler_characteristic",
        counts["euler_characteristic"] == 0,
        str(counts["euler_characteristic"]),
    )

    facet_dir_ok = all(
        tuple(f["vertices"]) in directed and f["cell"] < len(cells) for f in facets
    )
    ck.check(
        "facet_direction_is_cell_edge",
        facet_dir_ok,
        "a facet pair is not a directed cell edge",
    )
    owner_ok = all(
        tuple(f["vertices"])
        in {(cells[f["cell"]][k], cells[f["cell"]][(k + 1) % 3]) for k in range(3)}
        for f in facets
    )
    ck.check(
        "facet_owner_cell", owner_ok, "a facet is not an edge of its recorded cell"
    )

    # ---- circle predicate on the chord vertices ---------------------------
    radii = [math.hypot(v[0] - CX, v[1] - CY) for v in vertices[:SEGMENTS]]
    ck.check(
        "chord_vertices_on_circle",
        all(abs(r - RADIUS) <= ULP_ALLOWANCE for r in radii),
        f"max |r-R| = {max(abs(r - RADIUS) for r in radii):.3e}",
    )
    phase_err = []
    for i, v in enumerate(vertices[:SEGMENTS]):
        want = 2.0 * math.pi * i / SEGMENTS
        got = math.atan2(v[1] - CY, v[0] - CX) % (2.0 * math.pi)
        phase_err.append(min(abs(got - want), 2.0 * math.pi - abs(got - want)))
    ck.check(
        "chord_phase_rule",
        max(phase_err) <= ULP_ALLOWANCE,
        f"max phase error {max(phase_err):.3e}",
    )

    loop = vertices[:SEGMENTS]
    convex = [
        cross(loop[i], loop[(i + 1) % SEGMENTS], loop[(i + 2) % SEGMENTS])
        for i in range(SEGMENTS)
    ]
    ck.check(
        "chord_loop_strictly_convex",
        all(x > 0.0 for x in convex),
        f"min turn {min(convex):.3e}",
    )
    centre_left = [
        cross(loop[i], loop[(i + 1) % SEGMENTS], (CX, CY)) for i in range(SEGMENTS)
    ]
    ck.check(
        "chord_loop_contains_centre",
        all(x > 0.0 for x in centre_left),
        "the centre is not strictly interior",
    )
    ck.check(
        "chord_loop_simple",
        len({(round(p[0], 15), round(p[1], 15)) for p in loop}) == SEGMENTS,
        "duplicate chord vertex",
    )

    d_min = min(
        point_segment_distance((CX, CY), loop[i], loop[(i + 1) % SEGMENTS])
        for i in range(SEGMENTS)
    )
    r_max = max(radii)
    hausdorff = max(RADIUS - d_min, r_max - RADIUS)
    ck.check(
        "measured_hausdorff_within_request",
        hausdorff <= policy["requested_max_boundary_error_m"],
        f"{hausdorff:.6e} > {policy['requested_max_boundary_error_m']:.6e}",
    )
    ck.close(
        "measured_sagitta_matches_closed_form", RADIUS - d_min, sagitta(SEGMENTS), 1e-15
    )

    # ---- side predicate on the outer vertices -----------------------------
    on_bound = []
    inside = []
    for v in vertices[SEGMENTS:]:
        on_bound.append(v[0] == X_LO or v[0] == X_HI or v[1] == Y_LO or v[1] == Y_HI)
        inside.append(X_LO <= v[0] <= X_HI and Y_LO <= v[1] <= Y_HI)
    ck.check(
        "outer_vertices_on_exact_bound",
        all(on_bound),
        "an outer vertex is not exactly on a rectangle bound",
    )
    ck.check(
        "outer_vertices_in_rectangle",
        all(inside),
        "an outer vertex lies outside the rectangle",
    )

    side_of = {
        "x_low": lambda p: p[0] == X_LO,
        "x_high": lambda p: p[0] == X_HI,
        "y_low": lambda p: p[1] == Y_LO,
        "y_high": lambda p: p[1] == Y_HI,
    }
    predicate_ok = True
    for f in facets:
        a, b = (vertices[f["vertices"][0]], vertices[f["vertices"][1]])
        entity = f["entity"]
        if entity == "circle":
            ok = (
                all(k < SEGMENTS for k in f["vertices"])
                and abs(math.hypot(a[0] - CX, a[1] - CY) - RADIUS) <= ULP_ALLOWANCE
            )
        else:
            ok = side_of[entity](a) and side_of[entity](b)
        predicate_ok = predicate_ok and ok
    ck.check(
        "facet_side_predicate",
        predicate_ok,
        "a facet's stored entity contradicts its coordinates",
    )

    outward_ok = True
    for f in facets:
        a, b = vertices[f["vertices"][0]], vertices[f["vertices"][1]]
        third = next(vertices[k] for k in cells[f["cell"]] if k not in f["vertices"])
        nx, ny = (b[1] - a[1]), -(b[0] - a[0])
        outward_ok = (
            outward_ok and (nx * (third[0] - a[0]) + ny * (third[1] - a[1])) < 0.0
        )
    ck.check(
        "facet_normal_points_out_of_its_cell",
        outward_ok,
        "a facet's right-hand normal is not parent-outward",
    )

    # ---- the frozen quad diagonal -----------------------------------------
    check_quad_diagonal(doc, vertices, cells, undirected, ck, role)

    # ---- named partition --------------------------------------------------
    sets = doc["entity_sets"]
    expected_sizes = {"inlet": 14, "outlet": 2, "walls": 38, "cylinder": 50}
    for name, size in expected_sizes.items():
        ck.check(
            f"set_size_{name}",
            len(sets[name]["facets"]) == size,
            f"{len(sets[name]['facets'])} != {size}",
        )
    ck.check(
        "set_fluid_covers_cells",
        sets["fluid"]["cells"] == list(range(len(cells))),
        "fluid does not cover every cell exactly once",
    )
    union: list[int] = []
    for name in expected_sizes:
        union.extend(sets[name]["facets"])
    ck.check(
        "named_partition_disjoint",
        len(union) == len(set(union)),
        "named boundary sets overlap",
    )
    ck.check(
        "named_partition_complete",
        set(union) == set(range(len(facets))),
        "named boundary sets do not cover every facet exactly once",
    )
    membership = {
        "inlet": {"x_low"},
        "outlet": {"x_high"},
        "walls": {"y_low", "y_high"},
        "cylinder": {"circle"},
    }
    for name, wanted in membership.items():
        got = {facets[i]["entity"] for i in sets[name]["facets"]}
        ck.check(
            f"set_entities_{name}", got == wanted, f"{sorted(got)} != {sorted(wanted)}"
        )
    source_entities = {
        "inlet": [0],
        "outlet": [1],
        "walls": [2, 3],
        "cylinder": [4],
        "fluid": [0],
    }
    for name, wanted in source_entities.items():
        ck.check(
            f"source_entity_{name}",
            sets[name]["source_entities"] == wanted,
            str(sets[name]["source_entities"]),
        )

    # ---- measured areas and deficits --------------------------------------
    total_area = sum(areas)
    poly_area = 0.5 * sum(
        cross((CX, CY), loop[i], loop[(i + 1) % SEGMENTS]) for i in range(SEGMENTS)
    )
    rect_area = (X_HI - X_LO) * (Y_HI - Y_LO)
    ck.close(
        "mesh_area_is_rectangle_minus_polygon", total_area, rect_area - poly_area, 1e-14
    )
    perimeter = sum(
        math.dist(loop[i], loop[(i + 1) % SEGMENTS]) for i in range(SEGMENTS)
    )
    ck.close(
        "measured_area_deficit",
        mpmath.pi * RADIUS**2 - poly_area,
        ideal_area_deficit(SEGMENTS),
        1e-16,
    )
    ck.close(
        "measured_perimeter_deficit",
        2 * mpmath.pi * RADIUS - perimeter,
        ideal_perimeter_deficit(SEGMENTS),
        1e-15,
    )

    qualities = [
        mean_ratio_quality(vertices[c[0]], vertices[c[1]], vertices[c[2]])
        for c in cells
    ]
    ck.check(
        "cell_quality_gate",
        min(qualities) >= policy["min_mean_ratio_quality"],
        f"min mean-ratio {min(qualities):.6e}",
    )

    # ---- approximation policy ---------------------------------------------
    scale = max(
        abs(X_LO),
        abs(X_HI),
        abs(Y_LO),
        abs(Y_HI),
        abs(CX),
        abs(CY),
        RADIUS,
        sys.float_info.min,
    )
    allowance = 128.0 * sys.float_info.epsilon * scale
    ck.check(
        "allowance_scale",
        policy["allowance_scale_m"] == scale,
        f"{policy['allowance_scale_m']!r} != {scale!r}",
    )
    ck.check(
        "evaluation_allowance",
        policy["evaluation_allowance_m"] == allowance,
        f"{policy['evaluation_allowance_m']!r} != {allowance!r}",
    )
    eps_eff = policy["requested_max_boundary_error_m"] - allowance
    ck.check(
        "epsilon_effective",
        policy["epsilon_effective_m"] == eps_eff,
        f"{policy['epsilon_effective_m']!r} != {eps_eff!r}",
    )
    ck.check(
        "request_exceeds_allowance",
        policy["requested_max_boundary_error_m"] > allowance,
        "request must be strictly greater than the allowance",
    )
    ck.check(
        "segments_sufficient",
        sagitta(SEGMENTS) <= eps_eff,
        f"sagitta(50) = {mpmath.nstr(sagitta(SEGMENTS), 12)}",
    )
    ck.check(
        "segments_minimal",
        sagitta(SEGMENTS - 1) > eps_eff,
        f"sagitta(49) = {mpmath.nstr(sagitta(SEGMENTS - 1), 12)}",
    )
    ck.check(
        "accepted_segments",
        policy["accepted_segments"] == SEGMENTS,
        str(policy["accepted_segments"]),
    )
    ck.check(
        "max_segments", policy["max_segments"] == SEGMENTS, str(policy["max_segments"])
    )

    # The frozen strings carry 50 significant digits, so they are compared
    # relatively at 1e-48 against closed forms re-derived here at 60 digits.
    ideal = policy["ideal_m"]
    ck.close_rel(
        "ideal_sagitta_n49", mpmath.mpf(ideal["sagitta_n49_m"]), sagitta(49), 1e-48
    )
    ck.close_rel(
        "ideal_sagitta_n50", mpmath.mpf(ideal["sagitta_n50_m"]), sagitta(50), 1e-48
    )
    ck.close_rel(
        "ideal_area_deficit_n50",
        mpmath.mpf(ideal["area_deficit_n50_m2"]),
        ideal_area_deficit(50),
        1e-48,
    )
    ck.close_rel(
        "ideal_perimeter_deficit_n50",
        mpmath.mpf(ideal["perimeter_deficit_n50_m"]),
        ideal_perimeter_deficit(50),
        1e-48,
    )

    # ---- source facts -----------------------------------------------------
    ck.check(
        "source_schema",
        source["schema"] == "eqiora.planar-circular-hole-envelope/v1",
        source["schema"],
    )
    ck.check("source_sha256", source["sha256"] == SOURCE_SHA256, source["sha256"])
    ck.check(
        "source_bounds_strict",
        X_LO < X_HI and Y_LO < Y_HI,
        "bounds must increase strictly",
    )
    ck.check(
        "source_radius_positive",
        RADIUS > 0.0 and TOL > 0.0,
        "radius and tolerance must be positive",
    )
    clearances = [CX - X_LO, X_HI - CX, CY - Y_LO, Y_HI - CY]
    ck.check(
        "source_circle_strictly_interior",
        all(d > RADIUS + TOL for d in clearances),
        f"min clearance {min(clearances)!r}",
    )
    ck.check(
        "source_bounds_match_document",
        source["bounds_m"] == [[X_LO, X_HI], [Y_LO, Y_HI]],
        str(source["bounds_m"]),
    )
    ck.check(
        "source_circle_match_document",
        source["circle_center_m"] == [CX, CY] and source["circle_radius_m"] == RADIUS,
        str(source["circle_center_m"]),
    )

    print(f"  {path.name}  role={role}  sha256={digest}  bytes={len(payload)}")
    return doc


def check_pair(docs: dict[str, dict], ck: Checker) -> None:
    """Cross-file facts that make the wrong-diagonal falsifier decisive."""
    accepted = [name for name, d in docs.items() if d["role"] == ACCEPTED_ROLE]
    ck.check(
        "exactly_one_accepted_mesh",
        len(accepted) == 1,
        f"{len(accepted)} files declare role {ACCEPTED_ROLE!r}",
    )
    if set(docs) != {name for name, _ in EXPECTED_FILES}:
        return
    good = docs["mesh.json"]
    bad = docs["falsifier-wrong-diagonal.json"]
    ck.check(
        "falsifier_shares_the_accepted_vertices",
        good["vertices_m"] == bad["vertices_m"],
        "the falsifier must differ only in connectivity",
    )
    ck.check(
        "falsifier_shares_the_accepted_boundary_facets",
        [f["vertices"] for f in good["boundary_facets"]]
        == [f["vertices"] for f in bad["boundary_facets"]]
        and [f["entity"] for f in good["boundary_facets"]]
        == [f["entity"] for f in bad["boundary_facets"]],
        "the falsifier must share the boundary trace, so flux alone cannot detect it",
    )
    differing = sum(
        1
        for c in bad["cells"]
        if canonical_cell(c) not in {canonical_cell(g) for g in good["cells"]}
    )
    ck.check(
        "falsifier_differs_in_the_ray_pair_cells_only",
        differing == 2 * SEGMENTS,
        f"{differing} cells differ; the 4 crossed-corner fans must be shared",
    )


def main() -> int:
    here = pathlib.Path(__file__).resolve().parent
    args = sys.argv[1:]
    if args:
        selected = [(pathlib.Path(a), None) for a in args]
    else:
        selected = [(here / name, role) for name, role in EXPECTED_FILES]
        missing = [p.name for p, _ in selected if not p.exists()]
        if missing:
            print(f"missing mesh file(s): {missing}", file=sys.stderr)
            return 2
    ck = Checker()
    docs: dict[str, dict] = {}
    for path, expected_role in selected:
        docs[path.name] = check_mesh(path, ck, expected_role)
    check_pair(docs, ck)
    print(f"mesh checks: {ck.passed} passed, {len(ck.failures)} failed")
    for failure in ck.failures:
        print(f"  FAIL {failure}")
    print(f"mesh.result={'pass' if not ck.failures else 'fail'}")
    return 0 if not ck.failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
