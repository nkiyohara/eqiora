#!/usr/bin/env python3
"""Dual independent oracle gate: compare the two frozen routes.

Reads **only** the two packaged frozen JSON documents and the packaged shared
mesh. It assembles nothing, solves nothing, and reads no production code --
none exists for this capability. Its whole job is to decide whether the two
independently derived routes agree.

Three kinds of comparison live here, and they never borrow each other's rules.

1. **Physical observations** -- velocity, pressure, signed flux and reaction --
   are compared under the precommitted tolerance formula

       abs(a - b) <= floor + 2e-10 * scale

   with the four precommitted (floor, scale) pairs below. Neither the formula
   nor any floor or scale may be edited here: an unsatisfiable oracle is
   returned with the argument, never relaxed.

2. **Geometric selectors** -- probe targets, the selected cell, probe vertices
   and tie candidates -- carry no tolerance at all, and none is invented for
   them. Every selector is reconstructed from ``mesh/mesh.json`` in exact
   rational arithmetic and required to match exactly. A metre-valued mesh
   coordinate is a frozen input, not a measurement.

3. **Residuals** are not cross-route observations. Each route is required to
   satisfy the precommitted contract bound with its *own* selected target and
   its *own* recorded roundoff allowance. The two routes solve at different
   working precisions, so their residuals and allowances are never compared
   with each other.

    python3 compare_routes.py            # compare and rewrite the frozen report
    python3 compare_routes.py --check    # fail if the report would change

Exit status 0 means PASS (the routes agree). Any other status means RETURN.
Standard library only.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import sys
from fractions import Fraction

HERE = pathlib.Path(__file__).resolve().parent
CASE = HERE.parent
PYTHON_RESULT = CASE / "routes" / "python" / "result.json"
JULIA_RESULT = CASE / "routes" / "julia" / "expected" / "julia-route-frozen.json"
MESH = CASE / "mesh" / "mesh.json"
FALSIFIER_MESH = CASE / "mesh" / "falsifier-wrong-diagonal.json"
REPORT = HERE / "expected" / "agreement-report.json"

# ---------------------------------------------------------------------------
# Precommitted tolerance table. floor and scale are frozen inputs to this gate.
# ---------------------------------------------------------------------------
RELATIVE = 2e-10
TOLERANCE_TABLE = {
    "velocity": {"floor": 2e-12, "scale": 0.3, "unit": "m/s"},
    "pressure": {"floor": 2e-14, "scale": 0.0007317073170731707, "unit": "Pa"},
    "signed_flux": {"floor": 2e-13, "scale": 0.123, "unit": "m^2/s"},
    "reaction": {"floor": 2e-14, "scale": 0.0003, "unit": "N/m"},
}

# Geometric selectors are NOT in this table and get no entry of their own.
# A probe target, a selected cell and a probe vertex are frozen mesh geometry,
# so they are reconstructed exactly from mesh/mesh.json and required to match
# exactly. See SharedMeshGeometry below.

# ---------------------------------------------------------------------------
# Route identity. A relabelled route must fail this gate, not silently pass it.
# ---------------------------------------------------------------------------
PYTHON_SCHEMA = "eqiora.verify/exact-circular-hole-stokes-2d/route/python/v1"
PYTHON_ROUTE_NAME = "python-analytic-mini-p1"
PYTHON_SOLVE_ROUTE = "static-condensation+dense-LU"
JULIA_ROUTE_NAME = "julia"

# Unit-bearing container key sets. Renaming or dropping a unit suffix, adding
# an observation, or removing one, fails here before any number is compared.
PYTHON_VELOCITY_KEYS = {
    "target_m",
    "cell",
    "barycentre_m",
    "velocity_m_s",
    "tied_cells",
    "selection_margin_m2",
}
JULIA_VELOCITY_KEYS = {
    "target_m",
    "tied_cells",
    "barycentre_m",
    "u_x_m_per_s",
    "u_y_m_per_s",
}
PYTHON_PRESSURE_KEYS = {
    "name",
    "vertex",
    "position_m",
    "pressure_Pa",
    "tied_vertices",
    "selection_margin",
}
JULIA_PRESSURE_KEYS = {"name", "vertex_m", "p_Pa", "exact_tie_count", "tie_candidates"}
PYTHON_FLUX_KEYS = {"inlet", "outlet", "sum", "continuous_inlet_reference"}
JULIA_FLUX_KEYS = {
    "inlet_m2_per_s",
    "outlet_m2_per_s",
    "walls_m2_per_s",
    "cylinder_m2_per_s",
    "sum_m2_per_s",
}
PYTHON_CYLINDER_KEYS = {
    "constraint_force_on_fluid",
    "fluid_force_on_cylinder",
    "convention",
}
PYTHON_BALANCE_KEYS = {
    "constrained_reaction",
    "integrated_body_force",
    "integrated_traction",
    "sum",
}
JULIA_REACTION_KEYS = {
    "cylinder_constraint_force_on_fluid_N_per_m",
    "fluid_force_on_cylinder_N_per_m",
    "all_essential_constrained_reaction_N_per_m",
    "integrated_body_force_N_per_m",
    "integrated_applied_traction_N_per_m",
    "componentwise_sum_N_per_m",
}

# Probe inventories, in the frozen order. Reordering, dropping or adding a probe
# fails closed here.
VELOCITY_TARGETS = [[0.1, 0.2], [0.2, 0.3], [0.3, 0.2], [1.0, 0.2], [2.0, 0.2]]
PYTHON_PRESSURE_NAMES = [
    "cylinder_min_x",
    "cylinder_max_x",
    "cylinder_min_y",
    "cylinder_max_y",
    "outer_nearest_x_low_mid",
    "outer_nearest_x_high_mid",
]
JULIA_PRESSURE_NAMES = [
    "cylinder_min_x",
    "cylinder_max_x",
    "cylinder_min_y",
    "cylinder_max_y",
    "outer_near_inlet_mid",
    "outer_near_outlet_mid",
]
# The two routes spell the last two selectors differently. They denote the same
# geometric selector, and this gate proves that by reconstructing the selector
# from the shared mesh and requiring both routes to land on it exactly; the two
# vocabularies are pinned so that a *third* spelling, or a swap, fails closed
# rather than being absorbed by positional matching.
EXPECTED_TIE_COUNTS = [1, 1, 2, 2, 1, 1]

# The frozen contract's six pressure selectors, in the frozen order, stated as
# the rule rather than as an answer: the extreme cylinder vertex on each axis,
# then the outer-boundary vertex nearest each named point. An exact tie is
# broken by lexicographic coordinate order. SharedMeshGeometry evaluates these
# against mesh/mesh.json, so the expected vertices are derived here rather than
# copied from either route.
PRESSURE_SELECTOR_RULES = [
    {"set": "cylinder", "rule": "min", "axis": 0},
    {"set": "cylinder", "rule": "max", "axis": 0},
    {"set": "cylinder", "rule": "min", "axis": 1},
    {"set": "cylinder", "rule": "max", "axis": 1},
    {"set": "outer", "rule": "nearest", "point": [0.0, 0.2]},
    {"set": "outer", "rule": "nearest", "point": [2.2, 0.2]},
]

# Frozen mesh contract, checked against both routes independently.
MESH_CONTRACT = {
    "segments": 50,
    "vertices": 104,
    "cells": 104,
    "boundary_facets": 104,
    "interior_edges": 104,
    "euler_characteristic": 0,
    "outer_loop_vertices": 54,
    "inlet_facets": 14,
    "outlet_facets": 2,
    "wall_facets": 38,
    "cylinder_facets": 50,
    "quad_diagonal": "O_i--I_j",
    "cell_pair": ["(O_i,O_j,I_j)", "(O_i,I_j,I_i)"],
}
DOF_CONTRACT = {
    "velocity_p1": 208,
    "velocity_bubble": 208,
    "pressure": 104,
    "full_rows": 520,
    "prescribed_velocity": 206,
    "reduced_rows": 314,
    "gauge_rows": 0,
    "essential_vertices": 103,
    "free_vertices": 1,
}
SCALE_CONTRACT = {
    "L_m": 0.41,
    "U_m_per_s": 0.3,
    "P_Pa": 0.0007317073170731707,
    "G_per_s": 0.7317073170731707,
    "Theta_W_per_m": 8.999999999999999e-05,
    "Theta_mathematical_W_per_m": 9e-05,
    "mu_hat": 1.0,
}

# Provenance, split by what a future reader can actually resolve.
#
# The two routes were frozen in throwaway local worktrees whose branches were
# never pushed, and the contract was reviewed on a separate branch that was
# deleted after it squash-merged. None of those commit identifiers is a durable
# reference and each may resolve to nothing in another clone; the durable base
# this package was assembled on is PACKAGE_BASE, on merged main. Nothing in this
# gate, in the packaged evidence, or in any default repository check reads any
# of them, needs them, or depends on those worktrees still existing.
#
# What a future reader reproduces from instead is listed in DURABLE_ANCHORS:
# the packaged bytes themselves, their digests, the contract text embedded in
# each packaged document, and rerunning each route's own checks.
DURABLE_ANCHORS = [
    "the packaged route bytes under routes/ and mesh/, and the sha256 of each, "
    "recomputed by this gate on every run and recorded under 'compared'",
    "the contract text and digests embedded in the packaged documents "
    "themselves, including the shared mesh's own source digest",
    "rerunning each route in place: routes/python/oracle.py --check reproduces "
    "result.json byte for byte, and routes/julia/run.jl reruns byte-identical",
    "rerunning this gate, which regenerates expected/agreement-report.json byte "
    "for byte under --check",
]
PACKAGE_BASE = {
    "commit": "84b78c57f573c78dc7e84e655f6910a9908d0ac2",
    "note": (
        "Merged main, reachable from origin/main, on which this package was "
        "assembled. Recorded for orientation only: nothing here reads or "
        "resolves it."
    ),
}
NON_DURABLE_IDENTIFIERS = {
    "note": (
        "Informative only, and none of these is the base of this package, "
        "which is recorded under 'package_base'. The two route source commits "
        "name commits in local, unpushed session worktrees. The RFC 0082 "
        "review head was the exact head reviewed on a separate branch, which "
        "was deleted after it squash-merged into the package base. Each may "
        "therefore fail to resolve in another clone, nothing here depends on "
        "resolving them, and no future reproduction requires them."
    ),
    "python_route_source_commit": "e8bbd44b49315f0d4ee723ec73df53a4f8f6f2f0",
    "julia_route_source_commit": "dccbc318744c7cec4f6b73f5ba0fe60880af7583",
    "rfc_0082_review_head_commit": "dea1fd138fce92fd0127f5df9155b675159a58c3",
}
# Digests of the pre-packaging source documents, recorded as history. The
# source files are not part of this package; these values document what the
# optional packaging-fidelity differ was run against, and are not an input to
# any check here.
HISTORICAL_SOURCE_DIGESTS = {
    "python_source_mesh_sha256": (
        "2ec74b9f481a60b460c9bb8096821cd73eeb7e17ef18a7ae67828e605d17a8f2"
    ),
    "python_source_result_sha256": (
        "4037d358e613a016e21e04c9ff8fffa2475f056fb55a2f88c4c5828d957abfd7"
    ),
    "julia_source_frozen_sha256": (
        "2ad9a041c75906055b3a4ae3a2f2e05f3964cb5e62276a21e19f354586254dff"
    ),
}


class Gate:
    """A fail-closed ledger. Every assertion records its measured magnitude."""

    def __init__(self) -> None:
        self.records: list[dict] = []
        self.failures: list[str] = []
        self.max_difference: dict[str, float] = {key: 0.0 for key in TOLERANCE_TABLE}

    def structural(self, name: str, ok: bool, detail: str = "") -> bool:
        self.records.append(
            {"check": name, "kind": "structural", "passed": bool(ok), "detail": detail}
        )
        if not ok:
            self.failures.append(f"{name}: {detail}")
        return bool(ok)

    def numeric(self, name: str, family: str, python: float, julia: float) -> bool:
        entry = TOLERANCE_TABLE[family]
        limit = entry["floor"] + RELATIVE * entry["scale"]
        finite = _finite(python) and _finite(julia)
        if not finite:
            self.records.append(
                {
                    "check": name,
                    "kind": "numeric",
                    "family": family,
                    "passed": False,
                    "detail": f"non-finite value: python={python!r} julia={julia!r}",
                }
            )
            self.failures.append(f"{name}: non-finite value")
            return False
        difference = abs(python - julia)
        self.max_difference[family] = max(self.max_difference[family], difference)
        ok = difference <= limit
        self.records.append(
            {
                "check": name,
                "kind": "numeric",
                "family": family,
                "passed": ok,
                "python": python,
                "julia": julia,
                "abs_difference": difference,
                "limit": limit,
            }
        )
        if not ok:
            self.failures.append(
                f"{name}: |{python} - {julia}| = {difference} > {limit}"
            )
        return ok

    def bounded(self, name: str, value: float, limit: float, detail: str) -> bool:
        """One route's own quantity against one route's own precommitted bound.

        Used only for the residual criteria, which the frozen contract states
        per route: the value, the target and the allowance all come from the
        same document. Nothing here is a cross-route comparison.
        """
        if not (_finite(value) and _finite(limit)):
            self.records.append(
                {
                    "check": name,
                    "kind": "bounded",
                    "passed": False,
                    "detail": f"non-finite: value={value!r} limit={limit!r}",
                }
            )
            self.failures.append(f"{name}: non-finite value or bound")
            return False
        ok = value <= limit
        self.records.append(
            {
                "check": name,
                "kind": "bounded",
                "passed": ok,
                "value": value,
                "limit": limit,
                "detail": detail,
            }
        )
        if not ok:
            self.failures.append(f"{name}: {value} > {limit}")
        return ok


def _finite(value: object) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
    )


def _rational(value: object) -> Fraction:
    """Exact rational value of a parsed JSON number.

    ``Fraction.from_float`` is exact for every finite binary64 value, so a
    reconstruction built on it introduces no error of its own and therefore
    needs -- and is given -- no tolerance anywhere.
    """
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError(f"non-numeric coordinate {value!r}")
    if isinstance(value, int):
        return Fraction(value)
    if not math.isfinite(value):
        raise ValueError(f"non-finite coordinate {value!r}")
    return Fraction.from_float(value)


def _point(pair: object) -> tuple[Fraction, Fraction] | None:
    """Exact rational point, or None if the input is not a finite 2-vector."""
    if not isinstance(pair, list) or len(pair) != 2:
        return None
    if not all(_finite(component) for component in pair):
        return None
    return (_rational(pair[0]), _rational(pair[1]))


def _decimal(point: tuple[Fraction, Fraction]) -> list[float]:
    """Nearest binary64 rendering of an exact point, for the report only.

    No comparison uses this. It exists so the frozen record is readable.
    """
    return [float(point[0]), float(point[1])]


def _renderable(value: object) -> object:
    """Render sets in a stable order.

    ``set`` comparison is order-independent and correct, but ``repr(set)``
    depends on the interpreter's randomized string hashing, so embedding one in
    the frozen report would make the report irreproducible between processes.
    """
    return sorted(value) if isinstance(value, (set, frozenset)) else value


def _exact(gate: Gate, name: str, got: object, want: object) -> bool:
    return gate.structural(
        name, got == want, f"got {_renderable(got)!r}, want {_renderable(want)!r}"
    )


def _exact_point(
    gate: Gate, name: str, reported: object, want: tuple[Fraction, Fraction]
) -> bool:
    """A reported coordinate pair must be the frozen mesh coordinate exactly.

    Rendered by value rather than by exact rational repr, so the frozen report
    stays readable; the comparison itself is the exact rational one.
    """
    point = _point(reported)
    return gate.structural(
        name,
        point == want,
        f"got {reported!r}, want {_decimal(want)!r}",
    )


def _compare_vector(
    gate: Gate, name: str, family: str, python: list, julia: list
) -> None:
    if not gate.structural(
        f"{name}.arity",
        len(python) == len(julia) == 2,
        f"{len(python)} vs {len(julia)}",
    ):
        return
    for index, axis in enumerate("xy"):
        gate.numeric(f"{name}.{axis}", family, python[index], julia[index])


# ---------------------------------------------------------------------------
# Exact reconstruction of the shared mesh's geometric selectors
# ---------------------------------------------------------------------------
class SharedMeshGeometry:
    """Every geometric selector of the frozen contract, derived from the mesh.

    Built from ``mesh/mesh.json`` alone, in ``fractions.Fraction`` arithmetic
    over the parsed binary64 inputs. Barycentres, squared distances, the argmin,
    the tie set and the lexicographic tie break are all decided exactly, so no
    step of this reconstruction rounds and no step of it needs a tolerance.

    That matters because a selector disagreement is not a small numerical
    disagreement: the two tied cylinder pressure vertices differ by about
    ``1 Pa``. Deciding selectors under a physical tolerance would let exactly
    the failure this gate exists to catch pass unnoticed.
    """

    def __init__(self, mesh_doc: dict) -> None:
        self.vertices = [_point(vertex) for vertex in mesh_doc["vertices_m"]]
        if any(vertex is None for vertex in self.vertices):
            raise ValueError("the shared mesh carries a non-finite vertex")
        self.cells = [tuple(cell) for cell in mesh_doc["cells"]]
        self.barycentres = [
            (
                sum((self.vertices[i][0] for i in cell), Fraction(0)) / 3,
                sum((self.vertices[i][1] for i in cell), Fraction(0)) / 3,
            )
            for cell in self.cells
        ]
        # A cell is named by its lexicographically sorted vertex-coordinate
        # triple, never by its index: the contract requires the observations to
        # survive renumbering, so the gate must not depend on a numbering.
        self.triples = [
            tuple(sorted(self.vertices[i] for i in cell)) for cell in self.cells
        ]

        facets = mesh_doc["boundary_facets"]
        cylinder = {
            index
            for facet in mesh_doc["entity_sets"]["cylinder"]["facets"]
            for index in facets[facet]["vertices"]
        }
        self.cylinder_vertices = sorted(cylinder)
        self.outer_vertices = sorted(set(range(len(self.vertices))) - cylinder)

    def select_cell(self, target: list[float]) -> tuple[tuple, int]:
        """The contract's cell for one probe target: exact argmin, exact tie.

        Returns the selected cell's coordinate triple and the exact size of the
        minimum-squared-distance tie set. The tie break is the contract's
        lexicographically sorted vertex-coordinate triple.
        """
        point = _point(target)
        distances = [self._squared_distance(b, point) for b in self.barycentres]
        smallest = min(distances)
        tied = [i for i, d in enumerate(distances) if d == smallest]
        return min(self.triples[i] for i in tied), len(tied)

    def nearest_cell(self, reported: object) -> tuple[tuple, bool] | None:
        """The unique mesh cell whose exact barycentre is nearest ``reported``.

        Each route computes its own barycentre in its own precision, so the two
        reported values differ in the last bits. Mapping each to the nearest
        exact mesh barycentre recovers which cell was meant without ever
        comparing the two approximate coordinates to each other. Returns the
        cell's coordinate triple and whether that nearest cell is unique --
        uniqueness is decided exactly, by strict inequality, not by a margin.
        """
        point = _point(reported)
        if point is None:
            return None
        distances = [self._squared_distance(b, point) for b in self.barycentres]
        smallest = min(distances)
        nearest = [i for i, d in enumerate(distances) if d == smallest]
        return self.triples[nearest[0]], len(nearest) == 1

    def select_vertices(self, rule: dict) -> list[tuple[Fraction, Fraction]]:
        """The contract's tied vertex set for one pressure selector.

        Returned in lexicographic coordinate order, so the head of the list is
        the contract's selection and the whole list is the candidate set both
        routes must publish.
        """
        members = (
            self.cylinder_vertices if rule["set"] == "cylinder" else self.outer_vertices
        )
        if rule["rule"] == "nearest":
            anchor = _point(rule["point"])
            key = {i: self._squared_distance(self.vertices[i], anchor) for i in members}
            best = min(key.values())
        elif rule["rule"] == "min":
            key = {i: self.vertices[i][rule["axis"]] for i in members}
            best = min(key.values())
        else:
            key = {i: -self.vertices[i][rule["axis"]] for i in members}
            best = min(key.values())
        return sorted(self.vertices[i] for i in members if key[i] == best)

    @staticmethod
    def _squared_distance(
        a: tuple[Fraction, Fraction], b: tuple[Fraction, Fraction]
    ) -> Fraction:
        return (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2


# ---------------------------------------------------------------------------
# The comparison itself
# ---------------------------------------------------------------------------
def compare(
    python_doc: dict, julia_doc: dict, mesh_doc: dict
) -> tuple[Gate, SharedMeshGeometry]:
    """Run every check, and return the ledger beside the geometry it used.

    The report quotes the same reconstruction the checks ran against, rather
    than rebuilding a second one that a reader would have to prove identical.
    """
    gate = Gate()
    geometry = SharedMeshGeometry(mesh_doc)

    # -- route identity and units ------------------------------------------
    _exact(gate, "route.python.schema", python_doc.get("schema"), PYTHON_SCHEMA)
    _exact(
        gate, "route.python.name", python_doc["route"].get("name"), PYTHON_ROUTE_NAME
    )
    _exact(
        gate,
        "route.python.solve_route",
        python_doc["observations"].get("solve_route"),
        PYTHON_SOLVE_ROUTE,
    )
    _exact(
        gate,
        "route.python.reads_eqiora",
        python_doc["route"].get("reads_eqiora"),
        False,
    )
    _exact(gate, "route.julia.name", julia_doc.get("route"), JULIA_ROUTE_NAME)
    gate.structural(
        "route.methods_are_distinct",
        "no quadrature loop" in python_doc["route"]["assembly"],
        "the Python route must remain the closed-form route; the Julia route "
        "assembles by explicit 3x3 Gauss-Legendre Duffy quadrature",
    )

    # -- velocity probes ----------------------------------------------------
    python_velocity = python_doc["observations"]["velocity_probes"]
    julia_velocity = julia_doc["velocity_probes"]
    gate.structural(
        "velocity.count",
        len(python_velocity) == len(julia_velocity) == len(VELOCITY_TARGETS),
        f"python {len(python_velocity)}, julia {len(julia_velocity)}, "
        f"contract {len(VELOCITY_TARGETS)}",
    )
    for index, target in enumerate(VELOCITY_TARGETS):
        if index >= len(python_velocity) or index >= len(julia_velocity):
            break
        py = python_velocity[index]
        jl = julia_velocity[index]
        tag = f"velocity[{index}]"
        _exact(gate, f"{tag}.python_keys", set(py), PYTHON_VELOCITY_KEYS)
        _exact(gate, f"{tag}.julia_keys", set(jl), JULIA_VELOCITY_KEYS)
        _exact(gate, f"{tag}.target.python", py["target_m"], target)
        _exact(gate, f"{tag}.target.julia", jl["target_m"], target)

        # The contract's cell for this target, recomputed exactly from the
        # shared mesh: minimum squared distance from the target to every cell
        # barycentre, ties broken by the lexicographically sorted vertex
        # coordinate triple. Neither route's answer is consulted to obtain it.
        contract_triple, contract_ties = geometry.select_cell(target)
        _exact(
            gate, f"{tag}.selector.tie_count.python", py["tied_cells"], contract_ties
        )
        _exact(gate, f"{tag}.selector.tie_count.julia", jl["tied_cells"], contract_ties)
        for label, reported in (
            ("python", py["barycentre_m"]),
            ("julia", jl["barycentre_m"]),
        ):
            mapped = geometry.nearest_cell(reported)
            if mapped is None:
                gate.structural(
                    f"{tag}.selector.{label}_barycentre_maps_to_contract_cell",
                    False,
                    f"reported barycentre {reported!r} is not a finite 2-vector",
                )
                continue
            triple, unique = mapped
            gate.structural(
                f"{tag}.selector.{label}_barycentre_maps_to_contract_cell",
                unique and triple == contract_triple,
                f"reported barycentre maps to the mesh cell with vertices "
                f"{[_decimal(v) for v in triple]} (unique nearest: {unique}); the "
                f"contract selects the cell with vertices "
                f"{[_decimal(v) for v in contract_triple]}",
            )
        gate.numeric(
            f"{tag}.u_x", "velocity", py["velocity_m_s"][0], jl["u_x_m_per_s"]["f64"]
        )
        gate.numeric(
            f"{tag}.u_y", "velocity", py["velocity_m_s"][1], jl["u_y_m_per_s"]["f64"]
        )

    # -- pressure probes, both tie candidate sets, lexicographic selection ---
    python_pressure = python_doc["observations"]["pressure_probes"]
    julia_pressure = julia_doc["pressure_probes"]
    gate.structural(
        "pressure.count",
        len(python_pressure) == len(julia_pressure) == len(PYTHON_PRESSURE_NAMES),
        f"python {len(python_pressure)}, julia {len(julia_pressure)}, "
        f"contract {len(PYTHON_PRESSURE_NAMES)}",
    )
    _exact(
        gate,
        "pressure.python_label_vocabulary",
        [probe["name"] for probe in python_pressure],
        PYTHON_PRESSURE_NAMES,
    )
    _exact(
        gate,
        "pressure.julia_label_vocabulary",
        [probe["name"] for probe in julia_pressure],
        JULIA_PRESSURE_NAMES,
    )
    for index in range(min(len(python_pressure), len(julia_pressure))):
        py = python_pressure[index]
        jl = julia_pressure[index]
        tag = f"pressure[{index}:{PYTHON_PRESSURE_NAMES[index]}]"
        expected_ties = EXPECTED_TIE_COUNTS[index]
        _exact(
            gate, f"{tag}.python_keys", set(py) - {"tie_break"}, PYTHON_PRESSURE_KEYS
        )
        _exact(gate, f"{tag}.julia_keys", set(jl), JULIA_PRESSURE_KEYS)
        # The contract's tied vertex set for this selector, recomputed exactly
        # from the shared mesh in lexicographic coordinate order. A probe vertex
        # is a stored mesh coordinate, so it is required to match the mesh
        # bit-for-bit; no tolerance is applied to it, and none would be
        # meaningful, because the rejected tied candidate is about 1 Pa away.
        contract_tied = geometry.select_vertices(PRESSURE_SELECTOR_RULES[index])
        _exact(
            gate,
            f"{tag}.selector.tie_count.contract",
            len(contract_tied),
            expected_ties,
        )
        _exact(gate, f"{tag}.tie_count.python", py["tied_vertices"], expected_ties)
        _exact(gate, f"{tag}.tie_count.julia", jl["exact_tie_count"], expected_ties)
        _exact_point(
            gate,
            f"{tag}.selector.python_vertex_is_the_contract_vertex",
            py["position_m"],
            contract_tied[0],
        )
        _exact_point(
            gate,
            f"{tag}.selector.julia_vertex_is_the_contract_vertex",
            jl["vertex_m"],
            contract_tied[0],
        )
        gate.numeric(f"{tag}.p", "pressure", py["pressure_Pa"], jl["p_Pa"]["f64"])

        # Both routes must publish the same tie structure, including the
        # rejected candidate, which carries a materially different pressure.
        gate.structural(
            f"{tag}.python_tie_break_present_iff_tied",
            ("tie_break" in py) == (expected_ties > 1),
            f"tie_break present={'tie_break' in py}, tied={expected_ties}",
        )
        python_candidates = (
            py["tie_break"]["candidates"]
            if "tie_break" in py
            else [{"position_m": py["position_m"], "pressure_Pa": py["pressure_Pa"]}]
        )
        julia_candidates = jl["tie_candidates"]
        if not gate.structural(
            f"{tag}.candidate_count",
            len(python_candidates) == len(julia_candidates) == expected_ties,
            f"python {len(python_candidates)}, julia {len(julia_candidates)}, "
            f"expected {expected_ties}",
        ):
            continue
        # Each candidate coordinate is a stored mesh vertex, so it is required
        # to equal the reconstructed tied vertex in the same slot exactly. That
        # subsumes the two weaker statements this used to make -- that each
        # route's own list was lexicographically ordered, and that each route
        # selected its own list's minimum -- because the reconstructed list is
        # the contract's set in lexicographic order and its head is the
        # contract's selection.
        for slot, (py_candidate, jl_candidate) in enumerate(
            zip(python_candidates, julia_candidates)
        ):
            _exact_point(
                gate,
                f"{tag}.candidate[{slot}].selector.python",
                py_candidate["position_m"],
                contract_tied[slot],
            )
            _exact_point(
                gate,
                f"{tag}.candidate[{slot}].selector.julia",
                jl_candidate["vertex_m"],
                contract_tied[slot],
            )
            gate.numeric(
                f"{tag}.candidate[{slot}].p",
                "pressure",
                py_candidate["pressure_Pa"],
                jl_candidate["p_Pa"]["f64"],
            )
        if expected_ties > 1:
            pressure_limit = (
                TOLERANCE_TABLE["pressure"]["floor"]
                + RELATIVE * TOLERANCE_TABLE["pressure"]["scale"]
            )
            separation = abs(
                python_candidates[0]["pressure_Pa"]
                - python_candidates[1]["pressure_Pa"]
            )
            gate.structural(
                f"{tag}.rejected_candidate_is_materially_different",
                separation > pressure_limit,
                f"the tied candidates differ by {separation:.6e} Pa, "
                f"{separation / pressure_limit:.3g}x the pressure tolerance "
                f"{pressure_limit:.6e} Pa, so a selector disagreement between the "
                f"routes could never hide inside the tolerance",
            )

    # -- signed fluxes ------------------------------------------------------
    python_flux = python_doc["observations"]["signed_flux_m2_s"]
    julia_flux = julia_doc["fluxes"]
    _exact(gate, "flux.python_keys", set(python_flux), PYTHON_FLUX_KEYS)
    _exact(gate, "flux.julia_keys", set(julia_flux), JULIA_FLUX_KEYS)
    gate.numeric(
        "flux.inlet",
        "signed_flux",
        python_flux["inlet"],
        julia_flux["inlet_m2_per_s"]["f64"],
    )
    gate.numeric(
        "flux.outlet",
        "signed_flux",
        python_flux["outlet"],
        julia_flux["outlet_m2_per_s"]["f64"],
    )
    gate.numeric(
        "flux.sum", "signed_flux", python_flux["sum"], julia_flux["sum_m2_per_s"]["f64"]
    )
    gate.structural(
        "flux.inlet_sign_is_into_the_domain",
        python_flux["inlet"] < 0 and julia_flux["inlet_m2_per_s"]["f64"] < 0,
        "the parent-outward inlet flux must be negative in both routes",
    )
    gate.structural(
        "flux.outlet_sign_is_out_of_the_domain",
        python_flux["outlet"] > 0 and julia_flux["outlet_m2_per_s"]["f64"] > 0,
        "the parent-outward outlet flux must be positive in both routes",
    )
    gate.structural(
        "flux.julia_no_slip_walls_and_cylinder_are_exactly_zero",
        julia_flux["walls_m2_per_s"]["f64"] == 0.0
        and julia_flux["cylinder_m2_per_s"]["f64"] == 0.0,
        "the Julia route reports the no-slip partitions; the Python route does "
        "not emit them, so they are asserted one-sided and not compared",
    )

    # -- cylinder reaction, both labelled orientations ----------------------
    python_cylinder = python_doc["observations"]["cylinder_reaction_N_m"]
    python_balance = python_doc["observations"]["global_balance_N_m"]
    julia_reactions = julia_doc["reactions"]
    _exact(
        gate,
        "reaction.python_cylinder_keys",
        set(python_cylinder),
        PYTHON_CYLINDER_KEYS,
    )
    _exact(
        gate, "reaction.python_balance_keys", set(python_balance), PYTHON_BALANCE_KEYS
    )
    _exact(gate, "reaction.julia_keys", set(julia_reactions), JULIA_REACTION_KEYS)

    julia_on_fluid = [
        component["f64"]
        for component in julia_reactions["cylinder_constraint_force_on_fluid_N_per_m"]
    ]
    julia_on_cylinder = [
        component["f64"]
        for component in julia_reactions["fluid_force_on_cylinder_N_per_m"]
    ]
    _compare_vector(
        gate,
        "reaction.constraint_force_on_fluid",
        "reaction",
        python_cylinder["constraint_force_on_fluid"],
        julia_on_fluid,
    )
    _compare_vector(
        gate,
        "reaction.fluid_force_on_cylinder",
        "reaction",
        python_cylinder["fluid_force_on_cylinder"],
        julia_on_cylinder,
    )
    gate.structural(
        "reaction.orientations_are_exact_negations",
        all(
            python_cylinder["fluid_force_on_cylinder"][axis]
            == -python_cylinder["constraint_force_on_fluid"][axis]
            for axis in (0, 1)
        )
        and all(julia_on_cylinder[axis] == -julia_on_fluid[axis] for axis in (0, 1)),
        "each route must publish the two orientations as exact componentwise "
        "negations, so a consumer cannot confuse them",
    )
    gate.structural(
        "reaction.fluid_force_on_cylinder_acts_along_plus_x",
        python_cylinder["fluid_force_on_cylinder"][0] > 0 and julia_on_cylinder[0] > 0,
        "the fluid force on the cylinder must act downstream in both routes",
    )

    # -- global balance components -----------------------------------------
    for python_key, julia_key in (
        ("constrained_reaction", "all_essential_constrained_reaction_N_per_m"),
        ("integrated_body_force", "integrated_body_force_N_per_m"),
        ("integrated_traction", "integrated_applied_traction_N_per_m"),
        ("sum", "componentwise_sum_N_per_m"),
    ):
        _compare_vector(
            gate,
            f"balance.{python_key}",
            "reaction",
            python_balance[python_key],
            [component["f64"] for component in julia_reactions[julia_key]],
        )
    gate.structural(
        "balance.body_force_and_traction_are_exactly_zero",
        python_balance["integrated_body_force"] == [0.0, 0.0]
        and python_balance["integrated_traction"] == [0.0, 0.0]
        and all(
            component["f64"] == 0.0
            for key in (
                "integrated_body_force_N_per_m",
                "integrated_applied_traction_N_per_m",
            )
            for component in julia_reactions[key]
        ),
        "zero force potential and a (0,0) Pa outlet traction must integrate to "
        "exactly zero in both routes",
    )

    # -- mesh counts, partition, diagonal, cell-pair contract ----------------
    julia_mesh = julia_doc["mesh"]
    mesh_counts = mesh_doc["counts"]
    entity_sets = mesh_doc["entity_sets"]
    partition = {
        "inlet_facets": len(entity_sets["inlet"]["facets"]),
        "outlet_facets": len(entity_sets["outlet"]["facets"]),
        "wall_facets": len(entity_sets["walls"]["facets"]),
        "cylinder_facets": len(entity_sets["cylinder"]["facets"]),
    }
    for key in (
        "vertices",
        "cells",
        "boundary_facets",
        "interior_edges",
        "euler_characteristic",
    ):
        _exact(gate, f"mesh.shared.{key}", mesh_counts[key], MESH_CONTRACT[key])
        _exact(gate, f"mesh.julia.{key}", julia_mesh[key], MESH_CONTRACT[key])
    for key, value in partition.items():
        _exact(gate, f"mesh.shared.{key}", value, MESH_CONTRACT[key])
        _exact(gate, f"mesh.julia.{key}", julia_mesh[key], MESH_CONTRACT[key])
    _exact(
        gate, "mesh.julia.segments", julia_mesh["segments"], MESH_CONTRACT["segments"]
    )
    _exact(
        gate,
        "mesh.julia.outer_loop_vertices",
        julia_mesh["outer_loop_vertices"],
        MESH_CONTRACT["outer_loop_vertices"],
    )
    # The two vertex sets the selector reconstruction partitions the mesh into.
    # Anchoring them here means a mesh whose cylinder membership moved cannot
    # silently change which vertex the pressure selectors resolve to.
    _exact(
        gate,
        "mesh.shared.outer_loop_vertices",
        len(geometry.outer_vertices),
        MESH_CONTRACT["outer_loop_vertices"],
    )
    gate.structural(
        "mesh.shared.cylinder_and_outer_vertices_partition_the_mesh",
        len(geometry.cylinder_vertices) + len(geometry.outer_vertices)
        == MESH_CONTRACT["vertices"],
        f"cylinder {len(geometry.cylinder_vertices)} + outer "
        f"{len(geometry.outer_vertices)} must be all "
        f"{MESH_CONTRACT['vertices']} vertices, with no vertex in both",
    )
    _exact(
        gate,
        "mesh.shared.reconstructed_cell_barycentres",
        len(geometry.barycentres),
        MESH_CONTRACT["cells"],
    )
    _exact(
        gate,
        "mesh.shared.segments",
        mesh_doc["construction"]["segments"],
        MESH_CONTRACT["segments"],
    )
    gate.structural(
        "mesh.partition_covers_every_facet_exactly_once",
        sum(partition.values()) == MESH_CONTRACT["boundary_facets"]
        and len(
            set(entity_sets["inlet"]["facets"])
            | set(entity_sets["outlet"]["facets"])
            | set(entity_sets["walls"]["facets"])
            | set(entity_sets["cylinder"]["facets"])
        )
        == MESH_CONTRACT["boundary_facets"],
        "the four named boundary sets must partition all 104 facets",
    )
    _exact(gate, "mesh.shared.fluid_cells", len(entity_sets["fluid"]["cells"]), 104)
    for label, value in (
        ("shared", mesh_doc["construction"]["quad_diagonal"]),
        ("julia", julia_mesh["quad_diagonal"]),
        ("python", python_doc["mesh"]["accepted"]["quad_diagonal"]),
    ):
        _exact(
            gate, f"mesh.{label}.quad_diagonal", value, MESH_CONTRACT["quad_diagonal"]
        )
    _exact(
        gate,
        "mesh.julia.cell_pair",
        julia_mesh["cells_per_ray_pair"],
        MESH_CONTRACT["cell_pair"],
    )
    for label, spelling in (
        ("python", python_doc["mesh"]["accepted"]["quad_cells"]),
        ("shared", mesh_doc["construction"]["quad_cells"]),
    ):
        # The two documents join the ordered pair with different connectives
        # ("and" here, "then" in the mesh construction record). Only the two
        # triangles and their order are contractual, so the connective is
        # normalized away and everything else must match exactly.
        normalized = spelling.replace(" ", "").replace("and", "").replace("then", "")
        gate.structural(
            f"mesh.{label}.cell_pair",
            normalized == "".join(MESH_CONTRACT["cell_pair"]),
            f"got {spelling!r} (normalized {normalized!r}), want the ordered pair "
            f"{MESH_CONTRACT['cell_pair']}",
        )
    _exact(
        gate,
        "mesh.python.admissible_meshes",
        python_doc["mesh"]["admissible_meshes"],
        1,
    )
    _exact(
        gate,
        "mesh.python.falsifier_is_excluded_split",
        python_doc["mesh"]["wrong_contract_falsifier"]["quad_diagonal"],
        "I_i--O_j",
    )

    # -- scales -------------------------------------------------------------
    python_scales = python_doc["scales"]
    julia_scales = julia_doc["scales"]
    for name, want in SCALE_CONTRACT.items():
        python_value = {
            "L_m": python_scales["L_m"],
            "U_m_per_s": python_scales["U_m_s"],
            "P_Pa": python_scales["P_Pa"],
            "G_per_s": python_scales["G_1_s"],
            "Theta_W_per_m": python_scales["Theta_W_m"],
            "Theta_mathematical_W_per_m": python_scales["Theta_issue_spelling_W_m"],
            "mu_hat": python_scales["mu_hat"],
        }[name]
        julia_value = (
            julia_scales["mu_hat"]["f64"] if name == "mu_hat" else julia_scales[name]
        )
        _exact(gate, f"scales.python.{name}", python_value, want)
        _exact(gate, f"scales.julia.{name}", julia_value, want)
    _exact(gate, "scales.julia.mu_Pa_s", julia_scales["mu_Pa_s"], 0.001)
    _exact(gate, "scales.python.mu_kg_m_s", python_doc["model"]["mu_kg_m_s"], 0.001)
    gate.structural(
        "scales.Theta_is_one_ulp_below_the_exact_decimal",
        math.nextafter(SCALE_CONTRACT["Theta_W_per_m"], math.inf)
        == SCALE_CONTRACT["Theta_mathematical_W_per_m"],
        "both routes must record the binary64 Theta as exactly one ulp below "
        "the exact decimal, and below rather than above it",
    )

    # -- degrees of freedom -------------------------------------------------
    python_dimensions = python_doc["dimensions"]
    julia_dofs = julia_doc["dofs"]
    for name, want in DOF_CONTRACT.items():
        python_value = {
            "velocity_p1": python_dimensions["velocity_p1_dofs"],
            "velocity_bubble": python_dimensions["velocity_bubble_dofs"],
            "pressure": python_dimensions["pressure_dofs"],
            "full_rows": python_dimensions["full_system_rows"],
            "prescribed_velocity": python_dimensions["prescribed_velocity_dofs"],
            "reduced_rows": python_dimensions["reduced_system_rows"],
            "gauge_rows": python_dimensions["gauge_rows"],
            "essential_vertices": python_doc["claim_boundary"][
                "essential_velocity_vertices"
            ],
            "free_vertices": python_dimensions["free_velocity_vertices"],
        }[name]
        julia_value = {
            "velocity_p1": julia_dofs["p1_velocity"],
            "velocity_bubble": julia_dofs["bubble_velocity"],
            "pressure": julia_dofs["pressure"],
            "full_rows": julia_dofs["full"],
            "prescribed_velocity": julia_dofs["essential_velocity"],
            "reduced_rows": julia_dofs["reduced"],
            "gauge_rows": julia_doc["pressure_reference"]["gauge_rows"],
            "essential_vertices": julia_dofs["essential_vertices"],
            "free_vertices": julia_dofs["free_vertices"],
        }[name]
        _exact(gate, f"dofs.python.{name}", python_value, want)
        _exact(gate, f"dofs.julia.{name}", julia_value, want)
    _exact(
        gate, "dofs.python.interior_vertices", python_dimensions["interior_vertices"], 0
    )
    _exact(
        gate,
        "dofs.python.bubbles_are_cell_interior",
        python_doc["claim_boundary"]["bubble_velocity_unknowns_are_cell_interior"],
        True,
    )

    # -- residual target, roundoff, operator, RHS and solution scales --------
    python_residuals = python_doc["observations"]["residuals"]
    julia_residuals = julia_doc["residuals"]
    _exact(
        gate,
        "residuals.selected_target",
        python_residuals["solver_selected_target"],
        julia_residuals["selected_target"],
    )
    _exact(
        gate,
        "residuals.operator_inf_norm",
        python_residuals["reduced_matrix_inf_norm_dimensionless"],
        julia_residuals["A_hat_reduced_inf_norm"],
    )
    _exact(
        gate,
        "residuals.solution_inf_norm",
        python_residuals["solution_inf_norm_dimensionless"],
        julia_residuals["x_hat_inf_norm"],
    )
    _exact(
        gate,
        "residuals.rhs_2norm",
        python_residuals["reduced_rhs_2norm_dimensionless"],
        julia_residuals["b_hat_reduced_2norm"]["f64"],
    )
    # The residual criteria are per route, not cross-route. The frozen contract
    # requires that the independently reapplied true residual, and the weak
    # pressure-row residual, are each finite and no larger than that route's own
    # selected target plus that route's own roundoff allowance. The two routes
    # solve at different working precisions, so their residuals and their
    # allowances are deliberately NOT compared with each other -- doing so would
    # be a cross-route observation nobody precommitted.
    for label, target, allowance, values in (
        (
            "python",
            python_residuals["solver_selected_target"],
            python_residuals["roundoff_allowance"],
            (
                ("true_reduced", python_residuals["true_reduced_dimensionless"]),
                (
                    "weak_pressure_row",
                    python_residuals["weak_pressure_row_dimensionless"],
                ),
            ),
        ),
        (
            "julia",
            julia_residuals["selected_target"],
            julia_residuals["roundoff_allowance"],
            (
                ("true_reduced", julia_residuals["true_reduced_2norm"]["f64"]),
                (
                    "weak_pressure_row",
                    julia_residuals["weak_pressure_row_2norm"]["f64"],
                ),
                (
                    "weak_pressure_row_infnorm",
                    julia_residuals["weak_pressure_row_infnorm"]["f64"],
                ),
            ),
        ),
    ):
        limit = (
            target + allowance
            if _finite(target) and _finite(allowance)
            else float("nan")
        )
        for name, value in values:
            gate.bounded(
                f"residuals.{label}.{name}_within_own_target_plus_allowance",
                value,
                limit,
                f"own selected target {target!r} plus own recorded roundoff "
                f"allowance {allowance!r}; the bound is the frozen contract's "
                f"and is evaluated against this route's own values only",
            )

    # -- BoundaryTraction structure, and the absence of any gauge -----------
    python_reference = python_doc["observations"]["pressure_reference"]
    julia_reference = julia_doc["pressure_reference"]
    _exact(
        gate,
        "pressure_reference.python.kind",
        python_reference["kind"],
        "BoundaryTraction",
    )
    _exact(
        gate,
        "pressure_reference.julia.kind",
        julia_reference["kind"],
        "BoundaryTraction",
    )
    _exact(
        gate,
        "pressure_reference.python.traction_facets",
        python_reference["traction_partition_facets"],
        2,
    )
    _exact(
        gate,
        "pressure_reference.julia.traction_facets",
        julia_reference["traction_partition_facets"],
        2,
    )
    _exact(
        gate,
        "pressure_reference.python.traction_partition_nonempty",
        python_reference["traction_partition_nonempty"],
        True,
    )
    _exact(
        gate,
        "pressure_reference.python.gauge_row",
        python_reference["gauge_row_present"],
        False,
    )
    _exact(
        gate,
        "pressure_reference.python.gauge_multiplier",
        python_reference["gauge_multiplier_present"],
        False,
    )
    _exact(
        gate,
        "pressure_reference.python.zero_integral",
        python_reference["zero_integral_constraint_present"],
        False,
    )
    for key in (
        "gauge_rows",
        "gauge_columns",
        "gauge_multipliers",
        "zero_integral_constraints",
    ):
        _exact(gate, f"pressure_reference.julia.{key}", julia_reference[key], 0)
    _exact(
        gate,
        "pressure_reference.python.reduced_rows",
        python_reference["reduced_system_rows"],
        DOF_CONTRACT["reduced_rows"],
    )

    return gate, geometry


# ---------------------------------------------------------------------------
# Frozen report
# ---------------------------------------------------------------------------
def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_report(gate: Gate, geometry: SharedMeshGeometry) -> dict:
    passed = sum(1 for record in gate.records if record["passed"])
    failed = len(gate.records) - passed
    verdict = "PASS" if failed == 0 else "RETURN"
    families = {}
    for family, entry in TOLERANCE_TABLE.items():
        limit = entry["floor"] + RELATIVE * entry["scale"]
        measured = gate.max_difference[family]
        families[family] = {
            "unit": entry["unit"],
            "absolute_floor": entry["floor"],
            "relative_coefficient": RELATIVE,
            "physical_scale": entry["scale"],
            "tolerance": limit,
            "measured_max_abs_difference": measured,
            "margin_ratio": (limit / measured) if measured > 0 else None,
            "within_tolerance": measured <= limit,
        }
    return {
        "schema": "eqiora.verify/exact-circular-hole-stokes-2d/agreement/v1",
        "gate": "dual independent oracle gate",
        "verdict": verdict,
        "what_this_authorizes": (
            "Agreement of the two independently derived routes authorizes "
            "implementation against the frozen contract. It does NOT verify any "
            "production implementation: no production implementation of this "
            "capability exists, none was read, and none was executed here."
        ),
        "formula": "abs(a - b) <= absolute_floor + 2e-10 * physical_scale",
        "tolerances": families,
        "selector_identity": {
            "note": (
                "Geometric selectors carry no tolerance and are given none. "
                "Every probe target, selected cell, probe vertex and tie "
                "candidate is reconstructed from mesh/mesh.json in exact "
                "rational arithmetic and required to match exactly. Each "
                "route's reported cell barycentre is mapped to the unique "
                "nearest exact mesh-cell barycentre by exact squared distance; "
                "the two routes' approximate barycentre coordinates are never "
                "compared with each other."
            ),
            "source": "mesh/mesh.json",
            "arithmetic": (
                "fractions.Fraction.from_float over the parsed binary64 inputs; "
                "exact, so nothing here rounds and nothing here needs a bound"
            ),
            "cell_tie_break": (
                "lexicographically sorted vertex-coordinate triple, as frozen "
                "by the contract"
            ),
            "vertex_tie_break": "lexicographic coordinate order",
            "cell_barycentres_reconstructed": len(geometry.barycentres),
            "velocity_selectors_reconstructed": len(VELOCITY_TARGETS),
            "pressure_selectors_reconstructed": len(PRESSURE_SELECTOR_RULES),
        },
        "residual_bound": {
            "note": (
                "The residual criteria are per route, never cross-route. Each "
                "route's independently reapplied true residual and its weak "
                "pressure-row residual are required to be finite and within "
                "that route's own bound. The two routes solve at different "
                "working precisions, so neither their residuals nor their "
                "roundoff allowances are compared with each other."
            ),
            "bound": "residual <= own_selected_target + own_roundoff_allowance",
        },
        "checks": {
            "total": len(gate.records),
            "passed": passed,
            "failed": failed,
            "structural": sum(1 for r in gate.records if r["kind"] == "structural"),
            "numeric": sum(1 for r in gate.records if r["kind"] == "numeric"),
            "bounded": sum(1 for r in gate.records if r["kind"] == "bounded"),
        },
        "failures": gate.failures,
        "compared": {
            "python_route": {
                "result": "routes/python/result.json",
                "sha256": digest(PYTHON_RESULT),
                "method": "closed-form barycentric monomial cell blocks, exact "
                "bubble static condensation, dense LU at 40 decimal digits",
            },
            "julia_route": {
                "result": "routes/julia/expected/julia-route-frozen.json",
                "sha256": digest(JULIA_RESULT),
                "method": "explicit 3x3 Gauss-Legendre Duffy quadrature per cell, "
                "no condensation, dense LU at 256-bit BigFloat, mesh "
                "independently reconstructed",
            },
            "shared_mesh": {
                "file": "mesh/mesh.json",
                "sha256": digest(MESH),
                "role": "accepted",
            },
            "wrong_contract_falsifier_mesh": {
                "file": "mesh/falsifier-wrong-diagonal.json",
                "sha256": digest(FALSIFIER_MESH),
                "role": "wrong-contract-falsifier",
            },
            "comparison_route": {
                "file": "agreement/compare_routes.py",
                "sha256": digest(pathlib.Path(__file__).resolve()),
            },
        },
        "provenance": {
            "durable_anchors": DURABLE_ANCHORS,
            "package_base": PACKAGE_BASE,
            "non_durable_identifiers": NON_DURABLE_IDENTIFIERS,
            "historical_source_digests": HISTORICAL_SOURCE_DIGESTS,
        },
        "records": gate.records,
    }


def canonical_bytes(report: dict) -> bytes:
    text = json.dumps(report, indent=2, ensure_ascii=False, sort_keys=False)
    return (text + "\n").encode("utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compare the two frozen oracle routes under the precommitted "
        "tolerance formula"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the frozen agreement report would change; write nothing",
    )
    arguments = parser.parse_args(argv)

    python_doc = json.loads(PYTHON_RESULT.read_text(encoding="utf-8"))
    julia_doc = json.loads(JULIA_RESULT.read_text(encoding="utf-8"))
    mesh_doc = json.loads(MESH.read_text(encoding="utf-8"))

    gate, geometry = compare(python_doc, julia_doc, mesh_doc)
    report = build_report(gate, geometry)
    payload = canonical_bytes(report)

    for family, entry in report["tolerances"].items():
        ratio = entry["margin_ratio"]
        margin = "exact" if ratio is None else f"{ratio:.3g}x inside"
        print(
            f"  {family:<12} max|dP - dJ| = {entry['measured_max_abs_difference']:.6e} "
            f"{entry['unit']:<6} limit {entry['tolerance']:.6e}  ({margin})"
        )
    identity = report["selector_identity"]
    print(
        f"  {'selector':<12} exact       : "
        f"{identity['cell_barycentres_reconstructed']} cell barycentres, "
        f"{identity['velocity_selectors_reconstructed']} velocity and "
        f"{identity['pressure_selectors_reconstructed']} pressure selectors "
        f"rebuilt from {identity['source']} (no tolerance)"
    )
    print(f"  {'residual':<12} per route   : {report['residual_bound']['bound']}")
    checks = report["checks"]
    print(
        f"agreement checks: {checks['passed']} passed, {checks['failed']} failed "
        f"({checks['structural']} structural, {checks['numeric']} numeric, "
        f"{checks['bounded']} bounded)"
    )
    for failure in report["failures"]:
        print(f"  FAILED  {failure}", file=sys.stderr)

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    if arguments.check:
        current = REPORT.read_bytes() if REPORT.exists() else b""
        if current != payload:
            print(
                "agreement report would change; refusing to rewrite under --check",
                file=sys.stderr,
            )
            return 3
        print(f"agreement report reproduced byte for byte  sha256={digest(REPORT)}")
    else:
        REPORT.write_bytes(payload)
        print(f"agreement report  sha256={digest(REPORT)}  bytes={len(payload)}")

    print(f"DUAL INDEPENDENT ORACLE GATE: {report['verdict']}")
    return 0 if report["verdict"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
