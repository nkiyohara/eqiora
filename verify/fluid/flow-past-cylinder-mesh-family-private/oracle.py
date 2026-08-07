#!/usr/bin/env python3
"""Independent structural oracle for the private DFG cylinder MESH0 family.

This oracle predates and does not import the MESH0 implementation. It checks
the checked-in MSH syntax, exact receipts, direct geometry/topology measures,
signed-zero-normalized connectivity, provider/probe identities, ordered
refinement, and the identity-only crossed time fixture. It deliberately knows
nothing about force, pressure, Strouhal, a solver, or a scientific time step.
"""

from __future__ import annotations

import copy
import hashlib
import math
import struct
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parent
EXPECTED_PATH = ROOT / "expected" / "family-identities.toml"
REFERENCES = ROOT / "references"
SOURCE_SHA256 = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
CONTRACT_SHA256 = "52783dffb1164b1911167dcc64fc45ade81e7aaad14e93b51e7fd48f7b25d8e4"
PRIMARY_RECIPE_SHA256 = "e53ec57c6c30f29a4441899bd50d9d530cde0bd33bac4cd45fce5f1e013d9b43"
BIAS_RECIPE_SHA256 = "4fb346431e1703e79ba3b1c16d4d22b3751f62ffb65c81d533ac960509872c66"
EXECUTABLE_SHA256 = "0a923f7069d3ab91d142ed7afcc9e933144c88034e2119067146d2dd87cb4cac"
TIME_METHOD_SHA256 = hashlib.sha256(
    b"eqiora.mesh0.structural-time-method-identity/v1"
).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bits(value: float) -> str:
    return f"0x{struct.unpack('>Q', struct.pack('>d', value))[0]:016x}"


def value(bit_string: str) -> float:
    integer = int(bit_string, 16)
    return struct.unpack(">d", integer.to_bytes(8, "big"))[0]


def normalized_bits(value_: float) -> int:
    if value_ == 0.0:
        value_ = 0.0
    return struct.unpack(">Q", struct.pack(">d", value_))[0]


@dataclass
class Mesh:
    coordinates: dict[int, tuple[float, float]]
    triangles: list[tuple[int, int, int]]
    physical_memberships: int
    element_types: set[int]


def section(lines: list[str], name: str) -> list[str]:
    start = lines.index(f"${name}") + 1
    end = lines.index(f"$End{name}", start)
    return lines[start:end]


