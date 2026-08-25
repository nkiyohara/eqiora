"""Installed-wheel contract for Component binding to Python-owned Geometry."""

from importlib.resources import files

import pytest

import eqiora


def geometry(*, x_upper: float = 2.2) -> eqiora.geometry.Geometry:
    return (
        eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
            x_bounds=(0.0, x_upper),
            y_bounds=(0.0, 0.41),
            plane_z=0.0,
            depth=1.0,
            modeling_tolerance=1e-10,
        )
        .circular_through_cut(
            center=(0.2, 0.2),
            radius=0.05,
            boolean_tolerance=1e-10,
        )
        .planar_circular_section(
            classification_tolerance=1e-12,
            region="fluid",
            x_lower="inlet",
            x_upper="outlet",
            y_lower="walls",
            y_upper="walls",
            hole="cylinder",
        )
    )


def inputs(geometry: eqiora.geometry.Geometry):
    supports = {
        name: geometry.selection(name)
        for name in ("fluid", "inlet", "outlet", "walls", "cylinder")
    }
    channel_height = geometry.bounds[1][1] - geometry.bounds[1][0]
    parameters = {
        "dynamic_viscosity": 0.001,
        "zero_pressure": 0.0,
        "inlet_speed": 0.3,
        "channel_height": channel_height,
    }
    return supports, parameters


def bind(
    source: str,
    geometry: eqiora.geometry.Geometry,
    supports: dict[str, eqiora.geometry.GeometrySelection],
    parameters: dict[str, float],
) -> eqiora.Model:
    return eqiora.bind_component(
        source,
        component="SteadyFlowPastCylinder",
        geometry=geometry,
        supports=supports,
        parameters=parameters,
        filename="steady-flow-past-cylinder.eqi",
    )


def test_installed_eqi_and_python_geometry_produce_one_common_model() -> None:
    source_resource = files(eqiora).joinpath(
        "examples", "steady-flow-past-cylinder.eqi"
    )
    source = source_resource.read_text(encoding="utf-8")
    assert "box(" not in source
    assert "0.41" not in source
    assert "0.05" not in source

    authored = geometry()
    supports, parameters = inputs(authored)
    model = bind(source, authored, supports, parameters)
    encoded = model.to_json()
    assert b'"geometry-region"' in encoded
    assert b'"geometry-boundary"' in encoded
    assert b'"entity_set":"fluid"' in encoded
    for name in ("inlet", "outlet", "walls", "cylinder"):
        assert f'"entity_set":"{name}"'.encode() in encoded


def test_bindings_fail_closed_before_returning_a_model() -> None:
    source = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.eqi")
        .read_text(encoding="utf-8")
    )
    authored = geometry()
    supports, parameters = inputs(authored)

    missing = dict(supports)
    del missing["cylinder"]
    with pytest.raises(eqiora.ValidationError, match="cylinder"):
        bind(source, authored, missing, parameters)

    foreign = geometry(x_upper=2.3)
    foreign_supports = dict(supports)
    foreign_supports["inlet"] = foreign.selection("inlet")
    with pytest.raises(eqiora.ValidationError, match="foreign or stale"):
        bind(source, authored, foreign_supports, parameters)

    swapped = dict(supports)
    swapped["fluid"], swapped["inlet"] = swapped["inlet"], swapped["fluid"]
    with pytest.raises(eqiora.ValidationError, match="support"):
        bind(source, authored, swapped, parameters)

    raw_name: dict[str, object] = dict(supports)
    raw_name["inlet"] = "inlet"
    with pytest.raises(TypeError, match="GeometrySelection"):
        bind(source, authored, raw_name, parameters)  # type: ignore[arg-type]

    with pytest.raises(eqiora.ValidationError, match="extra"):
        bind(source, authored, supports, {**parameters, "extra": 1.0})

    with pytest.raises(eqiora.ValidationError, match="finite"):
        bind(source, authored, supports, {**parameters, "inlet_speed": float("nan")})

    with pytest.raises(eqiora.ValidationError, match="not bool"):
        bind(source, authored, supports, {**parameters, "inlet_speed": True})
