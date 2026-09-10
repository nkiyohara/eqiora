"""The maintained divider runs through the ordinary installed package and session."""
from pathlib import Path
import json
import shutil

import pytest

import eqiora

ROOT = Path(__file__).resolve().parents[3]
SOURCE = (
    ROOT / "docs/site/src/content/docs/reference/standard-packages/_examples/electrical.eqi"
).read_text()


def project(tmp_path, source=SOURCE):
    application = tmp_path / "divider"
    (application / "src").mkdir(parents=True)
    (application / "src/main.eqi").write_text(source)
    (application / "eqiora.toml").write_text(
        '[package]\nname = "org.example.Divider"\nversion = "0.1.0"\nentry = "main"\n'
    )
    store = tmp_path / "store"
    store.mkdir()
    resolution = eqiora.add_bundled_dependency(
        application, store, "Eqiora.Electrical.Basic", version="0.1.0"
    )
    return application, store, resolution


def assert_divider(model, upper="upper.positive", lower="lower.positive"):
    session = model.execution_session(end_time_s=1, max_step_s=1, inputs={})
    # Ohm and Kirchhoff independently give I = 12/(1000+2000), V = 2000*I.
    assert session.through(upper) == pytest.approx(0.004, abs=1e-12, rel=0)
    assert session.across(lower) == pytest.approx(8.0, abs=1e-10, rel=0)
    while session.advance():
        assert session.through(upper) == pytest.approx(0.004, abs=1e-12, rel=0)
        assert session.across(lower) == pytest.approx(8.0, abs=1e-10, rel=0)


def test_installed_divider_executes_and_reopens_offline(tmp_path):
    application, store, resolution = project(tmp_path)
    assert {node["identity"]["name"] for node in json.loads(resolution)["nodes"]} == {
        "org.example.Divider", "Eqiora.Electrical.Basic"
    }
    model = eqiora.compile_package(store, resolution, entry="VoltageDivider")
    assert_divider(model)
    basic = (ROOT / "crates/eqiora-api/packages/Eqiora.Electrical.Basic/src/basic.eqi").read_text()
    direct_source = SOURCE.replace("import Eqiora.Electrical.Basic.basic as electrical;", "").replace(
        "electrical.", ""
    )
    direct = eqiora.compile(source=basic + "\n" + direct_source, entry="VoltageDivider")
    assert_divider(direct)
    ports = {label.selector: label.graph_id for label in model.notation_labels()}
    assert ports["upper.positive.voltage"] is not None and ports["lower.positive.voltage"] is not None
    assert_divider(eqiora.Model.from_bytes(model.to_bytes()),
                   ports["upper.positive.voltage"], ports["lower.positive.voltage"])
    session = model.execution_session(end_time_s=1, max_step_s=1, inputs={})
    # P_upper = I²*1000 = .016 W; P_lower = I²*2000 = .032 W;
    # the source absorbs -12*.004 = -.048 W.
    powers = []
    for component, expected in (("upper", 0.016), ("lower", 0.032), ("source", -0.048)):
        drop = session.across(component + ".positive") - session.across(component + ".negative")
        power = drop * session.through(component + ".positive")
        assert power == pytest.approx(expected, abs=1e-12, rel=0)
        powers.append(power)
    assert sum(powers) == pytest.approx(0, abs=1e-12, rel=0)
    foreign = {label.selector: label.graph_id for label in direct.notation_labels()}["upper.positive.voltage"]
    assert foreign != ports["upper.positive.voltage"]
    for invalid in ("missing", model.model_id, foreign, "01ARZ3NDEKTSV4RRFFQ69G5FAV"):
        for observe in (session.across, session.through):
            with pytest.raises(ValueError, match="exact scalar physical Port"):
                observe(invalid)
    vendor = application / "vendor"
    vendor.mkdir()
    assert eqiora.vendor_project(application, store, vendor) == resolution
    shutil.rmtree(store)
    moved = tmp_path / "moved"
    application.rename(moved)
    assert eqiora.open_project(moved, moved / "vendor") == resolution
    replay = eqiora.compile_package(moved / "vendor", resolution, entry="VoltageDivider")
    assert replay.digest == model.digest
    assert_divider(replay)


def test_installed_divider_rejects_missing_ground(tmp_path):
    floating = SOURCE.replace("  instance ground: electrical.Ground();\n", "").replace(
        ", ground.terminal", ""
    )
    assert floating != SOURCE
    _, store, resolution = project(tmp_path, floating)
    model = eqiora.compile_package(store, resolution, entry="VoltageDivider")
    with pytest.raises(eqiora.ExecutionError, match="singular"):
        model.execution_session(end_time_s=1, max_step_s=1, inputs={})