def parse_msh(path: Path) -> Mesh:
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    require(section(lines, "MeshFormat") == ["4.1 0 8"], f"{path.name}: ASCII MSH 4.1")

    entity_lines = section(lines, "Entities")
    counts = [int(item) for item in entity_lines[0].split()]
    require(len(counts) == 4, f"{path.name}: entity count header")
    offset = 1
    physical = 0
    for dimension, count in enumerate(counts):
        for _ in range(count):
            fields = entity_lines[offset].split()
            offset += 1
            physical_index = 4 if dimension == 0 else 7
            physical_count = int(fields[physical_index])
            physical += physical_count
            required = physical_index + 1 + physical_count
            require(len(fields) >= required, f"{path.name}: complete entity record")
    require(offset == len(entity_lines), f"{path.name}: no trailing entity records")

    node_lines = section(lines, "Nodes")
    block_count, node_count, minimum_tag, maximum_tag = map(int, node_lines[0].split())
    offset = 1
    coordinates: dict[int, tuple[float, float]] = {}
    for _ in range(block_count):
        dimension, _entity, parametric, count = map(int, node_lines[offset].split())
        offset += 1
        require(dimension in (0, 1, 2), f"{path.name}: planar node entity")
        require(parametric == 0, f"{path.name}: no parametric node payload")
        tags = [int(node_lines[offset + index]) for index in range(count)]
        offset += count
        for tag in tags:
            xyz = [float(item) for item in node_lines[offset].split()]
            offset += 1
            require(len(xyz) == 3 and all(math.isfinite(item) for item in xyz),
                    f"{path.name}: finite XYZ node")
            require(xyz[2] == 0.0, f"{path.name}: exact XY plane")
            require(tag not in coordinates, f"{path.name}: unique node tag")
            coordinates[tag] = (xyz[0], xyz[1])
    require(offset == len(node_lines), f"{path.name}: no trailing node records")
    require(len(coordinates) == node_count, f"{path.name}: node count")
    require(min(coordinates) == minimum_tag and max(coordinates) == maximum_tag,
            f"{path.name}: node tag bounds")

    element_lines = section(lines, "Elements")
    block_count, element_count, _minimum_element, _maximum_element = map(
        int, element_lines[0].split()
    )
    offset = 1
    seen_elements = 0
    triangles: list[tuple[int, int, int]] = []
    element_types: set[int] = set()
    for _ in range(block_count):
        dimension, _entity, element_type, count = map(int, element_lines[offset].split())
        offset += 1
        element_types.add(element_type)
        require((dimension, element_type) in ((0, 15), (1, 1), (2, 2)),
                f"{path.name}: affine point/line/triangle elements only")
        for _ in range(count):
            record = [int(item) for item in element_lines[offset].split()]
            offset += 1
            seen_elements += 1
            if element_type == 2:
                require(len(record) == 4, f"{path.name}: triangle arity")
                triangle = tuple(record[1:])
                require(len(set(triangle)) == 3, f"{path.name}: nondegenerate indices")
                require(all(tag in coordinates for tag in triangle),
                        f"{path.name}: triangle references known nodes")
                triangles.append(triangle)
    require(offset == len(element_lines), f"{path.name}: no trailing elements")
    require(seen_elements == element_count and triangles, f"{path.name}: element counts")
    used = {tag for triangle in triangles for tag in triangle}
    require(used == set(coordinates), f"{path.name}: no isolated imported vertex")
    return Mesh(coordinates, triangles, physical, element_types)


def signed_area(mesh: Mesh, triangle: tuple[int, int, int]) -> float:
    a, b, c = (mesh.coordinates[tag] for tag in triangle)
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def canonical_topology(mesh: Mesh):
    keyed = {
        tag: (normalized_bits(point[0]), normalized_bits(point[1]))
        for tag, point in mesh.coordinates.items()
    }
    require(len(set(keyed.values())) == len(keyed), "normalized coordinate keys are unique")
    ordered_keys = tuple(sorted(keyed.values()))
    rank = {key: index for index, key in enumerate(ordered_keys)}
    triples = []
    for triangle in mesh.triangles:
        area = signed_area(mesh, triangle)
        require(math.isfinite(area) and area != 0.0, "finite nondegenerate triangle")
        oriented = triangle if area > 0.0 else (triangle[0], triangle[2], triangle[1])
        ranked = tuple(rank[keyed[tag]] for tag in oriented)
        rotations = (ranked, ranked[1:] + ranked[:1], ranked[2:] + ranked[:2])
        triples.append(min(rotations))
    return ordered_keys, tuple(sorted(triples))


def max_diameter(mesh: Mesh) -> float:
    result = 0.0
    for triangle in mesh.triangles:
        for first, second in ((0, 1), (1, 2), (2, 0)):
            a = mesh.coordinates[triangle[first]]
            b = mesh.coordinates[triangle[second]]
            result = max(result, math.hypot(a[0] - b[0], a[1] - b[1]))
    return result


def provider_tuple(provider: dict) -> tuple:
    return (
        provider["family_role"],
        provider["generator_name"],
        provider["generator_exact_version"],
        provider["generator_executable_sha256"],
        provider["recipe_template_sha256"],
    )


