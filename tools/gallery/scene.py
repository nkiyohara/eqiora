"""Frozen private scene for one accepted fixed-reference FSI trajectory."""

from __future__ import annotations

import math
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

import record


WIDTH = 1280
HEIGHT = 720
FPS = 30
FRAME_COUNT = 450
DURATION_S = 15.0
DISPLACEMENT_SCALE = 12.0
FIELD_RECT = (0.075, 0.16, 0.78, 0.70)
COLORBAR_RECT = (0.88, 0.19, 0.018, 0.64)
PNG_METADATA = {"Software": "Eqiora private gallery renderer/1"}
RESET_LABEL = "SCENE RESET — presentation fade, physics not reversed"
INTERPOLATION_LABEL = (
    "PRESENTATION INTERPOLATION t1 → t2 — not solved dynamics"
)
POSTER_RETURN_LABEL = "POSTER RETURN — accepted state 2, not a replay"


@dataclass(frozen=True)
class AcceptedStep:
    """Presentation inputs owned by one accepted source state."""

    ordinal: int
    time_s: float
    pressure_vertices: np.ndarray
    pressure: np.ndarray
    displacement: np.ndarray


@dataclass(frozen=True)
class SceneData:
    """Complete result-owned inputs used by the private renderer."""

    coordinates: np.ndarray
    cells: np.ndarray
    fluid_cells: np.ndarray
    solid_cells: np.ndarray
    interface_facets: np.ndarray
    steps: tuple[AcceptedStep, AcceptedStep]
    case_ids: tuple[str, str]
    run_digest: str


@dataclass(frozen=True)
class SceneProfile:
    """One fixed presentation profile shared by all frames and stills."""

    pressure_display_bound_pa: float
    x_limits: tuple[float, float]
    y_limits: tuple[float, float]


@dataclass(frozen=True)
class FramePlan:
    """Frozen presentation treatment for one frame."""

    segment: str
    state_position: int
    interpolation_tau: float | None
    state_layer_opacity: float
    poster_opacity: float
    segment_label: str | None
    segment_label_opacity: float


def validate_data(data: SceneData) -> None:
    """Fail closed when the accepted result cannot satisfy the frozen scene."""

    if data.coordinates.ndim != 2 or data.coordinates.shape[1] != 2:
        raise ValueError("gallery scene requires two-dimensional coordinates")
    if data.cells.ndim != 2 or data.cells.shape[1] != 3:
        raise ValueError("gallery scene requires affine triangle connectivity")
    if data.interface_facets.ndim != 2 or data.interface_facets.shape[1] != 2:
        raise ValueError("gallery scene requires two-vertex interface facets")
    if tuple(step.ordinal for step in data.steps) != (1, 2):
        raise ValueError("gallery scene requires accepted step ordinals 1 and 2")
    if not data.steps[0].time_s < data.steps[1].time_s:
        raise ValueError("gallery scene requires increasing accepted physical time")
    if not np.array_equal(
        data.steps[0].pressure_vertices, data.steps[1].pressure_vertices
    ):
        raise ValueError("gallery scene requires one fixed pressure support")
    vertex_count = len(data.coordinates)
    for name, values in (
        ("coordinates", data.coordinates),
        ("step-1 pressure", data.steps[0].pressure),
        ("step-2 pressure", data.steps[1].pressure),
        ("step-1 displacement", data.steps[0].displacement),
        ("step-2 displacement", data.steps[1].displacement),
    ):
        if not np.isfinite(values).all():
            raise ValueError(f"gallery scene {name} contains a non-finite value")
    if any(step.displacement.shape != (vertex_count, 2) for step in data.steps):
        raise ValueError("gallery scene displacement is not vertex-coindexed 2D data")
    if any(
        len(step.pressure) != len(step.pressure_vertices) for step in data.steps
    ):
        raise ValueError("gallery scene pressure values and support disagree")
    if any(
        np.any(index < 0) or np.any(index >= upper)
        for index, upper in (
            (data.cells, vertex_count),
            (data.fluid_cells, len(data.cells)),
            (data.solid_cells, len(data.cells)),
            (data.interface_facets, vertex_count),
            (data.steps[0].pressure_vertices, vertex_count),
        )
    ):
        raise ValueError("gallery scene connectivity is outside its accepted support")


