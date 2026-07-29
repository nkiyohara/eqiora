#!/usr/bin/env python3
"""Independent Python analytic oracle for the Eqiora exact circular-hole
steady Stokes 2D slice.

Steady coherent-SI Stokes on the exact circular-hole geometry, assembled from
closed-form affine MINI/P1 cell blocks and solved at elevated precision. This
route was written without reading any production implementation, any existing
``verify/fluid`` case, or the Julia route. Its only inputs are the frozen
contract witness, RFCs 0043/0045/0047/0081/0082, and the immutable mesh copy under
``../../mesh``.

    python3 oracle.py            # check, solve, falsify, rewrite result.json
    python3 oracle.py --check    # fail if result.json would change; write nothing

Requires ``mpmath``; everything else is standard library.

STATUS: **frozen, single route.** RFC 0082 now fixes the shared quad diagonal as
``O_i--I_j`` with cells ``(O_i, O_j, I_j)`` and ``(O_i, I_j, I_i)``, so exactly
one mesh is admissible and every observation below is frozen against it under
the precommitted contract tolerances.

This is **one** of the two routes the frozen contract requires. The Julia route
has not run, so no route-to-route agreement is claimed and **the dual
independent oracle gate has not passed**. Nothing here may be read as that
gate's result.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
MESH_DIR = HERE.parents[1] / "mesh"
sys.path.insert(0, str(MESH_DIR))
sys.path.insert(0, str(HERE))

try:
    import mpmath
except ImportError:  # pragma: no cover - dependency declaration
    print("FATAL: oracle.py requires mpmath (pip install mpmath)", file=sys.stderr)
    raise SystemExit(2)

import canonical_json  # noqa: E402
import stokes  # noqa: E402

DPS = 40
mpmath.mp.dps = DPS

SCHEMA = "eqiora.verify/exact-circular-hole-stokes-2d/route/python/v1"

# ---------------------------------------------------------------------------
# Frozen physical witness (frozen contract). Inputs are the exact binary64 spellings;
# all arithmetic below runs at DPS decimal digits.
# ---------------------------------------------------------------------------
MU = mpmath.mpf(0.001)  # kg/(m s)
H = mpmath.mpf(0.41)  # m, channel height
UMAX = mpmath.mpf(0.3)  # m/s
L = H
U = UMAX
P = MU * U / L  # Pa
G = U / L  # 1/s
THETA = P * U * L  # W/m
MU_HAT = MU * U / (P * L)
ORIGIN = (mpmath.mpf(0), mpmath.mpf(0))  # exact lower bounds

ISSUE_P = 0.0007317073170731707
ISSUE_G = 0.7317073170731707
ISSUE_THETA = 0.00009

VELOCITY_TARGETS = [
    (mpmath.mpf("0.10"), mpmath.mpf("0.20")),
    (mpmath.mpf("0.20"), mpmath.mpf("0.30")),
    (mpmath.mpf("0.30"), mpmath.mpf("0.20")),
    (mpmath.mpf("1.00"), mpmath.mpf("0.20")),
    (mpmath.mpf("2.00"), mpmath.mpf("0.20")),
]
OUTER_TARGETS = {
    "outer_nearest_x_low_mid": (mpmath.mpf("0"), mpmath.mpf("0.20")),
    "outer_nearest_x_high_mid": (mpmath.mpf("2.2"), mpmath.mpf("0.20")),
}

# Precommitted tolerance table (frozen contract).
SCALE_OF = {
    "velocity": (mpmath.mpf("2e-12"), U),
    "pressure": (mpmath.mpf("2e-14"), P),
    "flux": (mpmath.mpf("2e-13"), U * L),
    "reaction": (mpmath.mpf("2e-14"), P * L),
}
ROUTE_RELATIVE = mpmath.mpf("2e-10")
PRODUCTION_RELATIVE = mpmath.mpf("5e-7")

SOLVER = {
    "backend": "eqiora.reference",
    "algorithm": "MinimumResidual",
    "preconditioner": "Identity",
    "reduction": "Reproducible",
    "scalar": "f64",
    "relative_tolerance": 1e-11,
    "absolute_tolerance": 1e-13,
    "max_iterations": 10000,
}
F64_EPS = sys.float_info.epsilon

# The one accepted mesh, and the excluded split kept only as a falsifier input.
MESH_FILE = "mesh.json"
FALSIFIER_MESH_FILE = "falsifier-wrong-diagonal.json"
PINNED_MESH_SHA256 = {
    MESH_FILE: "ada2d08cde5b4e6bd13c97d3b76a45cad810d8eb7acf0f0edc82cd605acd2b39",
    FALSIFIER_MESH_FILE: (
        "eccb5642eab811cee1cad0cee8749f7f2a64d16ab300b041fa4efcbe7b61cd2f"
    ),
}
EXPECTED_ROLE = {MESH_FILE: "accepted", FALSIFIER_MESH_FILE: "wrong-contract-falsifier"}
EXPECTED_DIAGONAL = {MESH_FILE: "O_i--I_j", FALSIFIER_MESH_FILE: "I_i--O_j"}


def tolerance(kind: str, relative) -> mpmath.mpf:
    floor, scale = SCALE_OF[kind]
    return floor + relative * scale


class Checks:
    def __init__(self) -> None:
        self.records: list[dict] = []

    def check(self, name: str, ok: bool, detail: str = "") -> bool:
        self.records.append({"name": name, "passed": bool(ok), "detail": detail})
        return bool(ok)

    def below(self, name: str, value, limit, detail: str = "") -> bool:
        ok = self.check(
            name,
            abs(value) <= limit,
            detail
            or f"|{mpmath.nstr(value, 8)}| <= {mpmath.nstr(mpmath.mpf(limit), 8)}",
        )
        # Keep the magnitude, so the ledger is quantitative rather than boolean.
        self.records[-1]["measured"] = f2(abs(value))
        self.records[-1]["limit"] = f2(limit)
        return ok

    @property
    def failed(self) -> list[dict]:
        return [r for r in self.records if not r["passed"]]

    @property
    def passed(self) -> int:
        return sum(1 for r in self.records if r["passed"])


# ---------------------------------------------------------------------------
# Mesh contract
# ---------------------------------------------------------------------------


class Mesh:
    """The immutable mesh copy, revalidated independently of ``check_mesh.py``."""

    def __init__(self, document: dict) -> None:
        self.doc = document
        self.vertices = [
            (mpmath.mpf(x), mpmath.mpf(y)) for x, y in document["vertices_m"]
        ]
        self.cells = [tuple(c) for c in document["cells"]]
        self.facets = [
            (tuple(f["vertices"]), f["cell"], f["entity"])
            for f in document["boundary_facets"]
        ]
        self.sets = document["entity_sets"]

    def facet_indices(self, name: str) -> list[int]:
        return list(self.sets[name]["facets"])

    def facet_vertices(self, name: str) -> set[int]:
        return {v for i in self.facet_indices(name) for v in self.facets[i][0]}

    def outward(self, facet_index: int):
        """Parent-outward normal times facet length, from the adjacent fluid cell.

        The stored pair is the adjacent cell's directed edge, so the fluid lies
        to its left and the right-hand normal points out of the parent cell.
        """
        (a, b), _, _ = self.facets[facet_index]
        pa, pb = self.vertices[a], self.vertices[b]
        return (pb[1] - pa[1], -(pb[0] - pa[0]))

    def permuted(self, vertex_shift: int, cell_shift: int) -> "Mesh":
        """A pure reindexing: identical geometry, different vertex/cell labels."""
        n_v, n_c = len(self.vertices), len(self.cells)
        sigma = [(7 * v + vertex_shift) % n_v for v in range(n_v)]
        tau = [(11 * c + cell_shift) % n_c for c in range(n_c)]
        assert len(set(sigma)) == n_v and len(set(tau)) == n_c
        vertices = [None] * n_v
        for old, new in enumerate(sigma):
            vertices[new] = self.doc["vertices_m"][old]
        cells = [None] * n_c
        for old, new in enumerate(tau):
            c = self.cells[old]
            rotated = (sigma[c[1]], sigma[c[2]], sigma[c[0]])  # also rotate the triple
            cells[new] = list(rotated)
        facets = []
        for (a, b), cell, entity in self.facets:
            facets.append(
                {"vertices": [sigma[a], sigma[b]], "cell": tau[cell], "entity": entity}
            )
        order = sorted(
            range(len(facets)),
            key=lambda i: (facets[i]["entity"], facets[i]["vertices"]),
        )
        remap = {old: new for new, old in enumerate(order)}
        sets = {}
        for name, spec in self.sets.items():
            new_spec = dict(spec)
            if "facets" in spec:
                new_spec["facets"] = sorted(remap[i] for i in spec["facets"])
            if "cells" in spec:
                new_spec["cells"] = sorted(tau[c] for c in spec["cells"])
            sets[name] = new_spec
        doc = dict(self.doc)
        doc["vertices_m"] = vertices
        doc["cells"] = cells
        doc["boundary_facets"] = [facets[i] for i in order]
        doc["entity_sets"] = sets
        return Mesh(doc)


def revalidate(mesh: Mesh, ck: Checks, tag: str) -> None:
    """Structural revalidation inside the route, independent of check_mesh.py."""
    v, c, f = len(mesh.vertices), len(mesh.cells), len(mesh.facets)
    ck.check(f"{tag}.counts", (v, c, f) == (104, 104, 104), f"{(v, c, f)}")
    areas = [
        stokes.cell_geometry(*[mesh.vertices[k] for k in cell]).area
        for cell in mesh.cells
    ]
    ck.check(f"{tag}.cells_positive", all(a > 0 for a in areas), "")
    undirected: dict[frozenset[int], int] = {}
    directed: set[tuple[int, int]] = set()
    for cell in mesh.cells:
        for k in range(3):
            a, b = cell[k], cell[(k + 1) % 3]
            ck_key = frozenset((a, b))
            undirected[ck_key] = undirected.get(ck_key, 0) + 1
            directed.add((a, b))
    ck.check(f"{tag}.orientable", len(directed) == 3 * c, "a directed edge repeats")
    boundary = {k for k, n in undirected.items() if n == 1}
    ck.check(
        f"{tag}.boundary_matches_facets",
        boundary == {frozenset(fa[0]) for fa in mesh.facets},
        "",
    )
    ck.check(f"{tag}.euler", v - len(undirected) + c == 0, str(v - len(undirected) + c))
    sizes = {
        name: len(mesh.facet_indices(name))
        for name in ("inlet", "outlet", "walls", "cylinder")
    }
    ck.check(
        f"{tag}.named_sizes",
        sizes == {"inlet": 14, "outlet": 2, "walls": 38, "cylinder": 50},
        str(sizes),
    )
    ck.check(f"{tag}.fluid_cells", mesh.sets["fluid"]["cells"] == sorted(range(c)), "")
    union = [
        i
        for name in ("inlet", "outlet", "walls", "cylinder")
        for i in mesh.facet_indices(name)
    ]
    ck.check(
        f"{tag}.partition",
        sorted(union) == list(range(f)),
        "boundary partition is not exact",
    )


# ---------------------------------------------------------------------------
# Boundary data
# ---------------------------------------------------------------------------


def inlet_profile(y):
    """``g(y) = 4 Umax y (H - y) / H^2``."""
    return 4 * UMAX * y * (H - y) / H**2


def boundary_plan(
    mesh: Mesh, system: stokes.System, ck: Checks, tag: str, swap: bool = False
):
    """Essential closure of the velocity facets plus the prescribed P1 trace.

    With ``swap`` the inlet and outlet memberships are exchanged, which is the
    frozen swapped-membership falsifier.
    """
    inlet_name, outlet_name = ("outlet", "inlet") if swap else ("inlet", "outlet")
    inlet = mesh.facet_vertices(inlet_name)
    walls = mesh.facet_vertices("walls")
    cylinder = mesh.facet_vertices("cylinder")
    essential = inlet | walls | cylinder
    prescribed: dict[int, mpmath.mpf] = {}
    for v in sorted(essential):
        y = mesh.vertices[v][1]
        candidates = []
        if v in inlet:
            candidates.append((inlet_profile(y), mpmath.mpf(0)))
        if v in walls or v in cylinder:
            candidates.append((mpmath.mpf(0), mpmath.mpf(0)))
        first = candidates[0]
        for other in candidates[1:]:
            if max(abs(first[0] - other[0]), abs(first[1] - other[1])) > mpmath.mpf(
                "1e-30"
            ):
                ck.check(
                    f"{tag}.trace_closure_consistent",
                    False,
                    f"vertex {v} has conflicting traces",
                )
        prescribed[system.vel_p1(v, 0)] = first[0]
        prescribed[system.vel_p1(v, 1)] = first[1]
    if not swap:
        ck.check(
            f"{tag}.trace_closure_consistent", True, "inlet/wall corners agree at u = 0"
        )
        ck.check(
            f"{tag}.essential_vertices", len(essential) == 103, str(len(essential))
        )
        free_vertices = sorted(set(range(len(mesh.vertices))) - essential)
        ck.check(
            f"{tag}.free_velocity_vertex_is_outlet_midpoint",
            len(free_vertices) == 1
            and free_vertices[0] in mesh.facet_vertices(outlet_name),
            str(free_vertices),
        )
    outlet_facets = [mesh.facets[i][0] for i in mesh.facet_indices(outlet_name)]
    return prescribed, outlet_facets, inlet_name, outlet_name


# ---------------------------------------------------------------------------
# Geometric selectors (index free; invariant under reindexing)
# ---------------------------------------------------------------------------


def coordinate_key(mesh: Mesh, vertex: int):
    return (mesh.vertices[vertex][0], mesh.vertices[vertex][1])


def select_cell(mesh: Mesh, target):
    """Minimum squared barycentre distance; exact ties break lexicographically."""
    ranked = []
    for c, cell in enumerate(mesh.cells):
        bx = sum((mesh.vertices[k][0] for k in cell), mpmath.mpf(0)) / 3
        by = sum((mesh.vertices[k][1] for k in cell), mpmath.mpf(0)) / 3
        d2 = (bx - target[0]) ** 2 + (by - target[1]) ** 2
        key = sorted(coordinate_key(mesh, k) for k in cell)
        ranked.append((d2, key, c, (bx, by)))
    ranked.sort(key=lambda r: (r[0], r[1]))
    best = ranked[0]
    ties = sum(1 for r in ranked if r[0] == best[0])
    margin = ranked[1][0] - best[0] if len(ranked) > 1 else mpmath.mpf(0)
    return best[2], best[3], ties, margin


def select_extreme(mesh: Mesh, vertices: list[int], component: int, largest: bool):
    """Extreme coordinate over a vertex set; exact ties break lexicographically."""
    values = [mesh.vertices[v][component] for v in vertices]
    target = max(values) if largest else min(values)
    tied = [v for v in vertices if mesh.vertices[v][component] == target]
    chosen = min(tied, key=lambda v: coordinate_key(mesh, v))
    rest = [x for x in values if x != target]
    margin = (target - max(rest)) if largest else (min(rest) - target)
    return chosen, tied, abs(margin)


def select_nearest(mesh: Mesh, vertices: list[int], target):
    ranked = []
    for v in vertices:
        p = mesh.vertices[v]
        d2 = (p[0] - target[0]) ** 2 + (p[1] - target[1]) ** 2
        ranked.append((d2, coordinate_key(mesh, v), v))
    ranked.sort(key=lambda r: (r[0], r[1]))
    best = ranked[0]
    tied = [r[2] for r in ranked if r[0] == best[0]]
    return best[2], tied, ranked[1][0] - best[0]


# ---------------------------------------------------------------------------
# One complete solution and its observations
# ---------------------------------------------------------------------------


def solve_case(
    mesh: Mesh,
    ck: Checks,
    tag: str,
    form: stokes.Formulation | None = None,
    swap: bool = False,
    cross_check_direct_lu: bool = False,
):
    form = form or stokes.Formulation()
    system = stokes.assemble(mesh.vertices, mesh.cells, MU, form)
    prescribed, outlet_facets, inlet_name, outlet_name = boundary_plan(
        mesh, system, ck, tag, swap
    )
    traction = (mpmath.mpf(0), mpmath.mpf(0))  # zero outlet traction, Pa
    integrated_traction = stokes.add_facet_traction(
        system, mesh.vertices, outlet_facets, traction
    )

    matrix, rhs, free = stokes.build_reduced(system, prescribed)
    if form.include_bubble:
        x = stokes.condensed_solve(system, prescribed)
        route = "static-condensation+dense-LU"
    else:
        x = stokes.restore(system, prescribed, free, mpmath.lu_solve(matrix, rhs))
        route = "dense-LU"

    if cross_check_direct_lu:
        direct, _, _, _ = stokes.solve_reduced(system, prescribed)
        gap = max(abs(a - b) for a, b in zip(x, direct))
        ck.below(f"{tag}.solve_routes_agree", gap, mpmath.mpf("1e-25"))

    return {
        "mesh": mesh,
        "system": system,
        "prescribed": prescribed,
        "x": x,
        "free": free,
        "matrix": matrix,
        "rhs": rhs,
        "integrated_traction": integrated_traction,
        "inlet_name": inlet_name,
        "outlet_name": outlet_name,
        "solve_route": route,
        "form": form,
    }


def observe(case, ck: Checks, tag: str, full: bool = True) -> dict:  # noqa: C901 - one linear report
    mesh, system, x = case["mesh"], case["system"], case["x"]
    prescribed, free = case["prescribed"], case["free"]

    # --- residual / reactions on the uneliminated full system ---------------
    action = system.apply(x)
    residual = [
        action[i] - system.load.get(i, mpmath.mpf(0)) for i in range(system.size)
    ]

    def scale_of_dof(dof: int):
        return P if dof >= 2 * system.n_vertices + 2 * system.n_cells else U

    reduced_residual = [residual[i] for i in free]
    dimensionless = [scale_of_dof(dof) * residual[dof] / THETA for dof in free]
    true_residual = mpmath.sqrt(sum((r**2 for r in dimensionless), mpmath.mpf(0)))

    pressure_rows = [system.pressure(v) for v in range(system.n_vertices)]
    weak_physical = mpmath.sqrt(
        sum((residual[r] ** 2 for r in pressure_rows), mpmath.mpf(0))
    )
    weak_dimensionless = weak_physical * P / THETA

    # solver target and roundoff allowance on the dimensionless reduced system
    b_hat = [scale_of_dof(dof) * case["rhs"][k] / THETA for k, dof in enumerate(free)]
    x_hat = [x[dof] / scale_of_dof(dof) for dof in free]
    b_hat_norm = mpmath.sqrt(sum((v**2 for v in b_hat), mpmath.mpf(0)))
    b_hat_inf = max(abs(v) for v in b_hat)
    x_hat_inf = max(abs(v) for v in x_hat)
    n = len(free)
    a_hat_inf = mpmath.mpf(0)
    for k in range(n):
        row_sum = mpmath.mpf(0)
        sk = scale_of_dof(free[k])
        for j in range(n):
            value = case["matrix"][k, j]
            if value != 0:
                row_sum += abs(sk * value * scale_of_dof(free[j]) / THETA)
        a_hat_inf = max(a_hat_inf, row_sum)
    target = max(
        mpmath.mpf(SOLVER["relative_tolerance"]) * b_hat_norm,
        mpmath.mpf(SOLVER["absolute_tolerance"]),
    )
    roundoff = 4096 * F64_EPS * (1 + a_hat_inf * x_hat_inf + b_hat_inf)

    if full:
        ck.below(f"{tag}.true_residual", true_residual, target + roundoff)
        ck.below(
            f"{tag}.weak_continuity_residual", weak_dimensionless, target + roundoff
        )
        asym = mpmath.mpf(0)
        for k in range(n):
            for j in range(k + 1, n):
                asym = max(asym, abs(case["matrix"][k, j] - case["matrix"][j, k]))
        ck.check(
            f"{tag}.reduced_matrix_symmetric",
            asym == 0,
            f"max asymmetry {mpmath.nstr(asym, 6)}",
        )

    reactions = {dof: residual[dof] for dof in prescribed}

    def sum_reaction(vertex_set) -> list:
        out = [mpmath.mpf(0), mpmath.mpf(0)]
        for v in vertex_set:
            for d in range(2):
                out[d] += reactions.get(system.vel_p1(v, d), mpmath.mpf(0))
        return out

    cylinder_vertices = sorted(mesh.facet_vertices("cylinder"))
    cylinder_reaction = sum_reaction(cylinder_vertices)
    all_reaction = [mpmath.mpf(0), mpmath.mpf(0)]
    for dof, value in reactions.items():
        all_reaction[dof % 2] += value
    body_force = [mpmath.mpf(0), mpmath.mpf(0)]  # zero force potential
    total = [
        all_reaction[d] + body_force[d] + case["integrated_traction"][d]
        for d in range(2)
    ]

    # --- fluxes -------------------------------------------------------------
    def signed_flux(name: str):
        acc = mpmath.mpf(0)
        for i in mesh.facet_indices(name):
            (a, b), _, _ = mesh.facets[i]
            nx, ny = mesh.outward(i)
            ua = (x[system.vel_p1(a, 0)], x[system.vel_p1(a, 1)])
            ub = (x[system.vel_p1(b, 0)], x[system.vel_p1(b, 1)])
            acc += ((ua[0] + ub[0]) / 2) * nx + ((ua[1] + ub[1]) / 2) * ny
        return acc

    inlet_flux = signed_flux(case["inlet_name"])
    outlet_flux = signed_flux(case["outlet_name"])

    # --- probes -------------------------------------------------------------
    velocity_probes = []
    for target_point in VELOCITY_TARGETS:
        cell_index, barycentre, ties, margin = select_cell(mesh, target_point)
        cell = mesh.cells[cell_index]
        value = []
        for d in range(2):
            p1 = sum((x[system.vel_p1(k, d)] for k in cell), mpmath.mpf(0)) / 3
            bubble = (
                x[system.vel_bubble(cell_index, d)]
                if case["form"].include_bubble
                else mpmath.mpf(0)
            )
            # beta(barycentre) = bubble_scale / 27; the frozen normalization is 27
            value.append(p1 + bubble * mpmath.mpf(case["form"].bubble_scale) / 27)
        velocity_probes.append(
            {
                "target_m": [f2(target_point[0]), f2(target_point[1])],
                "cell": cell_index,
                "barycentre_m": [f2(barycentre[0]), f2(barycentre[1])],
                "velocity_m_s": [f2(value[0]), f2(value[1])],
                "tied_cells": ties,
                "selection_margin_m2": f2(margin),
                "_raw": value,
            }
        )

    cylinder_list = cylinder_vertices
    outer_list = sorted(
        {v for i, fa in enumerate(mesh.facets) if fa[2] != "circle" for v in fa[0]}
    )
    pressure_probes = []
    for name, vertices, comp, largest in (
        ("cylinder_min_x", cylinder_list, 0, False),
        ("cylinder_max_x", cylinder_list, 0, True),
        ("cylinder_min_y", cylinder_list, 1, False),
        ("cylinder_max_y", cylinder_list, 1, True),
    ):
        vertex, tied, margin = select_extreme(mesh, vertices, comp, largest)
        pressure_probes.append(
            _pressure_record(mesh, system, x, name, vertex, tied, margin)
        )
    for name, target_point in OUTER_TARGETS.items():
        vertex, tied, margin = select_nearest(mesh, outer_list, target_point)
        pressure_probes.append(
            _pressure_record(mesh, system, x, name, vertex, tied, margin)
        )
    if full:
        ck.check(
            f"{tag}.pressure_ties_break_lexicographically",
            all(
                p["vertex"] == min(p["_tied"], key=lambda v: coordinate_key(mesh, v))
                for p in pressure_probes
            ),
            "a tied pressure probe did not take the lexicographic minimum of the "
            "stored binary64 coordinates",
        )

    if full:
        ck.below(f"{tag}.flux_balance", inlet_flux + outlet_flux, mpmath.mpf("1e-8"))
        for d in range(2):
            ck.below(f"{tag}.momentum_balance_{'xy'[d]}", total[d], mpmath.mpf("1e-10"))
        ck.check(
            f"{tag}.cylinder_reaction_orientation",
            -cylinder_reaction[0] > 0,
            "fluid force on the cylinder must act along +x",
        )
        ck.check(
            f"{tag}.inlet_flux_is_inflow", inlet_flux < 0, "inlet flux must be negative"
        )
        ck.check(
            f"{tag}.outlet_flux_is_outflow",
            outlet_flux > 0,
            "outlet flux must be positive",
        )

    return {
        "solve_route": case["solve_route"],
        "velocity_probes": velocity_probes,
        "pressure_probes": pressure_probes,
        "signed_flux_m2_s": {
            "inlet": f2(inlet_flux),
            "outlet": f2(outlet_flux),
            "sum": f2(inlet_flux + outlet_flux),
            "continuous_inlet_reference": f2(-2 * UMAX * H / 3),
        },
        "cylinder_reaction_N_m": {
            "constraint_force_on_fluid": [
                f2(cylinder_reaction[0]),
                f2(cylinder_reaction[1]),
            ],
            "fluid_force_on_cylinder": [
                f2(-cylinder_reaction[0]),
                f2(-cylinder_reaction[1]),
            ],
            "convention": "the reported residual is the constraint force on the fluid; the fluid force on the cylinder is its componentwise negative",
        },
        "global_balance_N_m": {
            "constrained_reaction": [f2(all_reaction[0]), f2(all_reaction[1])],
            "integrated_body_force": [f2(body_force[0]), f2(body_force[1])],
            "integrated_traction": [
                f2(case["integrated_traction"][0]),
                f2(case["integrated_traction"][1]),
            ],
            "sum": [f2(total[0]), f2(total[1])],
        },
        "residuals": {
            "true_reduced_dimensionless": f2(true_residual),
            "weak_pressure_row_dimensionless": f2(weak_dimensionless),
            "weak_pressure_row_physical_m2_s": f2(weak_physical),
            "solver_selected_target": f2(target),
            "roundoff_allowance": f2(roundoff),
            "reduced_rhs_2norm_dimensionless": f2(b_hat_norm),
            "reduced_matrix_inf_norm_dimensionless": f2(a_hat_inf),
            "solution_inf_norm_dimensionless": f2(x_hat_inf),
        },
        "pressure_reference": {
            "kind": "BoundaryTraction",
            "gauge_row_present": False,
            "gauge_multiplier_present": False,
            "zero_integral_constraint_present": False,
            "traction_partition_facets": len(mesh.facet_indices(case["outlet_name"])),
            "traction_partition_nonempty": True,
            "reduced_system_rows": len(free),
        },
        "_raw": {
            "cylinder_reaction": cylinder_reaction,
            "all_reaction": all_reaction,
            "inlet_flux": inlet_flux,
            "outlet_flux": outlet_flux,
            "reduced_residual": reduced_residual,
        },
    }


def _pressure_record(mesh, system, x, name, vertex, tied, margin) -> dict:
    p = x[system.pressure(vertex)]
    record = {
        "name": name,
        "vertex": vertex,
        "position_m": [f2(mesh.vertices[vertex][0]), f2(mesh.vertices[vertex][1])],
        "pressure_Pa": f2(p),
        "tied_vertices": len(tied),
        "selection_margin": f2(margin),
        "_raw": p,
        "_tied": list(tied),
    }
    if len(tied) > 1:
        # An exact tie is resolved by the contract's lexicographic rule applied to the
        # *stored* binary64 coordinates. The tied candidates are emitted because
        # they carry materially different pressures: a one-ulp difference in a
        # reimplementation would silently select the other vertex.
        record["tie_break"] = {
            "rule": "lexicographic order of the stored binary64 coordinates",
            "candidates": [
                {
                    "vertex": v,
                    "position_m": [f2(mesh.vertices[v][0]), f2(mesh.vertices[v][1])],
                    "pressure_Pa": f2(x[system.pressure(v)]),
                }
                for v in sorted(tied, key=lambda v: coordinate_key(mesh, v))
            ],
        }
    return record


def f2(value) -> float:
    """Round an elevated-precision value to its binary64 spelling for output."""
    return float(mpmath.mpf(value))


def hp(value) -> str:
    return mpmath.nstr(mpmath.mpf(value), 30, strip_zeros=False)


# ---------------------------------------------------------------------------
# Cross-cutting comparisons
# ---------------------------------------------------------------------------

COMPARISONS = [
    (
        "velocity",
        lambda o: [p["_raw"][d] for p in o["velocity_probes"] for d in range(2)],
    ),
    ("pressure", lambda o: [p["_raw"] for p in o["pressure_probes"]]),
    ("flux", lambda o: [o["_raw"]["inlet_flux"], o["_raw"]["outlet_flux"]]),
    (
        "reaction",
        lambda o: (
            list(o["_raw"]["cylinder_reaction"]) + list(o["_raw"]["all_reaction"])
        ),
    ),
]


def compare(left: dict, right: dict, relative) -> dict:
    """Componentwise comparison under the contract's tolerance table."""
    worst = {}
    for kind, extract in COMPARISONS:
        limit = tolerance(kind, relative)
        a, b = extract(left), extract(right)
        deltas = [abs(x - y) for x, y in zip(a, b)]
        peak = max(deltas) if deltas else mpmath.mpf(0)
        worst[kind] = {
            "max_abs_difference": f2(peak),
            "tolerance": f2(limit),
            "ratio_to_tolerance": f2(peak / limit),
            "within_tolerance": bool(peak <= limit),
        }
    return worst


