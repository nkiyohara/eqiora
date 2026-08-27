#!/usr/bin/env python3
"""Produce one sealed exact-cylinder pressure publication candidate."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import importlib.util
import json
import locale
import math
import os
import platform
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any


OBSERVATION_SCHEMA = "eqiora.site.exact-cylinder-pressure-producer-observation/v1"
PNG_SOFTWARE = "Eqiora exact-cylinder gallery publication v1"
OUTPUT_NAMES = (
    "exact-cylinder-pressure.png",
    "producer-log.txt",
    "producer-observation.json",
)
SOURCE_ROLES = {
    "bindings/python/python/eqiora/matplotlib.py": ["plotting-adapter"],
    "examples/python/exact_cylinder_geometry.py": ["geometry-adapter"],
    "examples/python/exact_cylinder_mesh.py": ["mesh-realization-owner"],
    "examples/python/exact_cylinder_stokes.py": ["plain-python-snippet"],
    "examples/python/exact_cylinder_stokes_marimo.py": ["canonical-marimo-snippet"],
    "examples/steady-flow-past-cylinder.eqi": ["example-formula-owner"],
    "examples/steady-flow-past-cylinder.geometry.json": ["geometry"],
    "examples/steady-flow-past-cylinder.model.json": ["model", "scientific-formula"],
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi": ["current-package-formula-owner"],
    "tools/site/produce_exact_cylinder_pressure.py": ["producer-command"],
    "verify/fluid/packaged-steady-stokes-2d/models/direct.eqi": ["packaged-stokes-formula"],
    "verify/fluid/packaged-steady-stokes-2d/package-v0.1.0/src/incompressible.eqi": ["package-formula-owner"],
}
LINEAGE_METHODS = {
    "correspondence_digest": "Result.mesh(FieldRef).correspondence_digest",
    "evidence_run_digest": "fluid.steady_stokes_evidence(Result).run_digest",
    "geometry_digest": "Result.mesh(FieldRef).source_digest",
    "mesh_digest": "Result.mesh(FieldRef).digest",
    "model_digest": "Result.model_digest",
    "pressure_blocks": "Result.field(FieldRef).block_digests",
    "pressure_output": "Result.run_manifest().output_digests",
    "pressure_snapshot": "Result.field(FieldRef).digest",
    "realization_digest": "Result.run_manifest().realization_digest",
    "run_manifest_digest": "Result.run_manifest().digest",
}
EXPECTED_DIMENSION = (1, -1, -2, 0, 0, 0, 0)
EXPECTED_FIGURE_INCHES = (8.0, 5.2)
EXPECTED_PIXEL_SIZE = (1280, 832)
EXPECTED_DPI = 160
MAX_JSON_BYTES = 512 * 1024
MAX_PNG_BYTES = 16 * 1024 * 1024
MAX_WHEELS = 48


class ProducerError(RuntimeError):
    """The requested publication candidate cannot be produced safely."""


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256_file(path: Path, *, limit: int | None = None) -> str:
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        raise ProducerError(f"required input is not a regular file: {path}")
    if limit is not None and info.st_size > limit:
        raise ProducerError(f"required input exceeds its byte bound: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _canonical_json(value: Any) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=False,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            ).encode("utf-8")
            + b"\n"
        )
    except (TypeError, ValueError) as error:
        raise ProducerError(f"observation is not canonical JSON: {error}") from error


def _run_git(root: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if completed.returncode != 0 or completed.stderr:
        detail = completed.stderr.strip() or f"status {completed.returncode}"
        raise ProducerError(f"Git identity query failed: {detail}")
    return completed.stdout.strip()


def _require_clean_source(root: Path, expected_revision: str) -> tuple[str, str]:
    root = root.resolve(strict=True)
    revision = _run_git(root, "rev-parse", "HEAD")
    if revision != expected_revision or len(revision) != 40:
        raise ProducerError("repository HEAD differs from the requested full source revision")
    tree = _run_git(root, "rev-parse", "HEAD^{tree}")
    if len(tree) != 40:
        raise ProducerError("Git did not return a full source tree identity")
    if _run_git(root, "status", "--porcelain=v1", "--untracked-files=all"):
        raise ProducerError("publication production requires a clean exact source tree")
    return revision, tree


def _require_environment() -> None:
    if sys.version_info[:3] != (3, 13, 14) or sys.implementation.name != "cpython":
        raise ProducerError("producer requires CPython 3.13.14")
    if os.environ.get("MPLBACKEND") != "Agg":
        raise ProducerError("MPLBACKEND must be exactly Agg")
    if os.environ.get("TZ") != "UTC" or time.tzname != ("UTC", "UTC"):
        raise ProducerError("producer timezone must be exactly UTC")
    requested_locale = os.environ.get("LC_ALL")
    if requested_locale not in {"C.UTF-8", "C.utf8"}:
        raise ProducerError("LC_ALL must be C.UTF-8")
    observed_locale = locale.setlocale(locale.LC_ALL, requested_locale)
    if observed_locale not in {"C.UTF-8", "C.utf8"}:
        raise ProducerError("producer locale must be C.UTF-8")
    if os.environ.get("PYTHONPATH"):
        raise ProducerError("PYTHONPATH must be absent")
    if os.environ.get("PYTHONNOUSERSITE") != "1":
        raise ProducerError("PYTHONNOUSERSITE must be 1")
    if os.environ.get("PYTHONDONTWRITEBYTECODE") != "1":
        raise ProducerError("PYTHONDONTWRITEBYTECODE must be 1")
    if not sys.flags.dont_write_bytecode:
        raise ProducerError("producer interpreter must disable bytecode writes with -B")
    if os.environ.get("DISPLAY"):
        raise ProducerError("DISPLAY must be absent for the headless producer")


def _load_version_mapper(root: Path) -> tuple[str, str, str]:
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    cargo_version = cargo["workspace"]["package"]["version"]
    mapper_path = root / "tools" / "release" / "python_candidate_common.py"
    mapper_sha256 = _sha256_file(mapper_path)
    spec = importlib.util.spec_from_file_location("_eqiora_release_version_mapper", mapper_path)
    if spec is None or spec.loader is None:
        raise ProducerError("cannot load the accepted Cargo-to-Python version mapper")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    python_version = module.python_distribution_version(cargo_version)
    return cargo_version, python_version, mapper_sha256


def _require_installed_eqiora(root: Path, expected_python_version: str) -> tuple[Any, dict[str, str]]:
    import eqiora
    import eqiora._eqiora as native

    versions = {
        "eqiora.__version__": eqiora.__version__,
        "eqiora._eqiora.__version__": native.__version__,
        "importlib.metadata.version": importlib.metadata.version("eqiora"),
    }
    if set(versions.values()) != {expected_python_version}:
        raise ProducerError("installed Eqiora versions differ from the Cargo-derived Python identity")
    package_path = Path(eqiora.__file__).resolve(strict=True)
    native_path = Path(native.__file__).resolve(strict=True)
    if root == package_path or root in package_path.parents or root == native_path or root in native_path.parents:
        raise ProducerError("Eqiora resolved from the source tree instead of an installed wheel")
    if sys.prefix == sys.base_prefix:
        raise ProducerError("producer must run inside an isolated virtual environment")
    prefix = Path(sys.prefix).resolve(strict=True)
    if prefix not in package_path.parents or prefix not in native_path.parents:
        raise ProducerError("installed Eqiora does not resolve below the active virtual environment")
    versions["package_origin"] = package_path.as_posix()
    versions["extension_origin"] = native_path.as_posix()
    versions["extension_sha256"] = _sha256_file(native_path)
    return eqiora, versions


def _wheel_inputs(wheel_directory: Path) -> list[dict[str, str]]:
    from packaging.utils import canonicalize_name, parse_wheel_filename

    directory = wheel_directory.resolve(strict=True)
    if directory.is_symlink() or not directory.is_dir():
        raise ProducerError("wheel directory must be a real directory")
    wheels = sorted(directory.iterdir(), key=lambda item: item.name.encode("utf-8"))
    if not wheels or len(wheels) > MAX_WHEELS or any(path.suffix != ".whl" for path in wheels):
        raise ProducerError("wheel directory must contain only one bounded resolved wheel set")
    inputs: list[dict[str, str]] = []
    names: set[str] = set()
    for wheel in wheels:
        name, version, _, _ = parse_wheel_filename(wheel.name)
        canonical_name = canonicalize_name(name)
        if canonical_name in names:
            raise ProducerError(f"wheel directory contains duplicate distribution {canonical_name}")
        names.add(canonical_name)
        installed_version = importlib.metadata.version(canonical_name)
        if installed_version != str(version):
            raise ProducerError(f"installed version differs from supplied wheel for {canonical_name}")
        inputs.append(
            {
                "filename": wheel.name,
                "kind": "wheel",
                "name": canonical_name,
                "sha256": _sha256_file(wheel),
                "version": installed_version,
            }
        )
    required = {"eqiora", "matplotlib", "numpy", "pillow"}
    if not required.issubset(names):
        raise ProducerError("resolved wheel set is missing Eqiora or a renderer dependency")
    return inputs


def _native_input(name: str, version: str, raw_path: Path) -> dict[str, str]:
    if not version or any(ord(character) < 32 for character in version):
        raise ProducerError(f"{name} version is missing or malformed")
    path = raw_path.resolve(strict=True)
    return {
        "kind": "native-library",
        "name": name,
        "path": path.as_posix(),
        "sha256": _sha256_file(path),
        "version": version,
    }


def _source_files(root: Path, revision: str) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for relative, roles in sorted(SOURCE_ROLES.items()):
        pure = PurePosixPath(relative)
        if pure.is_absolute() or ".." in pure.parts:
            raise ProducerError(f"source path is not canonical: {relative}")
        path = root / relative
        working_sha = _sha256_file(path, limit=8 * 1024 * 1024)
        blob = _run_git(root, "rev-parse", f"{revision}:{relative}")
        if len(blob) != 40:
            raise ProducerError(f"source blob identity is incomplete: {relative}")
        committed = subprocess.run(
            ["git", "-C", str(root), "show", f"{revision}:{relative}"],
            check=False,
            capture_output=True,
            timeout=30,
        )
        if committed.returncode != 0 or committed.stderr:
            raise ProducerError(f"cannot read committed source bytes: {relative}")
        committed_sha = _sha256_bytes(committed.stdout)
        if working_sha != committed_sha:
            raise ProducerError(f"working source differs from exact revision: {relative}")
        records.append(
            {
                "git_blob": blob,
                "path": relative,
                "roles": roles,
                "sha256": committed_sha,
            }
        )
    return records


def _solve_once(eqiora: Any) -> tuple[Any, Any, Any, Any, Any, Any, Any]:
    from importlib.resources import files

    graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=(0.2, 0.2),
        radius=0.05,
        boolean_tolerance=1e-10,
    )
    geometry = graph.planar_circular_section(
        classification_tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )
    request = eqiora.meshing.ReferenceMesher(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    mesh_plan = eqiora.meshing.resolve(geometry, request)
    mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)
    model_bytes = files(eqiora).joinpath("examples", "steady-flow-past-cylinder.model.json").read_bytes()
    model = eqiora.replay(model_bytes)
    intent = eqiora.fluid.SteadyStokes(
        length_scale_m=0.41,
        velocity_scale_m_per_s=0.3,
        pressure_scale_pa=0.001 * 0.3 / 0.41,
        relative_tolerance=1e-6,
        absolute_tolerance=1e-13,
        maximum_iterations=10_000,
    )
    plan = eqiora.fluid.resolve(model, intent, mesh=mesh)
    run = eqiora.submit(model, plan=plan)
    result = run.result()
    return geometry, mesh_plan, mesh, model, plan, run, result


def _lineage(eqiora: Any, geometry: Any, mesh: Any, plan: Any, result: Any) -> tuple[dict[str, Any], Any]:
    if type(result) is not eqiora.Result or len(result.snapshots) != 1:
        raise ProducerError("canonical solve did not return one common Eqiora Result pressure snapshot")
    pressure = result.snapshots[0]
    selected_pressure = result.field(pressure.field)
    selected_mesh = result.mesh(pressure.field)
    if selected_pressure is not pressure or selected_mesh is not mesh:
        raise ProducerError("Result field or mesh replay differs from the solved pressure lineage")
    if pressure.field.model_digest != result.model_digest:
        raise ProducerError("pressure FieldRef is not bound to the Result Model")
    if pressure.mesh_digest != mesh.digest or plan.mesh_digest != mesh.digest:
        raise ProducerError("pressure or plan Mesh identity differs")
    if plan.geometry_digest != geometry.digest or mesh.source_digest != geometry.digest:
        raise ProducerError("Geometry-to-Mesh identity differs")
    if plan.correspondence_digest != mesh.correspondence_digest:
        raise ProducerError("Mesh correspondence identity differs")

    manifest = result.run_manifest()
    evidence = eqiora.fluid.steady_stokes_evidence(result)
    if evidence.run_digest != manifest.digest or manifest.realization_digest != plan.realization_digest:
        raise ProducerError("Result Run or Realization identity differs")
    if manifest.model_digest != result.model_digest or manifest.output_digests != [pressure.digest]:
        raise ProducerError("Run manifest does not bind the Result pressure output")
    if pressure.associations != ("vertex",) or pressure.value_shape != () or pressure.frame != "invariant":
        raise ProducerError("pressure snapshot is not the accepted invariant vertex scalar")
    if pressure.dimension != EXPECTED_DIMENSION:
        raise ProducerError("pressure snapshot does not carry the coherent-SI pressure dimension")

    values = pressure.values("vertex")
    if values.shape != (mesh.vertex_count,) or not bool(values.flags.writeable is False):
        raise ProducerError("pressure values do not match the immutable Result Mesh vertices")
    observed_minimum = float(values.min())
    observed_maximum = float(values.max())
    if not all(math.isfinite(value) for value in (observed_minimum, observed_maximum)):
        raise ProducerError("pressure snapshot contains a non-finite extrema observation")
    if observed_minimum != evidence.pressure_minimum or observed_maximum != evidence.pressure_maximum:
        raise ProducerError("pressure extrema differ from the accepted Result evidence")

    identities = {
        "correspondence_digest": mesh.correspondence_digest,
        "evidence_run_digest": evidence.run_digest,
        "geometry_digest": mesh.source_digest,
        "mesh_digest": mesh.digest,
        "model_digest": result.model_digest,
        "realization_digest": manifest.realization_digest,
        "run_manifest_digest": manifest.digest,
    }
    lineage = {
        "chain": [
            {"from": identities["model_digest"], "kind": "Model→Geometry", "to": identities["geometry_digest"]},
            {
                "from": identities["geometry_digest"],
                "kind": "Geometry→Correspondence",
                "to": identities["correspondence_digest"],
            },
            {
                "from": identities["correspondence_digest"],
                "kind": "Correspondence→Mesh",
                "to": identities["mesh_digest"],
            },
            {
                "from": identities["model_digest"],
                "kind": "Model→Realization",
                "to": identities["realization_digest"],
            },
            {
                "from": identities["mesh_digest"],
                "kind": "Mesh→Realization",
                "to": identities["realization_digest"],
            },
            {
                "from": identities["realization_digest"],
                "kind": "Realization→Run",
                "to": identities["run_manifest_digest"],
            },
            {
                "from": identities["run_manifest_digest"],
                "kind": "Run→ResultEvidence",
                "to": identities["evidence_run_digest"],
            },
            {
                "from": identities["run_manifest_digest"],
                "kind": "Result→PressureSnapshot",
                "to": pressure.digest,
            },
        ],
        "identities": identities,
        "methods": LINEAGE_METHODS,
        "pressure": {
            "association": "vertex scalar",
            "display_unit": "Pa",
            "field": "pressure",
            "field_id": pressure.field.id,
            "frame_selection": "single steady result; temporal interval not applicable",
            "mesh_digest": pressure.mesh_digest,
            "model_digest": pressure.field.model_digest,
            "ordered_block_digests": [digest for _, digest in pressure.block_digests],
            "ordered_output_digests": manifest.output_digests,
            "snapshot_digest": pressure.digest,
            "source_unit": "kg/(m*s^2)",
            "support_domain_id": pressure.support_domain_id,
            "value_range": {"maximum": observed_maximum, "minimum": observed_minimum},
        },
        "source_result": {
            "digest": manifest.digest,
            "digest_kind": "Result.run_manifest().digest",
        },
    }
    return lineage, pressure


def _render(eqiora: Any, result: Any, pressure: Any, output: Path) -> dict[str, Any]:
    import matplotlib
    import matplotlib.ft2font
    import numpy
    from PIL import Image

    if matplotlib.__version__ != "3.11.1" or matplotlib.get_backend().lower() != "agg":
        raise ProducerError("Matplotlib version or backend differs from the frozen renderer profile")
    import eqiora.matplotlib as eqplot

    figure = eqplot.plot_scalar_field(result, field=pressure.field)
    if tuple(float(value) for value in figure.get_size_inches()) != EXPECTED_FIGURE_INCHES:
        raise ProducerError("canonical adapter figure size differs from the frozen scene")
    if len(figure.axes) != 2:
        raise ProducerError("canonical adapter did not return one plot and one colorbar axes")
    plot_axes, colorbar_axes = figure.axes
    plot_axes.set_title("Exact-cylinder steady Stokes pressure")
    colorbar_axes.set_ylabel("Pressure (Pa)")
    if plot_axes.get_title() != "Exact-cylinder steady Stokes pressure":
        raise ProducerError("scene title could not be fixed")
    if colorbar_axes.get_ylabel() != "Pressure (Pa)":
        raise ProducerError("scene colorbar label could not be fixed")
    figure.savefig(
        output,
        format="png",
        dpi=EXPECTED_DPI,
        facecolor="white",
        metadata={"Software": PNG_SOFTWARE},
    )
    figure.clear()
    if output.lstat().st_size > MAX_PNG_BYTES:
        raise ProducerError("candidate PNG exceeds the publication byte cap")
    with Image.open(output) as image:
        image.load()
        pixel_size = image.size
        pixel_mode = image.mode
    if pixel_size != EXPECTED_PIXEL_SIZE or pixel_mode != "RGBA":
        raise ProducerError("candidate PNG dimensions or pixel mode differ from the frozen profile")
    return {
        "backend": "matplotlib/Agg",
        "freetype_version": matplotlib.ft2font.__freetype_version__,
        "matplotlib_version": matplotlib.__version__,
        "numpy_version": numpy.__version__,
        "pixel_mode": pixel_mode,
        "pixel_size": list(pixel_size),
    }


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--output-directory", type=Path, required=True)
    parser.add_argument("--wheel-directory", type=Path, required=True)
    parser.add_argument("--freetype-library", type=Path, required=True)
    parser.add_argument("--freetype-version", required=True)
    parser.add_argument("--libpng-library", type=Path, required=True)
    parser.add_argument("--libpng-version", required=True)
    parser.add_argument("--zlib-library", type=Path, required=True)
    parser.add_argument("--zlib-version", required=True)
    return parser.parse_args()


def _produce(arguments: argparse.Namespace) -> None:
    root = arguments.repository_root.resolve(strict=True)
    output = arguments.output_directory.absolute()
    if output.exists() or output.is_symlink():
        raise ProducerError("candidate output directory must not already exist")
    parent = output.parent.resolve(strict=True)
    if output.parent != parent or parent.is_symlink():
        raise ProducerError("candidate parent must be a resolved real directory")

    _require_environment()
    revision, tree = _require_clean_source(root, arguments.source_revision)
    cargo_version, expected_python_version, mapper_sha256 = _load_version_mapper(root)
    eqiora, eqiora_runtime = _require_installed_eqiora(root, expected_python_version)
    source_files = _source_files(root, revision)
    wheel_inputs = _wheel_inputs(arguments.wheel_directory)
    native_inputs = [
        _native_input("FreeType", arguments.freetype_version, arguments.freetype_library),
        _native_input("libpng", arguments.libpng_version, arguments.libpng_library),
        _native_input("zlib", arguments.zlib_version, arguments.zlib_library),
    ]
    python_input = {
        "kind": "runtime",
        "name": "Python",
        "path": Path(sys.executable).resolve(strict=True).as_posix(),
        "sha256": _sha256_file(Path(sys.executable).resolve(strict=True)),
        "version": platform.python_version(),
    }

    staging = Path(tempfile.mkdtemp(prefix=".eqiora-cylinder-pressure-", dir=parent))
    try:
        os.environ["MPLCONFIGDIR"] = str(staging / "matplotlib-config")
        (staging / "matplotlib-config").mkdir(mode=0o700)
        png = staging / OUTPUT_NAMES[0]

        geometry, mesh_plan, mesh, model, plan, run, result = _solve_once(eqiora)
        lineage, pressure = _lineage(eqiora, geometry, mesh, plan, result)
        renderer = _render(eqiora, result, pressure, png)
        if renderer["freetype_version"] != arguments.freetype_version:
            raise ProducerError("Matplotlib FreeType version differs from the supplied native identity")

        revision_after, tree_after = _require_clean_source(root, arguments.source_revision)
        if (revision_after, tree_after) != (revision, tree):
            raise ProducerError("source identity changed during production")
        script_source = next(
            item for item in source_files if item["path"] == "tools/site/produce_exact_cylinder_pressure.py"
        )
        argv_sha256 = _sha256_bytes(json.dumps(sys.argv, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))
        resolved_inputs = [python_input, *native_inputs, *wheel_inputs]
        resolved_inputs.sort(key=lambda item: item["name"])
        if len({item["name"] for item in resolved_inputs}) != len(resolved_inputs):
            raise ProducerError("resolved renderer input names are not unique")

        observation = {
            "candidate": {
                "directory_name": output.name,
                "files": list(OUTPUT_NAMES),
                "status": "produced-not-admitted",
            },
            "execution": {
                "cargo_version": cargo_version,
                "eqiora_runtime": eqiora_runtime,
                "expected_python_distribution_version": expected_python_version,
                "isolated_mode": bool(sys.flags.isolated),
                "python_no_user_site": bool(sys.flags.no_user_site),
                "solve_count": 1,
                "version_mapper_sha256": mapper_sha256,
            },
            "lineage": lineage,
            "media": {
                "byte_size": png.lstat().st_size,
                "mime": "image/png",
                "path": OUTPUT_NAMES[0],
                "pixel_mode": renderer["pixel_mode"],
                "sha256": _sha256_file(png, limit=MAX_PNG_BYTES),
                "size": renderer["pixel_size"],
            },
            "mesh_observation": {
                "boundary_facets": mesh_plan.boundary_facets,
                "cell_count": mesh.cell_count,
                "dimension": mesh.dimension,
                "vertex_count": mesh.vertex_count,
            },
            "producer_command": {
                "argv_sha256": argv_sha256,
                "path": script_source["path"],
                "sha256": script_source["sha256"],
            },
            "renderer": {
                "backend": renderer["backend"],
                "encoder": "matplotlib PNG",
                "environment": {
                    "architecture": platform.machine(),
                    "locale": locale.setlocale(locale.LC_ALL, None),
                    "os_name": platform.system(),
                    "os_version": platform.release(),
                    "resolved_inputs": resolved_inputs,
                    "timezone": "UTC",
                },
            },
            "result_observation": {
                "adapter": result.adapter,
                "adapter_version": result.adapter_version,
                "model_digest": model.digest,
                "run_status": str(run.status),
                "type": f"{type(result).__module__}.{type(result).__name__}",
            },
            "scene_profile": {
                "bounds_m": {"x": [0.0, 2.2], "y": [0.0, 0.41]},
                "colorbar_label": "Pressure (Pa)",
                "colormap": "viridis",
                "constant_metadata": {"Software": PNG_SOFTWARE},
                "dpi": EXPECTED_DPI,
                "facecolor": "white",
                "figure_inches": list(EXPECTED_FIGURE_INCHES),
                "format": "png",
                "mesh_overlay": True,
                "no_crop_resize_reencode": True,
                "normalization": "bound pressure snapshot minimum/maximum",
                "plot": "tripcolor",
                "shading": "gouraud",
                "title": "Exact-cylinder steady Stokes pressure",
                "triangulation": "accepted Result mesh explicit triangle connectivity",
            },
            "schema": OBSERVATION_SCHEMA,
            "source_files": source_files,
            "source_revision": revision,
            "source_tree": tree,
        }
        observation_bytes = _canonical_json(observation)
        if len(observation_bytes) > MAX_JSON_BYTES:
            raise ProducerError("producer observation exceeds its byte cap")
        observation_path = staging / OUTPUT_NAMES[2]
        observation_path.write_bytes(observation_bytes)

        log_lines = [
            "Eqiora exact-cylinder pressure publication producer v1",
            f"source_revision={revision}",
            f"source_tree={tree}",
            f"cargo_version={cargo_version}",
            f"python_distribution_version={expected_python_version}",
            "solve_count=1",
            f"run_manifest_digest={lineage['identities']['run_manifest_digest']}",
            f"pressure_snapshot_digest={lineage['pressure']['snapshot_digest']}",
            f"png_sha256={observation['media']['sha256']}",
            f"png_bytes={observation['media']['byte_size']}",
            f"observation_sha256={_sha256_bytes(observation_bytes)}",
            "status=produced-not-admitted",
        ]
        log_path = staging / OUTPUT_NAMES[1]
        log_path.write_text("\n".join(log_lines) + "\n", encoding="utf-8", newline="\n")
        shutil.rmtree(staging / "matplotlib-config")
        if sorted(path.name for path in staging.iterdir()) != sorted(OUTPUT_NAMES):
            raise ProducerError("candidate staging directory contains unexpected output")
        for path in staging.iterdir():
            path.chmod(0o444)
        staging.chmod(0o555)
        staging.rename(output)
    except BaseException:
        if staging.exists():
            staging.chmod(0o700)
            shutil.rmtree(staging)
        raise


def main() -> int:
    try:
        arguments = _arguments()
        _produce(arguments)
    except (OSError, ProducerError, subprocess.SubprocessError, ValueError) as error:
        print(f"exact-cylinder pressure production failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