def make_profile(data: SceneData) -> SceneProfile:
    """Derive only the pre-committed fixed presentation bounds."""

    validate_data(data)
    pressure_data_bound = max(
        float(np.max(np.abs(data.steps[0].pressure))),
        float(np.max(np.abs(data.steps[1].pressure))),
    )
    if not math.isfinite(pressure_data_bound) or pressure_data_bound <= 0.0:
        raise ValueError("gallery scene cannot invent a degenerate pressure scale")
    exponent = math.floor(math.log10(pressure_data_bound))
    grid = 10.0 ** (exponent - 1)
    pressure_display_bound = math.ceil(pressure_data_bound / grid) * grid
    if pressure_display_bound < pressure_data_bound:
        pressure_display_bound += grid

    drawn = [data.coordinates]
    drawn.extend(
        data.coordinates + DISPLACEMENT_SCALE * step.displacement
        for step in data.steps
    )
    joined = np.concatenate(drawn, axis=0)
    x_limits, y_limits = _padded_equal_aspect_limits(joined)
    return SceneProfile(pressure_display_bound, x_limits, y_limits)


def frame_plan(frame: int) -> FramePlan:
    """Return the exact non-physical/accepted treatment of one frame."""

    if isinstance(frame, bool) or not isinstance(frame, int) or not 0 <= frame < 450:
        raise ValueError("gallery frame index must be an integer from 0 through 449")
    if frame <= 44:
        return FramePlan("poster-open", 1, None, 1.0, 1.0, None, 0.0)
    if frame <= 89:
        opacity = 1.0 - (frame - 44) / 45.0
        return FramePlan(
            "neutral-establish", 1, None, opacity, 0.0, RESET_LABEL, 1.0
        )
    if frame <= 149:
        return FramePlan("state-1-hold", 0, None, 1.0, 0.0, None, 0.0)
    if frame <= 299:
        tau = (frame - 150) / 150.0
        return FramePlan(
            "presentation-blend",
            0,
            tau,
            1.0,
            0.0,
            INTERPOLATION_LABEL,
            1.0,
        )
    if frame <= 374:
        return FramePlan("state-2-hold", 1, None, 1.0, 0.0, None, 0.0)
    if frame <= 419:
        opacity = 1.0 - (frame - 374) / 45.0
        return FramePlan("neutral-reset", 1, None, opacity, 0.0, RESET_LABEL, 1.0)
    poster_opacity = (frame - 420) / 29.0
    return FramePlan(
        "poster-return",
        1,
        None,
        0.0,
        poster_opacity,
        POSTER_RETURN_LABEL,
        1.0 - poster_opacity,
    )


def render_poster(data: SceneData, profile: SceneProfile, destination: Path) -> None:
    """Render the lossless accepted-state-2 poster."""

    _render_single(
        data,
        profile,
        pressure=data.steps[1].pressure,
        displacement=data.steps[1].displacement,
        state_opacity=1.0,
        title=(
            f"Accepted step {data.steps[1].ordinal} • "
            f"t = {data.steps[1].time_s:g} s • "
            "solid displacement ×12 (presentation)"
        ),
        destination=destination,
    )


