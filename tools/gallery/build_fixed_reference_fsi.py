#!/usr/bin/env python3
"""Build one private media bundle from the installed fixed-reference FSI result."""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import struct
import subprocess
import sys
from importlib.metadata import distribution
from importlib.resources import files
from pathlib import Path
from typing import Any, Mapping

import numpy as np

import record
import scene


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).resolve()
RECORD_MODULE = Path(record.__file__).resolve()
SCENE_MODULE = Path(scene.__file__).resolve()
PROFILE_ENVIRONMENT = {
    "SOURCE_DATE_EPOCH": "0",
    "TZ": "UTC",
    "LC_ALL": "C",
    "PYTHONHASHSEED": "0",
    "MPLBACKEND": "Agg",
}


class BuildError(RuntimeError):
    """The private media bundle could not satisfy its frozen contract."""


def main(argv: list[str] | None = None) -> int:
    """Build, self-verify, and identify one development-only media bundle."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument(
        "--verify-determinism",
        action="store_true",
        help="repeat selected renders and both encodes inside the same profile",
    )
    parser.add_argument(
        "--keep-frames",
        action="store_true",
        help="retain the lossless frame sequence below the output directory",
    )
    arguments = parser.parse_args(argv)
    try:
        record_digest = build(
            arguments.output_dir,
            verify_determinism=arguments.verify_determinism,
            keep_frames=arguments.keep_frames,
        )
    except (BuildError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"private gallery build failed: {error}", file=sys.stderr)
        return 2
    print(record_digest)
    return 0


def build(
    output_directory: Path,
    *,
    verify_determinism: bool,
    keep_frames: bool,
) -> str:
    """Execute the complete installed-result to admitted-development-record path."""

    _require_process_profile()
    output_directory = output_directory.resolve()
    _prepare_output_directory(output_directory)
    eqiora = _installed_eqiora()
    matplotlib = _matplotlib()
    model_source = (
        files(eqiora).joinpath("examples", "fixed-reference-fsi.eqi").read_bytes()
    )
    model = eqiora.compile(
        model_source.decode("utf-8"), filename="fixed-reference-fsi.eqi"
    )
    result = eqiora.fsi.solve_fixed_reference_fsi(model)
    data = _scene_data(result)
    profile = scene.make_profile(data)
    encoder = _encoder_identity()

    poster_path = output_directory / record.OUTPUT_CONTRACTS["poster"][0]
    reduced_path = output_directory / record.OUTPUT_CONTRACTS["reduced_motion_still"][0]
    text_path = output_directory / record.OUTPUT_CONTRACTS["text_alternative"][0]
    scene.render_poster(data, profile, poster_path)
    scene.render_reduced_motion_still(data, profile, reduced_path)
    text = scene.text_alternative(data)
    text_path.write_text(text, encoding="utf-8", newline="\n")

    frames_directory = output_directory / "frames"
    frames_directory.mkdir()
    with _FramesDirectory(frames_directory, keep=keep_frames) as raw_frames:
        frames_directory = Path(raw_frames)
        frame_digests = _render_frames(
            data,
            profile,
            poster_path,
            frames_directory,
            verify_determinism=verify_determinism,
        )
        frame_sequence_sha256 = scene.frame_sequence_digest(frame_digests)
        webm_path = output_directory / record.OUTPUT_CONTRACTS["film_modern"][0]
        mp4_path = output_directory / record.OUTPUT_CONTRACTS["film_fallback"][0]
        webm_argv = _webm_argv(encoder["ffmpeg_path"], frames_directory, webm_path)
        mp4_argv = _mp4_argv(encoder["ffmpeg_path"], frames_directory, mp4_path)
        _run_encoder(webm_argv, output_directory)
        _run_encoder(mp4_argv, output_directory)
        webm_probe = _probe(encoder["ffprobe_path"], webm_path, "vp9")
        mp4_probe = _probe(encoder["ffprobe_path"], mp4_path, "h264")
        if verify_determinism:
            _verify_encode_determinism(
                output_directory,
                frames_directory,
                encoder["ffmpeg_path"],
                webm_path,
                mp4_path,
            )

    encoder["webm_argv"] = _recorded_argv(webm_argv, output_directory)
    encoder["mp4_argv"] = _recorded_argv(mp4_argv, output_directory)
    encoder["profile_sha256"] = record.content_digest(encoder, "profile_sha256")
    scene_value = scene.scene_record(data, profile, frame_sequence_sha256)
    lineage = _lineage(result)
    environment = _environment(eqiora, matplotlib)
    renderer = {
        "identity": "eqiora.gallery.private-fsi-renderer/1",
        "matplotlib_version": matplotlib.__version__,
        "backend": "Agg",
        "module_sha256": record.sha256_file(SCENE_MODULE),
        "png_metadata": dict(scene.PNG_METADATA),
    }
    source = {
        "build_revision": _build_revision(),
        "build_script_path": SCRIPT.relative_to(ROOT).as_posix(),
        "build_script_sha256": record.sha256_file(SCRIPT),
        "record_module_path": RECORD_MODULE.relative_to(ROOT).as_posix(),
        "record_module_sha256": record.sha256_file(RECORD_MODULE),
        "scene_module_path": SCENE_MODULE.relative_to(ROOT).as_posix(),
        "scene_module_sha256": record.sha256_file(SCENE_MODULE),
        "model_source_sha256": record.sha256_bytes(model_source),
        "result_digest": result.run_digest,
        "result_frame_input_sha256": _frame_input_digest(data),
        "eqiora_version": eqiora.__version__,
        "eqiora_module_is_installed": True,
    }
    outputs, facts = _outputs(
        output_directory,
        webm_probe=webm_probe,
        mp4_probe=mp4_probe,
    )
    accessibility = {
        "reduced_motion_still": record.OUTPUT_CONTRACTS["reduced_motion_still"][0],
        "text_alternative": record.OUTPUT_CONTRACTS["text_alternative"][0],
        "delivery_requirement": record.DELIVERY_REQUIREMENT,
        "dossier_route": record.DOSSIER_ROUTE,
        "description_sha256": record.sha256_bytes(text.encode("utf-8")),
    }
    value = record.development_record(
        lineage=lineage,
        source=source,
        scene=scene_value,
        renderer=renderer,
        encoder=encoder,
        environment=environment,
        outputs=outputs,
        accessibility=accessibility,
    )
    context = record.ProductionContext(
        protected_base_revision=source["build_revision"],
        registered_case_ids=frozenset(data.case_ids),
        lineage_by_run_digest={result.run_digest: record.lineage_digest(lineage)},
        contracts={},
    )
    payload = record.canonical_bytes(value)
    persisted = json.loads(payload)
    if record.canonical_bytes(persisted) != payload:
        raise BuildError("canonical build record did not survive its JSON round trip")
    admission = record.admit(
        persisted,
        context=context,
        output_facts=facts,
        text_alternative=text,
    )
    if admission.reasons != ("publication-status", "experience-id"):
        raise BuildError(
            "development record did not retain exactly its permanent publication "
            f"rejections: {admission.reasons}"
        )
    record_path = output_directory / "dev-build-record.json"
    record_path.write_bytes(payload)
    digest = record.sha256_bytes(payload)
    (output_directory / "dev-build-record.sha256").write_text(
        f"{digest}  {record_path.name}\n", encoding="ascii", newline="\n"
    )
    return digest


def _require_process_profile() -> None:
    wrong = {
        key: os.environ.get(key)
        for key, expected in PROFILE_ENVIRONMENT.items()
        if os.environ.get(key) != expected
    }
    if wrong:
        required = " ".join(
            f"{key}={value}" for key, value in PROFILE_ENVIRONMENT.items()
        )
        raise BuildError(f"producer environment is not frozen; invoke with {required}")


def _prepare_output_directory(destination: Path) -> None:
    if destination.exists():
        if not destination.is_dir() or any(destination.iterdir()):
            raise BuildError("output directory must be absent or empty")
    else:
        destination.mkdir(parents=True)


def _installed_eqiora():
    import eqiora

    origin = Path(eqiora.__file__).resolve()
    install_root = Path(distribution("eqiora").locate_file("")).resolve()
    if origin.is_relative_to(ROOT) or not origin.is_relative_to(install_root):
        raise BuildError(
            f"eqiora must be imported from an installed wheel, got {origin}"
        )
    return eqiora


def _matplotlib():
    import matplotlib

    if matplotlib.get_backend().lower() != "agg":
        raise BuildError("gallery media requires the headless Matplotlib Agg backend")
    return matplotlib


def _scene_data(result: Any) -> scene.SceneData:
    steps = tuple(
        scene.AcceptedStep(
            ordinal=int(step.ordinal),
            time_s=float(step.time_s),
            pressure_vertices=np.asarray(step.pressure_vertices).copy(),
            pressure=np.asarray(step.pressure).copy(),
            displacement=np.asarray(step.displacement).copy(),
        )
        for step in result.steps
    )
    if len(steps) != 2:
        raise BuildError("gallery media requires exactly two accepted result steps")
    return scene.SceneData(
        coordinates=np.asarray(result.coordinates).copy(),
        cells=np.asarray(result.cells).copy(),
        fluid_cells=np.asarray(result.fluid_cells).copy(),
        solid_cells=np.asarray(result.solid_cells).copy(),
        interface_facets=np.asarray(result.interface_facets).copy(),
        steps=(steps[0], steps[1]),
        case_ids=tuple(result.case_ids),
        run_digest=result.run_digest,
    )


def _render_frames(
    data: scene.SceneData,
    profile: scene.SceneProfile,
    poster: Path,
    frames: Path,
    *,
    verify_determinism: bool,
) -> list[str]:
    import matplotlib.image as image

    poster_pixels = image.imread(poster)
    digests: list[str] = []
    repeat_frames = {45, 90, 150, 300, 375, 420, 448}
    for frame in range(scene.FRAME_COUNT):
        destination = frames / f"frame-{frame:04d}.png"
        if frame <= 44 or frame == 449:
            scene.copy_poster_frame(poster, destination)
        else:
            scene.render_frame(
                data,
                profile,
                frame,
                poster_pixels=poster_pixels,
                destination=destination,
            )
        digest = record.sha256_file(destination)
        digests.append(digest)
        if verify_determinism and frame in repeat_frames:
            repeated = frames / f"repeat-{frame:04d}.png"
            scene.render_frame(
                data,
                profile,
                frame,
                poster_pixels=poster_pixels,
                destination=repeated,
            )
            if repeated.read_bytes() != destination.read_bytes():
                raise BuildError(f"frame {frame} did not reproduce in one profile")
            repeated.unlink()
    if (frames / "frame-0000.png").read_bytes() != (
        frames / "frame-0449.png"
    ).read_bytes():
        raise BuildError("film loop endpoints are not exact poster bytes")
    return digests


def _encoder_identity() -> dict[str, object]:
    ffmpeg_name = shutil.which("ffmpeg")
    ffprobe_name = shutil.which("ffprobe")
    if ffmpeg_name is None or ffprobe_name is None:
        raise BuildError("gallery media requires ffmpeg and ffprobe on PATH")
    ffmpeg = Path(ffmpeg_name).resolve()
    ffprobe = Path(ffprobe_name).resolve()
    version = subprocess.run(
        [str(ffmpeg), "-version"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=_subprocess_environment(),
    ).stdout.splitlines()
    first = next((line for line in version if line.startswith("ffmpeg version ")), "")
    configuration = next(
        (
            line.removeprefix("configuration: ")
            for line in version
            if line.startswith("configuration: ")
        ),
        "",
    )
    encoders = subprocess.run(
        [str(ffmpeg), "-hide_banner", "-encoders"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=_subprocess_environment(),
    ).stdout
    missing = [name for name in ("libvpx-vp9", "libx264") if name not in encoders]
    if missing:
        raise BuildError(f"ffmpeg omits required encoder(s): {', '.join(missing)}")
    return {
        "ffmpeg_path": str(ffmpeg),
        "ffmpeg_sha256": record.sha256_file(ffmpeg),
        "ffmpeg_version": first,
        "ffmpeg_configuration": configuration,
        "ffprobe_path": str(ffprobe),
        "ffprobe_sha256": record.sha256_file(ffprobe),
        "webm_argv": [],
        "mp4_argv": [],
        "profile_sha256": "",
    }


def _common_encoder_argv(ffmpeg: object, frames: Path, destination: Path) -> list[str]:
    return [
        str(ffmpeg),
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "image2",
        "-framerate",
        str(scene.FPS),
        "-start_number",
        "0",
        "-i",
        str(frames / "frame-%04d.png"),
        "-an",
        "-threads",
        "1",
        "-filter_threads",
        "1",
        "-filter_complex_threads",
        "1",
        "-fps_mode",
        "cfr",
        "-r",
        str(scene.FPS),
        "-pix_fmt",
        "yuv420p",
        "-map_metadata",
        "-1",
        "-map_chapters",
        "-1",
        "-bitexact",
        "-fflags",
        "+bitexact",
        "-flags:v",
        "+bitexact",
        str(destination),
    ]


def _webm_argv(ffmpeg: object, frames: Path, destination: Path) -> list[str]:
    argv = _common_encoder_argv(ffmpeg, frames, destination)
    argv[-1:-1] = [
        "-c:v",
        "libvpx-vp9",
        "-b:v",
        "0",
        "-crf",
        "30",
        "-deadline",
        "good",
        "-cpu-used",
        "2",
        "-row-mt",
        "0",
        "-tile-columns",
        "0",
        "-frame-parallel",
        "0",
        "-auto-alt-ref",
        "0",
        "-lag-in-frames",
        "0",
        "-g",
        str(scene.FPS),
    ]
    return argv


def _mp4_argv(ffmpeg: object, frames: Path, destination: Path) -> list[str]:
    argv = _common_encoder_argv(ffmpeg, frames, destination)
    argv[-1:-1] = [
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "20",
        "-profile:v",
        "high",
        "-level:v",
        "4.0",
        "-x264-params",
        "threads=1:sliced-threads=0:deterministic=1",
        "-movflags",
        "+faststart",
        "-g",
        str(scene.FPS),
    ]
    return argv


def _run_encoder(argv: list[str], cwd: Path) -> None:
    subprocess.run(argv, cwd=cwd, env=_subprocess_environment(), check=True)


def _probe(ffprobe: object, path: Path, expected_codec: str) -> dict[str, object]:
    output = subprocess.run(
        [
            str(ffprobe),
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,nb_read_frames,r_frame_rate,pix_fmt:format=duration,nb_streams",
            "-of",
            "json",
            str(path),
        ],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        env=_subprocess_environment(),
    ).stdout
    decoded = json.loads(output)
    streams = decoded.get("streams", [])
    format_data = decoded.get("format", {})
    if len(streams) != 1:
        raise BuildError(f"{path.name} does not contain exactly one video stream")
    stream = streams[0]
    facts = {
        "codec": stream.get("codec_name"),
        "width": int(stream.get("width", -1)),
        "height": int(stream.get("height", -1)),
        "frame_count": int(stream.get("nb_read_frames", -1)),
        "fps": stream.get("r_frame_rate"),
        "pix_fmt": stream.get("pix_fmt"),
        "duration_s": float(format_data.get("duration", "nan")),
        "stream_count": int(format_data.get("nb_streams", -1)),
    }
    if (
        facts["codec"] != expected_codec
        or facts["width"] != scene.WIDTH
        or facts["height"] != scene.HEIGHT
        or facts["frame_count"] != scene.FRAME_COUNT
        or facts["fps"] != f"{scene.FPS}/1"
        or facts["pix_fmt"] != "yuv420p"
        or abs(float(facts["duration_s"]) - scene.DURATION_S) > 0.05
        or facts["stream_count"] != 1
    ):
        raise BuildError(f"{path.name} probe facts drifted: {facts}")
    return facts


def _verify_encode_determinism(
    output_directory: Path,
    frames: Path,
    ffmpeg: object,
    webm: Path,
    mp4: Path,
) -> None:
    repeats = [
        (webm, output_directory / ".repeat.webm", _webm_argv),
        (mp4, output_directory / ".repeat.mp4", _mp4_argv),
    ]
    for accepted, repeated, builder in repeats:
        _run_encoder(builder(ffmpeg, frames, repeated), output_directory)
        if accepted.read_bytes() != repeated.read_bytes():
            raise BuildError(
                f"{accepted.name} did not reproduce in one recorded encoder profile"
            )
        repeated.unlink()


def _recorded_argv(argv: list[str], output_directory: Path) -> list[str]:
    return [
        Path(value).relative_to(output_directory).as_posix()
        if Path(value).is_absolute() and Path(value).is_relative_to(output_directory)
        else value
        for value in argv
    ]


def _lineage(result: Any) -> dict[str, object]:
    return {
        "model_digest": result.model_digest,
        "semantic_revision": int(result.semantic_revision),
        "geometry_digest": result.geometry_digest,
        "correspondence_digest": result.correspondence_digest,
        "mesh_digest": result.mesh_digest,
        "realization_digest": result.realization_digest,
        "realization_revision": int(result.realization_revision),
        "run_digest": result.run_digest,
        "run_manifest_sha256": record.sha256_bytes(bytes(result.run_manifest_json)),
        "state_digests": list(result.state_digests),
        "trajectory_digest": result.trajectory_digest,
        "scientific_case_ids": list(result.case_ids),
    }


def _frame_input_digest(data: scene.SceneData) -> str:
    return record.sha256_bytes(
        record.canonical_bytes(
            {
                "coordinates": data.coordinates.tolist(),
                "cells": data.cells.tolist(),
                "fluid_cells": data.fluid_cells.tolist(),
                "solid_cells": data.solid_cells.tolist(),
                "interface_facets": data.interface_facets.tolist(),
                "steps": [
                    {
                        "ordinal": step.ordinal,
                        "time_s": step.time_s,
                        "pressure_vertices": step.pressure_vertices.tolist(),
                        "pressure": step.pressure.tolist(),
                        "displacement": step.displacement.tolist(),
                    }
                    for step in data.steps
                ],
            }
        )
    )


def _environment(eqiora: Any, matplotlib: Any) -> dict[str, object]:
    value: dict[str, object] = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python_version": platform.python_version(),
        "eqiora_version": eqiora.__version__,
        "numpy_version": np.__version__,
        "matplotlib_version": matplotlib.__version__,
        "source_date_epoch": 0,
        "tz": "UTC",
        "locale": "C",
        "pythonhashseed": "0",
        "profile_sha256": "",
    }
    value["profile_sha256"] = record.content_digest(value, "profile_sha256")
    return value


def _outputs(
    directory: Path,
    *,
    webm_probe: Mapping[str, object],
    mp4_probe: Mapping[str, object],
) -> tuple[dict[str, object], dict[str, record.FileFact]]:
    probes = {"film_modern": webm_probe, "film_fallback": mp4_probe}
    values: dict[str, object] = {}
    facts: dict[str, record.FileFact] = {}
    for role, (filename, media_type) in record.OUTPUT_CONTRACTS.items():
        path = directory / filename
        fact = record.FileFact(record.sha256_file(path), path.stat().st_size)
        facts[role] = fact
        probe = probes.get(role)
        if role in {"poster", "reduced_motion_still"}:
            width, height = _png_dimensions(path)
            duration_s = fps = frame_count = stream_count = codec = None
        elif probe is not None:
            width, height = probe["width"], probe["height"]
            duration_s, fps, frame_count = (
                scene.DURATION_S,
                scene.FPS,
                scene.FRAME_COUNT,
            )
            stream_count, codec = probe["stream_count"], probe["codec"]
        else:
            width = height = duration_s = fps = frame_count = stream_count = codec = (
                None
            )
        values[role] = {
            "filename": filename,
            "media_type": media_type,
            "sha256": fact.sha256,
            "bytes": fact.bytes,
            "width": width,
            "height": height,
            "duration_s": duration_s,
            "fps": fps,
            "frame_count": frame_count,
            "has_audio": False,
            "stream_count": stream_count,
            "codec": codec,
        }
    return values, facts


def _png_dimensions(path: Path) -> tuple[int, int]:
    """Read the emitted PNG IHDR dimensions without trusting scene constants."""

    header = path.read_bytes()[:24]
    if (
        len(header) != 24
        or header[:8] != b"\x89PNG\r\n\x1a\n"
        or header[12:16] != b"IHDR"
    ):
        raise BuildError(f"{path.name} is not a canonical PNG output")
    return struct.unpack(">II", header[16:24])


def _build_revision() -> str:
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        env=_subprocess_environment(),
    ).stdout.strip()
    if len(revision) != 40 or any(
        character not in "0123456789abcdef" for character in revision
    ):
        raise BuildError("gallery build revision is not a full lowercase Git identity")
    return revision


def _subprocess_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.pop("PYTHONPATH", None)
    environment.update(PROFILE_ENVIRONMENT)
    return environment


class _FramesDirectory:
    def __init__(self, path: Path, *, keep: bool) -> None:
        self.path = path
        self.keep = keep

    def __enter__(self) -> str:
        return str(self.path)

    def __exit__(self, *unused: object) -> None:
        if not self.keep:
            shutil.rmtree(self.path)
        return None


if __name__ == "__main__":
    raise SystemExit(main())
