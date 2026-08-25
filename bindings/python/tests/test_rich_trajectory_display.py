from __future__ import annotations

import hashlib
import importlib.util
from importlib.resources import files

import numpy as np
import pytest

import eqiora
from eqiora._presentation import trajectory as presentation

WIDGET_MIME = "application/vnd.jupyter.widget-view+json"
TEXT_MIME = "text/plain"


def accepted_trajectory() -> eqiora.trajectory.Trajectory:
    source = files(eqiora).joinpath("examples", "fixed-reference-fsi.eqi").read_text()
    model = eqiora.compile(source, filename="fixed-reference-fsi.eqi")
    intent = eqiora.fsi.FixedMeshMonolithic(
        time_step_s=0.05,
        steps=2,
        initial_velocity_m_per_s=(0.0, 0.0),
        initial_free_interface_displacement_m=(0.02, 0.0),
        length_scale_m=2.0,
        velocity_scale_m_per_s=0.5,
        pressure_scale_pa=4.0,
        relative_tolerance=1.0e-11,
        absolute_tolerance=1.0e-13,
        maximum_iterations=20_000,
    )
    plan = eqiora.fsi.resolve(model, intent)
    return eqiora.submit(model, plan=plan).result().trajectory


def token(trajectory: eqiora.trajectory.Trajectory) -> dict[str, object]:
    return {
        "model_digest": trajectory.model_digest,
        "geometry_digest": trajectory.geometry_digest,
        "correspondence_digest": trajectory.correspondence_digest,
        "mesh_digest": trajectory.mesh_digest,
        "realization_digest": trajectory.realization_digest,
        "run_digest": trajectory.run_digest,
        "trajectory_digest": trajectory.digest,
        "coordinates": trajectory.coordinates,
        "cells": trajectory.cells,
        "states": trajectory.states,
    }


def test_adapter_captures_only_the_unique_consistent_scalar_vertex_field() -> None:
    trajectory = accepted_trajectory()
    payload = presentation._capture_exact_payload(trajectory, token(trajectory))
    assert payload["profile"] == "fixed-mesh-scalar-trajectory-2d/v1"
    assert payload["vertex_count"] == 9
    assert payload["triangle_count"] == 8
    assert payload["state_count"] == len(trajectory.states) == 2
    assert payload["state_digests"].split(",") == [
        state.digest for state in trajectory.states
    ]
    assert payload["frame"] == "invariant"
    assert len(payload["dimension"].split(",")) == 7

    selected = []
    for state in trajectory.states:
        candidates = [
            field
            for field in state.fields
            if field.value_shape == ()
            and field.frame == "invariant"
            and field.associations == ("vertex",)
        ]
        assert len(candidates) == 1
        selected.append(candidates[0])
    assert payload["field_id"] == selected[0].field.id == selected[1].field.id
    expected = b"".join(
        np.asarray(
            field.values("vertex")[field.support_indices("vertex")], dtype="<f8"
        ).tobytes()
        for field in selected
    )
    assert payload["values_f64_le"] == expected
    assert payload["values_sha256"] == hashlib.sha256(expected).hexdigest()


def test_adapter_rejects_identity_and_array_substitution() -> None:
    trajectory = accepted_trajectory()
    exact = token(trajectory)
    for name, mutant in (
        ("trajectory_digest", "0" * 64),
        ("coordinates", np.array(trajectory.coordinates, copy=True)),
        ("states", tuple(reversed(trajectory.states))),
    ):
        changed = dict(exact)
        changed[name] = mutant
        with pytest.raises(presentation._AdmissionError):
            presentation._capture_exact_payload(trajectory, changed)


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns rich protocol evidence",
)
def test_native_mime_hook_filters_and_reuses_one_immutable_delegate() -> None:
    trajectory = accepted_trajectory()
    assert trajectory._repr_mimebundle_(include=[]) == {}
    text = trajectory._repr_mimebundle_(include=[TEXT_MIME])
    assert set(text) == {TEXT_MIME}
    assert text[TEXT_MIME] == repr(trajectory)

    first = trajectory._repr_mimebundle_()
    second = trajectory._repr_mimebundle_()
    assert set(first) == {TEXT_MIME, WIDGET_MIME}
    assert first[TEXT_MIME] == repr(trajectory)
    assert first[WIDGET_MIME] == second[WIDGET_MIME]
    assert first[WIDGET_MIME]["version_major"] == 2
    assert first[WIDGET_MIME]["version_minor"] == 0


@pytest.mark.skipif(
    importlib.util.find_spec("anywidget") is None,
    reason="the exact eqiora[notebook] candidate profile owns immutable widget evidence",
)
def test_delegate_payload_and_identity_are_immutable() -> None:
    trajectory = accepted_trajectory()
    payload = presentation._capture_exact_payload(trajectory, token(trajectory))
    esm, css = presentation._load_assets()
    delegate = presentation._new_delegate(payload, esm, css)
    try:
        with pytest.raises(Exception):
            delegate.state_count = 3
        with pytest.raises(Exception):
            delegate.set_state({"times_f64_le": b"changed"})
        with pytest.raises(Exception):
            delegate._eqiora_n3_model_id = "changed"
    finally:
        delegate.close()


def test_assets_are_wheel_local_nonempty_and_do_not_request_network() -> None:
    esm, css = presentation._load_assets()
    assert "msg:custom" not in esm
    assert "save_changes" not in esm
    assert "__eqioraN3Oracle" in esm
    assert ".eqiora-trajectory" in css


def test_closed_delegate_is_closed_before_replacement(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class Delegate:
        def __init__(self, *, open_comm: bool) -> None:
            self.comm = type("Comm", (), {"_closed": not open_comm})()
            self.close_calls = 0

        def close(self) -> None:
            self.close_calls += 1

        def _repr_mimebundle_(self) -> dict[str, object]:
            return {WIDGET_MIME: {"version_major": 2, "version_minor": 0}}

    stale = Delegate(open_comm=False)
    replacement = Delegate(open_comm=True)
    monkeypatch.setattr(presentation, "_capture_exact_payload", lambda *_: {})
    monkeypatch.setattr(presentation.importlib.util, "find_spec", lambda *_: object())
    monkeypatch.setattr(presentation, "_load_assets", lambda: ("esm", "css"))
    monkeypatch.setattr(presentation, "_new_delegate", lambda *_: replacement)

    outcome, delegate, bundle = presentation.trajectory_mimebundle(
        object(), object(), stale
    )

    assert outcome == "rich"
    assert stale.close_calls == 1
    assert delegate is replacement
    assert bundle == {WIDGET_MIME: {"version_major": 2, "version_minor": 0}}