def validate_family(expected: dict) -> None:
    primary = expected["primary"]
    require(3 <= len(primary) <= 8, "primary cardinality")
    require([item["ordinal"] for item in primary] == list(range(len(primary))), "ordinals")
    require(len({item["provider_seed"] for item in primary}) == len(primary), "member seeds")
    for key in (
        "realized_geometry_sha256", "mesh_sha256", "correspondence_sha256",
        "realization_binding_sha256",
    ):
        require(len({item[key] for item in primary}) == len(primary), f"unique {key}")
    requested = [value(item["requested_boundary_error_bits"]) for item in primary]
    accepted = [value(item["accepted_boundary_error_bits"]) for item in primary]
    chords = [value(item["max_cylinder_chord_bits"]) for item in primary]
    diameters = [value(item["max_triangle_diameter_bits"]) for item in primary]
    segments = [item["circle_segments"] for item in primary]
    require(all(a > b for a, b in zip(requested, requested[1:])), "requested error decreases")
    require(all(a >= b for a, b in zip(accepted, accepted[1:])), "accepted error does not increase")
    require(all(a < b for a, b in zip(segments, segments[1:])), "segments increase")
    require(all(a > b for a, b in zip(chords, chords[1:])), "chords decrease")
    require(all(a > b for a, b in zip(diameters, diameters[1:])), "diameters decrease")
    require(all(item["source_sha256"] == SOURCE_SHA256 for item in [expected]), "exact source")
    primary_provider = provider_tuple(expected["primary_provider"])
    require(primary_provider[0] == "primary", "primary provider role")
    require(primary_provider[1:3] == ("gmsh", "4.13.1"), "primary provider name/version")
    require(primary_provider[3] == EXECUTABLE_SHA256, "primary executable receipt")
    require(primary_provider[4] == PRIMARY_RECIPE_SHA256, "primary recipe receipt")
    bias_provider = provider_tuple(expected["bias_provider"])
    require(bias_provider[0] == "bias", "bias provider role")
    require(bias_provider[1:3] == ("gmsh", "4.13.1"), "bias provider name/version")
    require(bias_provider[3] == EXECUTABLE_SHA256, "bias executable receipt")
    require(bias_provider[4] == BIAS_RECIPE_SHA256, "bias recipe receipt")
    require(primary_provider != bias_provider, "provider families differ")
    bias = expected["bias"]
    fine = primary[-1]
    require(bias["realized_geometry_sha256"] == fine["realized_geometry_sha256"],
            "bias shares fine Geometry")
    for key in ("mesh_sha256", "correspondence_sha256", "realization_binding_sha256"):
        require(bias[key] != fine[key], f"bias has distinct {key}")
    require(bias["provider_seed"] != fine["provider_seed"], "bias seed differs")


EXPECTED_PROBES = [
    {
        "label": "front", "source_boundary": "cylinder",
        "coordinate_bits": ["0x3fc3333333333333", "0x3fc999999999999a"],
        "eta_s_bits": ["0xbff0000000000000", "0x0000000000000000"],
    },
    {
        "label": "rear", "source_boundary": "cylinder",
        "coordinate_bits": ["0x3fd0000000000000", "0x3fc999999999999a"],
        "eta_s_bits": ["0x3ff0000000000000", "0x0000000000000000"],
    },
]


def validate_probes(probes: list[dict]) -> None:
    require(probes == EXPECTED_PROBES, "complete ordered ProbeInventoryIdentity")


def validate_time(fixture: dict, spatial_count: int) -> None:
    method = fixture["opaque_method_identity"]
    require(method == TIME_METHOD_SHA256 and len(bytes.fromhex(method)) == 32,
            "complete opaque method carrier")
    steps = [value(item) for item in fixture["ordered_step_bits"]]
    require(3 <= len(steps) <= 8, "time cardinality")
    require(all(math.isfinite(item) and item > 0.0 for item in steps), "positive finite steps")
    require(all(a > b for a, b in zip(steps, steps[1:])), "steps decrease")
    expected_cells = [f"{space}:{time}" for space in range(spatial_count) for time in range(len(steps))]
    require(fixture["cartesian_cells"] == expected_cells, "complete lexicographic Cartesian set")


def expect_reject(name: str, action) -> None:
    try:
        action()
    except (AssertionError, ValueError, KeyError):
        return
    raise AssertionError(f"mutant survived: {name}")


