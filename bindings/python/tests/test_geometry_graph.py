from __future__ import annotations

import inspect

import pytest

import eqiora


def construction():
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=(0.2, 0.2), radius=0.05)
    result = graph.subtract(rectangle, circle)
    return graph, rectangle, circle, result


def names(region, outer, cut):
    return {
        "fluid": region,
        "inlet": outer[0],
        "outlet": outer[1],
        "walls": (outer[2], outer[3]),
        "cylinder": cut,
    }


def test_handle_first_surface_has_no_lookup_or_classification_parameter():
    graph, rectangle, circle, result = construction()
    assert str(inspect.signature(graph.rectangle)) == "(*, x_bounds, y_bounds)"
    assert str(inspect.signature(graph.circle)) == "(*, center, radius)"
    assert str(inspect.signature(graph.subtract)) == "(rectangle, circle)"
    assert str(inspect.signature(graph.build)) == "(operation, /, *, named_topology)"
    assert not hasattr(result, "face_handle")
    assert rectangle.region.dimension == result.region.dimension == 2
    assert len(rectangle.boundaries) == 4
    assert len(circle.boundaries) == 1
    assert len(result.boundaries) == 5
    assert all(handle.dimension == 1 for handle in result.boundaries)


def test_rectangle_is_a_classification_free_geometry_for_structured_meshing():
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(-1.0, 3.0), y_bounds=(2.0, 5.0))
    x_lower, x_upper, y_lower, y_upper = rectangle.boundaries
    geometry = graph.build(
        rectangle,
        named_topology={
            "domain": rectangle.region,
            "left": x_lower,
            "right": x_upper,
            "bottom": y_lower,
            "top": y_upper,
        },
    )
    assert geometry.bounds == ((-1.0, 3.0), (2.0, 5.0))
    assert geometry.classification_tolerance is None
    assert geometry.selection_dimension("domain") == 2
    assert geometry.selection_dimension("bottom") == 1
    assert geometry.selection_dimension("top") == 1


def test_subtract_projects_predecessor_and_direct_result_handles_identically():
    graph, rectangle, circle, result = construction()
    predecessor = graph.build(
        result,
        named_topology=names(result.region, rectangle.boundaries, circle.boundaries[0]),
    )
    direct = graph.build(
        result,
        named_topology=names(result.region, result.boundaries[:4], result.boundaries[4]),
    )
    assert predecessor == direct
    assert direct.classification_tolerance is None
    assert direct.selection_names == ("cylinder", "inlet", "outlet", "walls", "fluid")


def test_foreign_deleted_stale_incomplete_and_mixed_handles_reject():
    graph, rectangle, circle, result = construction()
    foreign_graph = eqiora.geometry.GeometryGraph()
    foreign = foreign_graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    stale = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))

    invalid = [
        names(result.region, (foreign.boundaries[0], *rectangle.boundaries[1:]), circle.boundaries[0]),
        names(rectangle.region, rectangle.boundaries, circle.boundaries[0]),
        names(result.region, (stale.boundaries[0], *rectangle.boundaries[1:]), circle.boundaries[0]),
        {"fluid": result.region, "inlet": rectangle.boundaries[0]},
        {
            "mixed": (result.region, rectangle.boundaries[0]),
            "outlet": rectangle.boundaries[1],
            "walls": rectangle.boundaries[2:],
            "cylinder": circle.boundaries[0],
        },
    ]
    for mapping in invalid:
        with pytest.raises(eqiora.ValidationError):
            graph.build(result, named_topology=mapping)

    with pytest.raises(eqiora.ValidationError):
        foreign_graph.subtract(rectangle, circle)
    tangent = graph.circle(center=(0.05, 0.2), radius=0.05)
    with pytest.raises(eqiora.ValidationError):
        graph.subtract(rectangle, tangent)


def test_existing_specialized_surface_remains_for_the_later_atomic_migration():
    old = eqiora.geometry.CadAuthoredGraph
    assert hasattr(old, "rectangle_extrusion")
    assert hasattr(old, "circular_through_cut")
    assert hasattr(old, "planar_circular_section")
