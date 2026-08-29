#!/usr/bin/env python3
"""Validate one complete Eqiora Python candidate artifact set.

The manifest is the authority for filenames and hashes. This checker is used
both after an Actions artifact download and immediately before production
publication; it never rebuilds a candidate.
"""

from __future__ import annotations

import argparse
import email.parser
import hashlib
import json
import re
import sys
import tarfile
import tomllib
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from python_candidate_common import CandidateError, python_distribution_version


MANIFEST_FORMAT = "eqiora.python-distribution-candidate/v2"
V3_MANIFEST_FORMAT = "eqiora.python-distribution-candidate/v3"
FULL_SHA = re.compile(r"[0-9a-f]{40}")
SHA256 = re.compile(r"[0-9a-f]{64}")
SHA512_SRI = re.compile(r"sha512-[A-Za-z0-9+/]{86}==")
V2_REQUIRED_PROFILES = ("base", "jax", "matplotlib", "torch", "typing")
REQUIRED_PROFILES = ("base", "jax", "matplotlib", "notebook", "torch", "typing")

CONTRACT_SHA256 = "3f3a9f1a5b54bf5b874d996c8807bbb7e88439737fd245d69e7a8aeb7a1a87c1"
PROTECTED_BASE_SHA = "3dfb1086168afc6f9fb61f9ca43d21ca9953048b"
NODE_EXECUTABLE_SHA256 = "f3432a45b03b2da0d270095fdd8813dc34cbea73f5fc8b18c7a384b7cf9b333a"
NPM_PACKAGE_INTEGRITY = (
    "sha512-A74XL8OxmcegZDMWPkWb5bEQppg8HdYwW3rBD2sPoS4UQHVajfaxBkqyzLeJ3wR0kZ+"
    "5xoTjItxXaF7eIXUsyw=="
)
BROWSERS_JSON_SHA256 = "f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99"
PACKUMENT_INSTALL_SCRIPT_ADMISSIONS = (
    (
        "node_modules/fsevents",
        "fsevents",
        "2.3.2",
        "install",
        "node-gyp rebuild",
        "packument",
    ),
    (
        "node_modules/vite/node_modules/fsevents",
        "fsevents",
        "2.3.3",
        "install",
        "node-gyp rebuild",
        "packument",
    ),
)
INSTALL_CLASS_LIFECYCLE_NAMES = frozenset(
    {"preinstall", "install", "postinstall"}
)
LIFECYCLE_SOURCES = frozenset({"lockfile", "packument", "tarball"})
NOTEBOOK_CHECKS = frozenset(
    {
        "frontend:lock-integrity",
        "frontend:dependency-inventory",
        "cp313:marimo-0.23.16-exact-cylinder-stokes",
        "cp313:marimo-0.23.16-shared-semantic-viewer",
        "cp313:notebook-managed-chromium-r1234",
        "cp313:notebook-no-external-network",
        "cp313:notebook-cleanup-and-mutation",
    }
)
FAMILY_MEMBER_BYTES_LIMIT = 16_777_216
FAMILY_TOTAL_BYTES_LIMIT = 67_108_864


def _base_checks() -> frozenset[str]:
    checks = {
        "generated-public-api",
        "sdist-to-wheel-rebuild",
        "twine-strict",
        "cp312:numpy-2.1.0-floor",
    }
    for python in ("311", "312", "313", "314"):
        checks.update(
            {
                f"cp{python}:installed-wheel",
                f"cp{python}:base-and-numpy",
                f"cp{python}:packaged-mixed-boundary-elasticity-demo",
                f"cp{python}:async-and-cancellation",
                f"cp{python}:public-smoke-base",
                f"cp{python}:matplotlib-free-base",
            }
        )
    return frozenset(checks)


PROFILE_CHECKS = {
    "base": _base_checks(),
    "jax": frozenset({"cp313:jax", "cp313:public-smoke-jax"}),
    "matplotlib": frozenset(
        {
            "cp313:matplotlib",
            "cp313:packaged-exact-cylinder-pressure-demo",
            "cp313:packaged-mixed-boundary-displacement-demo",
        }
    ),
    "notebook": NOTEBOOK_CHECKS,
    "torch": frozenset({"cp313:torch", "cp313:public-smoke-torch"}),
    "typing": frozenset(
        {
            "cp311:strict-base-typing",
            "cp312:strict-base-typing",
            "cp313:strict-base-typing",
            "cp314:strict-base-typing",
            "cp313:complete-public-typing",
        }
    ),
}


class ManifestError(RuntimeError):
    """The retained candidate is malformed or differs from its manifest."""


@dataclass(frozen=True)
class Artifact:
    filename: str
    kind: str
    size: int
    sha256: str
    python: str | None


@dataclass(frozen=True)
class Candidate:
    version: str
    commit: str
    expected_tag: str
    artifacts: tuple[Artifact, ...]
    checks: frozenset[str]