def render_reduced_motion_still(
    data: SceneData, profile: SceneProfile, destination: Path
) -> None:
    """Render a distinct two-panel comparison that needs no motion."""

    Figure, FigureCanvasAgg, _, _, _, _, Normalize, ScalarMappable = _matplotlib()
    figure = Figure(figsize=(12.8, 7.2), dpi=100, facecolor="#ffffff")
    FigureCanvasAgg(figure)
    axes = [
        figure.add_axes((0.055, 0.19, 0.39, 0.66)),
        figure.add_axes((0.475, 0.19, 0.39, 0.66)),
    ]
    for panel, step in zip(axes, data.steps, strict=True):
        _draw_panel(
            figure,
            panel,
            data,
            profile,
            pressure=step.pressure,
            displacement=step.displacement,
            state_opacity=1.0,
            compact=True,
        )
        panel.set_title(
            f"Accepted step {step.ordinal} • t = {step.time_s:g} s",
            fontsize=11,
            color="#0f172a",
        )
    colorbar_axes = figure.add_axes((0.89, 0.23, 0.018, 0.56))
    scalar = ScalarMappable(
        norm=Normalize(
            vmin=-profile.pressure_display_bound_pa,
            vmax=profile.pressure_display_bound_pa,
        ),
        cmap="coolwarm",
    )
    colorbar = figure.colorbar(scalar, cax=colorbar_axes)
    colorbar.set_label("Fluid pressure [Pa] — fixed display scale", fontsize=10)
    figure.text(
        0.055,
        0.925,
        "Reduced-motion comparison • solid displacement ×12 (presentation)",
        fontsize=15,
        weight="bold",
        color="#0f172a",
    )
    _decorate_figure(figure, data)
    figure.savefig(destination, format="png", dpi=100, metadata=PNG_METADATA)


def render_frame(
    data: SceneData,
    profile: SceneProfile,
    frame: int,
    *,
    poster_pixels: np.ndarray,
    destination: Path,
) -> None:
    """Render one non-poster-copy frame from the frozen scene plan."""

    plan = frame_plan(frame)
    if frame <= 44 or frame == 449:
        raise ValueError("poster-copy frames must copy the emitted poster bytes")
    if plan.interpolation_tau is None:
        step = data.steps[plan.state_position]
        pressure = step.pressure
        displacement = step.displacement
    else:
        tau = plan.interpolation_tau
        pressure = (1.0 - tau) * data.steps[0].pressure + tau * data.steps[1].pressure
        displacement = (
            (1.0 - tau) * data.steps[0].displacement
            + tau * data.steps[1].displacement
        )
    title = _frame_title(data, plan)
    _render_single(
        data,
        profile,
        pressure=pressure,
        displacement=displacement,
        state_opacity=plan.state_layer_opacity,
        title=title,
        destination=destination,
        segment_label=plan.segment_label,
        segment_label_opacity=plan.segment_label_opacity,
        poster_pixels=poster_pixels if plan.poster_opacity > 0.0 else None,
        poster_opacity=plan.poster_opacity,
    )


def copy_poster_frame(poster: Path, destination: Path) -> None:
    """Copy exact poster bytes into an identity-bearing loop frame."""

    shutil.copyfile(poster, destination)


def frame_sequence_digest(frame_digests: list[str]) -> str:
    """Digest the complete ordered frame identity list."""

    if len(frame_digests) != FRAME_COUNT:
        raise ValueError("gallery frame sequence must contain exactly 450 digests")
    manifest = "".join(
        f"{index:04d} {value}\n" for index, value in enumerate(frame_digests)
    ).encode("ascii")
    return record.sha256_bytes(manifest)