def congruence_check(case, mesh: Mesh, ck: Checks, tag: str) -> dict:
    """RFC 0045: directly assembled dimensionless algebra equals ``D A D / Theta``."""
    normalized = [
        ((v[0] - ORIGIN[0]) / L, (v[1] - ORIGIN[1]) / L) for v in mesh.vertices
    ]
    hat = stokes.assemble(normalized, mesh.cells, MU_HAT, case["form"])
    system = case["system"]
    worst = mpmath.mpf(0)
    magnitude = mpmath.mpf(0)
    npd = 2 * system.n_vertices + 2 * system.n_cells
    for row, cols in system.matrix.items():
        srow = P if row >= npd else U
        for col, value in cols.items():
            scol = P if col >= npd else U
            want = srow * value * scol / THETA
            got = hat.matrix.get(row, {}).get(col, mpmath.mpf(0))
            worst = max(worst, abs(got - want))
            magnitude = max(magnitude, abs(want))
    ck.below(
        f"{tag}.congruence_A_hat_equals_DAD_over_Theta",
        worst / magnitude,
        mpmath.mpf("1e-30"),
    )
    ck.check(f"{tag}.mu_hat_is_one", abs(MU_HAT - 1) < mpmath.mpf("1e-30"), hp(MU_HAT))
    return {
        "max_relative_coefficient_difference": f2(worst / magnitude),
        "mu_hat": f2(MU_HAT),
        "normalized_origin_m": [f2(ORIGIN[0]), f2(ORIGIN[1])],
        "coordinate_scale_m": f2(L),
    }