def require_candidate_profile(candidate: Candidate, profile: str) -> None:
    """Require one closed profile from an already parsed candidate manifest."""

    try:
        required = PROFILE_CHECKS[profile]
    except KeyError as error:
        raise ManifestError(f"unknown candidate profile {profile!r}") from error
    missing = sorted(required - candidate.checks)
    if missing:
        raise ManifestError(
            f"candidate {profile} profile omits required check {missing[0]!r}"
        )


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _object(value: Any, location: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ManifestError(f"{location} must be an object")
    return value


def _text(value: Any, location: str) -> str:
    if not isinstance(value, str) or not value:
        raise ManifestError(f"{location} must be a nonempty string")
    return value


def _integer(value: Any, location: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ManifestError(f"{location} must be a nonnegative integer")
    return value


def load_candidate(path: Path) -> Candidate:
    """Read and validate the release-critical subset of one candidate manifest."""
    try:
        document = _object(json.loads(path.read_text(encoding="utf-8")), "manifest")
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ManifestError(f"cannot read candidate manifest: {error}") from error

    if document.get("format") != MANIFEST_FORMAT:
        raise ManifestError("candidate manifest has an unsupported format")
    if document.get("project") != "eqiora":
        raise ManifestError("candidate manifest project is not eqiora")
    if document.get("acceptance") != "complete":
        raise ManifestError("only a complete candidate may be published")

    version = _text(document.get("version"), "version")
    if version != "0.1.0a1":
        raise ManifestError("first public candidate version must be 0.1.0a1")

    source = _object(document.get("source"), "source")
    commit = _text(source.get("commit"), "source.commit")
    if FULL_SHA.fullmatch(commit) is None:
        raise ManifestError("source.commit must be a full lowercase SHA")
    if source.get("tree") != "clean":
        raise ManifestError("candidate source tree was not recorded as clean")
    expected_tag = _text(source.get("expected_tag"), "source.expected_tag")
    if expected_tag != f"v{version}":
        raise ManifestError("candidate expected tag does not match its version")
    tags = source.get("tags")
    if not isinstance(tags, list) or any(not isinstance(tag, str) for tag in tags):
        raise ManifestError("source.tags must be a string array")

    build = _object(document.get("build"), "build")
    if build.get("sdist_rebuilt") is not True:
        raise ManifestError("candidate wheels were not recorded as sdist rebuilds")
    family = _object(build.get("wheel_family"), "build.wheel_family")
    if family.get("implementation") != "CPython":
        raise ManifestError("candidate wheel implementation is not CPython")
    if family.get("ordinary_gil") is not True or family.get("abi3") is not False:
        raise ManifestError("candidate wheel ABI contract drifted")
    if family.get("versions") != ["3.11", "3.12", "3.13", "3.14"]:
        raise ManifestError("candidate CPython matrix drifted")
    if family.get("platform") != "manylinux_2_17_x86_64":
        raise ManifestError("candidate platform contract drifted")
    dependency_profiles = _object(
        build.get("dependency_profiles"), "build.dependency_profiles"
    )
    numpy_floor = _object(
        dependency_profiles.get("numpy_floor"),
        "build.dependency_profiles.numpy_floor",
    )
    expected_numpy_floor = {
        "python": "3.12",
        "requirement": "numpy==2.1.0",
        "observed": "2.1.0",
        "profile": "cp312:numpy-2.1.0-floor",
    }
    if numpy_floor != expected_numpy_floor:
        raise ManifestError("candidate NumPy floor profile drifted")

    raw_artifacts = document.get("artifacts")
    if not isinstance(raw_artifacts, list):
        raise ManifestError("artifacts must be an array")
    artifacts: list[Artifact] = []
    filenames: set[str] = set()
    for index, raw in enumerate(raw_artifacts):
        entry = _object(raw, f"artifacts[{index}]")
        filename = _text(entry.get("filename"), f"artifacts[{index}].filename")
        if (
            Path(filename).name != filename
            or filename in {".", ".."}
            or "/" in filename
            or "\\" in filename
            or "\0" in filename
        ):
            raise ManifestError(f"unsafe artifact filename {filename!r}")
        if filename in filenames:
            raise ManifestError(f"duplicate artifact filename {filename!r}")
        filenames.add(filename)
        kind = _text(entry.get("kind"), f"artifacts[{index}].kind")
        if kind not in {"sdist", "wheel"}:
            raise ManifestError(f"unsupported artifact kind {kind!r}")
        size = _integer(entry.get("size"), f"artifacts[{index}].size")
        digest = _text(entry.get("sha256"), f"artifacts[{index}].sha256")
        if SHA256.fullmatch(digest) is None:
            raise ManifestError(f"invalid SHA-256 for {filename}")
        python = entry.get("python")
        if kind == "wheel":
            python = _text(python, f"artifacts[{index}].python")
            compact = python.replace(".", "")
            if (
                not filename.startswith(f"eqiora-{version}-")
                or f"-cp{compact}-cp{compact}-" not in filename
                or "manylinux_2_17_x86_64" not in filename
                or not filename.endswith(".whl")
            ):
                raise ManifestError(
                    f"wheel filename does not match its declared interpreter: {filename}"
                )
        elif python is not None:
            raise ManifestError("sdist must not declare a Python interpreter")
        artifacts.append(Artifact(filename, kind, size, digest, python))

    sdists = [artifact for artifact in artifacts if artifact.kind == "sdist"]
    wheels = [artifact for artifact in artifacts if artifact.kind == "wheel"]
    if len(sdists) != 1 or sdists[0].filename != f"eqiora-{version}.tar.gz":
        raise ManifestError("candidate must contain the exact normalized sdist")
    if sorted(artifact.python for artifact in wheels) != [
        "3.11",
        "3.12",
        "3.13",
        "3.14",
    ]:
        raise ManifestError("candidate must contain one wheel per supported CPython")

    raw_checks = document.get("checks")
    if not isinstance(raw_checks, list) or not all(
        isinstance(check, str) and check for check in raw_checks
    ):
        raise ManifestError("checks must be a nonempty string array")
    if len(raw_checks) != len(set(raw_checks)):
        raise ManifestError("checks must not contain duplicates")
    candidate = Candidate(
        version,
        commit,
        expected_tag,
        tuple(artifacts),
        frozenset(raw_checks),
    )
    for profile in V2_REQUIRED_PROFILES:
        require_candidate_profile(candidate, profile)

    return candidate


def _canonical_json_bytes(value: Any) -> bytes:
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ManifestError(f"value has no canonical JSON encoding: {error}") from error


def _structured_sha256(value: Any) -> str:
    return hashlib.sha256(_canonical_json_bytes(value)).hexdigest()


def _exact_keys(value: Any, keys: set[str], location: str) -> dict[str, Any]:
    item = _object(value, location)
    if set(item) != keys:
        raise ManifestError(f"{location} schema has an unexpected key or missing member")
    return item


def _array(value: Any, location: str) -> list[Any]:
    if not isinstance(value, list):
        raise ManifestError(f"{location} must be an array")
    return value


def _boolean(value: Any, location: str) -> bool:
    if not isinstance(value, bool):
        raise ManifestError(f"{location} must be a boolean")
    return value


def _positive_integer(value: Any, location: str) -> int:
    result = _integer(value, location)
    if result == 0:
        raise ManifestError(f"{location} must be a positive integer")
    return result


def _sha(value: Any, location: str) -> str:
    result = _text(value, location)
    if SHA256.fullmatch(result) is None:
        raise ManifestError(f"{location} must be a lowercase SHA-256")
    return result


def _git_sha(value: Any, location: str) -> str:
    result = _text(value, location)
    if FULL_SHA.fullmatch(result) is None:
        raise ManifestError(f"{location} must be a full lowercase Git SHA")
    return result


def _basename(value: Any, location: str) -> str:
    result = _text(value, location)
    if (
        result in {".", ".."}
        or "/" in result
        or "\\" in result
        or "\0" in result
        or Path(result).name != result
    ):
        raise ManifestError(f"{location} must be a safe basename")
    return result


def _relative_path(value: Any, location: str) -> str:
    result = _text(value, location)
    parts = result.split("/")
    if (
        result.startswith("/")
        or "\\" in result
        or "\0" in result
        or any(part in {"", ".", ".."} for part in parts)
    ):
        raise ManifestError(f"{location} must be a safe relative path")
    return result


def _sorted_unique(
    values: list[Any], key: Any, location: str
) -> None:
    identities = [key(value) for value in values]
    if identities != sorted(identities) or len(identities) != len(set(identities)):
        raise ManifestError(f"{location} must be canonically sorted without duplicates")


def _safe_archive_name(name: str, location: str) -> str:
    if name.endswith("/"):
        name = name[:-1]
    return _relative_path(name, location)


@dataclass(frozen=True)
class _FamilyScan:
    sdist_members: dict[str, bytes]
    wheel_members: tuple[dict[str, bytes], ...]
    wheel_metadata: tuple[bytes, ...]
    activated: bool


def _scan_family(document: dict[str, Any], artifacts: Path) -> _FamilyScan:
    records = document.get("artifacts")
    if not isinstance(records, list):
        raise ManifestError("artifacts must be an array before schema selection")
    sdist_members: dict[str, bytes] = {}
    wheel_members: list[dict[str, bytes]] = []
    wheel_metadata: list[bytes] = []
    activated = False
    total_bytes = 0
    for index, raw in enumerate(records):
        record = _object(raw, f"artifacts[{index}]")
        filename = _basename(record.get("filename"), f"artifacts[{index}].filename")
        path = artifacts / filename
        if (
            path.is_symlink()
            or not path.is_file()
            or path.stat().st_nlink != 1
            or path.stat().st_size <= 0
            or path.stat().st_size > FAMILY_MEMBER_BYTES_LIMIT
        ):
            raise ManifestError(f"candidate family member is not one bounded regular file: {filename}")
        total_bytes += path.stat().st_size
        if total_bytes > FAMILY_TOTAL_BYTES_LIMIT:
            raise ManifestError("candidate family exceeds the raw byte ceiling")
        kind = record.get("kind")
        if kind == "sdist":
            try:
                with tarfile.open(path, mode="r:*") as archive:
                    for member in archive.getmembers():
                        name = _safe_archive_name(member.name, "sdist member")
                        if member.isdir():
                            continue
                        if not member.isfile():
                            raise ManifestError("sdist has an unsafe non-regular member")
                        source = archive.extractfile(member)
                        if source is None:
                            raise ManifestError("sdist regular member cannot be read")
                        payload = source.read()
                        if name in sdist_members:
                            raise ManifestError("sdist has duplicate member names")
                        sdist_members[name] = payload
            except (OSError, tarfile.TarError) as error:
                raise ManifestError(f"cannot safely scan sdist: {error}") from error
        elif kind == "wheel":
            members: dict[str, bytes] = {}
            try:
                with zipfile.ZipFile(path) as archive:
                    for member in archive.infolist():
                        name = _safe_archive_name(member.filename, "wheel member")
                        if member.is_dir():
                            continue
                        mode = member.external_attr >> 16
                        if mode and (mode & 0o170000) not in {0, 0o100000}:
                            raise ManifestError("wheel has an unsafe non-regular member")
                        if name in members:
                            raise ManifestError("wheel has duplicate member names")
                        members[name] = archive.read(member)
            except (OSError, zipfile.BadZipFile) as error:
                raise ManifestError(f"cannot safely scan wheel: {error}") from error
            metadata = [
                payload for name, payload in members.items() if name.endswith(".dist-info/METADATA")
            ]
            if len(metadata) != 1:
                raise ManifestError("wheel has an ambiguous METADATA member")
            wheel_members.append(members)
            wheel_metadata.append(metadata[0])
    if not sdist_members or len(wheel_members) != 4:
        raise ManifestError("candidate family must contain one sdist and four wheels")
    for name, payload in sdist_members.items():
        relative = name.split("/", maxsplit=1)[1] if "/" in name else name
        if (
            relative.startswith("bindings/python/frontend/")
        ):
            activated = True
    return _FamilyScan(sdist_members, tuple(wheel_members), tuple(wheel_metadata), activated)


def _retained_distribution_identity(scan: _FamilyScan) -> tuple[str, str]:
    roots = {name.split("/", maxsplit=1)[0] for name in scan.sdist_members}
    if len(roots) != 1:
        raise ManifestError("sdist has an ambiguous source root")
    root = roots.pop()

    def member(relative: str) -> bytes:
        try:
            return scan.sdist_members[f"{root}/{relative}"]
        except KeyError as error:
            raise ManifestError(
                f"v3 sdist omits retained identity member {relative}"
            ) from error

    try:
        cargo = tomllib.loads(member("Cargo.toml").decode("utf-8"))
        pyproject = tomllib.loads(member("pyproject.toml").decode("utf-8"))
        package = json.loads(member("bindings/python/frontend/package.json"))
        lock = json.loads(member("bindings/python/frontend/package-lock.json"))
    except (UnicodeDecodeError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        raise ManifestError(
            f"retained source package/lock identity cannot be parsed: {error}"
        ) from error

    workspace = cargo.get("workspace")
    workspace_package = workspace.get("package") if isinstance(workspace, dict) else None
    raw_version = (
        workspace_package.get("version")
        if isinstance(workspace_package, dict)
        else None
    )
    if not isinstance(raw_version, str) or not raw_version:
        raise ManifestError("retained Cargo version is not one nonempty string")

    project = pyproject.get("project")
    if not isinstance(project, dict):
        raise ManifestError("retained Python project table is unavailable")
    if "version" in project or project.get("dynamic") != ["version"]:
        raise ManifestError("retained Python version must be exactly dynamic")

    mirrors: tuple[tuple[str, object], ...] = (
        ("package.json.version", package.get("version") if isinstance(package, dict) else None),
        (
            "package-lock.json.version",
            lock.get("version") if isinstance(lock, dict) else None,
        ),
        (
            'package-lock.json.packages[""].version',
            (
                lock.get("packages", {}).get("", {}).get("version")
                if isinstance(lock, dict)
                and isinstance(lock.get("packages"), dict)
                and isinstance(lock["packages"].get(""), dict)
                else None
            ),
        ),
    )
    for location, value in mirrors:
        if not isinstance(value, str) or not value:
            raise ManifestError(f"retained frontend {location} is not one nonempty string")
        if value != raw_version:
            raise ManifestError(f"retained frontend {location} differs from raw Cargo")
    try:
        return raw_version, python_distribution_version(raw_version)
    except CandidateError as error:
        raise ManifestError(str(error)) from error


def _candidate_v3(document: dict[str, Any], scan: _FamilyScan) -> Candidate:
    if document.get("project") != "eqiora" or document.get("acceptance") != "complete":
        raise ManifestError("only a complete eqiora v3 candidate is supported")
    _raw_version, retained_version = _retained_distribution_identity(scan)
    version = _text(document.get("version"), "version")
    if version != retained_version:
        raise ManifestError("candidate version differs from retained Cargo authority")
    source = _object(document.get("source"), "source")
    commit = _git_sha(source.get("commit"), "source.commit")
    if source.get("tree") != "clean" or source.get("expected_tag") != f"v{version}":
        raise ManifestError("candidate source identity drifted")
    build = _object(document.get("build"), "build")
    if build.get("sdist_rebuilt") is not True:
        raise ManifestError("candidate wheels were not recorded as sdist rebuilds")
    family = _object(build.get("wheel_family"), "build.wheel_family")
    if family != {
        "implementation": "CPython",
        "ordinary_gil": True,
        "versions": ["3.11", "3.12", "3.13", "3.14"],
        "platform": "manylinux_2_17_x86_64",
        "abi3": False,
    }:
        raise ManifestError("candidate wheel family drifted")
    numpy_floor = _object(build.get("dependency_profiles"), "build.dependency_profiles").get("numpy_floor")
    if numpy_floor != {
        "python": "3.12",
        "requirement": "numpy==2.1.0",
        "observed": "2.1.0",
        "profile": "cp312:numpy-2.1.0-floor",
    }:
        raise ManifestError("candidate NumPy floor profile drifted")
    artifacts: list[Artifact] = []
    for index, raw in enumerate(_array(document.get("artifacts"), "artifacts")):
        entry = _object(raw, f"artifacts[{index}]")
        filename = _basename(entry.get("filename"), f"artifacts[{index}].filename")
        kind = _text(entry.get("kind"), f"artifacts[{index}].kind")
        size = _positive_integer(entry.get("size"), f"artifacts[{index}].size")
        digest = _sha(entry.get("sha256"), f"artifacts[{index}].sha256")
        python = entry.get("python")
        if kind == "wheel":
            python = _text(python, f"artifacts[{index}].python")
            compact = python.replace(".", "")
            if (
                entry.get("abi") != f"cp{compact}"
                or entry.get("platform") != "manylinux_2_17_x86_64"
            ):
                raise ManifestError("candidate wheel ABI/platform record drifted")
        elif kind == "sdist" and python is not None:
            raise ManifestError("sdist must not declare a Python interpreter")
        elif kind != "sdist":
            raise ManifestError("candidate has an unsupported artifact kind")
        artifacts.append(Artifact(filename, kind, size, digest, python))
    sdists = [artifact for artifact in artifacts if artifact.kind == "sdist"]
    wheels = [artifact for artifact in artifacts if artifact.kind == "wheel"]
    if (
        len(artifacts) != 5
        or len(sdists) != 1
        or sdists[0].filename != f"eqiora-{version}.tar.gz"
        or sorted(artifact.python for artifact in wheels)
        != ["3.11", "3.12", "3.13", "3.14"]
    ):
        raise ManifestError("candidate artifact family drifted")
    by_python = {artifact.python: artifact for artifact in wheels}
    for python in ("3.11", "3.12", "3.13", "3.14"):
        compact = python.replace(".", "")
        artifact = by_python[python]
        expected_name = (
            f"eqiora-{version}-cp{compact}-cp{compact}-"
            "manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
        )
        if artifact.filename != expected_name:
            raise ManifestError("candidate wheel filename/family drifted")

    expected_metadata_member = f"eqiora-{version}.dist-info/METADATA"
    for members, payload in zip(scan.wheel_members, scan.wheel_metadata, strict=True):
        metadata_members = [name for name in members if name.endswith(".dist-info/METADATA")]
        if metadata_members != [expected_metadata_member]:
            raise ManifestError("wheel METADATA ownership differs from candidate version")
        metadata = email.parser.BytesParser().parsebytes(payload)
        if metadata.get("Name") != "eqiora" or metadata.get("Version") != version:
            raise ManifestError("wheel metadata version differs from retained Cargo authority")
    checks_raw = _array(document.get("checks"), "checks")
    if any(not isinstance(value, str) or not value for value in checks_raw) or len(checks_raw) != len(set(checks_raw)):
        raise ManifestError("checks must be unique nonempty strings")
    candidate = Candidate(version, commit, f"v{version}", tuple(artifacts), frozenset(checks_raw))
    for profile in V2_REQUIRED_PROFILES:
        require_candidate_profile(candidate, profile)
    return candidate


def _validate_frontend(value: Any) -> dict[str, Any]:
    keys = {
        "node", "npm", "h2_receipt_sha256", "package_json_sha256",
        "package_lock_sha256", "source_inventory_sha256",
        "config_inventory_sha256", "locked_packages_sha256",
        "install_script_inventory_sha256", "node_executable_sha256",
        "npm_package_integrity", "runtime", "browser",
    }
    frontend = _exact_keys(value, keys, "build.frontend")
    fixed = {
        "node": "v24.18.1",
        "npm": "11.16.0",
        "node_executable_sha256": NODE_EXECUTABLE_SHA256,
        "npm_package_integrity": NPM_PACKAGE_INTEGRITY,
    }
    for name, expected in fixed.items():
        if frontend.get(name) != expected:
            raise ManifestError(f"build.frontend.{name} drifted")
    for name in (
        "h2_receipt_sha256", "package_json_sha256", "package_lock_sha256",
        "source_inventory_sha256", "config_inventory_sha256",
        "locked_packages_sha256", "install_script_inventory_sha256",
    ):
        _sha(frontend.get(name), f"build.frontend.{name}")
    runtime = _exact_keys(
        frontend.get("runtime"),
        {"python", "marimo", "resolved_environment_sha256"},
        "build.frontend.runtime",
    )
    expected_runtime = {
        "python": "3.13", "marimo": "0.23.16",
    }
    for name, expected in expected_runtime.items():
        if runtime.get(name) != expected:
            raise ManifestError(f"Notebook runtime identity drifted: {name}")
    _sha(runtime.get("resolved_environment_sha256"), "runtime.resolved_environment_sha256")
    browser = _exact_keys(
        frontend.get("browser"),
        {"playwright", "chromium_revision", "browser_version", "browsers_json_sha256", "platform", "downloaded_archive_sha256", "executable_sha256"},
        "build.frontend.browser",
    )
    expected_browser = {
        "playwright": "1.62.1", "chromium_revision": "1234",
        "browser_version": "151.0.7922.34", "browsers_json_sha256": BROWSERS_JSON_SHA256,
    }
    for name, expected in expected_browser.items():
        if browser.get(name) != expected:
            raise ManifestError(f"Notebook browser identity drifted: {name}")
    _text(browser.get("platform"), "browser.platform")
    _sha(browser.get("downloaded_archive_sha256"), "browser.downloaded_archive_sha256")
    _sha(browser.get("executable_sha256"), "browser.executable_sha256")
    return frontend


def _file_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(value, {"relative_path", "mode", "size", "sha256"}, location)
    _relative_path(record["relative_path"], f"{location}.relative_path")
    _integer(record["mode"], f"{location}.mode")
    _integer(record["size"], f"{location}.size")
    _sha(record["sha256"], f"{location}.sha256")
    return record


def _config_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(value, {"relative_path", "sha256"}, location)
    _relative_path(record["relative_path"], f"{location}.relative_path")
    _sha(record["sha256"], f"{location}.sha256")
    return record


def _artifact_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(value, {"filename", "kind", "size", "sha256"}, location)
    _basename(record["filename"], f"{location}.filename")
    if record["kind"] not in {"sdist", "wheel"}:
        raise ManifestError(f"{location}.kind is invalid")
    _positive_integer(record["size"], f"{location}.size")
    _sha(record["sha256"], f"{location}.sha256")
    return record


def _pin_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(value, {"name", "version"}, location)
    _text(record["name"], f"{location}.name")
    _text(record["version"], f"{location}.version")
    return record


def _script_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(value, {"name", "command", "sources"}, location)
    _text(record["name"], f"{location}.name")
    _text(record["command"], f"{location}.command")
    sources = _string_array(record["sources"], f"{location}.sources")
    if not sources or not set(sources).issubset(LIFECYCLE_SOURCES):
        raise ManifestError(f"{location}.sources contains an unsupported authority")
    return record


def _validate_install_script_record(
    record: dict[str, Any], script: dict[str, Any], location: str
) -> None:
    hook = script["name"]
    if hook not in INSTALL_CLASS_LIFECYCLE_NAMES:
        if "lockfile" in script["sources"]:
            raise ManifestError(
                f"{record['lock_path']} lifecycle hook {hook} has invalid lockfile provenance"
            )
        return
    if "tarball" in script["sources"]:
        raise ManifestError(
            f"H2 rejects install-class lifecycle script: "
            f"path={record['lock_path']} hook={hook} source=tarball"
        )
    if script["sources"] != ["lockfile", "packument"]:
        raise ManifestError(
            f"H2 install-class lifecycle provenance is partial: "
            f"path={record['lock_path']} hook={hook} source=lockfile/packument"
        )
    admission = (
        record["lock_path"],
        record["name"],
        record["version"],
        hook,
        script["command"],
        "packument",
    )
    if admission not in PACKUMENT_INSTALL_SCRIPT_ADMISSIONS:
        raise ManifestError(
            f"H2 rejects packument install-class lifecycle script: "
            f"path={record['lock_path']} hook={hook} source=packument"
        )


def _locked_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(
        value,
        {"lock_path", "name", "version", "resolved", "integrity", "selected_optional", "lifecycle_scripts"},
        location,
    )
    _relative_path(record["lock_path"], f"{location}.lock_path")
    _text(record["name"], f"{location}.name")
    _text(record["version"], f"{location}.version")
    resolved = _text(record["resolved"], f"{location}.resolved")
    if not resolved.startswith("https://registry.npmjs.org/"):
        raise ManifestError(f"{location}.resolved is not the accepted registry")
    if SHA512_SRI.fullmatch(_text(record["integrity"], f"{location}.integrity")) is None:
        raise ManifestError(f"{location}.integrity is not a valid sha512 SRI")
    _boolean(record["selected_optional"], f"{location}.selected_optional")
    scripts = [_script_record(item, f"{location}.lifecycle_scripts") for item in _array(record["lifecycle_scripts"], f"{location}.lifecycle_scripts")]
    _sorted_unique(scripts, lambda item: (item["name"].encode(), item["command"].encode()), f"{location}.lifecycle_scripts")
    for script in scripts:
        _validate_install_script_record(record, script, location)
    return record


def _validate_install_script_inventory(locked: list[dict[str, Any]]) -> None:
    admissions = tuple(
        (
            item["lock_path"],
            item["name"],
            item["version"],
            script["name"],
            script["command"],
            "packument",
        )
        for item in locked
        for script in item["lifecycle_scripts"]
        if script["name"] in INSTALL_CLASS_LIFECYCLE_NAMES
        and "packument" in script["sources"]
    )
    if admissions != PACKUMENT_INSTALL_SCRIPT_ADMISSIONS:
        raise ManifestError("H2 packument install-script admissions differ")


def _python_wheel_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(value, {"name", "version", "filename", "sha256"}, location)
    _text(record["name"], f"{location}.name")
    _text(record["version"], f"{location}.version")
    _basename(record["filename"], f"{location}.filename")
    _sha(record["sha256"], f"{location}.sha256")
    return record


def _string_array(value: Any, location: str, *, sorted_values: bool = True) -> list[str]:
    values = _array(value, location)
    if any(not isinstance(item, str) for item in values):
        raise ManifestError(f"{location} must contain only strings")
    if sorted_values:
        _sorted_unique(values, lambda item: item.encode("utf-8"), location)
    return values


def _run_record(value: Any, location: str) -> dict[str, Any]:
    record = _exact_keys(
        value,
        {"isolated_directory_id", "npm_ci_exit", "validation_exit", "external_request_count_after_npm_ci"},
        location,
    )
    _text(record["isolated_directory_id"], f"{location}.isolated_directory_id")
    for name in ("npm_ci_exit", "validation_exit", "external_request_count_after_npm_ci"):
        if _integer(record[name], f"{location}.{name}") != 0:
            raise ManifestError(f"{location}.{name} does not record PASS")
    return record


def _validate_receipt(
    path: Path,
    document: dict[str, Any],
    candidate: Candidate,
    frontend: dict[str, Any],
    scan: _FamilyScan,
) -> None:
    expected_name = f"eqiora-{candidate.version}-python-candidate-h2.json"
    if path.name != expected_name:
        raise ManifestError("H2 receipt filename differs from the candidate identity")
    try:
        raw = path.read_bytes()
        receipt = json.loads(raw)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ManifestError(f"cannot read H2 receipt: {error}") from error
    if raw != _canonical_json_bytes(receipt):
        raise ManifestError("H2 receipt bytes are not the frozen canonical JSON encoding")
    if hashlib.sha256(raw).hexdigest() != frontend["h2_receipt_sha256"]:
        raise ManifestError("H2 receipt SHA-256 differs from the detached manifest binding")
    receipt = _exact_keys(
        receipt,
        {"probe", "candidate", "environment", "inputs", "validation", "clean_run_1", "clean_run_2", "comparison", "browser", "python_host"},
        "receipt",
    )
    probe = _exact_keys(receipt["probe"], {"contract_sha256", "protected_base_sha", "writer_revision", "verdict"}, "receipt.probe")
    if probe["contract_sha256"] != CONTRACT_SHA256 or probe["protected_base_sha"] != PROTECTED_BASE_SHA or probe["verdict"] != "PASS":
        raise ManifestError("H2 probe identity or verdict drifted")
    writer_revision = _git_sha(probe["writer_revision"], "receipt.probe.writer_revision")
    candidate_record = _exact_keys(receipt["candidate"], {"project", "version", "source_commit", "artifacts"}, "receipt.candidate")
    source_commit = _git_sha(candidate_record["source_commit"], "receipt.candidate.source_commit")
    if candidate_record["project"] != "eqiora" or candidate_record["version"] != candidate.version or source_commit != candidate.commit or writer_revision != candidate.commit:
        raise ManifestError("H2 receipt belongs to another candidate/source version")
    receipt_artifacts = [_artifact_record(item, "receipt.candidate.artifacts") for item in _array(candidate_record["artifacts"], "receipt.candidate.artifacts")]
    _sorted_unique(receipt_artifacts, lambda item: item["filename"].encode(), "receipt.candidate.artifacts")
    expected_artifacts = sorted(
        ({"filename": item.filename, "kind": item.kind, "size": item.size, "sha256": item.sha256} for item in candidate.artifacts),
        key=lambda item: item["filename"].encode(),
    )
    if receipt_artifacts != expected_artifacts:
        raise ManifestError("H2 receipt candidate artifact inventory differs")
    environment = _exact_keys(
        receipt["environment"],
        {"os", "architecture", "libc", "node_version", "node_executable_sha256", "npm_version", "npm_package_integrity", "locale", "timezone", "source_date_epoch", "environment_allowlist"},
        "receipt.environment",
    )
    for name in ("os", "architecture", "libc"):
        _text(environment[name], f"receipt.environment.{name}")
    expected_environment = {
        "node_version": "v24.18.1", "node_executable_sha256": NODE_EXECUTABLE_SHA256,
        "npm_version": "11.16.0", "npm_package_integrity": NPM_PACKAGE_INTEGRITY,
        "locale": "C.UTF-8", "timezone": "UTC",
    }
    for name, expected in expected_environment.items():
        if environment[name] != expected:
            raise ManifestError(f"H2 environment identity drifted: {name}")
    _integer(environment["source_date_epoch"], "receipt.environment.source_date_epoch")
    allowlist = _string_array(environment["environment_allowlist"], "receipt.environment.environment_allowlist")
    if allowlist != ["HOME", "LANG", "LC_ALL", "PATH", "SOURCE_DATE_EPOCH", "TZ"]:
        raise ManifestError("H2 environment allowlist drifted")
    inputs = _exact_keys(
        receipt["inputs"],
        {"source_root_inventory", "package_json_sha256", "package_lock_sha256", "lockfile_version", "config_inventory", "direct_pins", "locked_packages"},
        "receipt.inputs",
    )
    source_inventory = [_file_record(item, "receipt.inputs.source_root_inventory") for item in _array(inputs["source_root_inventory"], "receipt.inputs.source_root_inventory")]
    _sorted_unique(source_inventory, lambda item: item["relative_path"].encode(), "receipt.inputs.source_root_inventory")
    configs = [_config_record(item, "receipt.inputs.config_inventory") for item in _array(inputs["config_inventory"], "receipt.inputs.config_inventory")]
    _sorted_unique(configs, lambda item: item["relative_path"].encode(), "receipt.inputs.config_inventory")
    pins = [_pin_record(item, "receipt.inputs.direct_pins") for item in _array(inputs["direct_pins"], "receipt.inputs.direct_pins")]
    _sorted_unique(pins, lambda item: (item["name"].encode(), item["version"].encode()), "receipt.inputs.direct_pins")
    locked = [_locked_record(item, "receipt.inputs.locked_packages") for item in _array(inputs["locked_packages"], "receipt.inputs.locked_packages")]
    _sorted_unique(locked, lambda item: item["lock_path"].encode(), "receipt.inputs.locked_packages")
    _validate_install_script_inventory(locked)
    if _integer(inputs["lockfile_version"], "receipt.inputs.lockfile_version") != 3:
        raise ManifestError("H2 lockfile version drifted")
    for name in ("package_json_sha256", "package_lock_sha256"):
        _sha(inputs[name], f"receipt.inputs.{name}")
        if inputs[name] != frontend[name]:
            raise ManifestError(f"H2 {name} differs from the manifest")
    validation = _exact_keys(receipt["validation"], {"npm_ci_command_argv", "offline_command_argv", "network_policy"}, "receipt.validation")
    if validation["npm_ci_command_argv"] != ["npm", "ci", "--ignore-scripts"] or validation["offline_command_argv"] != [["npm", "run", "typecheck"], ["npm", "run", "lint"]]:
        raise ManifestError("H2 exact validation commands drifted")
    if validation["network_policy"] != "registry-only-during-npm-ci;offline-after":
        raise ManifestError("H2 validation network identity drifted")
    run_1 = _run_record(receipt["clean_run_1"], "receipt.clean_run_1")
    run_2 = _run_record(receipt["clean_run_2"], "receipt.clean_run_2")
    if run_1["isolated_directory_id"] == run_2["isolated_directory_id"]:
        raise ManifestError("H2 clean validations did not use distinct scratch homes")
    comparison = _exact_keys(receipt["comparison"], {"acquired_inputs_equal", "diff"}, "receipt.comparison")
    if _boolean(comparison["acquired_inputs_equal"], "receipt.comparison.acquired_inputs_equal") is not True:
        raise ManifestError("H2 input comparison is not PASS")
    if _string_array(comparison["diff"], "receipt.comparison.diff"):
        raise ManifestError("H2 build comparison records differences")
    browser = _exact_keys(receipt["browser"], {"playwright_test_integrity", "playwright_core_integrity", "browsers_json_sha256", "browser_name", "revision", "browser_version", "platform", "downloaded_archive_sha256", "executable_sha256"}, "receipt.browser")
    for name in ("playwright_test_integrity", "playwright_core_integrity"):
        if SHA512_SRI.fullmatch(_text(browser[name], f"receipt.browser.{name}")) is None:
            raise ManifestError("H2 Playwright integrity is invalid")
    if browser["browsers_json_sha256"] != BROWSERS_JSON_SHA256 or browser["browser_name"] != "chromium" or browser["revision"] != "1234" or browser["browser_version"] != "151.0.7922.34":
        raise ManifestError("H2 browser identity drifted")
    _text(browser["platform"], "receipt.browser.platform")
    _sha(browser["downloaded_archive_sha256"], "receipt.browser.downloaded_archive_sha256")
    _sha(browser["executable_sha256"], "receipt.browser.executable_sha256")
    python_host = _exact_keys(receipt["python_host"], {"python", "resolved_environment_sha256", "wheels"}, "receipt.python_host")
    if python_host["python"] != "3.13":
        raise ManifestError("H2 Python host drifted")
    wheels = [_python_wheel_record(item, "receipt.python_host.wheels") for item in _array(python_host["wheels"], "receipt.python_host.wheels")]
    _sorted_unique(wheels, lambda item: item["filename"].encode(), "receipt.python_host.wheels")
    if not any(item["name"].lower() == "marimo" and item["version"] == "0.23.16" for item in wheels):
        raise ManifestError("H2 Python environment omits exact marimo")
    if python_host["resolved_environment_sha256"] != _structured_sha256(wheels) or python_host["resolved_environment_sha256"] != frontend["runtime"]["resolved_environment_sha256"]:
        raise ManifestError("H2 resolved Python environment preimage/hash differs")
    script_inventory = [{"lock_path": item["lock_path"], "name": item["name"], "version": item["version"], "lifecycle_scripts": item["lifecycle_scripts"]} for item in locked]
    bindings = {
        "source_inventory_sha256": _structured_sha256(source_inventory),
        "config_inventory_sha256": _structured_sha256(configs),
        "locked_packages_sha256": _structured_sha256(locked),
        "install_script_inventory_sha256": _structured_sha256(script_inventory),
    }
    for name, observed in bindings.items():
        if frontend[name] != observed:
            raise ManifestError(f"H2 structured inventory preimage/hash differs: {name}")
    roots = {name.split("/", maxsplit=1)[0] for name in scan.sdist_members}
    root = next(iter(roots))
    for record in source_inventory:
        retained = scan.sdist_members.get(f"{root}/bindings/python/frontend/{record['relative_path']}")
        if retained is None or len(retained) != record["size"] or hashlib.sha256(retained).hexdigest() != record["sha256"]:
            raise ManifestError("H2 source inventory differs from retained sdist source")
    for record in configs:
        retained = scan.sdist_members.get(f"{root}/bindings/python/frontend/{record['relative_path']}")
        if retained is None or hashlib.sha256(retained).hexdigest() != record["sha256"]:
            raise ManifestError("H2 config inventory differs from retained sdist source")
    package = scan.sdist_members.get(f"{root}/bindings/python/frontend/package.json")
    lock = scan.sdist_members.get(f"{root}/bindings/python/frontend/package-lock.json")
    if package is None or hashlib.sha256(package).hexdigest() != frontend["package_json_sha256"]:
        raise ManifestError("H2 package hash differs from retained sdist")
    if lock is None or hashlib.sha256(lock).hexdigest() != frontend["package_lock_sha256"]:
        raise ManifestError("H2 lock hash differs from retained sdist")


def load_candidate_family(
    manifest: Path,
    artifacts: Path,
    *,
    requested_profiles: tuple[str, ...] = (),
    h2_receipt: Path | None = None,
) -> Candidate:
    """Select v2/v3 only after scanning the complete retained artifact family."""
    try:
        document = _object(json.loads(manifest.read_text(encoding="utf-8")), "manifest")
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ManifestError(f"cannot read candidate manifest: {error}") from error
    scan = _scan_family(document, artifacts)
    checks = document.get("checks", [])
    manifest_signal = (
        document.get("format") == V3_MANIFEST_FORMAT
        or "frontend" in _object(document.get("build"), "build")
        or any(isinstance(check, str) and (check in NOTEBOOK_CHECKS or "notebook" in check.lower()) for check in checks)
        or "notebook" in requested_profiles
    )
    activated = scan.activated or manifest_signal
    if not activated:
        if document.get("format") != MANIFEST_FORMAT:
            raise ManifestError("signal-free candidate must use the v2 manifest")
        candidate = load_candidate(manifest)
        verify_artifacts(candidate, artifacts)
        for profile in requested_profiles:
            require_candidate_profile(candidate, profile)
        return candidate
    if document.get("format") != V3_MANIFEST_FORMAT:
        raise ManifestError("an N1 signal requires the fail-closed v3 candidate schema")
    candidate = _candidate_v3(document, scan)
    expected_family = {artifact.filename for artifact in candidate.artifacts}
    actual_family = {path.name for path in artifacts.iterdir()}
    if actual_family != expected_family:
        raise ManifestError("v3 candidate artifact directory is not the exact family")
    verify_artifacts(candidate, artifacts)
    raw_frontend = _object(document.get("build"), "build").get("frontend")
    if raw_frontend is None:
        raise ManifestError("v3 candidate requires build.frontend")
    frontend = _validate_frontend(raw_frontend)
    notebook_named = {
        check
        for check in candidate.checks
        if check in NOTEBOOK_CHECKS
        or "notebook" in check.lower()
        or check.startswith("frontend:")
        or check.startswith("cp313:marimo-")
    }
    if notebook_named != NOTEBOOK_CHECKS:
        raise ManifestError("v3 Notebook checks must be the exact closed set")
    require_candidate_profile(candidate, "notebook")
    if h2_receipt is None:
        raise ManifestError("v3 candidate requires its detached H2 receipt")
    _validate_receipt(h2_receipt, document, candidate, frontend, scan)
    for profile in requested_profiles:
        require_candidate_profile(candidate, profile)
    return candidate


def verify_artifacts(candidate: Candidate, directory: Path) -> None:
    """Require exactly the manifested distribution files and matching bytes."""
    if not directory.is_dir():
        raise ManifestError(f"artifact directory does not exist: {directory}")
    expected = {artifact.filename for artifact in candidate.artifacts}
    actual = {
        path.name
        for path in directory.iterdir()
        if path.name not in {".DS_Store"}
    }
    if actual != expected:
        raise ManifestError(
            f"artifact directory differs: missing={sorted(expected - actual)}, "
            f"unexpected={sorted(actual - expected)}"
        )
    by_name = {artifact.filename: artifact for artifact in candidate.artifacts}
    for filename in sorted(expected):
        path = directory / filename
        if not path.is_file() or path.is_symlink():
            raise ManifestError(f"artifact is not one regular file: {filename}")
        artifact = by_name[filename]
        if path.stat().st_size != artifact.size:
            raise ManifestError(f"artifact size differs: {filename}")
        if file_sha256(path) != artifact.sha256:
            raise ManifestError(f"artifact SHA-256 differs: {filename}")


def verify_manifest_hash(path: Path, expected: str | None) -> None:
    if expected is None:
        return
    if SHA256.fullmatch(expected) is None:
        raise ManifestError("expected manifest SHA-256 is malformed")
    if file_sha256(path) != expected:
        raise ManifestError("candidate manifest SHA-256 differs")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--manifest-sha256")
    parser.add_argument("--h2-receipt", type=Path)
    parser.add_argument("--expected-commit")
    parser.add_argument("--expected-tag")
    arguments = parser.parse_args()

    try:
        verify_manifest_hash(arguments.manifest, arguments.manifest_sha256)
        candidate = load_candidate_family(
            arguments.manifest,
            arguments.artifacts,
            h2_receipt=arguments.h2_receipt,
        )
        if (
            arguments.expected_commit is not None
            and candidate.commit != arguments.expected_commit
        ):
            raise ManifestError("candidate source commit differs from the request")
        if (
            arguments.expected_tag is not None
            and candidate.expected_tag != arguments.expected_tag
        ):
            raise ManifestError("candidate release tag differs from the request")
    except (ManifestError, OSError) as error:
        print(f"candidate verification failed: {error}", file=sys.stderr)
        return 2

    print(
        json.dumps(
            {
                "commit": candidate.commit,
                "expected_tag": candidate.expected_tag,
                "version": candidate.version,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
