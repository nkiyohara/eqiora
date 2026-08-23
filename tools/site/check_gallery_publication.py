#!/usr/bin/env python3
"""Fail-closed admission check for the private exact-cylinder gallery record."""

from __future__ import annotations

import argparse
import binascii
import hashlib
import json
import math
import os
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

MAX_JSON_BYTES, MAX_MEDIA_BYTES = 512 * 1024, 16 * 1024 * 1024
WIDTH, HEIGHT, DPI = 1280, 832, 160
PNG_SOFTWARE = "Eqiora exact-cylinder gallery publication v1"

ALT_TEXT = (
    "Pressure in pascals for the frozen 2D steady-Stokes exact-cylinder "
    "demonstration, shown with a viridis color scale and the 1,210-triangle "
    "affine mesh overlaid. Presentation image only; linked Result evidence "
    "carries the numerical claim."
)
PUBLIC_CLAIM = (
    "one frozen 2D steady incompressible Stokes exact-cylinder demonstration on "
    "the accepted exact Gmsh CLI 4.15.2 witness: 662 vertices, 1,210 affine "
    "triangles, 114 boundary facets partitioned inlet/outlet/walls/cylinder = "
    "14/2/48/50, and 548 interior vertices; "
    "rendered from its accepted public Result path and linked evidence."
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
    "examples/steady-flow-past-cylinder.model.json": ["model", "scientific-formula"],
    "packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi": ["current-package-formula-owner"],
    "tools/site/produce_exact_cylinder_pressure.py": ["producer-command"],
    "verify/fluid/packaged-steady-stokes-2d/models/direct.eqi": ["packaged-stokes-formula"],
    "verify/fluid/packaged-steady-stokes-2d/package-v0.1.0/src/incompressible.eqi": ["package-formula-owner"],
}

CASE_ROLES = {
    "artifacts.current-model-canonical-identity": "evidence",
    "fluid.exact-circular-hole-stokes-2d-gmsh": "evidence",
    "fluid.packaged-steady-stokes-2d": "evidence",
    "geometry.exact-circular-hole-geometry": "evidence",
    "interfaces.python-circular-hole-chordal-mesh": "evidence",
    "interfaces.python-exact-circular-hole-geometry": "evidence",
    "interfaces.python-exact-cylinder-pressure-still": "presentation-only",
    "interfaces.python-exact-cylinder-stokes-marimo": "evidence",
    "interfaces.python-exact-cylinder-stokes-result": "evidence",
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
            ["git", "-C", os.fspath(root), *arguments],
            check=False,
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
                "git",
                "-C",
                os.fspath(root),
                "merge-base",
                "--is-ancestor",
                revision,
                "HEAD",
            ],
            check=True,
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
        if case_id == "interfaces.python-exact-cylinder-pressure-still":
            boundary = document.get("claim_boundary", {})
            if type(boundary) is not dict:
                _fail("case-manifest", "pressure-still claim_boundary must be a table")
            for key in (
                "media_admission",
                "durable_image_provenance",
                "reproducible_image_bytes",
                "exact_pixels_or_dimensions",
                "scientific_validation_from_pixels",
            ):
                if boundary.get(key) is not False:
                    _fail(
                        "presentation-boundary",
                        f"pressure-still {key} must remain false",
                    )
        if case_id in observed:
            _fail("case-set", f"duplicate case {case_id}")
        observed[case_id] = role
        manifests.append(manifest)
    if manifests != sorted(manifests) or observed != CASE_ROLES:
        _fail("case-set", "case identities, roles, or order differ from the predicate")