# ---------------------------------------------------------------------------
# Falsifiers
# ---------------------------------------------------------------------------


def run_falsifiers(  # noqa: C901
    mesh: Mesh, base_obs: dict, ck: Checks, wrong_diagonal_mesh: Mesh
) -> list[dict]:
    out: list[dict] = []

    def resolved(
        name: str, category: str, description: str, detection: str, **kwargs
    ) -> None:
        case = solve_case(mesh, ck, f"falsifier.{name}", **kwargs)
        obs = observe(case, ck, f"falsifier.{name}", full=False)
        worst = compare(base_obs, obs, PRODUCTION_RELATIVE)
        detected = any(v["within_tolerance"] is False for v in worst.values())
        ck.check(f"falsifier.{name}.detected", detected, description)
        out.append(
            {
                "name": name,
                "category": category,
                "description": description,
                "detection": detection,
                "detected": detected,
                "divergence_vs_production_tolerance": worst,
            }
        )

    resolved(
        "vector_laplacian_viscosity",
        "formulation",
        "mu * int grad(u):grad(v) replaces 2 mu int sym(grad u):sym(grad v)",
        "frozen velocity, pressure, flux and reaction probes",
        form=stokes.Formulation(symmetric_gradient=False),
    )
    resolved(
        "coupling_sign_reversed_momentum",
        "formulation",
        "the momentum-row pressure coupling changes sign, breaking the mixed symmetry",
        "frozen velocity, pressure, flux and reaction probes",
        form=stokes.Formulation(coupling_sign_momentum=-1),
    )
    resolved(
        "inlet_outlet_membership_swapped",
        "boundary-data",
        "inlet and outlet entity memberships are exchanged before the essential closure",
        "frozen flux and reaction oracle",
        swap=True,
    )

    # Unnormalized bubble: the discrete space is unchanged, so the velocity
    # *field* is invariant. What breaks is the recovery convention, because
    # beta(barycentre) = 1 only for the 27-normalization.
    case = solve_case(
        mesh,
        ck,
        "falsifier.bubble_normalization_one",
        form=stokes.Formulation(bubble_scale=1),
    )
    obs = observe(case, ck, "falsifier.bubble_normalization_one", full=False)
    field_gap = compare(base_obs, obs, PRODUCTION_RELATIVE)
    mis = []
    for probe, base_probe in zip(obs["velocity_probes"], base_obs["velocity_probes"]):
        cell = probe["cell"]
        raw_bubble = case["x"][case["system"].vel_bubble(cell, 0)]
        p1 = probe["_raw"][0] - raw_bubble * mpmath.mpf(1) / 27
        mis.append(abs((p1 + raw_bubble) - base_probe["_raw"][0]))
    peak = max(mis)
    limit = tolerance("velocity", PRODUCTION_RELATIVE)
    ck.check(
        "falsifier.bubble_normalization_one.detected",
        peak > limit,
        "an unnormalized bubble whose coefficient is still read as the barycentre enrichment",
    )
    ck.check(
        "falsifier.bubble_normalization_one.field_is_invariant",
        all(v["within_tolerance"] for v in field_gap.values()),
        "rescaling the bubble basis is a change of basis: the MINI space and the solved field are unchanged",
    )
    out.append(
        {
            "name": "bubble_normalization_one",
            "category": "formulation",
            "description": "beta = lambda0 lambda1 lambda2 replaces 27 lambda0 lambda1 lambda2",
            "detection": "barycentre velocity recovery, which assumes beta(barycentre) = 1",
            "detected": bool(peak > limit),
            "note": (
                "rescaling the bubble basis leaves the MINI space and therefore the solved "
                "velocity field unchanged; only the coefficient interpretation breaks, by "
                "exactly the factor 27 on the enrichment part"
            ),
            "max_abs_velocity_error_m_s": f2(peak),
            "tolerance": f2(limit),
            "ratio_to_tolerance": f2(peak / limit),
            "solved_field_unchanged": bool(
                all(v["within_tolerance"] for v in field_gap.values())
            ),
        }
    )

    # Dropped bubble unknowns: MINI degenerates to unstable P1/P1 and, on this
    # all-boundary-vertex mesh, the reduced system loses rank outright.
    system = stokes.assemble(
        mesh.vertices, mesh.cells, MU, stokes.Formulation(include_bubble=False)
    )
    prescribed, outlet_facets, _, _ = boundary_plan(
        mesh, system, ck, "falsifier.bubble_dofs_omitted"
    )
    stokes.add_facet_traction(
        system, mesh.vertices, outlet_facets, (mpmath.mpf(0), mpmath.mpf(0))
    )
    # The bubble unknowns are not allocated at all, so they are excluded from the
    # reduced system rather than left as structurally empty rows.
    absent = dict(prescribed)
    for c in range(system.n_cells):
        for d in range(2):
            absent[system.vel_bubble(c, d)] = mpmath.mpf(0)
    matrix, rhs, free = stokes.build_reduced(system, absent)
    rank = _rank(matrix)
    singular = rank < len(free)
    ck.check(
        "falsifier.bubble_dofs_omitted.detected",
        singular,
        "dropping the bubble unknowns leaves an unstable P1/P1 pair with no solution",
    )
    out.append(
        {
            "name": "bubble_dofs_omitted",
            "category": "formulation",
            "description": "the two bubble velocity unknowns per cell are not allocated",
            "detection": "rank of the reduced mixed system",
            "detected": bool(singular),
            "reduced_rows": len(free),
            "reduced_rows_note": "2 free P1 velocity + 104 pressure; the bubble unknowns are not allocated",
            "numerical_rank": rank,
            "rank_deficiency": len(free) - rank,
        }
    )

    # Reversed inlet normal and reaction mislabelling need no second solve.
    inlet_flux = base_obs["_raw"]["inlet_flux"]
    outlet_flux = base_obs["_raw"]["outlet_flux"]
    reversed_sum = -inlet_flux + outlet_flux
    ck.check(
        "falsifier.inlet_normal_reversed.detected",
        abs(reversed_sum) > mpmath.mpf("1e-8"),
        "the frozen signed-flux balance |inlet + outlet| <= 1e-8 m^2/s fails",
    )
    out.append(
        {
            "name": "inlet_normal_reversed",
            "category": "boundary-data",
            "description": "the parent-outward inlet normal [-1, 0] is replaced by [+1, 0]",
            "detection": "signed flux balance |inlet_flux + outlet_flux| <= 1e-8 m^2/s",
            "detected": bool(abs(reversed_sum) > mpmath.mpf("1e-8")),
            "balance_with_reversed_normal_m2_s": f2(reversed_sum),
            "frozen_balance_limit_m2_s": 1e-8,
        }
    )

    reaction = base_obs["_raw"]["cylinder_reaction"]
    mislabelled_gap = max(abs(2 * r) for r in reaction)
    limit = tolerance("reaction", PRODUCTION_RELATIVE)
    ck.check(
        "falsifier.reaction_sign_mislabelled.detected",
        mislabelled_gap > limit and reaction[0] < 0,
        "reporting the constrained residual as fluid-on-cylinder without negation",
    )
    out.append(
        {
            "name": "reaction_sign_mislabelled",
            "category": "reporting",
            "description": "the constrained residual is reported as the fluid force on the cylinder without negation",
            "detection": "reaction-orientation assertion and the reaction tolerance",
            "detected": bool(mislabelled_gap > limit and reaction[0] < 0),
            "max_abs_error_N_m": f2(mislabelled_gap),
            "tolerance": f2(limit),
            "ratio_to_tolerance": f2(mislabelled_gap / limit),
            "constraint_force_on_fluid_x_is_negative": bool(reaction[0] < 0),
        }
    )

    # Mesh-contract falsifier: the excluded RFC 0082 quad diagonal. Both meshes
    # carry the same 104 vertices and the same 104 boundary facets, so the
    # signed fluxes are blind to the difference; only the probes and the
    # reaction separate them. That asymmetry is asserted in both directions.
    out.append(
        wrong_diagonal_falsifier(wrong_diagonal_mesh, base_obs, ck),
    )
    return out


