"""Frozen timeline and visual-input tests for the private FSI scene."""

from __future__ import annotations

import io
from pathlib import Path

import matplotlib.image as image
import numpy as np
import pytest

import record
import scene


def data() -> scene.SceneData:
    coordinates = np.array(
        [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]], dtype=float
    )
    cells = np.array([[0, 1, 2], [1, 3, 2]], dtype=np.uint32)
    displacement_1 = np.zeros((4, 2), dtype=float)
    displacement_1[1:] = [[0.002, 0.001], [0.0, 0.001], [0.003, 0.004]]
    displacement_2 = np.zeros((4, 2), dtype=float)
    displacement_2[1:] = [[0.004, 0.002], [0.0, 0.002], [0.006, 0.008]]
    pressure_vertices = np.array([0, 1, 2], dtype=np.uint32)
    return scene.SceneData(
        coordinates=coordinates,
        cells=cells,
        fluid_cells=np.array([0], dtype=np.uint32),
        solid_cells=np.array([1], dtype=np.uint32),
        interface_facets=np.array([[1, 2]], dtype=np.uint32),
        steps=(
            scene.AcceptedStep(
                ordinal=1,
                time_s=0.05,
                pressure_vertices=pressure_vertices,
                pressure=np.array([-0.123, 0.05, 0.08]),
                displacement=displacement_1,
            ),
            scene.AcceptedStep(
                ordinal=2,
                time_s=0.1,
                pressure_vertices=pressure_vertices.copy(),
                pressure=np.array([-0.1, 0.07, 0.12]),
                displacement=displacement_2,
            ),
        ),
        case_ids=(
            "fsi.fixed-reference-monolithic-step-2d",
            "artifacts.fixed-reference-fsi-spatial-trajectory",
        ),
        run_digest="1" * 64,
    )


def test_timeline_is_exact_exhaustive_and_never_reverses_geometry() -> None:
    plans = [scene.frame_plan(frame) for frame in range(scene.FRAME_COUNT)]
    assert scene.FRAME_COUNT == scene.FPS * scene.DURATION_S == 450
    assert [plans[first].segment for _, first, _, _ in record.EXPECTED_SEGMENTS] == [
        name for name, _, _, _ in record.EXPECTED_SEGMENTS
    ]
    assert plans[45].state_layer_opacity == pytest.approx(44.0 / 45.0)
    assert plans[89].state_layer_opacity == 0.0
    assert plans[375].state_layer_opacity == pytest.approx(44.0 / 45.0)
    assert plans[419].state_layer_opacity == 0.0
    assert all(plans[frame].state_position == 1 for frame in range(375, 450))
    tau = [plans[frame].interpolation_tau for frame in range(150, 300)]
    assert tau == pytest.approx([index / 150.0 for index in range(150)])
    assert plans[300].state_position == 1
    assert plans[420].poster_opacity == 0.0
    assert plans[449].poster_opacity == 1.0
    assert plans[449].segment_label_opacity == 0.0


def test_profile_rounds_outward_and_contains_all_drawn_coordinates() -> None:
    accepted = data()
    profile = scene.make_profile(accepted)
    assert profile.pressure_display_bound_pa == pytest.approx(0.13)
    assert profile.pressure_display_bound_pa > 0.123
    for step in accepted.steps:
        drawn = accepted.coordinates + scene.DISPLACEMENT_SCALE * step.displacement
        assert profile.x_limits[0] < drawn[:, 0].min()
        assert profile.x_limits[1] > drawn[:, 0].max()
        assert profile.y_limits[0] < drawn[:, 1].min()
        assert profile.y_limits[1] > drawn[:, 1].max()
    viewport_aspect = (
        scene.WIDTH * scene.FIELD_RECT[2] / (scene.HEIGHT * scene.FIELD_RECT[3])
    )
    data_aspect = (profile.x_limits[1] - profile.x_limits[0]) / (
        profile.y_limits[1] - profile.y_limits[0]
    )
    assert data_aspect == pytest.approx(viewport_aspect)


def test_profile_rejects_zero_pressure_instead_of_inventing_a_scale() -> None:
    accepted = data()
    zero_steps = tuple(
        scene.AcceptedStep(
            step.ordinal,
            step.time_s,
            step.pressure_vertices,
            np.zeros_like(step.pressure),
            step.displacement,
        )
        for step in accepted.steps
    )
    with pytest.raises(ValueError, match="degenerate pressure scale"):
        scene.make_profile(
            scene.SceneData(
                accepted.coordinates,
                accepted.cells,
                accepted.fluid_cells,
                accepted.solid_cells,
                accepted.interface_facets,
                zero_steps,
                accepted.case_ids,
                accepted.run_digest,
            )
        )


def test_scene_record_is_self_digested_and_names_presentation_only_blend() -> None:
    accepted = data()
    profile = scene.make_profile(accepted)
    value = scene.scene_record(accepted, profile, "f" * 64)
    assert value["profile_sha256"] == record.content_digest(
        value, "profile_sha256"
    )
    assert value["interpolation"]["kind"] == "presentation-only-linear"
    assert value["physical_time"]["presentation_time_is_not_physical_time"] is True


def test_text_alternative_contains_only_owned_states_and_mapping() -> None:
    text = scene.text_alternative(data())
    assert len(text) > 700
    for fragment in (
        "Fluid pressure [Pa]",
        "solid displacement [m]",
        "×12",
        "accepted step 1",
        "accepted step 2",
        "not solved",
        "never played backwards",
        "Run digest",
    ):
        assert fragment in " ".join(text.split())
    assert not any(
        token in text.lower() for token in record.FORBIDDEN_OBSERVABLE_TOKENS
    )


def test_poster_reduced_motion_and_selected_frames_are_decodable(
    tmp_path: Path,
) -> None:
    accepted = data()
    profile = scene.make_profile(accepted)
    poster = tmp_path / "poster.png"
    reduced = tmp_path / "reduced.png"
    scene.render_poster(accepted, profile, poster)
    scene.render_reduced_motion_still(accepted, profile, reduced)
    poster_pixels = image.imread(poster)
    reduced_pixels = image.imread(reduced)
    assert poster_pixels.shape[:2] == (scene.HEIGHT, scene.WIDTH)
    assert reduced_pixels.shape[:2] == (scene.HEIGHT, scene.WIDTH)
    assert np.ptp(poster_pixels[..., :3]) > 0.0
    assert not np.array_equal(poster_pixels, reduced_pixels)
    for frame in (45, 90, 150, 300, 375, 420, 448):
        destination = tmp_path / f"frame-{frame:04d}.png"
        scene.render_frame(
            accepted,
            profile,
            frame,
            poster_pixels=poster_pixels,
            destination=destination,
        )
        payload = destination.read_bytes()
        assert payload.startswith(b"\x89PNG\r\n\x1a\n")
        assert image.imread(io.BytesIO(payload), format="png").shape[:2] == (
            scene.HEIGHT,
            scene.WIDTH,
        )
    first = tmp_path / "frame-0000.png"
    last = tmp_path / "frame-0449.png"
    scene.copy_poster_frame(poster, first)
    scene.copy_poster_frame(poster, last)
    assert first.read_bytes() == last.read_bytes() == poster.read_bytes()


def test_complete_frame_identity_manifest_is_order_sensitive() -> None:
    values = [record.sha256_bytes(str(index).encode()) for index in range(450)]
    baseline = scene.frame_sequence_digest(values)
    values[0], values[1] = values[1], values[0]
    assert scene.frame_sequence_digest(values) != baseline
