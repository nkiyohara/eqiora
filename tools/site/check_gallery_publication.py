#!/usr/bin/env python3
"""Fail-closed admission check for the private exact-cylinder gallery record."""

from __future__ import annotations

import argparse
import binascii
import hashlib
import json
import math
import os
import re
import shutil
import stat
import struct
import subprocess
import sys
import tomllib
import zlib
from pathlib import Path, PurePosixPath
from typing import Any

RECORD_SCHEMA = "eqiora.site.gallery-publication/v1"
PREDICATE_SCHEMA = "eqiora.site.gallery-publication-predicate/v1"
RECEIPT_SCHEMA = "eqiora.site.gallery-publication-receipt/v1"
RESULT_SCHEMA = "eqiora.site.gallery-publication-check-result/v1"
ENTRY_ID = "exact-cylinder-steady-stokes"
RECEIPT_ID = "exact-cylinder-steady-stokes-publication-admission-v1"
FINAL_RECORD = "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json"
FINAL_MEDIA = "docs/site/src/assets/gallery/exact-cylinder-pressure.png"
ELASTICITY_ENTRY_ID = "mixed-boundary-elasticity"
ELASTICITY_PREDICATE_SCHEMA = (
    "eqiora.site.mixed-boundary-elasticity-publication-predicate/v1"
)
ELASTICITY_RECORD = (
    "docs/site/src/data/gallery/mixed-boundary-elasticity.publication.json"
)
ELASTICITY_MEDIA = (
    "docs/site/src/assets/gallery/mixed-boundary-elasticity-displacement.png"
)
ELASTICITY_WIDTH, ELASTICITY_HEIGHT, ELASTICITY_DPI = 1120, 960, 160
ELASTICITY_PNG_SOFTWARE = "Matplotlib version3.11.1, https://matplotlib.org/"
ELASTICITY_ALT = (
    "Reference and deformed meshes for the bounded 2D mixed-boundary "
    "linear-elasticity demonstration. The left edge is fixed, the other edges "
    "are traction-free, and the visible deformation scale is 1. Presentation only."
)
ELASTICITY_CLAIM = (
    "one bounded 2D mixed-boundary linear-elasticity workflow reaches a common "
    "Result and a caller-owned displacement figure through the root lifecycle"
)
ELASTICITY_NONCLAIMS = [
    "no general elasticity or arbitrary boundary data",
    "no stress, strain, or traction recovery from the common Result",
    "no unstructured, high-order, three-dimensional, nonlinear, or dynamic structure",
    "no convergence, performance, or production-scale claim",
    "no exact pixels or scientific validation from rendering",
]
ELASTICITY_SOURCE_ROLES = {
    "bindings/python/python/eqiora/matplotlib.py": ["plotting-adapter"],
    "examples/mixed-boundary-elasticity.eqi": ["component-source"],
    "examples/python/mixed_boundary_elasticity.py": ["shared-installed-workflow"],
}
ELASTICITY_CASE_ROLES = {
    "interfaces.python-mixed-boundary-elasticity-demo": "installed-product",
    "solid.mixed-boundary-elasticity-2d": "scientific-evidence",
}

MAX_JSON_BYTES, MAX_MEDIA_BYTES = 512 * 1024, 16 * 1024 * 1024
GIT_OBJECT_REPOSITORY_VARIABLE = "EQIORA_SITE_GIT_OBJECT_REPOSITORY"
SOURCE_SHA_VARIABLE = "EQIORA_SITE_SOURCE_SHA"
_GIT_EXECUTABLE = Path(shutil.which("git", path=os.defpath) or "/usr/bin/git").resolve()
_GIT_ENVIRONMENT = {
    "LC_ALL": "C",
    "LANG": "C",
    "TZ": "UTC",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_OPTIONAL_LOCKS": "0",
    "GIT_TERMINAL_PROMPT": "0",
    "GIT_NO_REPLACE_OBJECTS": "1",
}
WIDTH, HEIGHT, DPI = 1280, 832, 160
PNG_SOFTWARE = "Eqiora exact-cylinder gallery publication v1"

ALT_TEXT = (
    "Pressure in pascals for a 2D steady-Stokes exact-cylinder demonstration, "
    "shown with a viridis color scale and its current Gmsh mesh overlaid. "
    "Presentation image only; no numerical or mesh-output oracle."
)
PUBLIC_CLAIM = (
    "one presentation-only 2D steady incompressible Stokes exact-cylinder "
    "demonstration rendered through exact Geometry, typed Gmsh policy, and the "
    "root Result path; output counts, digests, numerical values, and pixels are "
    "not independently verified."
)
NONCLAIMS = (
    "no arbitrary geometry or provider selection|no 3D, curved, boundary-layer, or adaptive meshing|"
    "no mesh/PDE convergence|no drag/lift coefficient, scaled or mesh-independent force, or DFG value|"
    "no transient or Navier–Stokes behavior|no vortex shedding|no performance claim|"
    "no cross-platform mesh-byte identity or byte-reproducible Result|no pixel validation|"
    "API presence is neither verification nor maturity"
).split("|")

SOURCE_ROLES = {
    "bindings/python/python/eqiora/matplotlib.py": ["plotting-adapter"],
    "examples/python/exact_cylinder_geometry.py": ["geometry-adapter"],
    "examples/python/exact_cylinder_mesh.py": ["mesh-realization-owner"],
    "examples/python/exact_cylinder_stokes.py": ["plain-python-snippet"],
    "examples/python/exact_cylinder_stokes_marimo.py": ["canonical-marimo-snippet"],
    "examples/steady-flow-past-cylinder.eqi": ["example-formula-owner"],
    "examples/steady-flow-past-cylinder.geometry.json": ["geometry"],
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi": ["current-package-formula-owner"],
    "tools/site/produce_exact_cylinder_pressure.py": ["producer-command"],
    "verify/fluid/packaged-steady-stokes-2d/models/direct.eqi": ["packaged-stokes-formula"],
    "verify/fluid/packaged-steady-stokes-2d/package-v0.1.0/src/incompressible.eqi": ["package-formula-owner"],
}