def exercise_structural_mutants(expected: dict, meshes: dict[str, Mesh]) -> int:
    mutants = 0

    def family_mutant(name: str, update) -> None:
        nonlocal mutants
        candidate = copy.deepcopy(expected)
        update(candidate)
        expect_reject(name, lambda: validate_family(candidate))
        mutants += 1

    family_mutant("fixed-polygon", lambda c: c["primary"][1].update(
        circle_segments=c["primary"][0]["circle_segments"],
        realized_geometry_sha256=c["primary"][0]["realized_geometry_sha256"],
    ))
    family_mutant("duplicate-coarse-as-fine", lambda c: c["primary"][1].update(c["primary"][0]))
    family_mutant("reverse-level-order", lambda c: c["primary"].reverse())
    family_mutant("nondecreasing-chord", lambda c: c["primary"][1].update(
        max_cylinder_chord_bits=c["primary"][0]["max_cylinder_chord_bits"]))
    family_mutant("nondecreasing-diameter", lambda c: c["primary"][1].update(
        max_triangle_diameter_bits=c["primary"][0]["max_triangle_diameter_bits"]))
    family_mutant("reused-mesh", lambda c: c["primary"][1].update(
        mesh_sha256=c["primary"][0]["mesh_sha256"]))
    family_mutant("bias-role-swap", lambda c: c["bias_provider"].update(family_role="primary"))
    family_mutant("bias-recipe-swap", lambda c: c["bias_provider"].update(
        recipe_template_sha256=c["primary_provider"]["recipe_template_sha256"]))
    family_mutant("bias-seed-reuse", lambda c: c["bias"].update(
        provider_seed=c["primary"][-1]["provider_seed"]))
    family_mutant("provider-version-drift", lambda c: c["primary_provider"].update(
        generator_exact_version="4.13"))

    probe_mutations = []
    swapped = copy.deepcopy(expected["probe"]); swapped.reverse(); probe_mutations.append(("probe-order", swapped))
    top_bottom = copy.deepcopy(expected["probe"])
    top_bottom[0]["coordinate_bits"] = [bits(0.2), bits(0.25)]
    top_bottom[1]["coordinate_bits"] = [bits(0.2), bits(0.15)]
    probe_mutations.append(("different-on-circle-probes", top_bottom))
    wrong_normal = copy.deepcopy(expected["probe"])
    wrong_normal[0]["eta_s_bits"] = ["0x3ff0000000000000", "0x0000000000000000"]
    probe_mutations.append(("probe-normal", wrong_normal))
    wrong_boundary = copy.deepcopy(expected["probe"])
    wrong_boundary[0]["source_boundary"] = "walls"
    probe_mutations.append(("probe-boundary", wrong_boundary))
    for name, probes in probe_mutations:
        expect_reject(name, lambda probes=probes: validate_probes(probes))
        mutants += 1

    time_mutations = []
    short = copy.deepcopy(expected["structural_time_fixture"]); short["opaque_method_identity"] = short["opaque_method_identity"][:-2]
    time_mutations.append(("short-method", short))
    mixed = copy.deepcopy(expected["structural_time_fixture"]); mixed["opaque_method_identity"] = "11" * 32
    time_mutations.append(("mixed-method", mixed))
    reordered = copy.deepcopy(expected["structural_time_fixture"]); reordered["ordered_step_bits"].reverse()
    time_mutations.append(("reordered-time", reordered))
    nonfinite = copy.deepcopy(expected["structural_time_fixture"]); nonfinite["ordered_step_bits"][1] = bits(math.inf)
    time_mutations.append(("nonfinite-time", nonfinite))
    missing = copy.deepcopy(expected["structural_time_fixture"]); missing["cartesian_cells"].pop()
    time_mutations.append(("missing-cell", missing))
    duplicate = copy.deepcopy(expected["structural_time_fixture"]); duplicate["cartesian_cells"][-1] = duplicate["cartesian_cells"][0]
    time_mutations.append(("duplicate-cell", duplicate))
    diagonal = copy.deepcopy(expected["structural_time_fixture"]); diagonal["cartesian_cells"] = ["0:0", "1:1", "2:2"]
    time_mutations.append(("diagonal-only", diagonal))
    for name, fixture in time_mutations:
        expect_reject(name, lambda fixture=fixture: validate_time(fixture, 3))
        mutants += 1

    fine = meshes["primary-l2.msh"]
    fine_key = canonical_topology(fine)
    remap = {tag: index + 10_000 for index, tag in enumerate(reversed(list(fine.coordinates)))}
    permuted = Mesh(
        {remap[tag]: point for tag, point in reversed(list(fine.coordinates.items()))},
        [tuple(remap[tag] for tag in triangle) for triangle in reversed(fine.triangles)],
        0,
        set(fine.element_types),
    )
    require(canonical_topology(permuted) == fine_key, "index permutation aliases fine topology")
    mutants += 1
    signed = copy.deepcopy(fine)
    zero_tag = next(tag for tag, point in signed.coordinates.items() if point[0] == 0.0)
    point = signed.coordinates[zero_tag]
    signed.coordinates[zero_tag] = (-0.0, point[1])
    require(canonical_topology(signed) == fine_key, "signed-zero mutation aliases fine topology")
    mutants += 1
    require(canonical_topology(meshes["bias-fine.msh"]) != fine_key,
            "bias is a distinct embedded coordinate/incidence relation")
    return mutants


