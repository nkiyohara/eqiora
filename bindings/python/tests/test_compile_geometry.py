"""Focused installed-Python checks for the single root compile ingress."""

from importlib.resources import files
from pathlib import Path

import pytest

import eqiora


def geometry() -> eqiora.geometry.Geometry:
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=(0.2, 0.2), radius=0.05)
    fluid = graph.subtract(rectangle, circle)
    return graph.build(fluid, named_topology={
        "fluid": fluid.region,
        "inlet": rectangle.boundaries[0],
        "outlet": rectangle.boundaries[1],
        "walls": rectangle.boundaries[2:],
        "cylinder": circle.boundaries[0],
    })


def parameters(source: eqiora.geometry.Geometry) -> dict[str, float]:
    return {
        "dynamic_viscosity": 0.001,
        "zero_pressure": 0.0,
        "inlet_speed": 0.3,
        "channel_height": source.bounds[1][1] - source.bounds[1][0],
    }


def test_installed_path_and_loaded_source_have_one_model_meaning() -> None:
    source_path = files(eqiora).joinpath("examples", "steady-flow-past-cylinder.eqi")
    assert source_path.is_file()
    assert not files(eqiora).joinpath("examples", "steady-flow-past-cylinder.model.json").is_file()
    source = geometry()
    from_path = eqiora.compile(path=source_path, geometry=source, parameters=parameters(source))
    from_text = eqiora.compile(
        source=source_path.read_text(encoding="utf-8"),
        filename="logical/cylinder.eqi",
        geometry=source,
        parameters=parameters(source),
    )
    assert from_path.digest == from_text.digest
    assert eqiora.replay(from_path.to_json()).digest == from_path.digest


def test_source_shape_and_argument_admission_fail_closed(tmp_path: Path) -> None:
    root_source = """
model Main {
  field x: 1 = 1;
  relation balance continuous { x - 1 = 0; }
}
"""
    assert eqiora.compile(source=root_source).digest
    with pytest.raises(TypeError):
        eqiora.compile(root_source)  # type: ignore[call-arg]
    with pytest.raises(eqiora.ValidationError, match="exactly one"):
        eqiora.compile()
    with pytest.raises(eqiora.ValidationError, match="exactly one"):
        eqiora.compile(path=tmp_path / "x.eqi", source=root_source)
    with pytest.raises(eqiora.ValidationError, match="filename"):
        eqiora.compile(path=tmp_path / "x.eqi", filename="logical.eqi")
    with pytest.raises(eqiora.ValidationError, match="require geometry"):
        eqiora.compile(source=root_source, parameters={})

    invalid_utf8 = tmp_path / "invalid.eqi"
    invalid_utf8.write_bytes(b"\xff")
    with pytest.raises(eqiora.ValidationError, match="UTF-8"):
        eqiora.compile(path=invalid_utf8)

    oversized = tmp_path / "oversized.eqi"
    oversized.write_bytes(b" " * 8_388_609)
    with pytest.raises(eqiora.ValidationError, match="8388608-byte"):
        eqiora.compile(path=oversized)
    with pytest.raises(TypeError, match="Unicode"):
        eqiora.compile(path=b"invalid.eqi")


def test_component_and_parameter_inventory_are_source_owned() -> None:
    source_path = files(eqiora).joinpath("examples", "steady-flow-past-cylinder.eqi")
    source_text = source_path.read_text(encoding="utf-8")
    authored = geometry()
    values = parameters(authored)
    for invalid in (
        {name: value for name, value in values.items() if name != "channel_height"},
        {**values, "extra": 1.0},
    ):
        with pytest.raises(eqiora.ValidationError):
            eqiora.compile(source=source_text, geometry=authored, parameters=invalid)
    for invalid in (True, object(), float("nan"), float("inf")):
        with pytest.raises((TypeError, eqiora.ValidationError)):
            eqiora.compile(
                source=source_text,
                geometry=authored,
                parameters={**values, "inlet_speed": invalid},  # type: ignore[dict-item]
            )

    ambiguous = source_text + source_text.replace(
        "SteadyFlowPastCylinder", "OtherSteadyFlow", 1
    )
    with pytest.raises(eqiora.ValidationError, match="component="):
        eqiora.compile(source=ambiguous, geometry=authored, parameters=values)
    selected = eqiora.compile(
        source=ambiguous,
        geometry=authored,
        parameters=values,
        component="OtherSteadyFlow",
    )
    assert selected.digest
