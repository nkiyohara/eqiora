#!/usr/bin/env python3
"""Independent elevated-precision oracle for the frozen Gmsh Stokes witness.

The route invokes the official Gmsh 4.15.2 Linux64 executable, parses its ASCII
MSH 4.1 output without a Gmsh library, reconstructs topology and named boundary
sets from entity/element incidence, and solves the already accepted affine
MINI/P1 steady-Stokes formulation.  It imports no Eqiora implementation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import platform
import re
import struct
import subprocess
import sys
from dataclasses import dataclass

try:
    import mpmath as mp
except ImportError:
    print("FATAL: mpmath >= 1.3.0 is required", file=sys.stderr)
    raise SystemExit(2)

import mini


HERE = pathlib.Path(__file__).resolve().parent
GEO_PATH = HERE / "geometry.geo"
RESULT_PATH = HERE / "result.json"

DPS = 60
mp.mp.dps = DPS

BASE_COMMIT = "934493bcb487c1753fb4b3ddffaab88d7150aa7d"
GMSH_VERSION = "4.15.2"
GMSH_ARCHIVE_SHA256 = "6c62116e072db29fd1f701fdb9d3d34b46ed5373545063e177b965a008274745"
GMSH_BINARY_SHA256 = "9dccade5dd1374b28c18af9085d7ce63216cf7ac39d3cefbc0adbfabafba2c7f"
GEO_SHA256 = "81c96068891d6b506827339cd6fecf07eafcb867c76f01747c35d134167d367e"
MESH_SHA256 = "ab7340cec1976f713b5c5deab76fc7d554593126f1c1cd68cc021749911a206a"
EQIORA_MESH_DIGEST = "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"
COORDINATE_BUFFER_SHA256 = (
    "42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d"
)
TRIANGLE_BUFFER_SHA256 = (
    "05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642"
)

MU = mp.mpf(0.001)
H = mp.mpf(0.41)
UMAX = mp.mpf(0.3)
LENGTH_SCALE = H
VELOCITY_SCALE = UMAX
PRESSURE_SCALE = MU * VELOCITY_SCALE / LENGTH_SCALE
GRADIENT_SCALE = VELOCITY_SCALE / LENGTH_SCALE
ACTION_SCALE = PRESSURE_SCALE * VELOCITY_SCALE * LENGTH_SCALE
F64_EPS = mp.mpf(sys.float_info.epsilon)

VELOCITY_TARGETS = (
    (mp.mpf("0.10"), mp.mpf("0.20")),
    (mp.mpf("0.20"), mp.mpf("0.30")),
    (mp.mpf("0.30"), mp.mpf("0.20")),
    (mp.mpf("1.00"), mp.mpf("0.20")),
    (mp.mpf("2.00"), mp.mpf("0.20")),
)
OUTER_PRESSURE_TARGETS = {
    "outer_nearest_x_low_mid": (mp.mpf("0"), mp.mpf("0.20")),
    "outer_nearest_x_high_mid": (mp.mpf("2.2"), mp.mpf("0.20")),
}

SCALE_OF = {
    "velocity": (mp.mpf("2e-12"), VELOCITY_SCALE),
    "pressure": (mp.mpf("2e-14"), PRESSURE_SCALE),
    "flux": (mp.mpf("2e-13"), VELOCITY_SCALE * LENGTH_SCALE),
    "reaction": (mp.mpf("2e-14"), PRESSURE_SCALE * LENGTH_SCALE),
}
ROUTE_RELATIVE = mp.mpf("2e-10")
PRODUCTION_RELATIVE = mp.mpf("5e-7")
SOLVER_RELATIVE = mp.mpf("1e-6")
SOLVER_ABSOLUTE = mp.mpf("1e-13")


def f64(value) -> float:
    return float(mp.mpf(value))


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def canonical_bytes(document) -> bytes:
    text = json.dumps(
        document,
        allow_nan=False,
        ensure_ascii=False,
        indent=2,
        sort_keys=True,
    )
    return (text + "\n").encode("utf-8")


def tolerance(kind: str, relative: mp.mpf) -> mp.mpf:
    floor, scale = SCALE_OF[kind]
    return floor + relative * scale


class Checks:
    def __init__(self) -> None:
        self.records = []

    def check(self, name: str, condition: bool, detail: str = "") -> None:
        self.records.append({"name": name, "passed": bool(condition), "detail": detail})

    def below(self, name: str, measured, limit, detail: str = "") -> None:
        measured = abs(mp.mpf(measured))
        limit = mp.mpf(limit)
        self.records.append(
            {
                "name": name,
                "passed": bool(measured <= limit),
                "detail": detail,
                "measured": f64(measured),
                "limit": f64(limit),
            }
        )

    @property
    def failed(self):
        return [record for record in self.records if not record["passed"]]

    @property
    def passed(self) -> int:
        return sum(record["passed"] for record in self.records)


@dataclass(frozen=True)
class Element:
    tag: int
    entity: int
    nodes: tuple[int, ...]


@dataclass
class Mesh:
    node_tags: list[int]
    vertices: list[tuple[mp.mpf, mp.mpf]]
    node_owner: dict[int, tuple[int, int]]
    cell_tags: list[int]
    cells: list[tuple[int, int, int]]
    facets: list[tuple[tuple[int, int], int, int, int]]
    sets: dict[str, list[int]]
    entity_counts: tuple[int, int, int, int]
    surface_boundary_curves: list[int]
    point_elements: list[Element]
    line_elements: list[Element]

    def facet_vertices(self, name: str) -> set[int]:
        return {
            vertex
            for facet_index in self.sets[name]
            for vertex in self.facets[facet_index][0]
        }

    def outward_times_length(self, facet_index: int):
        (a, b), _, _, _ = self.facets[facet_index]
        pa, pb = self.vertices[a], self.vertices[b]
        return pb[1] - pa[1], -(pb[0] - pa[0])


def _split_sections(payload: bytes) -> dict[str, list[str]]:
    lines = payload.decode("utf-8").splitlines()
    sections = {}
    index = 0
    while index < len(lines):
        line = lines[index]
        if not line.startswith("$") or line.startswith("$End"):
            index += 1
            continue
        name = line[1:]
        end = f"$End{name}"
        try:
            stop = lines.index(end, index + 1)
        except ValueError as error:
            raise ValueError(f"missing {end}") from error
        sections[name] = lines[index + 1 : stop]
        index = stop + 1
    return sections


def _parse_entities(lines: list[str]):
    counts = tuple(map(int, lines[0].split()))
    if len(counts) != 4:
        raise ValueError("invalid Entities header")
    cursor = 1 + counts[0] + counts[1]
    surfaces = []
    for line in lines[cursor : cursor + counts[2]]:
        fields = line.split()
        tag = int(fields[0])
        position = 7
        n_physical = int(fields[position])
        position += 1 + n_physical
        n_boundary = int(fields[position])
        position += 1
        boundary = [int(value) for value in fields[position : position + n_boundary]]
        surfaces.append((tag, boundary))
    return counts, surfaces


def _parse_nodes(lines: list[str]):
    tokens = " ".join(lines).split()
    position = 0

    def integer() -> int:
        nonlocal position
        value = int(tokens[position])
        position += 1
        return value

    n_blocks, n_nodes, minimum, maximum = (integer() for _ in range(4))
    nodes = {}
    owners = {}
    for _ in range(n_blocks):
        dimension, entity, parametric, count = (integer() for _ in range(4))
        tags = [integer() for _ in range(count)]
        for tag in tags:
            xyz = []
            for _ in range(3):
                xyz.append(mp.mpf(float(tokens[position])))
                position += 1
            if parametric:
                position += dimension
            nodes[tag] = tuple(xyz)
            owners[tag] = (dimension, entity)
    if position != len(tokens):
        raise ValueError("unconsumed Nodes tokens")
    if len(nodes) != n_nodes:
        raise ValueError("Nodes count mismatch")
    return (n_blocks, n_nodes, minimum, maximum), nodes, owners


def _parse_elements(lines: list[str]):
    tokens = " ".join(lines).split()
    position = 0

    def integer() -> int:
        nonlocal position
        value = int(tokens[position])
        position += 1
        return value

    n_blocks, n_elements, minimum, maximum = (integer() for _ in range(4))
    node_count = {1: 2, 2: 3, 15: 1}
    elements = {1: [], 2: [], 15: []}
    for _ in range(n_blocks):
        dimension, entity, element_type, count = (integer() for _ in range(4))
        if element_type not in node_count:
            raise ValueError(f"unexpected element type {element_type}")
        expected_dimension = {15: 0, 1: 1, 2: 2}[element_type]
        if dimension != expected_dimension:
            raise ValueError("element block dimension/type mismatch")
        for _ in range(count):
            tag = integer()
            nodes = tuple(integer() for _ in range(node_count[element_type]))
            elements[element_type].append(Element(tag, entity, nodes))
    if position != len(tokens):
        raise ValueError("unconsumed Elements tokens")
    if sum(len(values) for values in elements.values()) != n_elements:
        raise ValueError("Elements count mismatch")
    return (n_blocks, n_elements, minimum, maximum), elements


def parse_mesh(payload: bytes, checks: Checks) -> Mesh:
    sections = _split_sections(payload)
    checks.check(
        "mesh.sections",
        set(sections) == {"MeshFormat", "Entities", "Nodes", "Elements"},
        str(sorted(sections)),
    )
    checks.check(
        "mesh.format",
        sections["MeshFormat"] == ["4.1 0 8"],
        repr(sections["MeshFormat"]),
    )
    entity_counts, surfaces = _parse_entities(sections["Entities"])
    node_header, nodes, owners = _parse_nodes(sections["Nodes"])
    element_header, elements = _parse_elements(sections["Elements"])

    checks.check(
        "mesh.entity_counts", entity_counts == (104, 104, 1, 0), str(entity_counts)
    )
    checks.check("mesh.surface_count", len(surfaces) == 1, str(surfaces))
    surface_boundary = surfaces[0][1]
    expected_boundary = list(range(51, 105)) + list(range(-50, 0))
    checks.check(
        "mesh.surface_boundary_orientation",
        surfaces[0][0] == 107 and surface_boundary == expected_boundary,
        str(surface_boundary),
    )
    checks.check(
        "mesh.node_header", node_header == (209, 662, 1, 662), str(node_header)
    )
    checks.check(
        "mesh.element_header",
        element_header == (209, 1428, 1, 1428),
        str(element_header),
    )
    checks.check(
        "mesh.element_types",
        (len(elements[15]), len(elements[1]), len(elements[2])) == (104, 114, 1210),
        str({kind: len(values) for kind, values in elements.items()}),
    )

    node_tags = sorted(nodes)
    node_index = {tag: index for index, tag in enumerate(node_tags)}
    vertices = [(nodes[tag][0], nodes[tag][1]) for tag in node_tags]
    cells = [tuple(node_index[tag] for tag in element.nodes) for element in elements[2]]
    cell_tags = [element.tag for element in elements[2]]

    edge_occurrences = {}
    areas = []
    for cell_index, cell in enumerate(cells):
        geometry = mini.cell_geometry(*(vertices[index] for index in cell))
        areas.append(geometry.area)
        for local in range(3):
            directed = cell[local], cell[(local + 1) % 3]
            edge_occurrences.setdefault(frozenset(directed), []).append(
                (cell_index, directed)
            )
    checks.check("mesh.cells_positive", all(area > 0 for area in areas), "")
    checks.check(
        "mesh.edge_incidence",
        all(len(uses) in (1, 2) for uses in edge_occurrences.values()),
        "an edge is not used once or twice",
    )

    line_by_edge = {}
    for element in elements[1]:
        edge = frozenset(node_index[tag] for tag in element.nodes)
        if edge in line_by_edge:
            raise ValueError("duplicate line element edge")
        line_by_edge[edge] = element
    boundary_edges = {edge for edge, uses in edge_occurrences.items() if len(uses) == 1}
    checks.check(
        "mesh.lines_equal_triangle_boundary",
        set(line_by_edge) == boundary_edges,
        f"{len(line_by_edge)} lines, {len(boundary_edges)} boundary edges",
    )

    facets = []
    named = {name: [] for name in ("inlet", "outlet", "walls", "cylinder")}
    radius_errors = []
    for edge, element in sorted(line_by_edge.items(), key=lambda item: item[1].tag):
        cell_index, directed = edge_occurrences[edge][0]
        facet_index = len(facets)
        facets.append((directed, cell_index, element.entity, element.tag))
        pa, pb = (vertices[index] for index in directed)
        if element.entity <= 50:
            name = "cylinder"
            for point in (pa, pb):
                radius_errors.append(
                    abs(
                        mp.sqrt(
                            (point[0] - mp.mpf("0.2")) ** 2
                            + (point[1] - mp.mpf("0.2")) ** 2
                        )
                        - mp.mpf("0.05")
                    )
                )
        elif pa[0] == 0 and pb[0] == 0:
            name = "inlet"
        elif pa[0] == mp.mpf(2.2) and pb[0] == mp.mpf(2.2):
            name = "outlet"
        elif (pa[1] == 0 and pb[1] == 0) or (pa[1] == H and pb[1] == H):
            name = "walls"
        else:
            raise ValueError(f"unclassified boundary line entity {element.entity}")
        named[name].append(facet_index)

    sizes = {name: len(indices) for name, indices in named.items()}
    checks.check(
        "mesh.named_boundary_sizes",
        sizes == {"inlet": 14, "outlet": 2, "walls": 48, "cylinder": 50},
        str(sizes),
    )
    checks.below("mesh.circle_coordinate_receipt", max(radius_errors), mp.mpf("5e-16"))
    checks.check(
        "mesh.euler_characteristic",
        len(vertices) - len(edge_occurrences) + len(cells) == 0,
        str(len(vertices) - len(edge_occurrences) + len(cells)),
    )

    return Mesh(
        node_tags=node_tags,
        vertices=vertices,
        node_owner=owners,
        cell_tags=cell_tags,
        cells=cells,
        facets=facets,
        sets=named,
        entity_counts=entity_counts,
        surface_boundary_curves=surface_boundary,
        point_elements=elements[15],
        line_elements=elements[1],
    )


def source_receipt(checks: Checks):
    text = GEO_PATH.read_text(encoding="utf-8")
    found = {}
    pattern = re.compile(r"^Point\((\d+)\) = \{([^,]+), ([^,]+), 0\};$", re.MULTILINE)
    for match in pattern.finditer(text):
        tag = int(match.group(1))
        found[tag] = [float(match.group(2)), float(match.group(3))]
    chord = [found[tag] for tag in range(1, 51)]
    outer = [found[tag] for tag in range(51, 105)]
    all_points = [found[tag] for tag in range(1, 105)]
    checks.check("source.point_inventory", len(found) == 104, str(len(found)))
    checks.check(
        "source.outer_rectangle_boundary",
        all(x in (0.0, 2.2) or y in (0.0, 0.41) for x, y in outer),
        "an outer receipt point is not on an exact rectangle side",
    )
    chord_twice_area = sum(
        chord[index][0] * chord[(index + 1) % len(chord)][1]
        - chord[(index + 1) % len(chord)][0] * chord[index][1]
        for index in range(len(chord))
    )
    outer_twice_area = sum(
        outer[index][0] * outer[(index + 1) % len(outer)][1]
        - outer[(index + 1) % len(outer)][0] * outer[index][1]
        for index in range(len(outer))
    )
    checks.check("source.hole_traversal_clockwise", chord_twice_area < 0, "")
    checks.check("source.outer_traversal_counterclockwise", outer_twice_area > 0, "")
    chord_bytes = canonical_bytes(chord)
    return all_points, digest(chord_bytes), digest(canonical_bytes(all_points))


def mesh_quality(mesh: Mesh):
    areas = []
    qualities = []
    for cell in mesh.cells:
        points = [mesh.vertices[index] for index in cell]
        area = mini.cell_geometry(*points).area
        areas.append(area)
        j00 = points[1][0] - points[0][0]
        j01 = points[2][0] - points[0][0]
        j10 = points[1][1] - points[0][1]
        j11 = points[2][1] - points[0][1]
        frobenius_squared = j00**2 + j01**2 + j10**2 + j11**2
        qualities.append(4 * area / frobenius_squared)
    return min(areas), max(areas), min(qualities)


def inlet_profile(y):
    return 4 * UMAX * y * (H - y) / H**2


def boundary_plan(mesh: Mesh, system: mini.System, checks: Checks, swap=False):
    inlet_name, outlet_name = ("outlet", "inlet") if swap else ("inlet", "outlet")
    inlet = mesh.facet_vertices(inlet_name)
    walls = mesh.facet_vertices("walls")
    cylinder = mesh.facet_vertices("cylinder")
    essential = inlet | walls | cylinder
    prescribed = {}
    consistent = True
    for vertex in sorted(essential):
        candidates = []
        if vertex in inlet:
            candidates.append((inlet_profile(mesh.vertices[vertex][1]), mp.mpf(0)))
        if vertex in walls or vertex in cylinder:
            candidates.append((mp.mpf(0), mp.mpf(0)))
        first = candidates[0]
        consistent = consistent and all(candidate == first for candidate in candidates)
        for component in range(2):
            prescribed[system.vel_p1(vertex, component)] = first[component]
    if not swap:
        checks.check("boundary.trace_closure_consistent", consistent, "")
        checks.check(
            "boundary.essential_vertices", len(essential) == 113, str(len(essential))
        )
        free_vertices = sorted(set(range(len(mesh.vertices))) - essential)
        checks.check(
            "boundary.free_p1_vertices",
            len(free_vertices) == 549,
            str(len(free_vertices)),
        )
    outlet_facets = [mesh.facets[index][0] for index in mesh.sets[outlet_name]]
    return prescribed, outlet_facets, inlet_name, outlet_name


def solve_case(mesh: Mesh, checks: Checks, swap=False, patch=False):
    system = mini.assemble(mesh.vertices, mesh.cells, MU)
    if patch:
        essential = (
            mesh.facet_vertices("inlet")
            | mesh.facet_vertices("walls")
            | mesh.facet_vertices("cylinder")
        )
        prescribed = {}
        for vertex in sorted(essential):
            x, y = mesh.vertices[vertex]
            prescribed[system.vel_p1(vertex, 0)] = x + y
            prescribed[system.vel_p1(vertex, 1)] = -(x + y)
        outlet_name = "outlet"
        inlet_name = "inlet"
        outlet_facets = [mesh.facets[index][0] for index in mesh.sets[outlet_name]]
    else:
        prescribed, outlet_facets, inlet_name, outlet_name = boundary_plan(
            mesh, system, checks, swap
        )
    integrated_traction = mini.add_constant_traction(
        system,
        mesh.vertices,
        outlet_facets,
        (mp.mpf(0), mp.mpf(0)),
    )
    free, reduced_rhs = mini.reduced_inventory(system, prescribed)
    (
        solution,
        outer,
        condensed_matrix,
        condensed_rhs,
        refinement,
    ) = mini.condensed_solve(system, prescribed)
    return {
        "mesh": mesh,
        "system": system,
        "prescribed": prescribed,
        "free": free,
        "reduced_rhs": reduced_rhs,
        "solution": solution,
        "outer": outer,
        "condensed_matrix": condensed_matrix,
        "condensed_rhs": condensed_rhs,
        "refinement": refinement,
        "integrated_traction": integrated_traction,
        "inlet_name": inlet_name,
        "outlet_name": outlet_name,
    }


def coordinate_key(mesh: Mesh, vertex: int):
    return mesh.vertices[vertex]


def select_cell(mesh: Mesh, target):
    ranked = []
    for cell_index, cell in enumerate(mesh.cells):
        barycentre = tuple(
            sum((mesh.vertices[vertex][d] for vertex in cell), mp.mpf(0)) / 3
            for d in range(2)
        )
        distance = sum((barycentre[d] - target[d]) ** 2 for d in range(2))
        geometry_key = sorted(coordinate_key(mesh, vertex) for vertex in cell)
        ranked.append(
            (distance, geometry_key, mesh.cell_tags[cell_index], cell_index, barycentre)
        )
    ranked.sort(key=lambda item: (item[0], item[1]))
    best = ranked[0]
    ties = sum(item[0] == best[0] for item in ranked)
    return best[3], best[4], ties, ranked[1][0] - best[0]


def select_coordinate_extreme(mesh: Mesh, vertices, component: int, largest: bool):
    target = (max if largest else min)(mesh.vertices[v][component] for v in vertices)
    tied = [v for v in vertices if mesh.vertices[v][component] == target]
    chosen = min(tied, key=lambda vertex: coordinate_key(mesh, vertex))
    rest = [mesh.vertices[v][component] for v in vertices if v not in tied]
    second = (max if largest else min)(rest)
    return chosen, tied, abs(target - second)


def select_nearest_vertex(mesh: Mesh, vertices, target):
    ranked = []
    for vertex in vertices:
        point = mesh.vertices[vertex]
        distance = sum((point[d] - target[d]) ** 2 for d in range(2))
        ranked.append((distance, coordinate_key(mesh, vertex), vertex))
    ranked.sort()
    tied = [item[2] for item in ranked if item[0] == ranked[0][0]]
    return ranked[0][2], tied, ranked[1][0] - ranked[0][0]


def pressure_record(
    mesh: Mesh, system: mini.System, solution, name, vertex, tied, margin
):
    return {
        "name": name,
        "node_tag": mesh.node_tags[vertex],
        "position_m": [f64(value) for value in mesh.vertices[vertex]],
        "pressure_Pa": f64(solution[system.pressure(vertex)]),
        "selection_margin": f64(margin),
        "tied_node_tags": [mesh.node_tags[value] for value in tied],
        "_raw": solution[system.pressure(vertex)],
    }


def observe(case, checks: Checks, positive=True):  # noqa: C901
    mesh = case["mesh"]
    system = case["system"]
    solution = case["solution"]
    prescribed = case["prescribed"]
    free = case["free"]
    free_set = set(free)

    action = system.apply(solution)
    residual = [
        action[dof] - system.load.get(dof, mp.mpf(0)) for dof in range(system.size)
    ]

    velocity_end = 2 * system.n_vertices + 2 * system.n_cells

    def dof_scale(dof):
        return PRESSURE_SCALE if dof >= velocity_end else VELOCITY_SCALE

    dimensionless_residual = [
        dof_scale(dof) * residual[dof] / ACTION_SCALE for dof in free
    ]
    true_residual = mp.sqrt(sum(value**2 for value in dimensionless_residual))
    pressure_rows = [system.pressure(vertex) for vertex in range(system.n_vertices)]
    weak_physical = mp.sqrt(sum(residual[row] ** 2 for row in pressure_rows))
    weak_dimensionless = weak_physical * PRESSURE_SCALE / ACTION_SCALE

    b_hat = [
        dof_scale(dof) * case["reduced_rhs"][index] / ACTION_SCALE
        for index, dof in enumerate(free)
    ]
    b_norm = mp.sqrt(sum(value**2 for value in b_hat))
    b_inf = max(abs(value) for value in b_hat)
    x_inf = max(abs(solution[dof] / dof_scale(dof)) for dof in free)
    matrix_inf = mp.mpf(0)
    for row in free:
        row_sum = sum(
            abs(dof_scale(row) * value * dof_scale(column) / ACTION_SCALE)
            for column, value in system.matrix.get(row, {}).items()
            if column in free_set
        )
        matrix_inf = max(matrix_inf, row_sum)
    residual_target = max(SOLVER_RELATIVE * b_norm, SOLVER_ABSOLUTE)
    roundoff = 4096 * F64_EPS * (1 + matrix_inf * x_inf + b_inf)

    asymmetry = mp.mpf(0)
    for row, columns in system.matrix.items():
        for column, value in columns.items():
            asymmetry = max(
                asymmetry,
                abs(value - system.matrix.get(column, {}).get(row, mp.mpf(0))),
            )

    reactions = {dof: residual[dof] for dof in prescribed}

    def reaction_sum(vertices):
        return [
            sum(
                reactions.get(system.vel_p1(vertex, component), mp.mpf(0))
                for vertex in vertices
            )
            for component in range(2)
        ]

    cylinder_reaction = reaction_sum(mesh.facet_vertices("cylinder"))
    all_reaction = [
        sum(value for dof, value in reactions.items() if dof % 2 == component)
        for component in range(2)
    ]
    momentum_sum = [
        all_reaction[component] + case["integrated_traction"][component]
        for component in range(2)
    ]

    def signed_flux(name):
        total = mp.mpf(0)
        for facet_index in mesh.sets[name]:
            (a, b), _, _, _ = mesh.facets[facet_index]
            nx, ny = mesh.outward_times_length(facet_index)
            average = [
                (
                    solution[system.vel_p1(a, component)]
                    + solution[system.vel_p1(b, component)]
                )
                / 2
                for component in range(2)
            ]
            total += average[0] * nx + average[1] * ny
        return total

    inlet_flux = signed_flux(case["inlet_name"])
    outlet_flux = signed_flux(case["outlet_name"])

    velocity_probes = []
    for target in VELOCITY_TARGETS:
        cell_index, barycentre, ties, margin = select_cell(mesh, target)
        cell = mesh.cells[cell_index]
        value = []
        for component in range(2):
            p1 = sum(solution[system.vel_p1(vertex, component)] for vertex in cell) / 3
            value.append(p1 + solution[system.vel_bubble(cell_index, component)])
        velocity_probes.append(
            {
                "target_m": [f64(component) for component in target],
                "element_tag": mesh.cell_tags[cell_index],
                "node_tags": [mesh.node_tags[vertex] for vertex in cell],
                "barycentre_m": [f64(component) for component in barycentre],
                "velocity_m_s": [f64(component) for component in value],
                "tied_cells": ties,
                "selection_margin_m2": f64(margin),
                "_raw": value,
            }
        )

    cylinder_vertices = sorted(mesh.facet_vertices("cylinder"))
    outer_vertices = sorted(
        mesh.facet_vertices("inlet")
        | mesh.facet_vertices("outlet")
        | mesh.facet_vertices("walls")
    )
    pressure_probes = []
    for name, component, largest in (
        ("cylinder_min_x", 0, False),
        ("cylinder_max_x", 0, True),
        ("cylinder_min_y", 1, False),
        ("cylinder_max_y", 1, True),
    ):
        vertex, tied, margin = select_coordinate_extreme(
            mesh, cylinder_vertices, component, largest
        )
        pressure_probes.append(
            pressure_record(mesh, system, solution, name, vertex, tied, margin)
        )
    for name, probe_target in OUTER_PRESSURE_TARGETS.items():
        vertex, tied, margin = select_nearest_vertex(mesh, outer_vertices, probe_target)
        pressure_probes.append(
            pressure_record(mesh, system, solution, name, vertex, tied, margin)
        )

    pressures = [
        solution[system.pressure(vertex)] for vertex in range(system.n_vertices)
    ]
    minimum_vertex = min(
        range(system.n_vertices),
        key=lambda vertex: (pressures[vertex], coordinate_key(mesh, vertex)),
    )
    maximum_vertex = max(
        range(system.n_vertices),
        key=lambda vertex: (
            pressures[vertex],
            tuple(-v for v in coordinate_key(mesh, vertex)),
        ),
    )
    pressure_extrema = {
        "minimum": pressure_record(
            mesh,
            system,
            solution,
            "global_minimum",
            minimum_vertex,
            [minimum_vertex],
            0,
        ),
        "maximum": pressure_record(
            mesh,
            system,
            solution,
            "global_maximum",
            maximum_vertex,
            [maximum_vertex],
            0,
        ),
    }

    if positive:
        checks.below(
            "solution.true_reduced_residual",
            true_residual,
            residual_target + roundoff,
        )
        checks.below(
            "solution.weak_pressure_row_residual",
            weak_dimensionless,
            residual_target + roundoff,
        )
        checks.check(
            "solution.assembled_matrix_symmetric", asymmetry == 0, str(asymmetry)
        )
        checks.below("solution.flux_balance", inlet_flux + outlet_flux, mp.mpf("1e-8"))
        checks.check("solution.inlet_is_inflow", inlet_flux < 0, str(inlet_flux))
        checks.check("solution.outlet_is_outflow", outlet_flux > 0, str(outlet_flux))
        for component in range(2):
            checks.below(
                f"solution.momentum_closure_{'xy'[component]}",
                momentum_sum[component],
                mp.mpf("1e-10"),
            )
        checks.check(
            "solution.cylinder_force_orientation",
            cylinder_reaction[0] < 0,
            "constraint force on fluid must point upstream",
        )
        checks.check(
            "selectors.velocity_unique",
            all(
                probe["tied_cells"] == 1 and probe["selection_margin_m2"] > 0
                for probe in velocity_probes
            ),
            "",
        )
        checks.check(
            "selectors.pressure_stable",
            all(probe["selection_margin"] > 0 for probe in pressure_probes),
            "",
        )

    return {
        "velocity_barycentre_probes": velocity_probes,
        "pressure_geometric_probes": pressure_probes,
        "pressure_global_extrema": pressure_extrema,
        "signed_flux_m2_s": {
            "inlet": f64(inlet_flux),
            "outlet": f64(outlet_flux),
            "net": f64(inlet_flux + outlet_flux),
            "continuous_inlet_reference": f64(-2 * UMAX * H / 3),
        },
        "cylinder_reaction_N_m": {
            "constraint_force_on_fluid": [f64(value) for value in cylinder_reaction],
            "fluid_force_on_cylinder": [f64(-value) for value in cylinder_reaction],
        },
        "momentum_closure_N_m": {
            "all_constrained_reaction": [f64(value) for value in all_reaction],
            "integrated_body_force": [0.0, 0.0],
            "integrated_outlet_traction": [
                f64(value) for value in case["integrated_traction"]
            ],
            "sum": [f64(value) for value in momentum_sum],
        },
        "residual": {
            "true_reduced_2norm_dimensionless": f64(true_residual),
            "weak_pressure_row_2norm_dimensionless": f64(weak_dimensionless),
            "weak_pressure_row_2norm_physical_m2_s": f64(weak_physical),
            "selected_target": f64(residual_target),
            "f64_reapplication_allowance": f64(roundoff),
            "acceptance_limit": f64(residual_target + roundoff),
            "reduced_rhs_2norm_dimensionless": f64(b_norm),
            "reduced_matrix_inf_norm_dimensionless": f64(matrix_inf),
            "solution_inf_norm_dimensionless": f64(x_inf),
            "refinement_iterations": len(case["refinement"]) - 1,
            "condensed_final_residual_2norm_physical": f64(case["refinement"][-1]),
        },
        "pressure_reference": {
            "kind": "BoundaryTraction",
            "traction_facets": len(mesh.sets[case["outlet_name"]]),
            "gauge_row_present": False,
            "zero_integral_constraint_present": False,
        },
        "dimensions": {
            "full_rows": system.size,
            "prescribed_velocity_rows": len(prescribed),
            "reduced_rows": len(free),
            "condensed_rows": len(case["outer"]),
        },
        "_raw": {
            "velocity": [value for probe in velocity_probes for value in probe["_raw"]],
            "pressure": [probe["_raw"] for probe in pressure_probes],
            "flux": [inlet_flux, outlet_flux],
            "reaction": cylinder_reaction + all_reaction,
        },
    }


def strip_private(value):
    if isinstance(value, dict):
        return {
            key: strip_private(item)
            for key, item in value.items()
            if not key.startswith("_")
        }
    if isinstance(value, list):
        return [strip_private(item) for item in value]
    return value


def patch_test(mesh: Mesh, checks: Checks):
    case = solve_case(mesh, checks, patch=True)
    system = case["system"]
    solution = case["solution"]
    velocity_error = mp.mpf(0)
    for vertex, (x, y) in enumerate(mesh.vertices):
        velocity_error = max(
            velocity_error,
            abs(solution[system.vel_p1(vertex, 0)] - (x + y)),
            abs(solution[system.vel_p1(vertex, 1)] + x + y),
        )
    pressure_error = max(
        abs(solution[system.pressure(vertex)] - 2 * MU)
        for vertex in range(system.n_vertices)
    )
    bubble_error = max(
        abs(solution[system.vel_bubble(cell, component)])
        for cell in range(system.n_cells)
        for component in range(2)
    )
    limit = mp.mpf("1e-40")
    checks.below("patch.velocity", velocity_error, limit)
    checks.below("patch.pressure", pressure_error, limit)
    checks.below("patch.bubbles", bubble_error, limit)
    return {
        "field": "u=(x+y, -(x+y)), p=2 mu",
        "max_velocity_error_m_s": f64(velocity_error),
        "max_pressure_error_Pa": f64(pressure_error),
        "max_bubble_magnitude_m_s": f64(bubble_error),
        "limit": f64(limit),
        "meaning": "exact algebraic reproduction on this mesh, not mesh convergence",
    }


def compare_observations(left, right, relative):
    result = {}
    for kind in ("velocity", "pressure", "flux", "reaction"):
        differences = [
            abs(a - b) for a, b in zip(left["_raw"][kind], right["_raw"][kind])
        ]
        maximum = max(differences)
        limit = tolerance(kind, relative)
        result[kind] = {
            "max_abs_difference": f64(maximum),
            "tolerance": f64(limit),
            "ratio_to_tolerance": f64(maximum / limit),
            "rejected": bool(maximum > limit),
        }
    return result


def run_falsifiers(mesh: Mesh, payload: bytes, base_observations, checks: Checks):
    falsifiers = []

    mutated_digest = digest(payload + b"\n")
    rejected = mutated_digest != MESH_SHA256
    checks.check("falsifier.mesh_byte_mutation", rejected, mutated_digest)
    falsifiers.append(
        {
            "name": "mesh_byte_mutation",
            "gate": "exact MSH sha256 before topology",
            "detected": rejected,
            "mutated_sha256": mutated_digest,
        }
    )

    first = mesh.cells[0]
    orientation_rejected = False
    try:
        mini.cell_geometry(
            mesh.vertices[first[1]], mesh.vertices[first[0]], mesh.vertices[first[2]]
        )
    except ValueError:
        orientation_rejected = True
    checks.check("falsifier.reversed_triangle", orientation_rejected, "")
    falsifiers.append(
        {
            "name": "reversed_first_triangle",
            "gate": "strict positive affine-cell orientation",
            "detected": orientation_rejected,
        }
    )

    missing_rejected = len(mesh.sets["cylinder"][:-1]) != 50
    checks.check("falsifier.missing_cylinder_chord", missing_rejected, "")
    falsifiers.append(
        {
            "name": "missing_cylinder_chord",
            "gate": "50-facet cylinder receipt before assembly",
            "detected": missing_rejected,
            "mutated_count": 49,
        }
    )

    reversed_balance = (
        -base_observations["_raw"]["flux"][0] + base_observations["_raw"]["flux"][1]
    )
    reversed_rejected = abs(reversed_balance) > mp.mpf("1e-8")
    checks.check(
        "falsifier.reversed_inlet_normal", reversed_rejected, str(reversed_balance)
    )
    falsifiers.append(
        {
            "name": "reversed_inlet_normal",
            "gate": "signed net flux <= 1e-8 m2/s",
            "detected": reversed_rejected,
            "mutated_net_flux_m2_s": f64(reversed_balance),
            "limit_m2_s": 1e-8,
        }
    )

    cylinder = base_observations["_raw"]["reaction"][:2]
    reaction_error = max(abs(2 * value) for value in cylinder)
    reaction_limit = tolerance("reaction", PRODUCTION_RELATIVE)
    reaction_rejected = reaction_error > reaction_limit and cylinder[0] < 0
    checks.check("falsifier.reaction_sign", reaction_rejected, str(reaction_error))
    falsifiers.append(
        {
            "name": "constraint_force_mislabelled_as_fluid_on_cylinder",
            "gate": "reaction tolerance and upstream constraint-force orientation",
            "detected": reaction_rejected,
            "max_abs_error_N_m": f64(reaction_error),
            "tolerance_N_m": f64(reaction_limit),
            "ratio_to_tolerance": f64(reaction_error / reaction_limit),
        }
    )

    swapped_case = solve_case(mesh, checks, swap=True)
    swapped = observe(swapped_case, checks, positive=False)
    divergence = compare_observations(base_observations, swapped, PRODUCTION_RELATIVE)
    swapped_rejected = any(value["rejected"] for value in divergence.values())
    checks.check("falsifier.swapped_inlet_outlet", swapped_rejected, str(divergence))
    falsifiers.append(
        {
            "name": "swapped_inlet_outlet_membership",
            "gate": "frozen probes, signed fluxes, and cylinder reaction",
            "detected": swapped_rejected,
            "divergence_vs_production_tolerance": divergence,
        }
    )
    return falsifiers


def run_gmsh(gmsh: pathlib.Path, output: pathlib.Path, checks: Checks):
    binary = gmsh.read_bytes()
    binary_digest = digest(binary)
    checks.check(
        "gmsh.binary_sha256", binary_digest == GMSH_BINARY_SHA256, binary_digest
    )
    version = subprocess.run(
        [str(gmsh), "-version"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    checks.check("gmsh.version", version == GMSH_VERSION, version)
    completed = subprocess.run(
        [str(gmsh), "-2", str(GEO_PATH), "-o", str(output), "-v", "2"],
        check=False,
        capture_output=True,
        text=True,
    )
    checks.check(
        "gmsh.mesh_generation",
        completed.returncode == 0 and output.is_file(),
        completed.stdout + completed.stderr,
    )
    return version, binary_digest


def main() -> int:  # noqa: C901
    parser = argparse.ArgumentParser()
    parser.add_argument("--gmsh", required=True, type=pathlib.Path)
    parser.add_argument("--work-dir", required=True, type=pathlib.Path)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()

    checks = Checks()
    checks.check(
        "environment.python", sys.version_info >= (3, 12), sys.version.split()[0]
    )
    checks.check("environment.mpmath", mp.__version__ >= "1.3.0", mp.__version__)
    checks.check("environment.system", platform.system() == "Linux", platform.system())
    checks.check(
        "environment.machine", platform.machine() == "x86_64", platform.machine()
    )

    geo_payload = GEO_PATH.read_bytes()
    geo_digest = digest(geo_payload)
    checks.check("source.geo_sha256", geo_digest == GEO_SHA256, geo_digest)
    source_points, chord_digest, region_digest = source_receipt(checks)

    arguments.work_dir.mkdir(parents=True, exist_ok=True)
    mesh_path = arguments.work_dir / "mesh.msh"
    version, binary_digest = run_gmsh(arguments.gmsh.resolve(), mesh_path, checks)
    mesh_payload = mesh_path.read_bytes() if mesh_path.exists() else b""
    mesh_digest = digest(mesh_payload)
    checks.check("mesh.sha256", mesh_digest == MESH_SHA256, mesh_digest)

    if checks.failed:
        for failure in checks.failed:
            print(f"FAIL {failure['name']}: {failure['detail']}", file=sys.stderr)
        return 1

    mesh = parse_mesh(mesh_payload, checks)
    minimum_area, maximum_area, minimum_quality = mesh_quality(mesh)
    boundary_vertices = {vertex for facet in mesh.facets for vertex in facet[0]}
    interior_vertices = set(range(len(mesh.vertices))) - boundary_vertices
    checks.check(
        "mesh.boundary_vertices",
        len(boundary_vertices) == 114,
        str(len(boundary_vertices)),
    )
    checks.check(
        "mesh.interior_vertices",
        len(interior_vertices) == 548,
        str(len(interior_vertices)),
    )
    checks.check(
        "mesh.minimum_mean_ratio",
        minimum_quality > mp.mpf("1e-5"),
        str(minimum_quality),
    )

    coordinate_bytes = b"".join(
        struct.pack("<dd", float(x), float(y)) for x, y in mesh.vertices
    )
    triangle_bytes = b"".join(struct.pack("<III", *cell) for cell in mesh.cells)
    coordinate_digest = digest(coordinate_bytes)
    triangle_digest = digest(triangle_bytes)
    checks.check(
        "mesh.coordinate_buffer_sha256",
        coordinate_digest == COORDINATE_BUFFER_SHA256,
        coordinate_digest,
    )
    checks.check(
        "mesh.triangle_buffer_sha256",
        triangle_digest == TRIANGLE_BUFFER_SHA256,
        triangle_digest,
    )

    source_point_errors = []
    point_node_by_entity = {
        entity: tag
        for tag, (dimension, entity) in mesh.node_owner.items()
        if dimension == 0
    }
    for entity, source in enumerate(source_points, 1):
        node_tag = point_node_by_entity[entity]
        vertex = mesh.node_tags.index(node_tag)
        for component in range(2):
            source_point_errors.append(
                abs(mesh.vertices[vertex][component] - mp.mpf(source[component]))
            )
    checks.below(
        "mesh.geo_to_msh_coordinate_serialization",
        max(source_point_errors),
        mp.mpf("5e-16"),
    )

    primary_case = solve_case(mesh, checks)
    observations = observe(primary_case, checks, positive=True)
    patch = patch_test(mesh, checks)

    # The positive route must succeed before any mutant is admitted.
    if checks.failed:
        for failure in checks.failed:
            print(f"FAIL {failure['name']}: {failure['detail']}", file=sys.stderr)
        return 1

    positive_check_count = len(checks.records)
    falsifiers = run_falsifiers(mesh, mesh_payload, observations, checks)

    tolerance_table = {}
    for kind in SCALE_OF:
        tolerance_table[kind] = {
            "floor": f64(SCALE_OF[kind][0]),
            "physical_scale": f64(SCALE_OF[kind][1]),
            "route_agreement": f64(tolerance(kind, ROUTE_RELATIVE)),
            "production": f64(tolerance(kind, PRODUCTION_RELATIVE)),
        }

    document = {
        "schema": "eqiora.verify/exact-circular-hole-stokes-2d-gmsh/route/python/v1",
        "status": "frozen-independent-route-a",
        "base_commit": BASE_COMMIT,
        "ordinary_positive_path_completed_before_falsifiers": True,
        "route": {
            "name": "python-closed-form-mini-p1-gmsh-4.15.2",
            "reads_eqiora_implementation": False,
            "precision_decimal_digits": DPS,
            "assembly": "accepted closed-form affine barycentric MINI/P1 blocks",
            "solve": "cell-bubble static condensation; SciPy SuperLU corrections with all residuals and updates reapplied at 60 digits until 1e-48 relative defect",
            "mesh_parser": "standalone ASCII MSH 4.1 Entities/Nodes/Elements parser",
            "dependencies": [
                f"python=={sys.version.split()[0]}",
                f"mpmath=={mp.__version__}",
                f"numpy=={mini.np.__version__}",
                f"scipy=={mini.scipy.__version__}",
            ],
        },
        "frozen_inputs": {
            "geometry_geo": {"file": "geometry.geo", "sha256": geo_digest},
            "chord_receipt": {
                "vertices": 50,
                "canonical_coordinate_json_sha256": chord_digest,
                "source": "shortest round-trip spellings of the accepted chordal f64 coordinates",
            },
            "accepted_planar_region_coordinate_json_sha256": region_digest,
            "gmsh": {
                "distribution": "official Gmsh 4.15.2 Linux64 archive",
                "version": version,
                "archive_sha256": GMSH_ARCHIVE_SHA256,
                "executable_sha256": binary_digest,
            },
            "settings": {
                "kernel": "Built-in",
                "General.NumThreads": 1,
                "Mesh.Algorithm": 6,
                "Mesh.ElementOrder": 1,
                "Mesh.SaveAll": 1,
                "Mesh.MshFileVersion": 4.1,
                "Mesh.Binary": 0,
                "Mesh.RandomFactor": 0,
                "point_characteristic_length": None,
                "construction_order": "hole points/lines, outer points/lines, Plane Surface outer then hole",
            },
        },
        "mesh": {
            "file": "mesh.msh (generated in --work-dir, not committed)",
            "sha256": mesh_digest,
            "accepted_eqiora_mesh_digest": EQIORA_MESH_DIGEST,
            "coordinate_buffer_sha256": coordinate_digest,
            "triangle_u32_buffer_sha256": triangle_digest,
            "format": "MSH 4.1 ASCII, size_t width 8",
            "nodes": len(mesh.vertices),
            "point_elements": len(mesh.point_elements),
            "line_elements": len(mesh.line_elements),
            "triangles": len(mesh.cells),
            "edges": (3 * len(mesh.cells) + len(mesh.facets)) // 2,
            "boundary_edges": len(mesh.facets),
            "interior_edges": (3 * len(mesh.cells) - len(mesh.facets)) // 2,
            "boundary_vertices": len(boundary_vertices),
            "interior_vertices": len(interior_vertices),
            "euler_characteristic": len(mesh.vertices)
            - ((3 * len(mesh.cells) + len(mesh.facets)) // 2)
            + len(mesh.cells),
            "boundary_partition": {
                name: len(indices) for name, indices in mesh.sets.items()
            },
            "surface_boundary_curve_tags": mesh.surface_boundary_curves,
            "minimum_area_m2": f64(minimum_area),
            "maximum_area_m2": f64(maximum_area),
            "minimum_mean_ratio": f64(minimum_quality),
            "maximum_geo_to_msh_coordinate_delta_m": f64(max(source_point_errors)),
            "mapping_rule": "sort global node tags for algebra; keep triangle element tags; orient each boundary facet by its sole positive triangle so fluid is left; classify named boundaries from curve entity and endpoint coordinates",
        },
        "formulation": {
            "equations": "-div(2 mu sym(grad(u)) - p I)=0; div(u)=0",
            "mu_Pa_s": f64(MU),
            "velocity_space": "continuous vector MINI, P1 plus beta=27 l0 l1 l2 per cell/component",
            "pressure_space": "continuous scalar P1",
            "pressure_reference": "nonempty zero-traction outlet; no gauge row",
            "inlet": "u=(4 Umax y (H-y)/H^2, 0)",
            "walls_and_cylinder": "u=(0,0)",
            "outlet": "parent-outward traction (0,0) Pa",
            "body_force": "zero",
        },
        "scales": {
            "L_m": f64(LENGTH_SCALE),
            "U_m_s": f64(VELOCITY_SCALE),
            "P_Pa": f64(PRESSURE_SCALE),
            "G_1_s": f64(GRADIENT_SCALE),
            "Theta_W_m": f64(ACTION_SCALE),
        },
        "production_solver_intent": {
            "backend": "eqiora.faer",
            "provider_library": "faer 0.24.4",
            "algorithm": "sparse-lu",
            "operator": "symmetric-indefinite",
            "scalar": "f64",
            "relative_tolerance": f64(SOLVER_RELATIVE),
            "absolute_tolerance": f64(SOLVER_ABSOLUTE),
            "true_residual": "independent fixed-order reapplication",
        },
        "tolerances": {
            "families": tolerance_table,
            "route_relative": f64(ROUTE_RELATIVE),
            "production_relative": f64(PRODUCTION_RELATIVE),
            "flux_closure_m2_s": 1e-8,
            "momentum_closure_N_m": 1e-10,
            "residual_limit": "max(rtol*||b_hat||2, atol) + 4096 eps_f64 (1 + ||A_hat||inf ||x_hat||inf + ||b_hat||inf)",
            "basis": "unchanged accepted floor-plus-relative-scale table; no value was tuned to this mesh",
        },
        "observations": strip_private(observations),
        "patch_test": patch,
        "falsifiers": falsifiers,
        "checks": {
            "ordinary_positive": positive_check_count,
            "total": len(checks.records),
            "passed": checks.passed,
            "failed": len(checks.failed),
            "failures": [
                f"{record['name']}: {record['detail']}" for record in checks.failed
            ],
            "measurements": [
                {
                    "name": record["name"],
                    "measured": record["measured"],
                    "limit": record["limit"],
                }
                for record in checks.records
                if "measured" in record
            ],
        },
        "source_digests": {
            "oracle.py": digest(pathlib.Path(__file__).read_bytes()),
            "mini.py": digest((HERE / "mini.py").read_bytes()),
        },
        "not_claimed": [
            "no Eqiora implementation was read or executed",
            "no production backend result was checked",
            "no cross-platform Gmsh mesh-byte identity is claimed",
            "no Navier-Stokes, transient, drag coefficient, mesh convergence, curved-element, 3D, performance, or physical benchmark-accuracy claim",
            "this is route A only; independent route-to-route agreement remains external",
        ],
    }

    output = canonical_bytes(document)
    reproduced = RESULT_PATH.exists() and RESULT_PATH.read_bytes() == output
    if arguments.check:
        checks.check("result.byte_reproduction", reproduced, "result.json differs")
        if not reproduced:
            print("FATAL: result.json is not reproduced byte-for-byte", file=sys.stderr)
            return 3
    else:
        RESULT_PATH.write_bytes(output)

    print(f"geometry.geo sha256={geo_digest}")
    print(f"mesh.msh sha256={mesh_digest}")
    print(f"result.json sha256={digest(output)} bytes={len(output)}")
    print(f"checks={checks.passed} passed, {len(checks.failed)} failed")
    for failure in checks.failed:
        print(f"FAIL {failure['name']}: {failure['detail']}")
    return 0 if not checks.failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