def wrong_diagonal_falsifier(mesh: Mesh, base_obs: dict, ck: Checks) -> dict:
    case = solve_case(mesh, ck, "falsifier.wrong_quad_diagonal")
    obs = observe(case, ck, "falsifier.wrong_quad_diagonal", full=False)
    route_gap = compare(base_obs, obs, ROUTE_RELATIVE)
    production_gap = compare(base_obs, obs, PRODUCTION_RELATIVE)

    probe_kinds = ("velocity", "pressure", "reaction")
    rejected_by_route = all(
        not route_gap[kind]["within_tolerance"] for kind in probe_kinds
    )
    rejected_by_production = all(
        not production_gap[kind]["within_tolerance"] for kind in probe_kinds
    )
    ck.check(
        "falsifier.wrong_quad_diagonal.rejected_by_route_tolerance",
        rejected_by_route,
        "the excluded I_i--O_j split must miss the frozen velocity, pressure and "
        "reaction observations by more than the route-agreement tolerance",
    )
    ck.check(
        "falsifier.wrong_quad_diagonal.rejected_by_production_tolerance",
        rejected_by_production,
        "the excluded split must also miss the looser production tolerance",
    )
    ck.check(
        "falsifier.wrong_quad_diagonal.flux_alone_cannot_detect_it",
        route_gap["flux"]["within_tolerance"],
        "both splits share the 104 vertices and the 104 boundary facets, so the "
        "signed fluxes agree: a flux-only check would accept the wrong mesh",
    )
    return {
        "name": "wrong_quad_diagonal",
        "category": "mesh-contract",
        "description": (
            "the excluded I_i--O_j quad diagonal replaces the RFC 0082 frozen "
            "O_i--I_j split, with every vertex, boundary facet and named set unchanged"
        ),
        "detection": "frozen velocity, pressure and cylinder-reaction observations",
        "detected": bool(rejected_by_route and rejected_by_production),
        "input_mesh": FALSIFIER_MESH_FILE,
        "divergence_vs_route_tolerance": route_gap,
        "divergence_vs_production_tolerance": production_gap,
        "flux_is_blind_to_it": bool(route_gap["flux"]["within_tolerance"]),
        "why_flux_is_blind": (
            "both splits carry the same 104 vertices and the same 104 boundary facets, "
            "and the signed flux depends only on the P1 boundary trace; the difference "
            "lives entirely in the interior connectivity"
        ),
    }


