"""Public Python contract for one exact rectangle-with-circular-hole geometry."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CANONICAL_EXAMPLE = (
    REPOSITORY_ROOT / "examples" / "steady-flow-past-cylinder.geometry.json"
)
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


def geometry(**overrides: object) -> object:
    arguments = STANDARD_ARGUMENTS | overrides
    return eqiora.geometry.RectangleWithCircularHole(**arguments)


def assert_structured_validation(**overrides: object) -> None:
    with pytest.raises(eqiora.ValidationError) as caught:
        geometry(**overrides)

    error = caught.value
    assert error.category == "validation"
    assert error.diagnostics
    assert all(diagnostic.code.startswith("EQ") for diagnostic in error.diagnostics)
    assert all(diagnostic.severity == "error" for diagnostic in error.diagnostics)
    assert all(diagnostic.message for diagnostic in error.diagnostics)


def test_standard_geometry_replays_the_existing_exact_fixture() -> None:
    reference = CANONICAL_EXAMPLE.read_bytes()
    assert reference.endswith(b"\n")

    authored = geometry()
    assert type(authored).__module__ == "eqiora._eqiora"
    assert authored.canonical_json == reference.removesuffix(b"\n")
    assert isinstance(authored.canonical_json, bytes)
    assert re.fullmatch(r"[0-9a-f]{64}", authored.digest)
    assert authored.bounds == ((0.0, 2.2), (0.0, 0.41))
    assert authored.circle_center == (0.2, 0.2)
    assert authored.circle_radius == 0.05
    assert authored.tolerance == 1e-12


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
    assert first.canonical_json == negative_zero.canonical_json
    assert swapped_roles != first
    assert swapped_roles.digest != first.digest
    assert changed_tolerance != first
    assert len({first, second, negative_zero, swapped_roles, changed_tolerance}) == 3


def test_geometry_value_and_public_collections_are_immutable() -> None:
    authored = geometry()

    with pytest.raises(AttributeError):
        authored.tolerance = 1e-6
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


def test_bounded_geometry_module_does_not_claim_generic_cad_or_handles() -> None:
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
        "selection",
    ):
        assert not hasattr(authored, unsupported_member)