def main() -> int:
    expected = tomllib.loads(EXPECTED_PATH.read_text(encoding="utf-8"))
    require(expected["schema"] == "eqiora.flow-past-cylinder-mesh-family-private-evidence/v1",
            "evidence schema")
    require(expected["contract_sha256"] == CONTRACT_SHA256, "accepted contract identity")
    require(expected["source_sha256"] == SOURCE_SHA256, "source identity")
    require(sha256(REFERENCES / "primary.geo") == PRIMARY_RECIPE_SHA256, "primary recipe hash")
    require(sha256(REFERENCES / "bias.geo") == BIAS_RECIPE_SHA256, "bias recipe hash")

    meshes: dict[str, Mesh] = {}
    members = [*expected["primary"], expected["bias"]]
    for member in members:
        path = REFERENCES / member["fixture"]
        require(sha256(path) == member["fixture_sha256"], f"{path.name}: exact fixture bytes")
        mesh = parse_msh(path)
        meshes[path.name] = mesh
        require(mesh.physical_memberships == 0, f"{path.name}: no physical groups")
        require(len(mesh.coordinates) == member["vertex_count"], f"{path.name}: vertices")
        require(len(mesh.triangles) == member["triangle_count"], f"{path.name}: triangles")
        require(bits(max_diameter(mesh)) == member["max_triangle_diameter_bits"],
                f"{path.name}: direct maximum triangle diameter")
        segment_count = member["circle_segments"]
        chord = 0.1 * math.sin(math.pi / segment_count)
        require(bits(chord) == member["max_cylinder_chord_bits"],
                f"{path.name}: direct maximum cylinder chord")
        for digest_name in (
            "realized_geometry_sha256", "mesh_sha256", "correspondence_sha256",
            "realization_binding_sha256",
        ):
            digest = member[digest_name]
            require(len(digest) == 64 and bytes.fromhex(digest), f"{path.name}: {digest_name}")

    validate_family(expected)
    validate_probes(expected["probe"])
    validate_time(expected["structural_time_fixture"], len(expected["primary"]))
    mutant_count = exercise_structural_mutants(expected, meshes)
    print("ordinary_positive=PASS")
    print(f"primary_members={len(expected['primary'])}")
    print("bias_members=1")
    print("space_time_cells=9")
    print(f"structural_mutants={mutant_count}")
    print("scientific_values_checked=none")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"oracle_error={error}", file=sys.stderr)
        raise