def patch_test(mesh: Mesh, ck: Checks) -> dict:
    """Exact-reproduction test of the whole formulation on this very mesh.

    ``u = (x + y, -(x + y))`` is divergence free and lies exactly in P1, so it
    lies exactly in MINI; ``sym(grad u) = diag(1, -1)``, so with ``p = 2 mu`` the
    exact traction on any ``x = const`` face is ``(2 mu - p, 0) = 0``. Zero body
    force closes it. The discrete solution must therefore reproduce the field
    exactly, with a constant pressure fixed only by the traction partition and
    with identically zero bubbles.

    This is not a convergence statement. It is an exact algebraic identity that
    fails if the viscous block, either pressure-coupling sign, the bubble block,
    the essential closure, the parent-outward normal or the natural-boundary
    handling is wrong. The vector-Laplacian variant fails it because its natural
    condition ``mu du/dn - p n = 0`` cannot be met by a field with a rotational
    part.
    """
    results = {}
    for label, form in (
        ("frozen_formulation", stokes.Formulation()),
        ("vector_laplacian", stokes.Formulation(symmetric_gradient=False)),
    ):
        system = stokes.assemble(mesh.vertices, mesh.cells, MU, form)
        essential = (
            mesh.facet_vertices("inlet")
            | mesh.facet_vertices("walls")
            | mesh.facet_vertices("cylinder")
        )
        prescribed = {}
        for v in sorted(essential):
            px, py = mesh.vertices[v]
            prescribed[system.vel_p1(v, 0)] = px + py
            prescribed[system.vel_p1(v, 1)] = -(px + py)
        outlet_facets = [mesh.facets[i][0] for i in mesh.facet_indices("outlet")]
        stokes.add_facet_traction(
            system, mesh.vertices, outlet_facets, (mpmath.mpf(0), mpmath.mpf(0))
        )
        x = stokes.condensed_solve(system, prescribed)
        velocity_error = max(
            max(
                abs(
                    x[system.vel_p1(v, 0)] - (mesh.vertices[v][0] + mesh.vertices[v][1])
                ),
                abs(x[system.vel_p1(v, 1)] + mesh.vertices[v][0] + mesh.vertices[v][1]),
            )
            for v in range(len(mesh.vertices))
        )
        pressure_error = max(
            abs(x[system.pressure(v)] - 2 * MU) for v in range(len(mesh.vertices))
        )
        bubble = max(
            abs(x[system.vel_bubble(c, d)])
            for c in range(len(mesh.cells))
            for d in range(2)
        )
        results[label] = {
            "max_velocity_error_m_s": f2(velocity_error),
            "max_pressure_error_Pa": f2(pressure_error),
            "max_bubble_magnitude_m_s": f2(bubble),
            "exact_pressure_Pa": f2(2 * MU),
        }
    limit = mpmath.mpf("1e-25")
    ck.below(
        "patch_test.velocity",
        mpmath.mpf(results["frozen_formulation"]["max_velocity_error_m_s"]),
        limit,
    )
    ck.below(
        "patch_test.pressure",
        mpmath.mpf(results["frozen_formulation"]["max_pressure_error_Pa"]),
        limit,
    )
    ck.below(
        "patch_test.bubbles_vanish",
        mpmath.mpf(results["frozen_formulation"]["max_bubble_magnitude_m_s"]),
        limit,
    )
    ck.check(
        "patch_test.vector_laplacian_fails",
        mpmath.mpf(results["vector_laplacian"]["max_pressure_error_Pa"])
        > tolerance("pressure", PRODUCTION_RELATIVE),
        "the vector-Laplacian natural condition cannot reproduce the exact field",
    )
    results["note"] = (
        "exact algebraic reproduction, not a convergence claim; it independently "
        "validates the viscous block, both pressure-coupling signs, the bubble block, "
        "the essential closure, the parent-outward normal and the natural traction "
        "boundary, on this exact mesh"
    )
    return results