CASE_ROLES = {
    "artifacts.current-model-canonical-identity": "evidence",
    "fluid.packaged-steady-stokes-2d": "evidence",
    "geometry.exact-circular-hole-geometry": "evidence",
    "interfaces.python-exact-circular-hole-geometry": "evidence",
    "interfaces.python-exact-cylinder-stokes-marimo": "evidence",
}
LINEAGE_METHODS = {
    "correspondence_digest": "Plan.correspondence_digest",
    "evidence_plan_key": "fluid.steady_stokes_evidence(Result).plan_key",
    "geometry_digest": "Plan.geometry_digest",
    "mesh_digest": "Plan.mesh_digest",
    "model_digest": "Result.model_digest",
    "plan_identity": "Plan.identity",
    "pressure_output": "Result.output(FieldRef)",
    "result_plan_key": "Result.plan_key",
}

RECEIPT_CHECKS = (
    "canonical-payload-and-wrapper|source-revision-tree-and-source-file-digests|"
    "registered-case-identities-and-presentation-only-boundary|model-realization-run-result-pressure-lineage|"
    "png-structure-crc-decode-dimensions-and-digests|exact-alt-caption-and-link-digests|"
    "renderer-scene-profile-and-environment-identities|claim-nonclaims-and-evidence-routes"
).split("|")
RECEIPT_NONCHECKS = (
    "image pixels are not scientific validation|"
    "no cross-platform mesh-byte identity or byte-reproducible Result claim|"
    "no new scientific oracle or equality"
).split("|")


class AdmissionError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def _fail(code: str, message: str) -> None:
    raise AdmissionError(code, message)


def _sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _canonical_value(value: Any) -> bytes:
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        _fail("json-value", f"value is not canonical JSON: {error}")


def _canonical_file(value: Any) -> bytes:
    return _canonical_value(value) + b"\n"


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail("json-duplicate", f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _read_regular(path: Path, limit: int, label: str) -> bytes:
    lexical = Path(os.path.abspath(path))
    try:
        info = lexical.lstat()
    except OSError as error:
        _fail("path", f"cannot stat {label}: {error}")
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        _fail("path", f"{label} must be a regular non-symlink file")
    if info.st_size > limit:
        _fail("raw-cap", f"{label} exceeds {limit} bytes")
    try:
        return lexical.read_bytes()
    except OSError as error:
        _fail("path", f"cannot read {label}: {error}")


def _load_canonical(path: Path, label: str) -> tuple[dict[str, Any], bytes]:
    raw = _read_regular(path, MAX_JSON_BYTES, label)
    try:
        value = json.loads(
            raw,
            object_pairs_hook=_pairs,
            parse_constant=lambda token: _fail("json-value", f"invalid {token}"),
        )
    except AdmissionError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError, ValueError) as error:
        _fail("json-parse", f"invalid {label}: {error}")
    if type(value) is not dict:
        _fail("json-type", f"{label} root must be an object")
    if raw != _canonical_file(value):
        _fail("json-canonical", f"{label} is not sorted compact UTF-8 JSON plus LF")
    return value, raw


