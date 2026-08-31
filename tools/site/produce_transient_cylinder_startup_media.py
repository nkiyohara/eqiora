#!/usr/bin/env python3
"""Render caller-owned media from the unverified transient cylinder startup."""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import numpy as np
from matplotlib.figure import Figure


ROOT = Path(__file__).resolve().parents[2]
EXAMPLES = ROOT / "examples" / "python"
FRAME_RATE = 2
FIGURE_SIZE = (12.0, 3.5)
DPI = 144


def _solve():
    sys.path.insert(0, str(EXAMPLES))
    try:
        from transient_cylinder_wake import solve

        return solve()
    finally:
        sys.path.pop(0)


def _render_frame(
    coordinates: np.ndarray,
    triangles: np.ndarray,
    values: np.ndarray,
    reference_values: np.ndarray,
    *,
    magnitude: float,
    delta_magnitude: float,
    step: int,
    time_s: float,
) -> Figure:
    figure = Figure(figsize=FIGURE_SIZE, dpi=DPI, facecolor="#ffffff")
    panels = (
        (
            f"Vorticity ω — step {step}, t = {time_s:g} s",
            values,
            magnitude,
            "ω [s⁻¹]",
        ),
        (
            "Change from first output Δω",
            values - reference_values,
            delta_magnitude,
            "Δω [s⁻¹]",
        ),
    )
    for index, (title, panel_values, scale, label) in enumerate(panels):
        axes = figure.add_axes((0.07 + index * 0.47, 0.2, 0.32, 0.64))
        axes.set_facecolor("#f8fafc")
        scalar = axes.tripcolor(
            coordinates[:, 0],
            coordinates[:, 1],
            triangles=triangles,
            facecolors=panel_values,
            shading="flat",
            cmap="coolwarm",
            vmin=-scale,
            vmax=scale,
        )
        axes.triplot(
            coordinates[:, 0],
            coordinates[:, 1],
            triangles,
            color="#0f172a",
            linewidth=0.25,
            alpha=0.18,
        )
        axes.set_xlim(float(coordinates[:, 0].min()), float(coordinates[:, 0].max()))
        axes.set_ylim(float(coordinates[:, 1].min()), float(coordinates[:, 1].max()))
        axes.set_aspect("equal", adjustable="box")
        axes.set_xlabel("x [m]")
        if index == 0:
            axes.set_ylabel("y [m]")
        axes.set_title(title)
        colorbar_axes = figure.add_axes((0.4 + index * 0.47, 0.2, 0.014, 0.64))
        colorbar = figure.colorbar(scalar, cax=colorbar_axes)
        colorbar.set_label(label)
    return figure


def _render_comparison(
    coordinates: np.ndarray,
    triangles: np.ndarray,
    frames: list[tuple[int, float, np.ndarray]],
    *,
    magnitude: float,
) -> Figure:
    figure = Figure(figsize=(12.0, 3.5), dpi=DPI, facecolor="#ffffff")
    scalar = None
    comparison = (
        (
            "First output ω",
            frames[0][0],
            frames[0][1],
            frames[0][2],
            magnitude,
            "ω [s⁻¹]",
        ),
        (
            "Final change Δω",
            frames[-1][0],
            frames[-1][1],
            frames[-1][2] - frames[0][2],
            max(float(np.abs(frames[-1][2] - frames[0][2]).max()), np.finfo(float).eps),
            "Δω [s⁻¹]",
        ),
    )
    for index, (title, step, time_s, values, scale, label) in enumerate(comparison):
        axes = figure.add_axes((0.07 + index * 0.44, 0.2, 0.32, 0.65))
        axes.set_facecolor("#f8fafc")
        scalar = axes.tripcolor(
            coordinates[:, 0],
            coordinates[:, 1],
            triangles=triangles,
            facecolors=values,
            shading="flat",
            cmap="coolwarm",
            vmin=-scale,
            vmax=scale,
        )
        axes.triplot(
            coordinates[:, 0],
            coordinates[:, 1],
            triangles,
            color="#0f172a",
            linewidth=0.25,
            alpha=0.2,
        )
        axes.set_xlim(float(coordinates[:, 0].min()), float(coordinates[:, 0].max()))
        axes.set_ylim(float(coordinates[:, 1].min()), float(coordinates[:, 1].max()))
        axes.set_aspect("equal", adjustable="box")
        axes.set_xlabel("x [m]")
        if index == 0:
            axes.set_ylabel("y [m]")
        axes.set_title(f"{title} · step {step} · t = {time_s:g} s")
        colorbar_axes = figure.add_axes((0.4 + index * 0.44, 0.2, 0.014, 0.65))
        colorbar = figure.colorbar(scalar, cax=colorbar_axes)
        colorbar.set_label(label)
    figure.suptitle("Unverified cylinder-flow startup: first output and final change")
    return figure