def _rank(matrix) -> int:
    """Gaussian elimination rank at a threshold well above the working precision."""
    n = matrix.rows
    a = [[matrix[i, j] for j in range(matrix.cols)] for i in range(n)]
    scale = max((abs(v) for row in a for v in row), default=mpmath.mpf(0))
    threshold = scale * mpmath.mpf("1e-25")
    rank = 0
    row = 0
    for col in range(matrix.cols):
        pivot = max(range(row, n), key=lambda r: abs(a[r][col]), default=None)
        if pivot is None or abs(a[pivot][col]) <= threshold:
            continue
        a[row], a[pivot] = a[pivot], a[row]
        for r in range(row + 1, n):
            factor = a[r][col] / a[row][col]
            if factor != 0:
                for c in range(col, matrix.cols):
                    a[r][c] -= factor * a[row][c]
        rank += 1
        row += 1
        if row == n:
            break
    return rank


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in node.items() if not k.startswith("_")}
    if isinstance(node, list):
        return [strip(v) for v in node]
    return node


def describe_claim_boundary(mesh: Mesh, case, obs: dict, ck: Checks) -> dict:
    """The topology facts that bound what this route may be read to claim.

    Every one of the 104 vertices of the RFC 0082 reference topology lies on the
    boundary. 103 are essential, the single free velocity vertex is the outlet
    midpoint, and the only other velocity unknowns are the cell-interior MINI
    bubbles. The reported cylinder vector is therefore the algebraic
    constrained-vertex force on this deliberately coarse mesh.
    """
    system, prescribed = case["system"], case["prescribed"]
    boundary_vertices = {v for facet in mesh.facets for v in facet[0]}
    essential_vertices = sorted({dof // 2 for dof in prescribed})
    free_vertices = sorted(set(range(len(mesh.vertices))) - set(essential_vertices))
    bubble_dofs = [
        system.vel_bubble(c, d) for c in range(system.n_cells) for d in range(2)
    ]

    ck.check(
        "claim.every_vertex_is_a_boundary_vertex",
        len(boundary_vertices) == len(mesh.vertices) == 104,
        f"{len(boundary_vertices)} of {len(mesh.vertices)}",
    )
    ck.check(
        "claim.essential_velocity_vertices",
        len(essential_vertices) == 103,
        str(len(essential_vertices)),
    )
    ck.check(
        "claim.single_free_velocity_vertex_is_the_outlet_midpoint",
        len(free_vertices) == 1
        and free_vertices[0] in mesh.facet_vertices("outlet")
        # the stored coordinates are binary64, so compare in that spelling
        and [f2(c) for c in mesh.vertices[free_vertices[0]]] == [2.2, 0.2],
        str([[f2(c) for c in mesh.vertices[v]] for v in free_vertices]),
    )
    ck.check(
        "claim.bubble_velocities_are_cell_interior_unknowns",
        not (set(bubble_dofs) & set(prescribed))
        and set(bubble_dofs) <= set(case["free"]),
        "a bubble unknown was constrained by a boundary trace",
    )

    pressures = [abs(p["_raw"]) for p in obs["pressure_probes"]]
    reaction = obs["_raw"]["cylinder_reaction"]
    reaction_magnitude = mpmath.sqrt(reaction[0] ** 2 + reaction[1] ** 2)
    return {
        "headline": (
            "All 104 mesh vertices are boundary vertices; 103 are essential and the "
            "only free velocity vertex is the outlet midpoint [2.2, 0.2] m. The MINI "
            "bubble velocities remain cell-interior unknowns. The reported cylinder "
            "vector is an algebraic constrained-vertex force on this deliberately "
            "coarse mesh, not drag and not a physically scaled force claim."
        ),
        "mesh_vertices": len(mesh.vertices),
        "boundary_vertices": len(boundary_vertices),
        "interior_vertices": len(mesh.vertices) - len(boundary_vertices),
        "essential_velocity_vertices": len(essential_vertices),
        "free_velocity_vertices": len(free_vertices),
        "free_velocity_vertex_position_m": [
            [f2(c) for c in mesh.vertices[v]] for v in free_vertices
        ],
        "bubble_velocity_unknowns": len(bubble_dofs),
        "bubble_velocity_unknowns_are_cell_interior": True,
        "why_the_recovered_magnitudes_are_large": (
            "with no interior velocity vertex the discrete velocity is almost entirely "
            "the prescribed P1 trace plus the cell bubbles, and the pressure is whatever "
            "enforces weak incompressibility of that nearly fixed field"
        ),
        "max_abs_probe_pressure_Pa": f2(max(pressures)),
        "max_abs_probe_pressure_over_P": f2(max(pressures) / P),
        "cylinder_reaction_magnitude_N_m": f2(reaction_magnitude),
        "cylinder_reaction_magnitude_over_P_L": f2(reaction_magnitude / (P * L)),
        "the_cylinder_vector_is": (
            "the algebraic constrained-vertex force on this mesh, in the existing API "
            "convention of the constraint force on the fluid"
        ),
        "the_cylinder_vector_is_not": [
            "drag",
            "a physically scaled force",
            "a mesh-independent force",
            "a drag or lift coefficient",
            "a DFG or Schaefer-Turek benchmark value",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Python analytic Stokes oracle for the frozen contract"
    )
    parser.add_argument(
        "--check", action="store_true", help="fail if result.json would change"
    )
    args = parser.parse_args()

    started = time.perf_counter()
    ck = Checks()

    ck.check(
        "dependency.mpmath",
        mpmath.__version__ >= "1.3.0",
        f"mpmath {mpmath.__version__}",
    )
    ck.check("dependency.python", sys.version_info >= (3, 12), sys.version.split()[0])
    ck.check("scale.P", float(P) == ISSUE_P, hp(P))
    ck.check("scale.G", float(G) == ISSUE_G, hp(G))
    # Theta = P U L = mu Umax^2. The contract spells it 0.00009 W/m, which is exact
    # only for exact-decimal inputs: every binary64 evaluation from the binary64
    # spellings of mu, Umax and H lands one ulp lower, at 8.999999999999999e-05.
    theta_ulps = (
        abs(THETA - mpmath.mpf(ISSUE_THETA))
        / mpmath.mpf(ISSUE_THETA)
        / mpmath.mpf(F64_EPS)
    )
    ck.check(
        "scale.Theta_within_one_ulp",
        theta_ulps <= 1,
        f"{hp(THETA)} ({mpmath.nstr(theta_ulps, 4)} ulp)",
    )
    ck.check(
        "scale.Theta_equals_mu_Umax_squared",
        abs(THETA - MU * UMAX**2) < mpmath.mpf("1e-40"),
        hp(THETA),
    )

    mesh_digests: dict[str, str] = {}
    meshes: dict[str, Mesh] = {}
    for name in (MESH_FILE, FALSIFIER_MESH_FILE):
        payload = (MESH_DIR / name).read_bytes()
        digest = hashlib.sha256(payload).hexdigest()
        mesh_digests[name] = digest
        ck.check(f"mesh.{name}.sha256", digest == PINNED_MESH_SHA256[name], digest)
        document = json.loads(payload.decode("utf-8"))
        ck.check(
            f"mesh.{name}.role",
            document["role"] == EXPECTED_ROLE[name],
            f"{document['role']!r} != {EXPECTED_ROLE[name]!r}",
        )
        ck.check(
            f"mesh.{name}.quad_diagonal",
            document["construction"]["quad_diagonal"] == EXPECTED_DIAGONAL[name],
            document["construction"]["quad_diagonal"],
        )
        meshes[name] = Mesh(document)
        revalidate(meshes[name], ck, f"mesh.{name}")

    mesh = meshes[MESH_FILE]
    case = solve_case(mesh, ck, "frozen", cross_check_direct_lu=True)
    raw_obs = observe(case, ck, "frozen")
    observations = strip(raw_obs)
    congruence = congruence_check(case, mesh, ck, "frozen")

    patch = patch_test(mesh, ck)
    falsifiers = run_falsifiers(mesh, raw_obs, ck, meshes[FALSIFIER_MESH_FILE])

    permuted = mesh.permuted(vertex_shift=13, cell_shift=5)
    revalidate(permuted, ck, "reindexed")
    permuted_case = solve_case(permuted, ck, "reindexed")
    permuted_obs = observe(permuted_case, ck, "reindexed")
    invariance = compare(raw_obs, permuted_obs, mpmath.mpf(0))
    worst_invariance = max(
        mpmath.mpf(v["max_abs_difference"]) for v in invariance.values()
    )
    ck.below("reindexing.observations_invariant", worst_invariance, mpmath.mpf("1e-25"))
    same_geometry = all(
        [
            [permuted.vertices[p["vertex"]][0], permuted.vertices[p["vertex"]][1]]
            == [mesh.vertices[q["vertex"]][0], mesh.vertices[q["vertex"]][1]]
            for p, q in zip(permuted_obs["pressure_probes"], raw_obs["pressure_probes"])
        ]
    )
    ck.check("reindexing.selectors_pick_the_same_geometry", same_geometry, "")

    claim_boundary = describe_claim_boundary(mesh, case, raw_obs, ck)

    document = {
        "schema": SCHEMA,
        "status": "frozen-single-route",
        "frozen": True,
        "frozen_scope": (
            "every observation, residual, balance, selector and structural pressure-"
            "reference fact below is frozen for the one accepted mesh under the "
            "precommitted contract tolerances. No formula, expected physics, physical "
            "scale or tolerance was changed after selection"
        ),
        "dual_independent_oracle_gate": {
            "passed": False,
            "required_routes": 2,
            "routes_that_have_run": 1,
            "this_route": "python-analytic-mini-p1",
            "pending_route": "julia (contract route 2)",
            "statement": (
                "This document is ONE frozen route. The Julia route has not run, so no "
                "route-to-route agreement is claimed, measured or implied, and the dual "
                "independent oracle gate of the frozen contract HAS NOT PASSED. Nothing here may "
                "be read as that gate's result or as permission to begin implementation"
            ),
        },
        "claim_boundary": claim_boundary,
        "route": {
            "name": "python-analytic-mini-p1",
            "assembly": "closed-form barycentric monomial integrals, element by element; no quadrature loop",
            "solve": "exact static condensation of the bubble block, then dense LU at elevated precision",
            "cross_check": "independent dense LU on the uncondensed reduced system",
            "precision_decimal_digits": DPS,
            "dependencies": ["python>=3.12", f"mpmath=={mpmath.__version__}"],
            "reads_eqiora": False,
        },
        "model": {
            "mu_kg_m_s": f2(MU),
            "channel_height_m": f2(H),
            "max_inlet_speed_m_s": f2(UMAX),
            "body_force": "identically zero (zero force potential)",
            "inlet": "trace(u) + normal(isotropic_lift(g)) = 0 with g(y) = 4 Umax y (H-y)/H^2; parent-outward normal [-1, 0] gives u = [g(y), 0]",
            "outlet": "constant parent-outward traction [0, 0] Pa",
            "walls_and_cylinder": "trace(u) = [0, 0] m/s",
            "trace_closure": "a vertex shared by an essential and an outlet facet stays fixed; the outlet facet still contributes its full-system traction action",
        },
        "scales": {
            "L_m": f2(L),
            "U_m_s": f2(U),
            "P_Pa": f2(P),
            "G_1_s": f2(G),
            "Theta_W_m": f2(THETA),
            "Theta_issue_spelling_W_m": ISSUE_THETA,
            "Theta_note": "Theta = P U L = mu Umax^2; the contract's 0.00009 W/m is the exact-decimal reading and every binary64 evaluation is one ulp lower",
            "mu_hat": f2(MU_HAT),
            "normalized_origin_m": [f2(ORIGIN[0]), f2(ORIGIN[1])],
        },
        "solver_selection": dict(SOLVER),
        "dimensions": {
            "vertices": 104,
            "cells": 104,
            "boundary_facets": 104,
            "velocity_p1_dofs": 208,
            "velocity_bubble_dofs": 208,
            "pressure_dofs": 104,
            "full_system_rows": 520,
            "prescribed_velocity_dofs": 206,
            "reduced_system_rows": 314,
            "gauge_rows": 0,
            "interior_vertices": 0,
            "free_velocity_vertices": 1,
            "note": "see claim_boundary; the RFC 0082 reference topology is one ray-cast annulus",
        },
        "mesh": {
            "directory": "../../mesh",
            "accepted": {
                "file": MESH_FILE,
                "role": EXPECTED_ROLE[MESH_FILE],
                "sha256": mesh_digests[MESH_FILE],
                "quad_diagonal": EXPECTED_DIAGONAL[MESH_FILE],
                "quad_cells": "(O_i, O_j, I_j) and (O_i, I_j, I_i)",
            },
            "wrong_contract_falsifier": {
                "file": FALSIFIER_MESH_FILE,
                "role": EXPECTED_ROLE[FALSIFIER_MESH_FILE],
                "sha256": mesh_digests[FALSIFIER_MESH_FILE],
                "quad_diagonal": EXPECTED_DIAGONAL[FALSIFIER_MESH_FILE],
                "status": "not a mesh and not a second admissible reading of RFC 0082; the negative input of the wrong_quad_diagonal falsifier only",
            },
            "admissible_meshes": 1,
        },
        "quad_diagonal": {
            "where": "RFC 0082, section 'Reference topology'",
            "text": (
                "for adjacent ray indices i and j = (i + 1) mod n, inner circle vertices "
                "I_i, I_j, and outer rectangle hits O_i, O_j, the shared quad diagonal is "
                "O_i--I_j. The two cells are (O_i, O_j, I_j) and (O_i, I_j, I_i), with "
                "their stored order normalized to positive orientation."
            ),
            "frozen": True,
            "consumed_here": "O_i--I_j",
            "why_it_had_to_be_stated": (
                "all 50 ray-pair quads are strictly convex, so positivity alone admits "
                "either split; the earlier return asked for exactly this sentence"
            ),
            "consequence_if_wrong": "see the wrong_quad_diagonal falsifier",
        },
        "observations": observations,
        "congruence": congruence,
        "patch_test": patch,
        "falsifiers": strip(falsifiers),
        "reindexing_invariance": {
            "permutation": "vertex v -> (7v + 13) mod 104, cell c -> (11c + 5) mod 104, each triple rotated by one",
            "max_abs_observation_difference": f2(worst_invariance),
            "selectors_pick_the_same_geometry": same_geometry,
            "detail": strip(invariance),
        },
        "limitations": [
            "ONE route only: the Julia route has not run, no route-to-route agreement is claimed, and the dual independent oracle gate has not passed",
            "the mesh copy's binary64 coordinates depend on the platform libm; RFC 0082 explicitly does not claim cross-platform mesh-byte identity, so the production inventory comparison must be tolerance-based rather than bitwise",
            "the cylinder min-y and max-y pressure probes are exact two-way ties in binary64; the contract's lexicographic rule resolves them, but the tied candidates carry materially different pressures (both are emitted under pressure_probes[].tie_break), so the rule must be applied to the stored coordinates rather than to recomputed ones",
            "the RFC 0082 ideal closed forms are reproduced only when the radius is the exact decimal 1/20; the binary64 spelling of 0.05 shifts them by one relative half-ulp",
            "this is a coarse executable demonstration: no PDE accuracy, mesh convergence, drag or lift claim is made",
            "the RFC 0082 mesh has no interior velocity vertices, so the recovered pressure and cylinder reaction exceed their physical scales P = 7.3e-4 Pa and P L = 3e-4 N/m by about four orders of magnitude; see claim_boundary for the measured ratios. On an adequate straight-channel mesh the same assembly recovers the analytic Poiseuille drop (see reference_channel.py, a diagnostic outside this frozen route). The cylinder reaction here is an algebraic constraint force, not a drag measurement",
        ],
        "checks": {
            "total": len(ck.records),
            "passed": ck.passed,
            "failed": len(ck.failed),
            "names": [r["name"] for r in ck.records],
            "measurements": [
                {"name": r["name"], "measured": r["measured"], "limit": r["limit"]}
                for r in ck.records
                if "measured" in r
            ],
            "failures": [f"{r['name']}: {r['detail']}" for r in ck.failed],
        },
    }

    payload = canonical_json.dump_bytes(document)
    target = HERE / "result.json"
    reproduced = target.exists() and target.read_bytes() == payload
    if args.check:
        if not reproduced:
            print("FATAL: result.json is not reproduced byte-for-byte", file=sys.stderr)
            return 3
    else:
        target.write_bytes(payload)

    elapsed = time.perf_counter() - started
    print(
        f"result.json  sha256={hashlib.sha256(payload).hexdigest()}  bytes={len(payload)}"
    )
    for name, digest in mesh_digests.items():
        print(f"{name}  role={EXPECTED_ROLE[name]}  sha256={digest}")
    print(
        f"oracle checks: {ck.passed} passed, {len(ck.failed)} failed  ({elapsed:.1f}s)"
    )
    for record in ck.failed:
        print(f"  FAIL {record['name']}: {record['detail']}")
    print(f"oracle.result={'pass' if not ck.failed else 'fail'}")
    print("oracle.frozen=true  (one route; RFC 0082 quad diagonal O_i--I_j)")
    print("oracle.dual_oracle_gate=not-passed  (the Julia route has not run)")
    return 0 if not ck.failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
