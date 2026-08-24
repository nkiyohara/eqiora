"""Public Python contract for one exact Geometry projected from authored CAD."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CANONICAL_EXAMPLE = (
    REPOSITORY_ROOT / "examples" / "steady-flow-past-cylinder.geometry.json"
)
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "exact_cylinder_geometry.py"
STANDARD_CANONICAL_JSON = (
    b'{"schema":"eqiora.planar-circular-hole-envelope/v1"'
    b',"encoding":"eqiora.canonical-json/v1"'
    b',"kind":"axis-aligned-rectangle-with-circular-hole-v1"'
    b',"length_unit":"metre"'
    b',"tolerance_m":1e-12'
    b',"bounds":[[0.0,2.2],[0.0,0.41]]'
    b',"circle":{"center":[0.2,0.2],"radius_m":0.05}'
    b',"entity_sets":['
    b'{"name":"cylinder","dimension":1,"members":[4]}'
    b',{"name":"inlet","dimension":1,"members":[0]}'
    b',{"name":"outlet","dimension":1,"members":[1]}'
    b',{"name":"walls","dimension":1,"members":[2,3]}'
    b',{"name":"fluid","dimension":2,"members":[0]}]}'
)
STANDARD_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
DISTINCT_Y_CANONICAL_JSON = (
    b'{"schema":"eqiora.planar-circular-hole-envelope/v1"'
    b',"encoding":"eqiora.canonical-json/v1"'
    b',"kind":"axis-aligned-rectangle-with-circular-hole-v1"'
    b',"length_unit":"metre"'
    b',"tolerance_m":1e-12'
    b',"bounds":[[0.0,2.2],[0.0,0.41]]'
    b',"circle":{"center":[0.2,0.2],"radius_m":0.05}'
    b',"entity_sets":['
    b'{"name":"ceiling","dimension":1,"members":[3]}'
    b',{"name":"cylinder","dimension":1,"members":[4]}'
    b',{"name":"floor","dimension":1,"members":[2]}'
    b',{"name":"inlet","dimension":1,"members":[0]}'
    b',{"name":"outlet","dimension":1,"members":[1]}'
    b',{"name":"fluid","dimension":2,"members":[0]}]}'
)
DISTINCT_Y_DIGEST = "51ece8fa2d8709d932b0c758d59c187e4fd572f73217c31dcbe407f8d873be7f"
OFF_AXIS_CANONICAL_JSON = (
    b'{"schema":"eqiora.planar-circular-hole-envelope/v1"'
    b',"encoding":"eqiora.canonical-json/v1"'
    b',"kind":"axis-aligned-rectangle-with-circular-hole-v1"'
    b',"length_unit":"metre"'
    b',"tolerance_m":1e-12'
    b',"bounds":[[0.0,2.2],[0.0,0.41]]'
    b',"circle":{"center":[0.3,0.2],"radius_m":0.05}'
    b',"entity_sets":['
    b'{"name":"cylinder","dimension":1,"members":[4]}'
    b',{"name":"inlet","dimension":1,"members":[0]}'
    b',{"name":"outlet","dimension":1,"members":[1]}'
    b',{"name":"walls","dimension":1,"members":[2,3]}'
    b',{"name":"fluid","dimension":2,"members":[0]}]}'
)
OFF_AXIS_DIGEST = "552ebf459396ed5bc7f72ab48f34046baa828b6af808794e861bd958dc613881"
STANDARD_ARGUMENTS: dict[str, Any] = {
    "bounds": ((0.0, 2.2), (0.0, 0.41)),
    "circle_center": (0.2, 0.2),
    "circle_radius": 0.05,
    "tolerance": 1e-12,
    "region": "fluid",
    "x_lower": "inlet",
    "x_upper": "outlet",
    "y_lower": "walls",
    "y_upper": "walls",
    "hole": "cylinder",
}
EXPECTED_DEMO_STDOUT = [
    STANDARD_DIGEST,
    "cylinder 1",
    "inlet 1",
    "outlet 1",
    "walls 1",
    "fluid 2",
]
ISOLATED_GEOMETRY_PROGRAM = """
import eqiora

graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=1e-10,
).circular_through_cut(
    center=(0.2, 0.2),
    radius=0.05,
    boolean_tolerance=1e-10,
)
geometry = graph.planar_circular_section(
    classification_tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)
print(geometry.digest)
for selection in geometry.selection_names:
    print(selection, geometry.selection_dimension(selection))