def scene_record(
    data: SceneData, profile: SceneProfile, frame_sequence_sha256: str
) -> dict[str, object]:
    """Construct the canonical scene-profile projection for the build record."""

    labels = {
        "poster-open": "accepted state 2 poster",
        "neutral-establish": RESET_LABEL,
        "state-1-hold": "accepted state 1",
        "presentation-blend": INTERPOLATION_LABEL,
        "state-2-hold": "accepted state 2",
        "neutral-reset": RESET_LABEL,
        "poster-return": POSTER_RETURN_LABEL,
    }
    segments = [
        {
            "name": name,
            "first_frame": first,
            "last_frame": last,
            "frame_count": last - first + 1,
            "seconds": (last - first + 1) / FPS,
            "kind": kind,
            "label": labels[name],
        }
        for name, first, last, kind in record.EXPECTED_SEGMENTS
    ]
    value: dict[str, object] = {
        "profile_id": "fixed-reference-fsi-development-film/1",
        "profile_sha256": "",
        "width": WIDTH,
        "height": HEIGHT,
        "fps": FPS,
        "frame_count": FRAME_COUNT,
        "duration_s": DURATION_S,
        "primary_quantity": "fluid_pressure_pa",
        "geometry_state": "solid_displacement_m_times_12",
        "pressure_display_bound_pa": profile.pressure_display_bound_pa,
        "displacement_scale": DISPLACEMENT_SCALE,
        "axis_limits": {"x": list(profile.x_limits), "y": list(profile.y_limits)},
        "physical_time": {
            "state_1_time_s": data.steps[0].time_s,
            "state_2_time_s": data.steps[1].time_s,
            "interval_s": data.steps[1].time_s - data.steps[0].time_s,
            "presentation_time_is_not_physical_time": True,
        },
        "segments": segments,
        "interpolation": {
            "kind": "presentation-only-linear",
            "fields": ["fluid_pressure_pa", "solid_displacement_m"],
            "first_frame": 150,
            "last_frame": 299,
            "tau_rule": "(frame-150)/150",
            "label": INTERPOLATION_LABEL,
        },
        "frame_sequence_sha256": frame_sequence_sha256,
        "fields_presented": [
            "fluid_pressure_pa",
            "solid_displacement_geometry_m",
        ],
        "per_frame_autoranging": False,
        "watermark": record.WATERMARK,
    }
    value["profile_sha256"] = record.content_digest(value, "profile_sha256")
    return value


def text_alternative(data: SceneData) -> str:
    """Describe only accepted states, result-owned geometry, and presentation."""

    return f"""{record.WATERMARK}

This silent presentation shows the result-owned two-dimensional partition:
{len(data.coordinates)} vertices, {len(data.cells)} triangles,
{len(data.fluid_cells)} fluid cells, {len(data.solid_cells)} solid cells, and
{len(data.interface_facets)} conforming interface facets. Fluid pressure [Pa]
is the only primary field and uses one fixed symmetric display scale throughout.
The solid displacement [m] is presentation geometry drawn at an explicit ×12
exaggeration, with reference and deformed outlines distinguished by line style
as well as color.

The source owns accepted step 1 at t1 = {data.steps[0].time_s:g} s and accepted
step 2 at t2 = {data.steps[1].time_s:g} s. The moving middle frames are
presentation interpolation between those two accepted states and are not solved
dynamics or a continuous physical-time claim. A labelled neutral scene reset
changes opacity only; the physics is never played backwards. The reduced-motion
image places accepted step 1 and accepted step 2 side by side with the same
scale, geometry, units, and lineage.

Evidence: {data.case_ids[0]} and {data.case_ids[1]}.
Run digest {data.run_digest}.
"""


def _render_single(
    data: SceneData,
    profile: SceneProfile,
    *,
    pressure: np.ndarray,
    displacement: np.ndarray,
    state_opacity: float,
    title: str,
    destination: Path,
    segment_label: str | None = None,
    segment_label_opacity: float = 0.0,
    poster_pixels: np.ndarray | None = None,
    poster_opacity: float = 0.0,
) -> None:
    Figure, FigureCanvasAgg, _, _, _, _, Normalize, ScalarMappable = _matplotlib()
    figure = Figure(figsize=(12.8, 7.2), dpi=100, facecolor="#ffffff")
    FigureCanvasAgg(figure)
    axes = figure.add_axes(FIELD_RECT)
    _draw_panel(
        figure,
        axes,
        data,
        profile,
        pressure=pressure,
        displacement=displacement,
        state_opacity=state_opacity,
        compact=False,
    )
    axes.set_title(title, fontsize=13, color="#0f172a", pad=11)
    colorbar_axes = figure.add_axes(COLORBAR_RECT)
    scalar = ScalarMappable(
        norm=Normalize(
            vmin=-profile.pressure_display_bound_pa,
            vmax=profile.pressure_display_bound_pa,
        ),
        cmap="coolwarm",
    )
    colorbar = figure.colorbar(scalar, cax=colorbar_axes)
    colorbar.set_label("Fluid pressure [Pa] — fixed display scale", fontsize=10)
    _decorate_figure(figure, data)
    if poster_pixels is not None and poster_opacity > 0.0:
        overlay = figure.add_axes((0.0, 0.0, 1.0, 1.0), zorder=50)
        overlay.imshow(poster_pixels, interpolation="nearest", alpha=poster_opacity)
        overlay.set_axis_off()
    if segment_label is not None and segment_label_opacity > 0.0:
        figure.text(
            0.5,
            0.105,
            segment_label,
            ha="center",
            va="center",
            fontsize=10.5,
            weight="bold",
            color="#7f1d1d",
            alpha=segment_label_opacity,
            zorder=80,
            bbox={
                "boxstyle": "round,pad=0.35",
                "facecolor": "#fff7ed",
                "edgecolor": "#fdba74",
                "alpha": 0.94 * segment_label_opacity,
            },
        )
    figure.savefig(destination, format="png", dpi=100, metadata=PNG_METADATA)