def _closed(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if type(value) is not dict:
        _fail("shape", f"{label} must be an object")
    actual = set(value)
    if actual != keys:
        _fail(
            "shape",
            f"{label} keys differ; missing={sorted(keys - actual)}, extra={sorted(actual - keys)}",
        )
    return value


def _list(value: Any, label: str, maximum: int) -> list[Any]:
    if type(value) is not list or len(value) > maximum:
        _fail("shape", f"{label} must be a list with at most {maximum} entries")
    return value


def _text(value: Any, label: str, maximum: int = 4096) -> str:
    if type(value) is not str or not value or len(value.encode("utf-8")) > maximum:
        _fail("shape", f"{label} must be nonempty text bounded by {maximum} bytes")
    if any(ord(character) < 32 for character in value):
        _fail("shape", f"{label} contains a control character")
    return value


def _hex(value: Any, label: str, length: int = 64) -> str:
    if type(value) is not str or len(value) != length:
        _fail("identity", f"{label} must be {length} lowercase hexadecimal characters")
    if any(character not in "0123456789abcdef" for character in value):
        _fail("identity", f"{label} must be lowercase hexadecimal")
    return value


def _number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        _fail("shape", f"{label} must be a finite number")
    try:
        converted = float(value)
    except (OverflowError, ValueError):
        _fail("shape", f"{label} must fit a finite binary64 value")
    if not math.isfinite(converted):
        _fail("shape", f"{label} must be finite")
    return converted


def _relative(value: Any, label: str) -> str:
    text = _text(value, label, 512)
    if "\\" in text or text.startswith("/"):
        _fail("path", f"{label} must be a repository-relative POSIX path")
    parsed = PurePosixPath(text)
    if str(parsed) != text or any(part in {"", ".", ".."} for part in parsed.parts):
        _fail("path", f"{label} is not normalized")
    return text


def _git(root: Path, *arguments: str) -> bytes:
    try:
        result = subprocess.run(
            [
                os.fspath(_GIT_EXECUTABLE),
                "--no-replace-objects",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
                "-C",
                os.fspath(root),
                *arguments,
            ],
            check=False,
            env=_GIT_ENVIRONMENT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        _fail("source-git", f"Git inspection failed: {error}")
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip()
        _fail("source-git", f"git {' '.join(arguments)} failed: {detail}")
    return result.stdout


def _git_object_repository(source_root: Path) -> Path:
    raw = os.environ.get(GIT_OBJECT_REPOSITORY_VARIABLE)
    if raw is None:
        authority = source_root
    else:
        candidate = Path(raw)
        if not candidate.is_absolute():
            _fail("source-git", "Git object repository must be absolute")
        try:
            authority = candidate.resolve(strict=True)
        except OSError as error:
            _fail("source-git", f"Git object repository is unavailable: {error}")
        if candidate != authority:
            _fail("source-git", "Git object repository path must be canonical")
        if not authority.is_dir() or candidate.is_symlink():
            _fail("source-git", "Git object repository must be a real directory")
        if os.path.lexists(source_root / ".git"):
            _fail("source-git", "archive source unexpectedly contains .git")
        if authority == source_root or authority.is_relative_to(source_root):
            _fail("source-git", "Git object repository overlaps archive source")

    top_level = _git(authority, "rev-parse", "--show-toplevel").decode(
        "utf-8", "strict"
    )
    if top_level != f"{authority}\n":
        _fail("source-git", "Git object repository top level differs")
    head = _git(authority, "rev-parse", "--verify", "HEAD^{commit}").decode(
        "ascii", "strict"
    ).strip()
    expected_head = os.environ.get(SOURCE_SHA_VARIABLE)
    if expected_head is not None:
        if re.fullmatch(r"[0-9a-f]{40}", expected_head) is None:
            _fail("source-git", "site source SHA is malformed")
        if head != expected_head:
            _fail("source-git", "Git object repository HEAD differs from site source SHA")
    return authority


def _source_blob(root: Path, revision: str, path: str) -> bytes:
    return _git(root, "cat-file", "blob", f"{revision}:{path}")


def _check_source(wrapper: dict[str, Any], payload: dict[str, Any], root: Path) -> None:
    revision = _hex(wrapper["source_revision"], "source_revision", 40)
    tree = _hex(wrapper["source_tree"], "source_tree", 40)
    resolved = _git(root, "rev-parse", "--verify", f"{revision}^{{commit}}").strip().decode()
    if resolved != revision:
        _fail("source-revision", "source_revision does not resolve exactly")
    resolved_tree = _git(root, "rev-parse", f"{revision}^{{tree}}").strip().decode()
    if resolved_tree != tree:
        _fail("source-tree", "source_tree does not match source_revision")
    try:
        subprocess.run(
            [
                os.fspath(_GIT_EXECUTABLE),
                "--no-replace-objects",
                "-c",
                "core.fsmonitor=false",
                "-C",
                os.fspath(root),
                "merge-base",
                "--is-ancestor",
                revision,
                "HEAD",
            ],
            check=True,
            env=_GIT_ENVIRONMENT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=30,
        )
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        _fail("source-revision", "source_revision is not an ancestor of repository HEAD")

    sources = _list(payload["source_files"], "source_files", len(SOURCE_ROLES))
    if len(sources) != len(SOURCE_ROLES):
        _fail("source-set", "source_files does not contain the exact required set")
    observed: dict[str, list[str]] = {}
    paths: list[str] = []
    for index, raw in enumerate(sources):
        item = _closed(raw, {"path", "roles", "sha256"}, f"source_files[{index}]")
        path = _relative(item["path"], f"source_files[{index}].path")
        roles = _list(item["roles"], f"source_files[{index}].roles", 4)
        if not roles or any(type(role) is not str for role in roles):
            _fail("source-set", f"source_files[{index}].roles is invalid")
        digest = _hex(item["sha256"], f"source_files[{index}].sha256")
        if digest != _sha(_source_blob(root, revision, path)):
            _fail("source-digest", f"source digest differs for {path}")
        if path in observed:
            _fail("source-set", f"duplicate source path {path}")
        observed[path] = roles
        paths.append(path)
    if paths != sorted(paths) or observed != SOURCE_ROLES:
        _fail("source-set", "source paths, roles, or order differ from the predicate")


def _case_path(case_id: str) -> str:
    area, name = case_id.split(".", 1)
    return f"verify/{area}/{name}/case.toml"


def _dossier_route(case_id: str, revision: str) -> str:
    readme = str(PurePosixPath(_case_path(case_id)).with_name("README.md"))
    return f"https://github.com/nkiyohara/eqiora/blob/{revision}/{readme}"


def _check_cases(payload: dict[str, Any], root: Path, revision: str) -> None:
    cases = _list(payload["evidence_cases"], "evidence_cases", len(CASE_ROLES))
    if len(cases) != len(CASE_ROLES):
        _fail("case-set", "evidence_cases does not contain the exact required set")
    observed: dict[str, str] = {}
    manifests: list[str] = []
    for index, raw in enumerate(cases):
        item = _closed(
            raw,
            {"dossier_route", "id", "manifest_path", "manifest_sha256", "role"},
            f"evidence_cases[{index}]",
        )
        case_id = _text(item["id"], f"evidence_cases[{index}].id", 128)
        if case_id not in CASE_ROLES:
            _fail("case-set", f"unknown evidence case {case_id}")
        manifest = _relative(item["manifest_path"], f"evidence_cases[{index}].manifest_path")
        role = _text(item["role"], f"evidence_cases[{index}].role", 32)
        if manifest != _case_path(case_id):
            _fail("case-set", f"manifest path is not canonical for {case_id}")
        if item["dossier_route"] != _dossier_route(case_id, revision):
            _fail("case-route", f"dossier route differs for {case_id}")
        _source_blob(root, revision, str(PurePosixPath(manifest).with_name("README.md")))
        raw_manifest = _source_blob(root, revision, manifest)
        if _hex(item["manifest_sha256"], f"{case_id}.manifest_sha256") != _sha(raw_manifest):
            _fail("case-digest", f"manifest digest differs for {case_id}")
        try:
            document = tomllib.loads(raw_manifest.decode("utf-8"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            _fail("case-manifest", f"invalid manifest for {case_id}: {error}")
        if document.get("id") != case_id:
            _fail("case-manifest", f"manifest id differs for {case_id}")
        if case_id in observed:
            _fail("case-set", f"duplicate case {case_id}")
        observed[case_id] = role
        manifests.append(manifest)
    if manifests != sorted(manifests) or observed != CASE_ROLES:
        _fail("case-set", "case identities, roles, or order differ from the predicate")


def _check_lineage(lineage: Any) -> None:
    item = _closed(
        lineage,
        {"chain", "identities", "methods", "pressure", "result_binding"},
        "lineage",
    )
    identities = _closed(
        item["identities"],
        {
            "correspondence_digest",
            "evidence_plan_key",
            "geometry_digest",
            "mesh_digest",
            "model_digest",
            "plan_identity",
            "result_plan_key",
        },
        "lineage.identities",
    )
    for key, value in identities.items():
        _hex(value, f"lineage.identities.{key}")
    methods = _closed(item["methods"], set(LINEAGE_METHODS), "lineage.methods")
    if methods != LINEAGE_METHODS:
        _fail("lineage-method", "lineage methods differ from the accepted public Result owners")
    if identities["evidence_plan_key"] != identities["plan_identity"]:
        _fail("lineage", "Result evidence does not bind the accepted Plan")
    if identities["result_plan_key"] != identities["plan_identity"]:
        _fail("lineage", "Result does not bind the accepted Plan")

    result_binding = _closed(item["result_binding"], {"identity_kind", "identity"}, "lineage.result_binding")
    if result_binding["identity_kind"] != "Result.plan_key":
        _fail("lineage", "source Result must use its public Plan binding")
    if result_binding["identity"] != identities["result_plan_key"]:
        _fail("lineage", "source Result does not bind result_plan_key")

    pressure = _closed(
        item["pressure"],
        {
            "association",
            "display_unit",
            "field",
            "frame_selection",
            "mesh_digest",
            "model_digest",
            "components",
            "vertex_count",
            "source_unit",
            "value_range",
        },
        "lineage.pressure",
    )
    expected = {
        "field": "pressure",
        "association": "vertex scalar",
        "source_unit": "kg/(m*s^2)",
        "display_unit": "Pa",
        "frame_selection": "single steady result; temporal interval not applicable",
    }
    for key, value in expected.items():
        if pressure[key] != value:
            _fail("lineage", f"pressure {key} differs")
    if pressure["mesh_digest"] != identities["mesh_digest"]:
        _fail("lineage", "pressure output mesh does not bind lineage mesh")
    if pressure["model_digest"] != identities["model_digest"]:
        _fail("lineage", "pressure FieldRef does not bind the Result Model")
    if pressure["components"] != 1 or not isinstance(pressure["vertex_count"], int) or pressure["vertex_count"] <= 0:
        _fail("lineage", "pressure output shape differs")
    value_range = _closed(pressure["value_range"], {"maximum", "minimum"}, "lineage.pressure.value_range")
    if _number(value_range["minimum"], "pressure minimum") > _number(value_range["maximum"], "pressure maximum"):
        _fail("lineage", "pressure value range is reversed")
    expected_chain = [
        {
            "from": identities["model_digest"],
            "kind": "Model→Geometry",
            "to": identities["geometry_digest"],
        },
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
            "kind": "Model→Plan",
            "to": identities["plan_identity"],
        },
        {
            "from": identities["mesh_digest"],
            "kind": "Mesh→Plan",
            "to": identities["plan_identity"],
        },
        {
            "from": identities["plan_identity"],
            "kind": "Plan→ResultBinding",
            "to": identities["result_plan_key"],
        },
        {
            "from": identities["result_plan_key"],
            "kind": "ResultBinding→Evidence",
            "to": identities["evidence_plan_key"],
        },
    ]
    if item["chain"] != expected_chain:
        _fail("lineage", "Model→Geometry→Mesh→Plan→Result/evidence chain differs")


def _paeth(left: int, above: int, upper_left: int) -> int:
    estimate = left + above - upper_left
    dl = abs(estimate - left)
    da = abs(estimate - above)
    dul = abs(estimate - upper_left)
    return left if dl <= da and dl <= dul else above if da <= dul else upper_left


def _decode_png(
    raw: bytes,
    *,
    width: int = WIDTH,
    height: int = HEIGHT,
    dpi: int = DPI,
    software: str = PNG_SOFTWARE,
) -> tuple[dict[str, Any], bytes, list[str]]:
    if not raw.startswith(b"\x89PNG\r\n\x1a\n"):
        _fail("png", "media has no PNG signature")
    offset = 8
    chunks: list[tuple[str, bytes]] = []
    while offset < len(raw):
        if len(chunks) >= 64 or offset + 12 > len(raw):
            _fail("png", "PNG chunk structure is truncated or excessive")
        length = struct.unpack(">I", raw[offset : offset + 4])[0]
        if length > MAX_MEDIA_BYTES or offset + 12 + length > len(raw):
            _fail("png", "PNG chunk length exceeds its bounded file")
        type_bytes = raw[offset + 4 : offset + 8]
        data = raw[offset + 8 : offset + 8 + length]
        expected_crc = struct.unpack(">I", raw[offset + 8 + length : offset + 12 + length])[0]
        if binascii.crc32(type_bytes + data) & 0xFFFFFFFF != expected_crc:
            _fail("png", "PNG chunk CRC differs")
        try:
            chunk_type = type_bytes.decode("ascii")
        except UnicodeDecodeError:
            _fail("png", "PNG chunk type is not ASCII")
        if len(chunk_type) != 4 or not chunk_type.isalpha():
            _fail("png", "PNG chunk type is malformed")
        if chunk_type[0].isupper() and chunk_type not in {"IHDR", "IDAT", "IEND"}:
            _fail("png", f"unknown critical PNG chunk {chunk_type}")
        chunks.append((chunk_type, data))
        offset += 12 + length
        if chunk_type == "IEND":
            break
    if offset != len(raw) or not chunks or chunks[0][0] != "IHDR" or chunks[-1] != ("IEND", b""):
        _fail("png", "PNG has trailing bytes or wrong terminal chunks")
    types = [kind for kind, _ in chunks]
    if types.count("IHDR") != 1 or types.count("IEND") != 1 or "IDAT" not in types:
        _fail("png", "PNG mandatory chunks differ")
    idat_indices = [index for index, kind in enumerate(types) if kind == "IDAT"]
    if idat_indices != list(range(idat_indices[0], idat_indices[-1] + 1)):
        _fail("png", "PNG IDAT chunks are not consecutive")
    if types.count("tEXt") != 1 or types.count("pHYs") != 1:
        _fail("png", "PNG must contain one constant tEXt and one pHYs chunk")
    if types[:3] != ["IHDR", "tEXt", "pHYs"] or types[-1] != "IEND" or any(kind != "IDAT" for kind in types[3:-1]):
        _fail("png", "PNG chunk profile differs from the frozen encoder output")
    text_data = next(data for kind, data in chunks if kind == "tEXt")
    if text_data != b"Software\0" + software.encode("ascii"):
        _fail("png", "PNG Software metadata differs")
    phys = next(data for kind, data in chunks if kind == "pHYs")
    pixels_per_metre = round(dpi / 0.0254)
    if phys != struct.pack(">IIB", pixels_per_metre, pixels_per_metre, 1):
        _fail("png", "PNG physical resolution differs from fixed DPI")

    header = chunks[0][1]
    if len(header) != 13:
        _fail("png", "PNG IHDR length differs")
    observed_width, observed_height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", header)
    if (observed_width, observed_height, depth, color, compression, filtering, interlace) != (
        width,
        height,
        8,
        6,
        0,
        0,
        0,
    ):
        _fail("png", "PNG dimensions or RGBA encoding profile differs")
    compressed = b"".join(data for kind, data in chunks if kind == "IDAT")
    expected_size = height * (1 + width * 4)
    inflater = zlib.decompressobj()
    try:
        filtered = inflater.decompress(compressed, expected_size + 1)
        if inflater.unconsumed_tail or len(filtered) > expected_size:
            _fail("png", "PNG decoded data exceeds its fixed dimensions")
        filtered += inflater.flush(expected_size + 1 - len(filtered))
    except zlib.error as error:
        _fail("png", f"PNG IDAT cannot be decoded: {error}")
    if len(filtered) != expected_size or not inflater.eof or inflater.unused_data or inflater.unconsumed_tail:
        _fail("png", "PNG decoded size or compressed stream boundary differs")

    row_size = width * 4
    previous = bytearray(row_size)
    decoded = bytearray()
    cursor = 0
    visible_colors: set[bytes] = set()
    for _ in range(height):
        filter_type = filtered[cursor]
        encoded = filtered[cursor + 1 : cursor + 1 + row_size]
        cursor += row_size + 1
        if filter_type > 4:
            _fail("png", "PNG row uses an unknown filter")
        row = bytearray(encoded) if filter_type == 0 else bytearray(row_size)
        if filter_type:
            for index, byte in enumerate(encoded):
                left = row[index - 4] if index >= 4 else 0
                above = previous[index]
                upper_left = previous[index - 4] if index >= 4 else 0
                predictor = (left, above, (left + above) // 2, _paeth(left, above, upper_left))[filter_type - 1]
                row[index] = (byte + predictor) & 0xFF
        for index in range(0, row_size, 4):
            red, green, blue, alpha = row[index : index + 4]
            composed = bytes(((channel * alpha + 255 * (255 - alpha) + 127) // 255 for channel in (red, green, blue)))
            visible_colors.add(composed)
            if len(visible_colors) > 1:
                break
        decoded.extend(row)
        previous = row
    if len(visible_colors) < 2:
        _fail("png", "PNG is blank/uniform after RGBA composition over its frozen white scene")
    return (
        {
            "bit_depth": depth,
            "color_type": color,
            "height": observed_height,
            "interlace": interlace,
            "pixel_mode": "RGBA",
            "width": observed_width,
        },
        bytes(decoded),
        types,
    )


def _check_media(payload: dict[str, Any], media_path: Path) -> None:
    media = _closed(
        payload["media"],
        {
            "bit_depth",
            "byte_size",
            "chunk_types",
            "color_type",
            "decoded_pixel_sha256",
            "height",
            "interlace",
            "mime",
            "nonblank",
            "path",
            "pixel_mode",
            "sha256",
            "width",
        },
        "media",
    )
    if media["path"] != FINAL_MEDIA or media["mime"] != "image/png" or media["nonblank"] is not True:
        _fail("media", "media path, MIME, or nonblank declaration differs")
    raw = _read_regular(media_path, MAX_MEDIA_BYTES, "pressure media")
    if media["byte_size"] != len(raw) or _hex(media["sha256"], "media.sha256") != _sha(raw):
        _fail("media-digest", "media byte size or SHA-256 differs")
    structure, decoded, chunk_types = _decode_png(raw)
    for key, value in structure.items():
        if media[key] != value:
            _fail("png-record", f"recorded PNG {key} differs")
    if media["chunk_types"] != chunk_types:
        _fail("png-record", "recorded PNG chunk order differs")
    if _hex(media["decoded_pixel_sha256"], "media.decoded_pixel_sha256") != _sha(decoded):
        _fail("png-record", "decoded pixel digest differs")


def _check_text(payload: dict[str, Any], revision: str) -> None:
    item = _closed(
        payload["text"],
        {"alt", "alt_sha256", "caption", "caption_links", "caption_sha256"},
        "text",
    )
    caption = (
        "Pressure (Pa), frozen exact-cylinder steady-Stokes demonstration at "
        f"{revision}; presentation only, not validation."
    )
    if item["alt"] != ALT_TEXT or item["caption"] != caption:
        _fail("text", "alt text or caption differs from the frozen wording")
    if _hex(item["alt_sha256"], "text.alt_sha256") != _sha(ALT_TEXT.encode("utf-8")):
        _fail("text-digest", "alt-text digest differs")
    if _hex(item["caption_sha256"], "text.caption_sha256") != _sha(caption.encode("utf-8")):
        _fail("text-digest", "caption digest differs")
    expected_links: list[dict[str, str]] = []
    if item["caption_links"] != expected_links:
        _fail("text", "caption links differ")


def _check_renderer(payload: dict[str, Any]) -> None:
    renderer = _closed(
        payload["renderer"],
        {"backend", "encoder", "environment", "producer_command"},
        "renderer",
    )
    if renderer["backend"] != "matplotlib/Agg" or renderer["encoder"] != "matplotlib PNG":
        _fail("renderer", "renderer or encoder identity differs")
    command = _closed(
        renderer["producer_command"],
        {"argv_sha256", "path", "sha256"},
        "renderer.producer_command",
    )
    if command["path"] != "tools/site/produce_exact_cylinder_pressure.py":
        _fail("renderer", "producer command path differs")
    _hex(command["argv_sha256"], "renderer.producer_command.argv_sha256")
    expected_source = next(item for item in payload["source_files"] if item["path"] == command["path"])
    if _hex(command["sha256"], "renderer.producer_command.sha256") != expected_source["sha256"]:
        _fail("renderer", "producer command digest differs from source identity")

    environment = _closed(
        renderer["environment"],
        {
            "architecture",
            "locale",
            "os_name",
            "os_version",
            "resolved_inputs",
            "timezone",
        },
        "renderer.environment",
    )
    for key in ("architecture", "locale", "os_name", "os_version"):
        _text(environment[key], f"renderer.environment.{key}", 256)
    if environment["timezone"] != "UTC":
        _fail("environment", "producer timezone must be UTC")
    inputs = _list(environment["resolved_inputs"], "renderer.environment.resolved_inputs", 64)
    if len(inputs) < 5:
        _fail("environment", "renderer input inventory is incomplete")
    names: list[str] = []
    versions: dict[str, str] = {}
    for index, raw in enumerate(inputs):
        item = _closed(raw, {"kind", "name", "sha256", "version"}, f"resolved_inputs[{index}]")
        kind = _text(item["kind"], f"resolved_inputs[{index}].kind", 32)
        if kind not in {"runtime", "wheel", "native-library"}:
            _fail("environment", f"resolved_inputs[{index}].kind is unsupported")
        name = _text(item["name"], f"resolved_inputs[{index}].name", 128)
        version = _text(item["version"], f"resolved_inputs[{index}].version", 128)
        _hex(item["sha256"], f"resolved_inputs[{index}].sha256")
        names.append(name)
        versions[name] = version
    if names != sorted(names) or len(names) != len(set(names)):
        _fail("environment", "renderer input names must be unique and sorted")
    required = {"FreeType", "Python", "libpng", "matplotlib", "numpy"}
    if not required.issubset(versions):
        _fail("environment", "renderer input inventory lacks a required identity")
    if versions["Python"] != "3.13.14" or versions["matplotlib"] != "3.11.1":
        _fail(
            "environment",
            "Python or Matplotlib version differs from the frozen profile",
        )

    scene = _closed(
        payload["scene_profile"],
        {
            "bounds_m",
            "colorbar_label",
            "colormap",
            "constant_metadata",
            "dpi",
            "facecolor",
            "figure_inches",
            "format",
            "mesh_overlay",
            "no_crop_resize_reencode",
            "normalization",
            "plot",
            "shading",
            "title",
            "triangulation",
        },
        "scene_profile",
    )
    expected = {
        "bounds_m": {"x": [0.0, 2.2], "y": [0.0, 0.41]},
        "colorbar_label": "Pressure (Pa)",
        "colormap": "viridis",
        "constant_metadata": {"Software": PNG_SOFTWARE},
        "dpi": DPI,
        "facecolor": "white",
        "figure_inches": [8.0, 5.2],
        "format": "png",
        "mesh_overlay": True,
        "no_crop_resize_reencode": True,
        "normalization": "bound pressure output minimum/maximum",
        "plot": "tripcolor",
        "shading": "gouraud",
        "title": "Exact-cylinder steady Stokes pressure",
        "triangulation": "accepted Result mesh explicit triangle connectivity",
    }
    if scene != expected:
        _fail("scene-profile", "scene profile differs from the frozen renderer projection")


def _check_claim(payload: dict[str, Any], revision: str) -> None:
    claim = _closed(
        payload["claim"],
        {
            "case_dossier_routes",
            "evidence_route",
            "nonclaims",
            "pixels_are_validation",
            "public_claim",
        },
        "claim",
    )
    if claim["public_claim"] != PUBLIC_CLAIM or claim["nonclaims"] != NONCLAIMS:
        _fail("claim", "public claim or full nonclaim list differs")
    if claim["evidence_route"] != "/evidence/" or claim["pixels_are_validation"] is not False:
        _fail("claim", "evidence route or pixel boundary differs")
    routes = [_dossier_route(case_id, revision) for case_id in sorted(CASE_ROLES)]
    if claim["case_dossier_routes"] != routes:
        _fail("claim", "case dossier routes differ")


def _check_wrapper(wrapper: dict[str, Any], payload: dict[str, Any]) -> None:
    _closed(
        wrapper,
        {
            "admission",
            "entry_id",
            "publication_payload",
            "publication_payload_sha256",
            "schema",
            "source_revision",
            "source_tree",
        },
        "record",
    )
    if wrapper["schema"] != RECORD_SCHEMA or wrapper["entry_id"] != ENTRY_ID:
        _fail("record", "record schema or entry identity differs")
    expected_payload = _sha(_canonical_value(payload))
    if _hex(wrapper["publication_payload_sha256"], "publication_payload_sha256") != expected_payload:
        _fail("payload-digest", "publication payload digest differs")
    admission = _closed(wrapper["admission"], {"predicate", "receipt", "status"}, "admission")
    if admission["status"] != "accepted" or admission["predicate"] != PREDICATE_SCHEMA:
        _fail("admission", "admission status or predicate differs")
    receipt = _closed(admission["receipt"], {"id", "sha256"}, "admission.receipt")
    if receipt["id"] != RECEIPT_ID:
        _fail("admission", "receipt logical identity differs")
    _hex(receipt["sha256"], "admission.receipt.sha256")


def _component_hash(payload: dict[str, Any], key: str) -> str:
    return _sha(_canonical_value(payload[key]))


def _check_receipt(
    wrapper: dict[str, Any],
    payload: dict[str, Any],
    receipt: dict[str, Any],
    raw: bytes,
) -> None:
    _closed(
        receipt,
        {
            "alt_sha256",
            "caption_sha256",
            "checks",
            "claim_sha256",
            "environment_sha256",
            "lineage_sha256",
            "media_sha256",
            "nonchecks",
            "predicate",
            "publication_payload_sha256",
            "receipt_id",
            "renderer_sha256",
            "scene_profile_sha256",
            "schema",
            "source_revision",
            "status",
        },
        "receipt",
    )
    expected = {
        "schema": RECEIPT_SCHEMA,
        "receipt_id": RECEIPT_ID,
        "predicate": PREDICATE_SCHEMA,
        "status": "accepted",
        "source_revision": wrapper["source_revision"],
        "publication_payload_sha256": wrapper["publication_payload_sha256"],
        "media_sha256": payload["media"]["sha256"],
        "alt_sha256": payload["text"]["alt_sha256"],
        "caption_sha256": payload["text"]["caption_sha256"],
        "lineage_sha256": _component_hash(payload, "lineage"),
        "renderer_sha256": _component_hash(payload, "renderer"),
        "scene_profile_sha256": _component_hash(payload, "scene_profile"),
        "environment_sha256": _sha(_canonical_value(payload["renderer"]["environment"])),
        "claim_sha256": _component_hash(payload, "claim"),
        "checks": RECEIPT_CHECKS,
        "nonchecks": RECEIPT_NONCHECKS,
    }
    if receipt != expected:
        _fail("receipt", "receipt does not bind every frozen publication component")
    if _sha(raw) != wrapper["admission"]["receipt"]["sha256"]:
        _fail("receipt-digest", "external receipt digest differs from wrapper identity")


def check_publication(
    *,
    repository_root: Path,
    record_path: Path,
    media_path: Path,
    receipt_path: Path | None,
) -> dict[str, Any]:
    root = repository_root.resolve()
    if not root.is_dir():
        _fail("path", "repository root is not a directory")
    git_root = _git_object_repository(root)
    wrapper, _ = _load_canonical(record_path, "publication record")
    payload = wrapper.get("publication_payload")
    if type(payload) is not dict:
        _fail("shape", "publication_payload must be an object")
    _closed(
        payload,
        {
            "claim",
            "evidence_cases",
            "lineage",
            "media",
            "renderer",
            "scene_profile",
            "source_files",
            "text",
        },
        "publication_payload",
    )
    _check_wrapper(wrapper, payload)
    _check_source(wrapper, payload, git_root)
    _check_cases(payload, git_root, wrapper["source_revision"])
    _check_lineage(payload["lineage"])
    _check_text(payload, wrapper["source_revision"])
    _check_renderer(payload)
    _check_claim(payload, wrapper["source_revision"])
    _check_media(payload, media_path)
    if receipt_path is not None:
        receipt, raw_receipt = _load_canonical(receipt_path, "admission receipt")
        _check_receipt(wrapper, payload, receipt, raw_receipt)
    return {
        "entry_id": ENTRY_ID,
        "media_sha256": payload["media"]["sha256"],
        "mode": "verify-receipt" if receipt_path is not None else "verify-installed",
        "predicate": PREDICATE_SCHEMA,
        "publication_payload_sha256": wrapper["publication_payload_sha256"],
        "receipt_sha256": wrapper["admission"]["receipt"]["sha256"],
        "schema": RESULT_SCHEMA,
        "source_revision": wrapper["source_revision"],
        "status": "accepted",
    }


def check_elasticity_publication(
    *, repository_root: Path, record_path: Path, media_path: Path
) -> dict[str, Any]:
    """Validate the concrete second gallery consumer without a generic media schema."""

    root = repository_root.resolve()
    if not root.is_dir():
        _fail("path", "repository root is not a directory")
    git_root = _git_object_repository(root)
    wrapper, _ = _load_canonical(record_path, "elasticity publication record")
    _closed(
        wrapper,
        {
            "admission",
            "entry_id",
            "publication_payload",
            "publication_payload_sha256",
            "schema",
            "source_revision",
            "source_tree",
        },
        "elasticity record",
    )
    if wrapper["schema"] != RECORD_SCHEMA or wrapper["entry_id"] != ELASTICITY_ENTRY_ID:
        _fail("record", "elasticity record schema or entry identity differs")
    admission = _closed(
        wrapper["admission"], {"predicate", "status"}, "elasticity admission"
    )
    if admission != {
        "predicate": ELASTICITY_PREDICATE_SCHEMA,
        "status": "accepted",
    }:
        _fail("admission", "elasticity admission differs")

    payload = wrapper["publication_payload"]
    _closed(
        payload,
        {"claim", "evidence_cases", "lineage", "media", "renderer", "source_files", "text"},
        "elasticity publication payload",
    )
    if wrapper["publication_payload_sha256"] != _sha(_canonical_value(payload)):
        _fail("payload-digest", "elasticity publication payload digest differs")

    revision = _hex(wrapper["source_revision"], "source_revision", 40)
    tree = _hex(wrapper["source_tree"], "source_tree", 40)
    if _git(git_root, "rev-parse", "--verify", f"{revision}^{{commit}}").strip().decode() != revision:
        _fail("source-revision", "elasticity source revision does not resolve exactly")
    if _git(git_root, "rev-parse", f"{revision}^{{tree}}").strip().decode() != tree:
        _fail("source-tree", "elasticity source tree differs")
    ancestry = subprocess.run(
        [
            os.fspath(_GIT_EXECUTABLE),
            "--no-replace-objects",
            "-c",
            "core.fsmonitor=false",
            "-C",
            os.fspath(git_root),
            "merge-base",
            "--is-ancestor",
            revision,
            "HEAD",
        ],
        check=False,
        env=_GIT_ENVIRONMENT,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=30,
    )
    if ancestry.returncode != 0:
        _fail("source-revision", "elasticity source revision is not an ancestor of HEAD")

    sources = _list(payload["source_files"], "elasticity source_files", len(ELASTICITY_SOURCE_ROLES))
    observed_sources: dict[str, list[str]] = {}
    for index, raw_item in enumerate(sources):
        item = _closed(raw_item, {"path", "roles", "sha256"}, f"elasticity source_files[{index}]")
        path = _relative(item["path"], f"elasticity source_files[{index}].path")
        roles = _list(item["roles"], f"elasticity source_files[{index}].roles", 2)
        if not roles or any(type(role) is not str for role in roles):
            _fail("source-set", "elasticity source roles are invalid")
        if item["sha256"] != _sha(_source_blob(git_root, revision, path)):
            _fail("source-digest", f"elasticity source digest differs for {path}")
        observed_sources[path] = roles
    if list(observed_sources) != sorted(ELASTICITY_SOURCE_ROLES) or observed_sources != ELASTICITY_SOURCE_ROLES:
        _fail("source-set", "elasticity source paths or roles differ")

    cases = _list(payload["evidence_cases"], "elasticity evidence_cases", len(ELASTICITY_CASE_ROLES))
    observed_cases: dict[str, str] = {}
    for index, raw_item in enumerate(cases):
        item = _closed(
            raw_item,
            {"dossier_route", "id", "manifest_path", "manifest_sha256", "role"},
            f"elasticity evidence_cases[{index}]",
        )
        case_id = _text(item["id"], f"elasticity evidence_cases[{index}].id", 128)
        if case_id not in ELASTICITY_CASE_ROLES:
            _fail("case-set", f"unknown elasticity case {case_id}")
        manifest = _case_path(case_id)
        if item["manifest_path"] != manifest or item["role"] != ELASTICITY_CASE_ROLES[case_id]:
            _fail("case-set", f"elasticity case binding differs for {case_id}")
        if item["dossier_route"] != _dossier_route(case_id, revision):
            _fail("case-route", f"elasticity dossier route differs for {case_id}")
        if item["manifest_sha256"] != _sha(_source_blob(git_root, revision, manifest)):
            _fail("case-digest", f"elasticity manifest digest differs for {case_id}")
        observed_cases[case_id] = item["role"]
    if list(observed_cases) != sorted(ELASTICITY_CASE_ROLES) or observed_cases != ELASTICITY_CASE_ROLES:
        _fail("case-set", "elasticity case identities or order differ")

    lineage = _closed(
        payload["lineage"],
        {"field", "identities", "methods"},
        "elasticity lineage",
    )
    identities = _closed(
        lineage["identities"],
        {
            "correspondence_digest",
            "evidence_plan_key",
            "geometry_digest",
            "mesh_digest",
            "model_digest",
            "plan_identity",
            "result_plan_key",
        },
        "elasticity lineage identities",
    )
    for key, value in identities.items():
        _hex(value, f"elasticity lineage identities.{key}")
    if not (
        identities["plan_identity"]
        == identities["result_plan_key"]
        == identities["evidence_plan_key"]
    ):
        _fail("lineage", "elasticity Plan, Result, and observation identities differ")
    methods = {
        "displacement_output": "Result.output(Plan.field)",
        "evidence_plan_key": "solid.linear_elasticity_evidence(Result).plan_key",
        "model_digest": "Result.model_digest",
        "plan_identity": "Plan.identity",
        "result_plan_key": "Result.plan_key",
    }
    if lineage["methods"] != methods:
        _fail("lineage-method", "elasticity lineage method owners differ")
    field = _closed(
        lineage["field"],
        {"association", "components", "field", "model_digest", "source_unit", "vertex_count"},
        "elasticity displacement field",
    )
    if field != {
        "association": "vertex vector",
        "components": 2,
        "field": "displacement",
        "model_digest": identities["model_digest"],
        "source_unit": "m",
        "vertex_count": 289,
    }:
        _fail("lineage", "elasticity displacement projection differs")

    text_item = _closed(payload["text"], {"alt", "caption"}, "elasticity text")
    expected_caption = (
        "Reference and scale-1 deformed meshes for the bounded mixed-boundary "
        f"elasticity workflow at {revision}; presentation only, not validation."
    )
    if text_item != {"alt": ELASTICITY_ALT, "caption": expected_caption}:
        _fail("text", "elasticity alt text or caption differs")
    claim = _closed(
        payload["claim"],
        {"case_dossier_routes", "evidence_route", "nonclaims", "pixels_are_validation", "public_claim"},
        "elasticity claim",
    )
    if claim != {
        "case_dossier_routes": [_dossier_route(case_id, revision) for case_id in sorted(ELASTICITY_CASE_ROLES)],
        "evidence_route": "/evidence/",
        "nonclaims": ELASTICITY_NONCLAIMS,
        "pixels_are_validation": False,
        "public_claim": ELASTICITY_CLAIM,
    }:
        _fail("claim", "elasticity claim boundary differs")
    if payload["renderer"] != {
        "backend": "matplotlib/Agg",
        "displacement_scale": 1.0,
        "figure_source": "eqiora.matplotlib.plot_deformed_field",
    }:
        _fail("renderer", "elasticity renderer profile differs")

    media = _closed(
        payload["media"],
        {
            "bit_depth", "byte_size", "chunk_types", "color_type",
            "height", "interlace", "mime", "nonblank", "path", "pixel_mode",
            "sha256", "width",
        },
        "elasticity media",
    )
    if media["path"] != ELASTICITY_MEDIA or media["mime"] != "image/png" or media["nonblank"] is not True:
        _fail("media", "elasticity media path, MIME, or nonblank declaration differs")
    raw_media = _read_regular(media_path, MAX_MEDIA_BYTES, "elasticity media")
    if media["byte_size"] != len(raw_media) or media["sha256"] != _sha(raw_media):
        _fail("media-digest", "elasticity media byte size or digest differs")
    structure, _decoded, chunks = _decode_png(
        raw_media,
        width=ELASTICITY_WIDTH,
        height=ELASTICITY_HEIGHT,
        dpi=ELASTICITY_DPI,
        software=ELASTICITY_PNG_SOFTWARE,
    )
    for key, value in structure.items():
        if media[key] != value:
            _fail("png-record", f"elasticity PNG {key} differs")
    if media["chunk_types"] != chunks:
        _fail("png-record", "elasticity PNG chunks differ")
    return {
        "entry_id": ELASTICITY_ENTRY_ID,
        "media_sha256": media["sha256"],
        "mode": "verify-installed",
        "predicate": ELASTICITY_PREDICATE_SCHEMA,
        "publication_payload_sha256": wrapper["publication_payload_sha256"],
        "schema": RESULT_SCHEMA,
        "source_revision": revision,
        "status": "accepted",
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="mode", required=True)
    receipt = subparsers.add_parser("verify-receipt")
    receipt.add_argument("--repository-root", required=True, type=Path)
    receipt.add_argument("--record", required=True, type=Path)
    receipt.add_argument("--receipt", required=True, type=Path)
    receipt.add_argument("--media", required=True, type=Path)
    installed = subparsers.add_parser("verify-installed")
    installed.add_argument("--repository-root", required=True, type=Path)
    installed.add_argument("--record", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        root_input = Path(os.path.abspath(arguments.repository_root))
        root = root_input.resolve()
        record = Path(os.path.abspath(arguments.record))
        if arguments.mode == "verify-installed":
            records = {
                Path(os.path.abspath(root_input / FINAL_RECORD)): FINAL_MEDIA,
                Path(os.path.abspath(root_input / ELASTICITY_RECORD)): ELASTICITY_MEDIA,
            }
            if record not in records:
                _fail("path", "installed record is not an admitted concrete gallery profile")
            media = Path(os.path.abspath(root_input / records[record]))
            if record.name == Path(ELASTICITY_RECORD).name:
                result = check_elasticity_publication(
                    repository_root=root,
                    record_path=record,
                    media_path=media,
                )
            else:
                result = check_publication(
                    repository_root=root,
                    record_path=record,
                    media_path=media,
                    receipt_path=None,
                )
        else:
            result = check_publication(
                repository_root=root,
                record_path=record,
                media_path=Path(os.path.abspath(arguments.media)),
                receipt_path=Path(os.path.abspath(arguments.receipt)),
            )
    except AdmissionError as error:
        detail = str(error).encode("unicode_escape", "backslashreplace").decode("ascii")
        print(f"gallery publication check: {error.code}: {detail}", file=sys.stderr)
        return 1
    except (IndexError, KeyError, OverflowError, TypeError, ValueError):
        print(
            "gallery publication check: malformed-input: bounded input has an unsupported shape",
            file=sys.stderr,
        )
        return 1
    print(_canonical_value(result).decode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