"""


def geometry(**overrides: object) -> object:
    arguments = STANDARD_ARGUMENTS | overrides
    unknown = set(arguments) - set(STANDARD_ARGUMENTS)
    if unknown:
        unexpected = min(unknown)
        raise TypeError(f"unsupported Geometry authoring argument: {unexpected}")
    (x_bounds, y_bounds) = arguments["bounds"]
    graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=arguments["circle_center"],
        radius=arguments["circle_radius"],
        boolean_tolerance=1e-10,
    )
    return graph.planar_circular_section(
        classification_tolerance=arguments["tolerance"],
        region=arguments["region"],
        x_lower=arguments["x_lower"],
        x_upper=arguments["x_upper"],
        y_lower=arguments["y_lower"],
        y_upper=arguments["y_upper"],
        hole=arguments["hole"],
    )


def assert_structured_validation(**overrides: object) -> None:
    with pytest.raises(eqiora.ValidationError) as caught:
        geometry(**overrides)

    error = caught.value
    assert error.category == "validation"
    assert error.diagnostics
    assert all(diagnostic.code.startswith("EQ") for diagnostic in error.diagnostics)
    assert all(diagnostic.severity == "error" for diagnostic in error.diagnostics)
    assert all(diagnostic.message for diagnostic in error.diagnostics)


def test_standard_geometry_replays_the_frozen_exact_identity() -> None:
    authored = geometry()
    assert type(authored).__module__ == "eqiora._eqiora"
    assert len(STANDARD_CANONICAL_JSON) == 511
    assert type(authored).__name__ == "Geometry"
    assert authored.dimension == 2
    assert authored.canonical_bytes == STANDARD_CANONICAL_JSON
    assert authored.digest == STANDARD_DIGEST
    assert isinstance(authored.canonical_bytes, bytes)
    assert re.fullmatch(r"[0-9a-f]{64}", authored.digest)
    assert authored.bounds == ((0.0, 2.2), (0.0, 0.41))
    assert authored.classification_tolerance == 1e-12

    if CANONICAL_EXAMPLE.is_file():
        assert CANONICAL_EXAMPLE.read_bytes() == STANDARD_CANONICAL_JSON + b"\n"


def test_fixed_roles_form_the_canonical_named_selection_catalogue() -> None:
    authored = geometry()

    assert authored.selection_names == (
        "cylinder",
        "inlet",
        "outlet",
        "walls",
        "fluid",
    )
    assert isinstance(authored.selection_names, tuple)
    assert {
        name: authored.selection_dimension(name) for name in authored.selection_names
    } == {
        "cylinder": 1,
        "inlet": 1,
        "outlet": 1,
        "walls": 1,
        "fluid": 2,
    }
    assert authored.selection_names.count("walls") == 1

    with pytest.raises(eqiora.ValidationError) as caught:
        authored.selection_dimension("missing")
    assert caught.value.category == "validation"
    assert caught.value.diagnostics


def test_selection_handles_are_immutable_and_revision_bound() -> None:
    authored = geometry()
    inlet = authored.selection("inlet")

    assert type(inlet).__module__ == "eqiora._eqiora"
    assert type(inlet).__name__ == "GeometrySelection"
    assert inlet.name == "inlet"
    assert inlet.dimension == 1
    assert inlet.source_digest == authored.digest
    assert inlet == authored.selection("inlet")
    assert inlet != authored.selection("outlet")

    with pytest.raises(AttributeError):
        inlet.name = "outlet"
    with pytest.raises(eqiora.ValidationError) as caught:
        authored.selection("missing")
    assert caught.value.category == "validation"
    assert caught.value.diagnostics


def test_distinct_y_roles_pin_lower_and_upper_canonical_members() -> None:
    oriented = geometry(y_lower="floor", y_upper="ceiling")

    assert len(DISTINCT_Y_CANONICAL_JSON) == 556
    assert oriented.canonical_bytes == DISTINCT_Y_CANONICAL_JSON
    assert oriented.digest == DISTINCT_Y_DIGEST
    assert oriented.selection_names == (
        "ceiling",
        "cylinder",
        "floor",
        "inlet",
        "outlet",
        "fluid",
    )


def test_off_axis_center_pins_authored_section_coordinate_order() -> None:
    off_axis = geometry(circle_center=(0.3, 0.2))

    assert len(OFF_AXIS_CANONICAL_JSON) == 511
    assert off_axis.canonical_bytes == OFF_AXIS_CANONICAL_JSON
    assert off_axis.digest == OFF_AXIS_DIGEST


def test_identity_is_exact_hashable_and_normalizes_signed_zero() -> None:
    first = geometry()
    second = geometry()
    negative_zero = geometry(
        bounds=((-0.0, 2.2), (-0.0, 0.41)),
    )
    swapped_roles = geometry(x_lower="outlet", x_upper="inlet")
    changed_tolerance = geometry(tolerance=2e-12)

    assert first == second == negative_zero
    assert hash(first) == hash(second) == hash(negative_zero)
    assert first.digest == negative_zero.digest
    assert first.canonical_bytes == negative_zero.canonical_bytes
    assert swapped_roles != first
    assert swapped_roles.digest != first.digest
    assert changed_tolerance != first
    assert len({first, second, negative_zero, swapped_roles, changed_tolerance}) == 3


def test_geometry_value_and_public_collections_are_immutable() -> None:
    authored = geometry()

    with pytest.raises(AttributeError):
        authored.classification_tolerance = 1e-6
    with pytest.raises(AttributeError):
        authored.bounds = ((0.0, 1.0), (0.0, 1.0))
    with pytest.raises(TypeError):
        authored.selection_names[0] = "renamed"


@pytest.mark.parametrize(
    "overrides",
    [
        {"bounds": ((0.0, 0.0), (0.0, 1.0))},
        {"bounds": ((1.0, 0.0), (0.0, 1.0))},
        {"bounds": ((float("-inf"), 1.0), (0.0, 1.0))},
        {"bounds": ((0.0, float("nan")), (0.0, 1.0))},
        {"circle_center": (float("nan"), 0.2)},
        {"circle_center": (0.2, float("inf"))},
        {"circle_radius": 0.0},
        {"circle_radius": -0.05},
        {"circle_radius": float("nan")},
        {"circle_radius": float("inf")},
        {"tolerance": 0.0},
        {"tolerance": -1e-12},
        {"tolerance": float("nan")},
        {"tolerance": float("inf")},
        {
            "bounds": ((0.0, 1.0), (0.0, 1.0)),
            "circle_center": (0.1, 0.5),
            "circle_radius": 0.1,
        },
        {
            "bounds": ((0.0, 1.0), (0.0, 1.0)),
            "circle_center": (-0.2, 0.5),
            "circle_radius": 0.1,
        },
        {
            "bounds": ((0.0, 1.0), (0.0, 1.0)),
            "circle_center": (0.1875, 0.5),
            "circle_radius": 0.125,
            "tolerance": 0.0625,
        },
        {"region": ""},
        {"hole": "  "},
        {"region": "walls"},
    ],
)
def test_invalid_geometry_and_ambiguous_selection_names_fail_closed(
    overrides: dict[str, object],
) -> None:
    assert_structured_validation(**overrides)


@pytest.mark.parametrize(
    "unsupported",
    [
        {"mesh_size": 0.01},
        {"circle_segments": 32},
        {"approximation_tolerance": 1e-4},
    ],
)
def test_constructor_has_no_numerical_realization_policy(
    unsupported: dict[str, object],
) -> None:
    with pytest.raises(TypeError):
        geometry(**unsupported)


def test_bounded_geometry_module_does_not_claim_generic_cad_or_selection_algebra() -> None:
    assert not hasattr(eqiora.geometry, "RectangleWithCircularHole")
    with pytest.raises(TypeError):
        eqiora.geometry.Geometry()

    for unsupported_type in (
        "Rectangle",
        "Circle",
        "Region",
        "Boundary",
        "GeometryRegion",
        "GeometryBoundary",
    ):
        assert not hasattr(eqiora.geometry, unsupported_type)

    authored = geometry()
    for unsupported_member in (
        "subtract",
        "union",
        "intersection",
        "mesh",
        "region",
        "boundary",
        "select",
    ):
        assert not hasattr(authored, unsupported_member)


def test_public_geometry_program_runs_in_an_isolated_subprocess(
    tmp_path: Path,
) -> None:
    completed = subprocess.run(
        [sys.executable, "-I", "-c", ISOLATED_GEOMETRY_PROGRAM],
        cwd=tmp_path,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )

    assert completed.stderr == ""
    assert completed.stdout.splitlines() == EXPECTED_DEMO_STDOUT


def test_checked_in_python_demo_runs_from_installed_package() -> None:
    if not PYTHON_DEMO.is_file():
        pytest.skip("consumer tree does not carry the checked-in Python example")

    completed = subprocess.run(
        [sys.executable, "-I", str(PYTHON_DEMO)],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )

    assert completed.stderr == ""
    assert completed.stdout.splitlines() == EXPECTED_DEMO_STDOUT