def _draw_panel(
    figure: Any,
    axes: Any,
    data: SceneData,
    profile: SceneProfile,
    *,
    pressure: np.ndarray,
    displacement: np.ndarray,
    state_opacity: float,
    compact: bool,
) -> None:
    _, _, LineCollection, PolyCollection, Triangulation, _, _, _ = _matplotlib()
    coordinates = data.coordinates
    fluid_triangles = data.cells[data.fluid_cells]
    solid_triangles = data.cells[data.solid_cells]
    axes.set_facecolor("#f8fafc")
    axes.add_collection(
        PolyCollection(
            coordinates[fluid_triangles],
            facecolors="#dbeafe",
            edgecolors="none",
            alpha=0.44,
            zorder=1,
        )
    )
    axes.add_collection(
        PolyCollection(
            coordinates[solid_triangles],
            facecolors="#ffedd5",
            edgecolors="none",
            alpha=0.60,
            zorder=1,
        )
    )
    pressure_vertices = data.steps[0].pressure_vertices
    local = {int(vertex): index for index, vertex in enumerate(pressure_vertices)}
    try:
        local_triangles = np.asarray(
            [[local[int(vertex)] for vertex in cell] for cell in fluid_triangles],
            dtype=np.int32,
        )
    except KeyError as error:
        raise ValueError("fluid triangle lies outside accepted pressure support") from error
    pressure_coordinates = coordinates[pressure_vertices]
    triangulation = Triangulation(
        pressure_coordinates[:, 0], pressure_coordinates[:, 1], local_triangles
    )
    if state_opacity > 0.0:
        axes.tripcolor(
            triangulation,
            pressure,
            shading="gouraud",
            cmap="coolwarm",
            vmin=-profile.pressure_display_bound_pa,
            vmax=profile.pressure_display_bound_pa,
            alpha=state_opacity,
            zorder=2,
        )
    mesh_edges = _triangle_edges(data.cells)
    axes.add_collection(
        LineCollection(
            coordinates[list(mesh_edges)],
            colors="#475569",
            linewidths=0.45 if compact else 0.55,
            alpha=0.42,
            zorder=3,
        )
    )
    solid_edges = _triangle_edges(solid_triangles)
    axes.add_collection(
        LineCollection(
            coordinates[list(solid_edges)],
            colors="#475569",
            linewidths=0.9,
            linestyles="dashed",
            alpha=0.82,
            label="Solid reference geometry",
            zorder=4,
        )
    )
    if state_opacity > 0.0:
        deformed = coordinates + DISPLACEMENT_SCALE * displacement
        axes.add_collection(
            LineCollection(
                deformed[list(solid_edges)],
                colors="#c2410c",
                linewidths=1.8,
                alpha=0.98 * state_opacity,
                label="Solid displacement ×12 [m] (presentation)",
                zorder=5,
            )
        )
    axes.add_collection(
        LineCollection(
            coordinates[data.interface_facets],
            colors="#0e7490",
            linewidths=3.0 if not compact else 2.3,
            alpha=0.98,
            label="Conforming interface",
            zorder=6,
        )
    )
    axes.set_xlim(*profile.x_limits)
    axes.set_ylim(*profile.y_limits)
    axes.set_aspect("equal", adjustable="box")
    axes.set_xlabel("x [m]")
    axes.set_ylabel("y [m]")
    if not compact:
        axes.text(
            0.025,
            0.94,
            "fluid region",
            transform=axes.transAxes,
            fontsize=9,
            color="#1d4ed8",
            va="top",
        )
        axes.text(
            0.79,
            0.94,
            "solid region",
            transform=axes.transAxes,
            fontsize=9,
            color="#9a3412",
            va="top",
        )
        axes.legend(loc="lower center", ncols=3, fontsize=8.5, framealpha=0.94)