def _check_lineage(lineage: Any) -> None:
    item = _closed(
        lineage,
        {"chain", "identities", "methods", "pressure", "source_result"},
        "lineage",
    )
    identities = _closed(
        item["identities"],
        {
            "correspondence_digest",
            "evidence_run_digest",
            "geometry_digest",
            "mesh_digest",
            "model_digest",
            "realization_digest",
            "run_manifest_digest",
        },
        "lineage.identities",
    )
    for key, value in identities.items():
        _hex(value, f"lineage.identities.{key}")
    methods = _closed(item["methods"], set(LINEAGE_METHODS), "lineage.methods")
    if methods != LINEAGE_METHODS:
        _fail("lineage-method", "lineage methods differ from the accepted public Result owners")
    if identities["evidence_run_digest"] != identities["run_manifest_digest"]:
        _fail("lineage", "ResultEvidence Run digest does not equal the Run manifest digest")

    source_result = _closed(item["source_result"], {"digest_kind", "digest"}, "lineage.source_result")
    if source_result["digest_kind"] != "Result.run_manifest().digest":
        _fail("lineage", "source Result must use Result.run_manifest().digest")
    if source_result["digest"] != identities["run_manifest_digest"]:
        _fail("lineage", "source Result digest does not bind run_manifest_digest")

    pressure = _closed(
        item["pressure"],
        {
            "association",
            "display_unit",
            "field",
            "frame_selection",
            "mesh_digest",
            "model_digest",
            "ordered_block_digests",
            "ordered_output_digests",
            "snapshot_digest",
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
        _fail("lineage", "pressure snapshot mesh does not bind lineage mesh")
    if pressure["model_digest"] != identities["model_digest"]:
        _fail("lineage", "pressure FieldRef does not bind the Result Model")
    _hex(pressure["snapshot_digest"], "lineage.pressure.snapshot_digest")
    for key in ("ordered_block_digests", "ordered_output_digests"):
        values = _list(pressure[key], f"lineage.pressure.{key}", 256)
        if not values:
            _fail("lineage", f"lineage.pressure.{key} must not be empty")
        for index, digest in enumerate(values):
            _hex(digest, f"lineage.pressure.{key}[{index}]")
    if pressure["ordered_output_digests"] != [pressure["snapshot_digest"]]:
        _fail("lineage", "Run manifest output order does not contain the one pressure snapshot")
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
            "to": pressure["snapshot_digest"],
        },
    ]
    if item["chain"] != expected_chain:
        _fail("lineage", "Model→Geometry→Mesh→Realization→Run→Result/pressure chain differs")


def _paeth(left: int, above: int, upper_left: int) -> int:
    estimate = left + above - upper_left
    dl = abs(estimate - left)
    da = abs(estimate - above)
    dul = abs(estimate - upper_left)
    return left if dl <= da and dl <= dul else above if da <= dul else upper_left


def _decode_png(raw: bytes) -> tuple[dict[str, Any], bytes, list[str]]:
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
    if text_data != b"Software\0" + PNG_SOFTWARE.encode("ascii"):
        _fail("png", "PNG Software metadata differs")
    phys = next(data for kind, data in chunks if kind == "pHYs")
    pixels_per_metre = round(DPI / 0.0254)
    if phys != struct.pack(">IIB", pixels_per_metre, pixels_per_metre, 1):
        _fail("png", "PNG physical resolution differs from fixed DPI")

    header = chunks[0][1]
    if len(header) != 13:
        _fail("png", "PNG IHDR length differs")
    width, height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", header)
    if (width, height, depth, color, compression, filtering, interlace) != (
        WIDTH,
        HEIGHT,
        8,
        6,
        0,
        0,
        0,
    ):
        _fail("png", "PNG dimensions or RGBA encoding profile differs")
    compressed = b"".join(data for kind, data in chunks if kind == "IDAT")
    expected_size = HEIGHT * (1 + WIDTH * 4)
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

    row_size = WIDTH * 4
    previous = bytearray(row_size)
    decoded = bytearray()
    cursor = 0
    visible_colors: set[bytes] = set()
    for _ in range(HEIGHT):
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
            "height": height,
            "interlace": interlace,
            "pixel_mode": "RGBA",
            "width": width,
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
    expected_links = [
        {
            "case_id": "interfaces.python-exact-cylinder-stokes-result",
            "label": "Result evidence",
            "route": _dossier_route("interfaces.python-exact-cylinder-stokes-result", revision),
        },
        {
            "case_id": "interfaces.python-exact-cylinder-pressure-still",
            "label": "Pressure-still presentation case",
            "route": _dossier_route("interfaces.python-exact-cylinder-pressure-still", revision),
        },
    ]
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
        "normalization": "bound pressure snapshot minimum/maximum",
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
    _check_source(wrapper, payload, root)
    _check_cases(payload, root, wrapper["source_revision"])
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
            expected_record = Path(os.path.abspath(root_input / FINAL_RECORD))
            if record != expected_record:
                _fail("path", f"installed record must be {FINAL_RECORD}")
            result = check_publication(
                repository_root=root,
                record_path=record,
                media_path=Path(os.path.abspath(root_input / FINAL_MEDIA)),
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