def _save_png(figure: Figure, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    figure.savefig(
        path,
        format="png",
        dpi=DPI,
        metadata={"Software": "Eqiora transient cylinder startup presentation v1"},
    )


def _encode(ffmpeg: str, frames: Path, output: Path, *, codec: str) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    command = [
        ffmpeg,
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-framerate",
        str(FRAME_RATE),
        "-i",
        str(frames / "frame-%02d.png"),
        "-an",
        "-c:v",
        codec,
        "-pix_fmt",
        "yuv420p",
    ]
    if codec == "libx264":
        command.extend(("-crf", "24", "-movflags", "+faststart"))
    else:
        command.extend(("-crf", "32", "-b:v", "0"))
    command.append(str(output))
    subprocess.run(command, check=True, timeout=120)


def produce(poster: Path, reduced_motion: Path, webm: Path, mp4: Path) -> None:
    plan, result, _, _, _, _ = _solve()
    trajectory = result.trajectory
    coordinates = np.asarray(trajectory.coordinates)
    triangles = np.asarray(trajectory.cells)
    frames = []
    for state in trajectory.states:
        vorticity = state.curl(plan.capability.velocity)
        frames.append((state.step, state.time_s, np.asarray(vorticity.values("cell"))))
    if len(frames) != 10:
        raise RuntimeError(
            "the startup media requires exactly ten accepted output states"
        )
    magnitude = max(float(np.abs(values).max()) for _, _, values in frames)
    if not np.isfinite(magnitude) or magnitude <= 0.0:
        raise RuntimeError("the startup vorticity scale must be finite and nonzero")
    delta_magnitude = max(
        float(np.abs(values - frames[0][2]).max()) for _, _, values in frames
    )
    if not np.isfinite(delta_magnitude) or delta_magnitude <= 0.0:
        raise RuntimeError("the startup vorticity change must be finite and nonzero")

    _save_png(
        _render_frame(
            coordinates,
            triangles,
            frames[-1][2],
            frames[0][2],
            magnitude=magnitude,
            delta_magnitude=delta_magnitude,
            step=frames[-1][0],
            time_s=frames[-1][1],
        ),
        poster,
    )
    _save_png(
        _render_comparison(coordinates, triangles, frames, magnitude=magnitude),
        reduced_motion,
    )

    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg is None:
        raise RuntimeError("ffmpeg is required to encode startup video")
    with tempfile.TemporaryDirectory(
        prefix="eqiora-cylinder-startup-", dir=Path.home()
    ) as raw:
        frame_directory = Path(raw)
        for index, (step, time_s, values) in enumerate(frames):
            _save_png(
                _render_frame(
                    coordinates,
                    triangles,
                    values,
                    frames[0][2],
                    magnitude=magnitude,
                    delta_magnitude=delta_magnitude,
                    step=step,
                    time_s=time_s,
                ),
                frame_directory / f"frame-{index:02d}.png",
            )
        _encode(ffmpeg, frame_directory, webm, codec="libvpx-vp9")
        _encode(ffmpeg, frame_directory, mp4, codec="libx264")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--poster", type=Path, required=True)
    parser.add_argument("--reduced-motion", type=Path, required=True)
    parser.add_argument("--webm", type=Path, required=True)
    parser.add_argument("--mp4", type=Path, required=True)
    arguments = parser.parse_args()
    produce(arguments.poster, arguments.reduced_motion, arguments.webm, arguments.mp4)


if __name__ == "__main__":
    main()