def _decorate_figure(figure: Any, data: SceneData) -> None:
    figure.text(
        0.025,
        0.965,
        record.WATERMARK,
        ha="left",
        va="top",
        fontsize=11,
        weight="bold",
        color="#b91c1c",
        bbox={
            "boxstyle": "square,pad=0.28",
            "facecolor": "#fff1f2",
            "edgecolor": "#fda4af",
        },
    )
    figure.text(
        0.5,
        0.035,
        f"Evidence: {data.case_ids[0]} • {data.case_ids[1]} • "
        f"Run {data.run_digest[:12]}…",
        ha="center",
        va="bottom",
        fontsize=8.2,
        color="#334155",
    )


def _frame_title(data: SceneData, plan: FramePlan) -> str:
    if plan.segment in {"neutral-establish", "neutral-reset", "poster-return"}:
        return "Fixed-reference partition • presentation reset • no physical-time advance"
    if plan.segment == "state-1-hold":
        step = data.steps[0]
        return (
            f"Accepted step {step.ordinal} • t = {step.time_s:g} s • "
            "solid displacement ×12 (presentation)"
        )
    if plan.segment == "state-2-hold":
        step = data.steps[1]
        return (
            f"Accepted step {step.ordinal} • t = {step.time_s:g} s • "
            "solid displacement ×12 (presentation)"
        )
    return "Accepted states t1 → t2 • solid displacement ×12 (presentation)"


def _padded_equal_aspect_limits(
    coordinates: np.ndarray,
) -> tuple[tuple[float, float], tuple[float, float]]:
    x_min, y_min = np.min(coordinates, axis=0)
    x_max, y_max = np.max(coordinates, axis=0)
    x_span = float(x_max - x_min)
    y_span = float(y_max - y_min)
    if not x_span > 0.0 or not y_span > 0.0:
        raise ValueError("gallery scene cannot frame a degenerate coordinate box")
    x_low = float(x_min - 0.05 * x_span)
    x_high = float(x_max + 0.05 * x_span)
    y_low = float(y_min - 0.05 * y_span)
    y_high = float(y_max + 0.05 * y_span)
    viewport_aspect = (WIDTH * FIELD_RECT[2]) / (HEIGHT * FIELD_RECT[3])
    data_aspect = (x_high - x_low) / (y_high - y_low)
    if data_aspect < viewport_aspect:
        target_span = (y_high - y_low) * viewport_aspect
        center = 0.5 * (x_low + x_high)
        x_low, x_high = center - 0.5 * target_span, center + 0.5 * target_span
    elif data_aspect > viewport_aspect:
        target_span = (x_high - x_low) / viewport_aspect
        center = 0.5 * (y_low + y_high)
        y_low, y_high = center - 0.5 * target_span, center + 0.5 * target_span
    return (x_low, x_high), (y_low, y_high)


def _triangle_edges(cells: np.ndarray) -> tuple[tuple[int, int], ...]:
    return tuple(
        sorted(
            {
                tuple(sorted((int(cell[first]), int(cell[second]))))
                for cell in cells
                for first, second in ((0, 1), (1, 2), (2, 0))
            }
        )
    )


def _matplotlib():
    from matplotlib.backends.backend_agg import FigureCanvasAgg
    from matplotlib.cm import ScalarMappable
    from matplotlib.collections import LineCollection, PolyCollection
    from matplotlib.colors import Normalize
    from matplotlib.figure import Figure
    from matplotlib.tri import Triangulation

    return (
        Figure,
        FigureCanvasAgg,
        LineCollection,
        PolyCollection,
        Triangulation,
        np,
        Normalize,
        ScalarMappable,
    )
