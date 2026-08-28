#!/usr/bin/env python3
"""Execute the independent N1 frontend H2 predicate and publish its receipt."""

from __future__ import annotations

import argparse
import base64
import hashlib
import ipaddress
import json
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from email.parser import BytesParser
from pathlib import Path, PurePosixPath
from typing import Any

from python_candidate_common import (
    CandidateError,
    checked_run,
    home_scratch_parent,
    python_distribution_version,
)


ROOT = Path(__file__).resolve().parents[2]
FRONTEND = Path("bindings/python/frontend")
CONTRACT_SHA256 = "3f3a9f1a5b54bf5b874d996c8807bbb7e88439737fd245d69e7a8aeb7a1a87c1"
PROTECTED_BASE_SHA = "3dfb1086168afc6f9fb61f9ca43d21ca9953048b"
NODE_VERSION = "v24.18.1"
NODE_SHA256 = "f3432a45b03b2da0d270095fdd8813dc34cbea73f5fc8b18c7a384b7cf9b333a"
NPM_VERSION = "11.16.0"
NPM_INTEGRITY = (
    "sha512-A74XL8OxmcegZDMWPkWb5bEQppg8HdYwW3rBD2sPoS4UQHVajfaxBkqyzLeJ3wR0kZ+"
    "5xoTjItxXaF7eIXUsyw=="
)
BROWSERS_JSON_SHA256 = (
    "f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99"
)
PLAYWRIGHT_CORE_PACKAGE_SHA256 = (
    "07c47543631fef9508760365dee9fbe958c562093ec8d122543949ed231f233f"
)
PLAYWRIGHT_CORE_BUNDLE_SHA256 = (
    "9393fa79e1c67c74edc26b610d65a4f7ed73d345a762465cc88340a33a2454ac"
)
PLAYWRIGHT_CORE_INTEGRITY = (
    "sha512-wPYSwEBJY9GHraISXqyqtx0na0LpO3XEX7jNDhntbex7tzUS7kLnZsOlFruFJB4Hi/"
    "rhDMjXGqHewDZ68nYZVw=="
)
PLAYWRIGHT_TEST_INTEGRITY = (
    "sha512-DTcUc8qii+cpHvtOwggMtBRMjKZHXYWdw8syRYu2vtzuq4Wxphqq4NfCs5Zt44L6mA8r"
    "fDfj+PHnxFc/FeK6mQ=="
)
PLAYWRIGHT_BROWSER_URL = (
    "https://cdn.playwright.dev/builds/cft/151.0.7922.34/linux64/"
    "chrome-headless-shell-linux64.zip"
)
PLAYWRIGHT_BROWSER_MEMBER = "chrome-headless-shell-linux64/chrome-headless-shell"
PLAYWRIGHT_BROWSER_ARCHIVE_BYTES = 120_231_126
PLAYWRIGHT_BROWSER_ARCHIVE_SHA256 = (
    "3cfc2bd00d1bafcf8a68dc74c9c92bb7150ddc8d26ade948a776316e1cec4f14"
)
PLAYWRIGHT_BROWSER_MEMBER_COUNT = 287
PLAYWRIGHT_BROWSER_EXPANDED_BYTES = 273_378_828
PLAYWRIGHT_BROWSER_LARGEST_MEMBER_BYTES = 196_975_952
PLAYWRIGHT_BROWSER_EXECUTABLE_SHA256 = (
    "e11fc9ce65c96313476f7ee9844b6fb6a9220fb048693cfe9eee00acf4170a9f"
)
PLAYWRIGHT_BROWSER_INVENTORY_SHA256 = (
    "960a12d7e14cd59583eb0dd74065ed111d48167f7867ace8ff4ce578f2b64f3a"
)
FAMILY_MEMBER_BYTES_LIMIT = 16_777_216
FAMILY_TOTAL_BYTES_LIMIT = 67_108_864
SOURCE_MEMBER_COUNT_LIMIT = 50_000
SOURCE_MEMBER_BYTES_LIMIT = 67_108_864
SOURCE_TOTAL_BYTES_LIMIT = 536_870_912
LOCKED_PACKAGE_COUNT_LIMIT = 2_048
LOCKED_PACKAGE_BYTES_LIMIT = 1_073_741_824
PYTHON_WHEEL_COUNT_LIMIT = 256
PYTHON_WHEEL_BYTES_LIMIT = 1_073_741_824
ABSTRACT_MEMBER_STEPS_LIMIT = 104_646
ABSTRACT_BYTE_STEPS_LIMIT = 4_755_686_114
CONTENT_BOUND_BROWSER_PROFILE = {
    "platform": "linux-x86_64",
    "browser": "Chromium Headless Shell 151.0.7922.34",
    "playwright_revision": "1234",
    "url": PLAYWRIGHT_BROWSER_URL,
    "raw_archive_bytes": PLAYWRIGHT_BROWSER_ARCHIVE_BYTES,
    "raw_archive_sha256": PLAYWRIGHT_BROWSER_ARCHIVE_SHA256,
    "zip_member_count": PLAYWRIGHT_BROWSER_MEMBER_COUNT,
    "total_expanded_bytes": PLAYWRIGHT_BROWSER_EXPANDED_BYTES,
    "largest_expanded_member_bytes": PLAYWRIGHT_BROWSER_LARGEST_MEMBER_BYTES,
    "largest_member": PLAYWRIGHT_BROWSER_MEMBER,
    "executable_sha256": PLAYWRIGHT_BROWSER_EXECUTABLE_SHA256,
    "closed_member_inventory_sha256": PLAYWRIGHT_BROWSER_INVENTORY_SHA256,
}
CONTENT_BOUND_RESOURCE_LIMITS = {
    "family_member_count": 5,
    "family_member_bytes": FAMILY_MEMBER_BYTES_LIMIT,
    "family_total_bytes": FAMILY_TOTAL_BYTES_LIMIT,
    "source_member_count": SOURCE_MEMBER_COUNT_LIMIT,
    "source_member_bytes": SOURCE_MEMBER_BYTES_LIMIT,
    "source_total_bytes": SOURCE_TOTAL_BYTES_LIMIT,
    "locked_package_count": LOCKED_PACKAGE_COUNT_LIMIT,
    "locked_package_bytes": LOCKED_PACKAGE_BYTES_LIMIT,
    "resolved_python_wheel_count": PYTHON_WHEEL_COUNT_LIMIT,
    "resolved_python_wheel_bytes": PYTHON_WHEEL_BYTES_LIMIT,
    "host_scenarios": 2,
    "member_steps": ABSTRACT_MEMBER_STEPS_LIMIT,
    "byte_steps": ABSTRACT_BYTE_STEPS_LIMIT,
}
DIRECT_PINS = {
    "typescript": "7.0.2",
    "@biomejs/biome": "2.5.6",
    "@playwright/test": "1.62.1",
}
CONFIG_NAMES = (
    "biome.json",
    "playwright.config.ts",
    "tsconfig.json",
)
ENVIRONMENT_ALLOWLIST = (
    "HOME",
    "LANG",
    "LC_ALL",
    "PATH",
    "SOURCE_DATE_EPOCH",
    "TZ",
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
GIT_SHA_RE = re.compile(r"[0-9a-f]{40}")
IMPORT_RE = re.compile(
    r"(?:\bimport\s*(?:\([^)]*\)|[^;]*?\sfrom\s*)|\bexport\s+[^;]*?\sfrom\s*)"
    r"[\"']([^\"']+)[\"']"
)
URL_RE = re.compile(rb"(?:https?:)?//[^\s\"'`)]+", re.IGNORECASE)
LIFECYCLE_NAMES = frozenset(
    {
        "preinstall",
        "install",
        "postinstall",
        "prepublish",
        "preprepare",
        "prepare",
        "postprepare",
    }
)
INSTALL_CLASS_LIFECYCLE_NAMES = frozenset(
    {"preinstall", "install", "postinstall"}
)
LIFECYCLE_SOURCES = frozenset({"lockfile", "packument", "tarball"})


@dataclass(frozen=True)
class SourceIdentity:
    """Exact source revision consumed by the detached H2 executor."""

    commit: str
    tags: tuple[str, ...]


@dataclass(frozen=True)
class CandidateFamily:
    sdist: Path
    wheels: tuple[Path, ...]
    version: str
    inventory: tuple[dict[str, object], ...]


@dataclass(frozen=True)
class H2Workspace:
    root: Path
    home: Path
    npm_cache: Path
    temporary: Path
    frontend: Path
    installation: Path
    output: Path
    browser_cache: Path


@dataclass(frozen=True)
class AcquiredInputs:
    node: Path
    npm: Path
    npm_tarball: Path
    browser_archive: Path
    browser_executable: Path
    browser_archive_sha256: str
    browser_executable_sha256: str
    browser_platform: str
    playwright_test_integrity: str
    playwright_core_integrity: str
    python_wheels: tuple[dict[str, object], ...]
    package_manifests: tuple[tuple[str, dict[str, Any]], ...]
    package_packuments: tuple[tuple[str, dict[str, Any]], ...]
    locked_package_bytes: int
    python_wheel_bytes: int
    browser_member_count: int
    browser_expanded_regular_bytes: int


@dataclass(frozen=True)
class RunObservation:
    external_request_count: int
    acquisition: AcquiredInputs


_ACQUISITION_IDENTITY_FIELDS = (
    "browser_archive_sha256",
    "browser_executable_sha256",
    "browser_platform",
    "playwright_test_integrity",
    "playwright_core_integrity",
    "python_wheels",
    "package_manifests",
    "package_packuments",
    "locked_package_bytes",
    "python_wheel_bytes",
    "browser_member_count",
    "browser_expanded_regular_bytes",
)
_ACQUISITION_COLLECTION_FIELDS = frozenset(
    {"python_wheels", "package_manifests", "package_packuments"}
)
_ABSENT_MEMBER = object()


def canonical_json_bytes(value: object) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def structured_sha256(value: object) -> str:
    return hashlib.sha256(canonical_json_bytes(value)).hexdigest()


def require_content_bound_resources(observed: dict[str, object]) -> dict[str, int]:
    """Validate the frozen content-bound H2 input and abstract-work record."""

    numeric_fields = {
        "family_member_count",
        "family_largest_member_bytes",
        "family_bytes",
        "source_member_count",
        "source_largest_member_bytes",
        "source_bytes",
        "locked_package_count",
        "locked_package_bytes",
        "resolved_python_wheel_count",
        "resolved_python_wheel_bytes",
        "browser_archive_bytes",
        "browser_archive_member_count",
        "browser_extracted_regular_bytes",
        "browser_largest_expanded_member_bytes",
        "host_scenarios",
    }
    identity_fields = {
        "browser_archive_sha256",
        "browser_largest_member",
        "browser_member_inventory_sha256",
        "browser_executable_sha256",
    }
    if set(observed) != numeric_fields | identity_fields:
        raise CandidateError("H2 resource observation fields differ")
    if any(
        type(observed[name]) is not int or int(observed[name]) < 0
        for name in numeric_fields
    ):
        raise CandidateError("H2 resource observation is not a nonnegative integer")
    if any(type(observed[name]) is not str for name in identity_fields):
        raise CandidateError("H2 resource identity is not text")

    exact_numeric = {
        "family_member_count": CONTENT_BOUND_RESOURCE_LIMITS[
            "family_member_count"
        ],
        "browser_archive_bytes": CONTENT_BOUND_BROWSER_PROFILE[
            "raw_archive_bytes"
        ],
        "browser_archive_member_count": CONTENT_BOUND_BROWSER_PROFILE[
            "zip_member_count"
        ],
        "browser_extracted_regular_bytes": CONTENT_BOUND_BROWSER_PROFILE[
            "total_expanded_bytes"
        ],
        "browser_largest_expanded_member_bytes": CONTENT_BOUND_BROWSER_PROFILE[
            "largest_expanded_member_bytes"
        ],
        "host_scenarios": CONTENT_BOUND_RESOURCE_LIMITS["host_scenarios"],
    }
    maximum_numeric = {
        "family_largest_member_bytes": CONTENT_BOUND_RESOURCE_LIMITS[
            "family_member_bytes"
        ],
        "family_bytes": CONTENT_BOUND_RESOURCE_LIMITS["family_total_bytes"],
        "source_member_count": CONTENT_BOUND_RESOURCE_LIMITS[
            "source_member_count"
        ],
        "source_largest_member_bytes": CONTENT_BOUND_RESOURCE_LIMITS[
            "source_member_bytes"
        ],
        "source_bytes": CONTENT_BOUND_RESOURCE_LIMITS["source_total_bytes"],
        "locked_package_count": CONTENT_BOUND_RESOURCE_LIMITS[
            "locked_package_count"
        ],
        "locked_package_bytes": CONTENT_BOUND_RESOURCE_LIMITS[
            "locked_package_bytes"
        ],
        "resolved_python_wheel_count": CONTENT_BOUND_RESOURCE_LIMITS[
            "resolved_python_wheel_count"
        ],
        "resolved_python_wheel_bytes": CONTENT_BOUND_RESOURCE_LIMITS[
            "resolved_python_wheel_bytes"
        ],
    }
    if any(observed[name] != expected for name, expected in exact_numeric.items()):
        raise CandidateError("H2 resource exact identity differs")
    if any(observed[name] > maximum for name, maximum in maximum_numeric.items()):
        raise CandidateError("H2 resource component exceeds its frozen ceiling")

    exact_identity = {
        "browser_archive_sha256": CONTENT_BOUND_BROWSER_PROFILE[
            "raw_archive_sha256"
        ],
        "browser_largest_member": CONTENT_BOUND_BROWSER_PROFILE["largest_member"],
        "browser_member_inventory_sha256": CONTENT_BOUND_BROWSER_PROFILE[
            "closed_member_inventory_sha256"
        ],
        "browser_executable_sha256": CONTENT_BOUND_BROWSER_PROFILE[
            "executable_sha256"
        ],
    }
    if any(observed[name] != expected for name, expected in exact_identity.items()):
        raise CandidateError("H2 resource content identity differs")

    member_steps = (
        int(observed["family_member_count"])
        + 2 * int(observed["source_member_count"])
        + 2 * int(observed["locked_package_count"])
        + int(observed["resolved_python_wheel_count"])
        + int(observed["browser_archive_member_count"])
        + int(observed["host_scenarios"])
    )
    byte_steps = (
        int(observed["family_bytes"])
        + 2 * int(observed["source_bytes"])
        + 2 * int(observed["locked_package_bytes"])
        + int(observed["resolved_python_wheel_bytes"])
        + int(observed["browser_archive_bytes"])
        + int(observed["browser_extracted_regular_bytes"])
    )
    if (
        member_steps > CONTENT_BOUND_RESOURCE_LIMITS["member_steps"]
        or byte_steps > CONTENT_BOUND_RESOURCE_LIMITS["byte_steps"]
    ):
        raise CandidateError("H2 abstract work exceeds its frozen ceiling")
    return {"member_steps": member_steps, "byte_steps": byte_steps}


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def source_identity() -> SourceIdentity:
    """Require a clean source tree without importing the producer runtime."""

    status = checked_run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        capture=True,
    )
    if status:
        raise CandidateError("H2 requires a clean source tree")
    commit = checked_run(["git", "rev-parse", "HEAD"], capture=True)
    if GIT_SHA_RE.fullmatch(commit) is None:
        raise CandidateError("git did not return a full H2 source commit")
    tags = tuple(
        line
        for line in checked_run(
            ["git", "tag", "--points-at", commit],
            capture=True,
        ).splitlines()
        if line
    )
    return SourceIdentity(commit=commit, tags=tags)


def safe_extract_sdist(archive: Path, destination: Path) -> Path:
    """Extract one regular source distribution without path traversal."""

    root = destination.resolve()
    with tarfile.open(archive, mode="r:gz") as source:
        members = source.getmembers()
        if len(members) > SOURCE_MEMBER_COUNT_LIMIT:
            raise CandidateError("sdist exceeds the source member ceiling")
        seen: set[str] = set()
        total_bytes = 0
        for member in members:
            raw_name = member.name.rstrip("/") if member.isdir() else member.name
            relative = PurePosixPath(raw_name)
            target = (root / member.name).resolve()
            if (
                not raw_name
                or "\\" in raw_name
                or relative.is_absolute()
                or relative.as_posix() != raw_name
                or any(part in {"", ".", ".."} for part in relative.parts)
                or not target.is_relative_to(root)
                or raw_name in seen
            ):
                raise CandidateError(f"sdist path escapes its root: {member.name}")
            seen.add(raw_name)
            if not (member.isfile() or member.isdir()):
                raise CandidateError(
                    f"sdist contains a non-regular member: {member.name}"
                )
            if member.isfile():
                if member.size < 0 or member.size > SOURCE_MEMBER_BYTES_LIMIT:
                    raise CandidateError(
                        "sdist member exceeds the expanded byte ceiling"
                    )
                total_bytes += member.size
                if total_bytes > SOURCE_TOTAL_BYTES_LIMIT:
                    raise CandidateError("sdist exceeds the expanded byte ceiling")
        try:
            source.extractall(destination, members=members)
        except BaseException:
            shutil.rmtree(destination, ignore_errors=True)
            raise

    children = [path for path in destination.iterdir() if path.is_dir()]
    if len(children) != 1:
        raise CandidateError("sdist must contain exactly one top-level directory")
    extracted = children[0]
    required = (
        extracted / "Cargo.toml",
        extracted / "Cargo.lock",
        extracted / "pyproject.toml",
        extracted / "crates/eqiora-python/Cargo.toml",
    )
    missing = [
        str(path.relative_to(extracted)) for path in required if not path.is_file()
    ]
    if missing:
        raise CandidateError(f"sdist is incomplete: {', '.join(missing)}")
    return extracted


def _retained_distribution_version(extracted: Path) -> str:
    """Derive the Python version from retained Cargo and raw frontend mirrors."""

    try:
        cargo = tomllib.loads((extracted / "Cargo.toml").read_text(encoding="utf-8"))
        pyproject = tomllib.loads(
            (extracted / "pyproject.toml").read_text(encoding="utf-8")
        )
        package = json.loads(
            (extracted / FRONTEND / "package.json").read_text(encoding="utf-8")
        )
        lock = json.loads(
            (extracted / FRONTEND / "package-lock.json").read_text(encoding="utf-8")
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        raise CandidateError(f"retained release identity cannot be parsed: {error}") from error

    workspace = cargo.get("workspace")
    workspace_package = workspace.get("package") if isinstance(workspace, dict) else None
    raw_version = (
        workspace_package.get("version")
        if isinstance(workspace_package, dict)
        else None
    )
    if not isinstance(raw_version, str) or not raw_version:
        raise CandidateError("retained Cargo version is not one nonempty string")

    project = pyproject.get("project")
    if not isinstance(project, dict):
        raise CandidateError("retained Python project table is unavailable")
    if "version" in project or project.get("dynamic") != ["version"]:
        raise CandidateError("retained Python version must be exactly dynamic")

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
            raise CandidateError(f"retained frontend {location} is not one nonempty string")
        if value != raw_version:
            raise CandidateError(f"retained frontend {location} differs from raw Cargo")
    return python_distribution_version(raw_version)


def _sdist_resource_usage(archive: Path) -> tuple[int, int, int]:
    with tarfile.open(archive, mode="r:gz") as source:
        members = source.getmembers()
    member_count = len(members)
    expanded_bytes = sum(member.size for member in members if member.isfile())
    largest_member_bytes = max(
        (member.size for member in members if member.isfile()), default=0
    )
    if (
        member_count > SOURCE_MEMBER_COUNT_LIMIT
        or expanded_bytes > SOURCE_TOTAL_BYTES_LIMIT
        or largest_member_bytes > SOURCE_MEMBER_BYTES_LIMIT
    ):
        raise CandidateError("sdist resource identity changed after admission")
    return member_count, expanded_bytes, largest_member_bytes


def _utf8(value: str) -> bytes:
    return value.encode("utf-8")


def _relative_path(value: str) -> str:
    path = PurePosixPath(value)
    if (
        not value
        or "\\" in value
        or "\0" in value
        or path.is_absolute()
        or value != path.as_posix()
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise CandidateError(f"unsafe receipt path: {value!r}")
    return value


def _basename(value: str) -> str:
    if (
        not value
        or value in {".", ".."}
        or "/" in value
        or "\\" in value
        or "\0" in value
    ):
        raise CandidateError(f"unsafe artifact basename: {value!r}")
    return value


def _file_record(
    path: Path, root: Path, *, relative: str | None = None
) -> dict[str, object]:
    if path.is_symlink() or not path.is_file():
        raise CandidateError(f"H2 input is not one regular file: {path}")
    name = relative if relative is not None else path.relative_to(root).as_posix()
    _relative_path(name)
    status = path.stat()
    return {
        "relative_path": name,
        "mode": stat.S_IMODE(status.st_mode),
        "size": status.st_size,
        "sha256": file_sha256(path),
    }


def _regular_tree_inventory(root: Path) -> tuple[dict[str, object], ...]:
    if not root.is_dir() or root.is_symlink():
        raise CandidateError(f"H2 tree is unavailable: {root}")
    records: list[dict[str, object]] = []
    for path in sorted(
        root.rglob("*"), key=lambda item: _utf8(item.relative_to(root).as_posix())
    ):
        if path.is_symlink():
            raise CandidateError(f"H2 tree contains a symbolic link: {path}")
        if path.is_dir():
            continue
        if not path.is_file():
            raise CandidateError(f"H2 tree contains a non-regular path: {path}")
        records.append(_file_record(path, root))
    return tuple(records)


def family_inventory(directory: Path) -> tuple[dict[str, object], ...]:
    if not directory.is_dir() or directory.is_symlink():
        raise CandidateError("H2 artifact family must be one directory")
    paths = tuple(sorted(directory.iterdir(), key=lambda item: _utf8(item.name)))
    total_bytes = 0
    for path in paths:
        if path.is_symlink() or not path.is_file() or path.stat().st_nlink != 1:
            raise CandidateError(
                f"H2 family member is not one regular file: {path.name}"
            )
        size = path.stat().st_size
        if size > FAMILY_MEMBER_BYTES_LIMIT:
            raise CandidateError("H2 family member exceeds the raw byte ceiling")
        total_bytes += size
        if total_bytes > FAMILY_TOTAL_BYTES_LIMIT:
            raise CandidateError("H2 artifact family exceeds the raw byte ceiling")
    records: list[dict[str, object]] = []
    for path in paths:
        records.append(
            {
                "filename": _basename(path.name),
                "kind": "sdist" if path.name.endswith(".tar.gz") else "wheel",
                "size": path.stat().st_size,
                "sha256": file_sha256(path),
            }
        )
    return tuple(records)


def _require_exact_maturin_wheel(
    path: Path,
    *,
    version: str,
    compact_python: str,
) -> None:
    """Admit one exact Maturin 1.14.1 dual-alias wheel."""

    expected_name = (
        f"eqiora-{version}-cp{compact_python}-cp{compact_python}-"
        "manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
    )
    if path.name != expected_name:
        raise CandidateError(f"H2 wheel identity drifted: {path.name}")

    expected_tags = (
        f"cp{compact_python}-cp{compact_python}-manylinux_2_17_x86_64",
        f"cp{compact_python}-cp{compact_python}-manylinux2014_x86_64",
    )
    prefix = f"eqiora-{version}-cp{compact_python}-cp{compact_python}-"
    physical_platforms = path.name.removeprefix(prefix).removesuffix(".whl").split(".")
    if tuple(physical_platforms) != (
        "manylinux_2_17_x86_64",
        "manylinux2014_x86_64",
    ):
        raise CandidateError(f"H2 wheel platform aliases drifted: {path.name}")
    filename_tags = {
        f"cp{compact_python}-cp{compact_python}-{platform}"
        for platform in physical_platforms
    }
    if filename_tags != set(expected_tags):
        raise CandidateError(f"H2 wheel expanded tags drifted: {path.name}")

    expected_member = f"eqiora-{version}.dist-info/WHEEL"
    expected_payload = (
        "Wheel-Version: 1.0\n"
        "Generator: maturin (1.14.1)\n"
        "Root-Is-Purelib: false\n"
        f"Tag: {expected_tags[0]}\n"
        f"Tag: {expected_tags[1]}\n"
    ).encode("utf-8")
    try:
        with zipfile.ZipFile(path, mode="r") as archive:
            wheel_members = tuple(
                member
                for member in archive.infolist()
                if member.filename.endswith(".dist-info/WHEEL")
            )
            metadata_members = tuple(
                member
                for member in archive.infolist()
                if member.filename.endswith(".dist-info/METADATA")
            )
            if len(wheel_members) != 1:
                raise CandidateError(
                    f"H2 wheel has ambiguous WHEEL metadata: {path.name}"
                )
            if len(metadata_members) != 1:
                raise CandidateError(
                    f"H2 wheel has ambiguous distribution metadata: {path.name}"
                )
            member = wheel_members[0]
            member_mode = member.external_attr >> 16
            if member.filename != expected_member or not stat.S_ISREG(member_mode):
                raise CandidateError(
                    f"H2 wheel has invalid WHEEL ownership or mode: {path.name}"
                )
            payload = archive.read(member)
            metadata_member = metadata_members[0]
            metadata_mode = metadata_member.external_attr >> 16
            expected_metadata_member = f"eqiora-{version}.dist-info/METADATA"
            if (
                metadata_member.filename != expected_metadata_member
                or not stat.S_ISREG(metadata_mode)
            ):
                raise CandidateError(
                    f"H2 wheel has invalid distribution metadata ownership: {path.name}"
                )
            metadata = BytesParser().parsebytes(archive.read(metadata_member))
    except (OSError, NotImplementedError, zipfile.BadZipFile) as error:
        raise CandidateError(f"H2 wheel archive is invalid: {path.name}") from error
    if payload != expected_payload:
        raise CandidateError(f"H2 wheel metadata drifted: {path.name}")
    if metadata.get("Name") != "eqiora" or metadata.get("Version") != version:
        raise CandidateError(f"H2 wheel distribution version drifted: {path.name}")


def admit_candidate_family(directory: Path) -> CandidateFamily:
    inventory = family_inventory(directory)
    if len(inventory) != 5 or any(int(item["size"]) <= 0 for item in inventory):
        raise CandidateError("H2 requires exactly one nonempty sdist and four wheels")
    paths = {path.name: path for path in directory.iterdir()}
    sdists = [path for path in paths.values() if path.name.endswith(".tar.gz")]
    if len(sdists) != 1:
        raise CandidateError("H2 requires exactly one .tar.gz source distribution")
    match = re.fullmatch(r"eqiora-(.+)\.tar\.gz", sdists[0].name)
    if match is None:
        raise CandidateError("H2 source distribution has the wrong identity")
    version = match.group(1)
    if re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:(?:a|b|rc)(?:0|[1-9][0-9]*))?", version) is None:
        raise CandidateError("H2 source distribution version is not normalized")
    wheels = [path for path in paths.values() if path.name.endswith(".whl")]
    if len(wheels) != 4 or len(wheels) + 1 != len(paths):
        raise CandidateError("H2 family contains a non-distribution file")
    by_python: dict[str, Path] = {}
    for path in wheels:
        wheel = re.fullmatch(
            rf"eqiora-{re.escape(version)}-cp(311|312|313|314)-cp\1-"
            r"manylinux_2_17_x86_64\.manylinux2014_x86_64\.whl",
            path.name,
        )
        if wheel is None:
            raise CandidateError(f"H2 wheel identity drifted: {path.name}")
        python = wheel.group(1)
        if python in by_python:
            raise CandidateError(f"H2 repeats CPython {python}")
        _require_exact_maturin_wheel(
            path,
            version=version,
            compact_python=python,
        )
        by_python[python] = path
    if set(by_python) != {"311", "312", "313", "314"}:
        raise CandidateError("H2 requires the exact CPython 3.11-3.14 family")
    if family_inventory(directory) != inventory:
        raise CandidateError("H2 artifact family changed during admission")
    return CandidateFamily(
        sdists[0],
        tuple(by_python[name] for name in ("311", "312", "313", "314")),
        version,
        inventory,
    )


def create_isolated_build_workspaces(scratch: Path) -> tuple[H2Workspace, ...]:
    if not scratch.resolve().is_relative_to(Path.home().resolve()):
        raise CandidateError("H2 build scratch must remain below home")
    workspaces: list[H2Workspace] = []
    for index in (1, 2):
        root = scratch / f"clean-run-{index}"
        workspace = H2Workspace(
            root=root,
            home=root / "home",
            npm_cache=root / "npm-cache",
            temporary=root / "tmp",
            frontend=root / "frontend",
            installation=root / "frontend/node_modules",
            output=root / "frontend/dist",
            browser_cache=root / "browser-cache",
        )
        for path in (
            workspace.root,
            workspace.home,
            workspace.npm_cache,
            workspace.temporary,
            workspace.browser_cache,
        ):
            path.mkdir(parents=True, exist_ok=False)
        workspaces.append(workspace)
    return tuple(workspaces)


def stage_frontend(extracted: Path, workspace: H2Workspace) -> None:
    source = extracted / FRONTEND
    if not source.is_dir() or source.is_symlink():
        raise CandidateError("retained sdist omits the frozen frontend root")
    shutil.copytree(source, workspace.frontend, symlinks=False)
    forbidden = (
        "node_modules",
        "dist",
        "test-results",
        "playwright-report",
        "coverage",
    )
    if any((workspace.frontend / name).exists() for name in forbidden):
        raise CandidateError(
            "retained sdist frontend contains generated or ambient output"
        )


def _frontend_environment(
    workspace: H2Workspace, source_date_epoch: int
) -> dict[str, str]:
    return {
        "HOME": str(workspace.home),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PATH": os.environ.get("PATH", ""),
        "SOURCE_DATE_EPOCH": str(source_date_epoch),
        "TZ": "UTC",
        "npm_config_cache": str(workspace.npm_cache),
        "TMPDIR": str(workspace.temporary),
        "PLAYWRIGHT_BROWSERS_PATH": str(workspace.browser_cache),
    }


def run_frontend_commands(
    workspace: H2Workspace,
    *,
    source_date_epoch: int,
    run: Callable[..., str] = checked_run,
) -> None:
    environment = _frontend_environment(workspace, source_date_epoch)
    for argv in (
        ["npm", "ci", "--ignore-scripts"],
        ["npm", "run", "typecheck"],
        ["npm", "run", "lint"],
    ):
        run(argv, cwd=workspace.frontend, extra_environment=environment)


def _clean_environment(extra: dict[str, str]) -> dict[str, str]:
    return {
        name: extra[name]
        for name in (
            *ENVIRONMENT_ALLOWLIST,
            "npm_config_cache",
            "TMPDIR",
            "PLAYWRIGHT_BROWSERS_PATH",
        )
    }


def _run_process(
    argv: list[str], *, cwd: Path, extra_environment: dict[str, str], offline: bool
) -> tuple[str, int]:
    environment = _clean_environment(extra_environment)
    command = argv
    trace: Path | None = None
    if offline:
        environment.update(
            {
                "npm_config_offline": "true",
                "HTTP_PROXY": "http://127.0.0.1:9",
                "HTTPS_PROXY": "http://127.0.0.1:9",
                "ALL_PROXY": "http://127.0.0.1:9",
                "NO_PROXY": "",
            }
        )
        strace = shutil.which("strace")
        if strace is None:
            raise CandidateError("H2 cannot observe post-install network attempts")
        with tempfile.NamedTemporaryFile(
            prefix="h2-network-",
            suffix=".trace",
            dir=environment["TMPDIR"],
            delete=False,
        ) as trace_file:
            trace = Path(trace_file.name)
        command = [
            strace,
            "-f",
            "-qq",
            "-e",
            "trace=network",
            "-o",
            str(trace),
            *command,
        ]
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=environment,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    external_requests = 0
    if trace is not None:
        try:
            external_requests = _external_connect_count(trace)
        finally:
            trace.unlink(missing_ok=True)
        if external_requests:
            raise CandidateError(
                f"H2 observed {external_requests} post-install external network attempt(s)"
            )
    if completed.returncode != 0:
        raise subprocess.CalledProcessError(
            completed.returncode, command, output=completed.stdout
        )
    return completed.stdout.strip(), external_requests


def _external_connect_count(trace: Path) -> int:
    count = 0
    for line in trace.read_text(encoding="utf-8", errors="strict").splitlines():
        if (
            not any(call in line for call in ("connect(", "sendto(", "sendmsg("))
            or "sa_family=AF_UNIX" in line
        ):
            continue
        address: str | None = None
        ipv4 = re.search(r'inet_addr\("([^\"]+)"\)', line)
        ipv6 = re.search(r'inet_pton\(AF_INET6, "([^\"]+)"', line)
        if ipv4 is not None:
            address = ipv4.group(1)
        elif ipv6 is not None:
            address = ipv6.group(1)
        elif "sa_family=AF_INET" in line:
            raise CandidateError("H2 could not classify an observed network connect")
        else:
            continue
        try:
            loopback = ipaddress.ip_address(address).is_loopback
        except ValueError as error:
            raise CandidateError("H2 observed a malformed network address") from error
        if not loopback:
            count += 1
    return count


def _download(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "eqiora-h2/1"})
    try:
        with (
            urllib.request.urlopen(request, timeout=180) as source,
            destination.open("wb") as output,
        ):
            shutil.copyfileobj(source, output)
    except (OSError, urllib.error.URLError) as error:
        destination.unlink(missing_ok=True)
        raise CandidateError(f"H2 input download failed: {url}: {error}") from error
    if not destination.is_file() or destination.stat().st_size == 0:
        raise CandidateError(f"H2 input download was empty: {url}")


def _download_exact_browser(destination: Path) -> None:
    request = urllib.request.Request(
        PLAYWRIGHT_BROWSER_URL, headers={"User-Agent": "eqiora-h2/1"}
    )
    digest = hashlib.sha256()
    stored = 0
    try:
        with (
            urllib.request.urlopen(request, timeout=180) as source,
            destination.open("wb") as output,
        ):
            while True:
                remaining = PLAYWRIGHT_BROWSER_ARCHIVE_BYTES - stored
                block = source.read(min(1024 * 1024, remaining + 1))
                if not block:
                    break
                if len(block) > remaining:
                    raise CandidateError(
                        "H2 managed browser response exceeds its raw byte ceiling"
                    )
                output.write(block)
                digest.update(block)
                stored += len(block)
    except (OSError, urllib.error.URLError, CandidateError) as error:
        destination.unlink(missing_ok=True)
        if isinstance(error, CandidateError):
            raise
        raise CandidateError(
            f"H2 input download failed: {PLAYWRIGHT_BROWSER_URL}: {error}"
        ) from error
    if (
        stored != PLAYWRIGHT_BROWSER_ARCHIVE_BYTES
        or digest.hexdigest() != PLAYWRIGHT_BROWSER_ARCHIVE_SHA256
    ):
        destination.unlink(missing_ok=True)
        raise CandidateError("H2 managed browser archive identity differs")


def _verify_sri(path: Path, integrity: str) -> None:
    prefix, separator, encoded = integrity.partition("-")
    if prefix != "sha512" or not separator:
        raise CandidateError("H2 lock integrity is not sha512 SRI")
    try:
        expected = base64.b64decode(encoded, validate=True)
    except ValueError as error:
        raise CandidateError("H2 lock integrity is malformed") from error
    if hashlib.sha512(path.read_bytes()).digest() != expected:
        raise CandidateError(f"H2 downloaded package differs from its SRI: {path.name}")


def _node_and_npm_identity(workspace: H2Workspace) -> tuple[Path, Path, Path]:
    node_raw = shutil.which("node")
    if node_raw is None:
        raise CandidateError("H2 requires the exact Node executable")
    node = Path(node_raw).resolve()
    environment = os.environ.copy()
    environment.pop("FORCE_COLOR", None)
    try:
        completed = subprocess.run(
            [str(node), "--version"],
            check=False,
            capture_output=True,
            env=environment,
        )
    except OSError as error:
        raise CandidateError("H2 Node invocation failed") from error
    expected_version = f"{NODE_VERSION}\n".encode("ascii")
    if (
        completed.returncode != 0
        or completed.stdout != expected_version
        or completed.stderr
    ):
        raise CandidateError(f"H2 requires Node {NODE_VERSION}")
    if file_sha256(node) != NODE_SHA256:
        raise CandidateError("H2 Node executable identity differs")
    tarball = workspace.root / f"npm-{NPM_VERSION}.tgz"
    _download(f"https://registry.npmjs.org/npm/-/npm-{NPM_VERSION}.tgz", tarball)
    observed = "sha512-" + base64.b64encode(
        hashlib.sha512(tarball.read_bytes()).digest()
    ).decode("ascii")
    if observed != NPM_INTEGRITY:
        raise CandidateError("H2 npm package integrity differs")
    tool_root = workspace.root / "npm-tool"
    tool_root.mkdir()
    with tarfile.open(tarball, "r:gz") as source:
        root = tool_root.resolve()
        members = source.getmembers()
        for member in members:
            target = (root / member.name).resolve()
            if not target.is_relative_to(root) or not member.isfile():
                raise CandidateError("H2 npm package contains a non-regular path")
        source.extractall(tool_root, members=members)
    npm_cli = tool_root / "package/bin/npm-cli.js"
    if not npm_cli.is_file():
        raise CandidateError("H2 npm package omits its CLI")
    executable_root = workspace.root / "tool-bin"
    executable_root.mkdir()
    npm = executable_root / "npm"
    npm.symlink_to(npm_cli)
    environment = os.environ.copy()
    environment["PATH"] = os.pathsep.join((str(node.parent), str(executable_root)))
    observed_version = subprocess.run(
        [str(npm), "--version"],
        check=True,
        text=True,
        capture_output=True,
        env=environment,
    ).stdout.strip()
    if observed_version != NPM_VERSION:
        raise CandidateError(f"H2 requires npm {NPM_VERSION}")
    return node, tarball, npm


def _wheel_metadata(path: Path) -> tuple[str, str]:
    with zipfile.ZipFile(path) as archive:
        filename_parts = path.name.split("-")
        if len(filename_parts) < 5:
            raise CandidateError(f"Python wheel filename is malformed: {path.name}")
        dist_info = f"{filename_parts[0]}-{filename_parts[1]}.dist-info"
        metadata_path = f"{dist_info}/METADATA"
        metadata_names = [name for name in archive.namelist() if name == metadata_path]
        if len(metadata_names) != 1:
            raise CandidateError(f"Python wheel has ambiguous metadata: {path.name}")
        message = BytesParser().parsebytes(archive.read(metadata_names[0]))
        name = message.get("Name")
        version = message.get("Version")
        if not name or not version:
            raise CandidateError(f"Python wheel omits name or version: {path.name}")
        return name, version


def _acquire_python_wheels(
    workspace: H2Workspace,
) -> tuple[tuple[dict[str, object], ...], int]:
    interpreter_raw = shutil.which("python3.13")
    if interpreter_raw is None:
        raise CandidateError("H2 requires CPython 3.13 for the frozen host inputs")
    interpreter = Path(interpreter_raw).resolve()
    observed = subprocess.run(
        [
            str(interpreter),
            "-I",
            "-c",
            "import platform,sys;print(platform.python_implementation());print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ],
        check=True,
        text=True,
        capture_output=True,
    ).stdout.splitlines()
    if observed != ["CPython", "3.13"]:
        raise CandidateError("H2 Python host is not CPython 3.13")
    destination = workspace.root / "python-wheels"
    destination.mkdir()
    subprocess.run(
        [
            str(interpreter),
            "-I",
            "-m",
            "pip",
            "download",
            "--disable-pip-version-check",
            "--only-binary=:all:",
            "--dest",
            str(destination),
            "marimo==0.23.16",
        ],
        cwd=workspace.root,
        check=True,
        env={
            "HOME": str(workspace.home),
            "PATH": os.environ.get("PATH", ""),
            "TMPDIR": str(workspace.temporary),
        },
    )
    paths = sorted(destination.iterdir(), key=lambda item: _utf8(item.name))
    if not paths or any(
        path.suffix != ".whl" or path.is_symlink() or not path.is_file()
        for path in paths
    ):
        raise CandidateError("H2 Python resolution did not produce only wheels")
    if len(paths) > PYTHON_WHEEL_COUNT_LIMIT:
        raise CandidateError("H2 Python resolution exceeds the wheel count ceiling")
    resolved_bytes = sum(path.stat().st_size for path in paths)
    if resolved_bytes > PYTHON_WHEEL_BYTES_LIMIT:
        raise CandidateError("H2 Python resolution exceeds the raw byte ceiling")
    records: list[dict[str, object]] = []
    names: set[str] = set()
    for path in paths:
        name, version = _wheel_metadata(path)
        normalized = re.sub(r"[-_.]+", "-", name).lower()
        if normalized in names:
            raise CandidateError(f"H2 Python resolution repeats {name}")
        names.add(normalized)
        records.append(
            {
                "name": name,
                "version": version,
                "filename": _basename(path.name),
                "sha256": file_sha256(path),
            }
        )
    if "marimo" not in names:
        raise CandidateError("H2 Python host resolution omits a frozen direct input")
    records.sort(key=lambda item: _utf8(str(item["filename"])))
    return tuple(records), resolved_bytes


def _lock_entry(lock: dict[str, Any], path: str) -> dict[str, Any]:
    value = lock.get("packages", {}).get(path)
    if not isinstance(value, dict):
        raise CandidateError(f"H2 lock omits {path}")
    return value


def _package_manifest_from_archive(archive: Path, lock_path: str) -> dict[str, Any]:
    package_basename = lock_path.rstrip("/").rsplit("/", 1)[-1]
    manifest_names = {
        "package/package.json",
        f"{package_basename}/package.json",
    }
    with tarfile.open(archive, "r:gz") as source:
        matches = [
            member
            for member in source.getmembers()
            if member.name in manifest_names and member.isfile()
        ]
        if len(matches) != 1:
            raise CandidateError(f"H2 package omits package.json: {lock_path}")
        extracted = source.extractfile(matches[0])
        if extracted is None:
            raise CandidateError(f"H2 cannot read package.json: {lock_path}")
        value = json.loads(extracted.read().decode("utf-8"))
    if not isinstance(value, dict):
        raise CandidateError(f"H2 package manifest is not an object: {lock_path}")
    return value


def _safe_extract_registry_package(archive: Path, destination: Path) -> Path:
    destination.mkdir()
    root = destination.resolve()
    with tarfile.open(archive, "r:gz") as source:
        members = source.getmembers()
        for member in members:
            target = (root / member.name).resolve()
            if not target.is_relative_to(root) or not (
                member.isfile() or member.isdir()
            ):
                raise CandidateError("H2 registry package contains an unsafe path")
        source.extractall(destination, members=members)
    package = destination / "package"
    if not package.is_dir() or package.is_symlink():
        raise CandidateError("H2 registry package omits its package root")
    return package


def _prefetch_lock_packages(
    workspace: H2Workspace, lock: dict[str, Any]
) -> tuple[
    tuple[tuple[str, dict[str, Any]], ...],
    tuple[tuple[str, dict[str, Any]], ...],
    Path,
    int,
]:
    packages = lock.get("packages")
    if not isinstance(packages, dict):
        raise CandidateError("H2 package lock omits its package inventory")
    locked_package_count = sum(lock_path != "" for lock_path in packages)
    if locked_package_count > LOCKED_PACKAGE_COUNT_LIMIT:
        raise CandidateError("H2 package lock exceeds the package entry ceiling")
    archive_root = workspace.root / "lock-packages"
    archive_root.mkdir()
    manifests: list[tuple[str, dict[str, Any]]] = []
    packuments: list[tuple[str, dict[str, Any]]] = []
    playwright_core: Path | None = None
    acquired_bytes = 0
    package_names: set[str] = set()
    for lock_path, value in packages.items():
        if lock_path == "":
            continue
        if not isinstance(value, dict):
            raise CandidateError(f"H2 lock entry is not an object: {lock_path}")
        package_names.add(_package_name(lock_path, value))
    packument_root = workspace.root / "lock-packuments"
    packument_root.mkdir()
    for package_name in sorted(package_names, key=_utf8):
        destination = packument_root / (
            hashlib.sha256(package_name.encode("utf-8")).hexdigest() + ".json"
        )
        encoded_name = urllib.parse.quote(package_name, safe="@")
        _download(f"https://registry.npmjs.org/{encoded_name}", destination)
        acquired_bytes += destination.stat().st_size
        if acquired_bytes > LOCKED_PACKAGE_BYTES_LIMIT:
            destination.unlink(missing_ok=True)
            raise CandidateError("H2 lock acquisition exceeds the raw byte ceiling")
        try:
            packument = json.loads(destination.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
            raise CandidateError(
                f"H2 registry packument is malformed: {package_name}"
            ) from error
        if (
            not isinstance(packument, dict)
            or packument.get("name") != package_name
            or not isinstance(packument.get("versions"), dict)
        ):
            raise CandidateError(
                f"H2 registry packument identity differs: {package_name}"
            )
        packuments.append((package_name, packument))
    for lock_path, value in sorted(packages.items(), key=lambda item: _utf8(item[0])):
        if lock_path == "":
            continue
        if not isinstance(value, dict):
            raise CandidateError(f"H2 lock entry is not an object: {lock_path}")
        resolved = value.get("resolved")
        integrity = value.get("integrity")
        if (
            not isinstance(resolved, str)
            or not resolved.startswith("https://registry.npmjs.org/")
            or not isinstance(integrity, str)
        ):
            raise CandidateError(f"H2 lock package is not registry-bound: {lock_path}")
        archive = archive_root / (
            hashlib.sha256(lock_path.encode("utf-8")).hexdigest() + ".tgz"
        )
        _download(resolved, archive)
        acquired_bytes += archive.stat().st_size
        if acquired_bytes > LOCKED_PACKAGE_BYTES_LIMIT:
            archive.unlink(missing_ok=True)
            raise CandidateError("H2 lock acquisition exceeds the raw byte ceiling")
        _verify_sri(archive, integrity)
        manifests.append(
            (lock_path, _package_manifest_from_archive(archive, lock_path))
        )
        if lock_path == "node_modules/playwright-core":
            playwright_core = _safe_extract_registry_package(
                archive, workspace.root / "playwright-core-package"
            )
    if playwright_core is None:
        raise CandidateError("H2 lock prefetch omits playwright-core")
    return tuple(manifests), tuple(packuments), playwright_core, acquired_bytes


def _browser_archive_inventory(
    archive_path: Path,
) -> tuple[dict[str, object], ...]:
    with zipfile.ZipFile(archive_path) as archive:
        members = archive.infolist()
        names = [member.filename for member in members]
        if len(names) != len(set(names)):
            raise CandidateError("managed browser archive repeats a path")
        records = tuple(
            sorted(
                (
                    {
                        "path": member.filename,
                        "kind": "directory" if member.is_dir() else "file",
                        "external_attr": member.external_attr,
                        "compression": member.compress_type,
                        "compressed_size": member.compress_size,
                        "expanded_size": member.file_size,
                        "crc32": f"{member.CRC:08x}",
                    }
                    for member in members
                ),
                key=lambda item: _utf8(str(item["path"])),
            )
        )
    expanded_bytes = sum(int(record["expanded_size"]) for record in records)
    largest = max((int(record["expanded_size"]) for record in records), default=0)
    if (
        len(records) != PLAYWRIGHT_BROWSER_MEMBER_COUNT
        or expanded_bytes != PLAYWRIGHT_BROWSER_EXPANDED_BYTES
        or largest != PLAYWRIGHT_BROWSER_LARGEST_MEMBER_BYTES
        or structured_sha256(records) != PLAYWRIGHT_BROWSER_INVENTORY_SHA256
    ):
        raise CandidateError("H2 managed browser archive inventory differs")
    largest_records = tuple(
        record for record in records if int(record["expanded_size"]) == largest
    )
    if (
        len(largest_records) != 1
        or largest_records[0]["path"] != PLAYWRIGHT_BROWSER_MEMBER
        or largest_records[0]["kind"] != "file"
    ):
        raise CandidateError("H2 managed browser executable inventory differs")
    return records


def _safe_extract_zip(archive_path: Path, destination: Path) -> None:
    if destination.exists() or destination.is_symlink():
        raise CandidateError("managed browser extraction directory must be absent")
    inventory = _browser_archive_inventory(archive_path)
    expected_streamed_total = sum(
        int(record["expanded_size"])
        for record in inventory
        if record["kind"] == "file"
    )
    destination.mkdir()
    root = destination.resolve()
    streamed_total = 0
    try:
        with zipfile.ZipFile(archive_path) as archive:
            for member in archive.infolist():
                relative = PurePosixPath(member.filename)
                path = (root / member.filename).resolve()
                mode = member.external_attr >> 16
                if (
                    not member.filename
                    or "\\" in member.filename
                    or relative.is_absolute()
                    or any(part in {"", ".", ".."} for part in relative.parts)
                    or not path.is_relative_to(root)
                    or stat.S_ISLNK(mode)
                ):
                    raise CandidateError(
                        "managed browser archive contains an unsafe path"
                    )
                if member.is_dir():
                    if mode and not stat.S_ISDIR(mode):
                        raise CandidateError(
                            "managed browser archive contains a non-directory entry"
                        )
                    path.mkdir(parents=True, exist_ok=True)
                    continue
                if member.file_size > PLAYWRIGHT_BROWSER_LARGEST_MEMBER_BYTES or (
                    mode and not stat.S_ISREG(mode)
                ):
                    raise CandidateError(
                        "managed browser archive contains an oversized or non-regular entry"
                    )
                path.parent.mkdir(parents=True, exist_ok=True)
                streamed_member = 0
                with archive.open(member) as source, path.open("wb") as output:
                    while block := source.read(1024 * 1024):
                        streamed_member += len(block)
                        streamed_total += len(block)
                        if (
                            streamed_member > PLAYWRIGHT_BROWSER_LARGEST_MEMBER_BYTES
                            or streamed_total > PLAYWRIGHT_BROWSER_EXPANDED_BYTES
                        ):
                            raise CandidateError(
                                "managed browser expansion exceeds its byte ceiling"
                            )
                        output.write(block)
                if streamed_member != member.file_size:
                    raise CandidateError(
                        "managed browser member expansion differs from its inventory"
                    )
                if mode:
                    path.chmod(stat.S_IMODE(mode))
        if streamed_total != expected_streamed_total:
            raise CandidateError("managed browser expansion is incomplete")
    except BaseException:
        shutil.rmtree(destination, ignore_errors=True)
        raise


def _acquire_browser(
    workspace: H2Workspace,
    node: Path,
    lock: dict[str, Any],
    playwright_core: Path,
) -> tuple[Path, Path, str, str, str]:
    test = _lock_entry(lock, "node_modules/@playwright/test")
    core = _lock_entry(lock, "node_modules/playwright-core")
    test_integrity = str(test.get("integrity", ""))
    core_integrity = str(core.get("integrity", ""))
    if (
        test.get("version") != "1.62.1"
        or test_integrity != PLAYWRIGHT_TEST_INTEGRITY
        or core.get("version") != "1.62.1"
        or core_integrity != PLAYWRIGHT_CORE_INTEGRITY
    ):
        raise CandidateError("H2 Playwright lock identity differs")
    expected_package = workspace.root / "playwright-core-package" / "package"
    if (
        playwright_core != expected_package
        or playwright_core.is_symlink()
        or not playwright_core.is_dir()
    ):
        raise CandidateError("H2 Playwright package is not the verified extraction")
    package_json = playwright_core / "package.json"
    browsers = playwright_core / "browsers.json"
    core_bundle = playwright_core / "lib/coreBundle.js"
    expected_files = (
        (package_json, PLAYWRIGHT_CORE_PACKAGE_SHA256),
        (browsers, BROWSERS_JSON_SHA256),
        (core_bundle, PLAYWRIGHT_CORE_BUNDLE_SHA256),
    )
    if any(
        path.is_symlink() or not path.is_file() or file_sha256(path) != digest
        for path, digest in expected_files
    ):
        raise CandidateError("H2 Playwright package byte identity differs")
    document = json.loads(browsers.read_text(encoding="utf-8"))
    matches = [
        item
        for item in document.get("browsers", [])
        if item.get("name") == "chromium-headless-shell"
    ]
    if (
        len(matches) != 1
        or matches[0].get("revision") != "1234"
        or matches[0].get("browserVersion") != "151.0.7922.34"
    ):
        raise CandidateError("H2 managed Chromium identity differs")
    browser_cache = workspace.browser_cache.resolve()
    if (
        workspace.browser_cache.is_symlink()
        or not browser_cache.is_dir()
        or any(browser_cache.iterdir())
    ):
        raise CandidateError(
            "H2 managed browser cache directory must be exact, initially empty, "
            "and its extraction directory absent"
        )
    object.__setattr__(workspace, "browser_cache", browser_cache)
    extracted = browser_cache / "chromium_headless_shell-1234"
    executable = extracted / PLAYWRIGHT_BROWSER_MEMBER
    expected_observation = {
        "name": "chromium-headless-shell",
        "browserName": "chromium",
        "revision": "1234",
        "browserVersion": "151.0.7922.34",
        "installType": "download-by-default",
        "directory": str(extracted),
        "executablePath": str(executable),
        "downloadURLs": [PLAYWRIGHT_BROWSER_URL],
    }
    program = (
        "const p=require('path'),r=process.argv[1];"
        "const e=require(p.join(r,'lib/coreBundle.js')).registry.registry"
        ".findExecutable('chromium-headless-shell');"
        "process.stdout.write(JSON.stringify({name:e.name,"
        "browserName:e.browserName,revision:e.revision,"
        "browserVersion:e.browserVersion,installType:e.installType,"
        "directory:e.directory,executablePath:e.executablePath(),"
        "downloadURLs:e.downloadURLs}));"
    )

    def probe() -> dict[str, object]:
        environment = _clean_environment(_frontend_environment(workspace, 0))
        environment["PLAYWRIGHT_BROWSERS_PATH"] = str(browser_cache)
        completed = subprocess.run(
            [str(node), "-e", program, str(playwright_core)],
            cwd=workspace.frontend,
            check=True,
            text=True,
            capture_output=True,
            env=environment,
        )
        observed = json.loads(completed.stdout)
        if not isinstance(observed, dict) or observed != expected_observation:
            raise CandidateError("H2 Playwright registry observation differs")
        return observed

    probe()
    archive = workspace.root / "chromium-headless-shell-1234.zip"
    if archive.exists() or archive.is_symlink():
        raise CandidateError("H2 managed browser archive path must be absent")
    _download_exact_browser(archive)
    _browser_archive_inventory(archive)
    with zipfile.ZipFile(archive) as browser_zip:
        executable_members = [
            member
            for member in browser_zip.infolist()
            if PurePosixPath(member.filename).name
            in {"headless_shell", "chrome-headless-shell"}
        ]
        if (
            len(executable_members) != 1
            or executable_members[0].filename != PLAYWRIGHT_BROWSER_MEMBER
            or not stat.S_ISREG(executable_members[0].external_attr >> 16)
        ):
            raise CandidateError(
                "H2 managed browser archive has an ambiguous executable"
            )
    _safe_extract_zip(archive, extracted)
    if (
        executable.is_symlink()
        or not executable.is_file()
        or executable.stat().st_nlink != 1
    ):
        raise CandidateError("H2 managed browser executable is not one regular file")
    executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
    archive_sha256 = file_sha256(archive)
    executable_sha256 = file_sha256(executable)
    if (
        archive_sha256 != PLAYWRIGHT_BROWSER_ARCHIVE_SHA256
        or executable_sha256 != PLAYWRIGHT_BROWSER_EXECUTABLE_SHA256
    ):
        raise CandidateError("H2 managed browser byte identity differs")
    version = subprocess.run(
        [str(executable), "--version"], check=True, text=True, capture_output=True
    ).stdout.strip()
    if version not in {
        "HeadlessChrome 151.0.7922.34",
        "Google Chrome 151.0.7922.34",
        "Google Chrome for Testing 151.0.7922.34",
        "Chromium 151.0.7922.34",
    }:
        raise CandidateError(f"H2 managed browser version differs: {version}")
    probe()
    if (
        file_sha256(archive) != archive_sha256
        or file_sha256(executable) != executable_sha256
    ):
        raise CandidateError("H2 managed browser changed after acquisition")
    return archive, executable, "linux-x86_64", test_integrity, core_integrity


def acquire_inputs(workspace: H2Workspace) -> AcquiredInputs:
    node, npm_tarball, npm = _node_and_npm_identity(workspace)
    lock = json.loads(
        (workspace.frontend / "package-lock.json").read_text(encoding="utf-8")
    )
    (
        package_manifests,
        package_packuments,
        playwright_core,
        locked_package_bytes,
    ) = _prefetch_lock_packages(workspace, lock)
    python_wheels, python_wheel_bytes = _acquire_python_wheels(workspace)
    archive, executable, browser_platform, test_integrity, core_integrity = (
        _acquire_browser(workspace, node, lock, playwright_core)
    )
    browser_records = _browser_archive_inventory(archive)
    return AcquiredInputs(
        node=node,
        npm=npm,
        npm_tarball=npm_tarball,
        browser_archive=archive,
        browser_executable=executable,
        browser_archive_sha256=file_sha256(archive),
        browser_executable_sha256=file_sha256(executable),
        browser_platform=browser_platform,
        playwright_test_integrity=test_integrity,
        playwright_core_integrity=core_integrity,
        python_wheels=python_wheels,
        package_manifests=package_manifests,
        package_packuments=package_packuments,
        locked_package_bytes=locked_package_bytes,
        python_wheel_bytes=python_wheel_bytes,
        browser_member_count=len(browser_records),
        browser_expanded_regular_bytes=sum(
            int(record["expanded_size"])
            for record in browser_records
            if record["kind"] == "file"
        ),
    )


def _observe_output(
    workspace: H2Workspace,
    acquisition: AcquiredInputs,
    external_request_count: int,
) -> RunObservation:
    if (
        acquisition.browser_archive.is_symlink()
        or not acquisition.browser_archive.is_file()
        or file_sha256(acquisition.browser_archive)
        != acquisition.browser_archive_sha256
        or acquisition.browser_executable.is_symlink()
        or not acquisition.browser_executable.is_file()
        or file_sha256(acquisition.browser_executable)
        != acquisition.browser_executable_sha256
    ):
        raise CandidateError("H2 acquired browser changed during frontend execution")
    if workspace.output.exists():
        raise CandidateError("H2 host validation unexpectedly emitted build output")
    return RunObservation(
        external_request_count,
        acquisition,
    )


def _package_name(lock_path: str, value: dict[str, Any]) -> str:
    name = value.get("name")
    if isinstance(name, str) and name:
        return name
    suffix = lock_path.removeprefix("node_modules/")
    parts = suffix.split("node_modules/")[-1].split("/")
    return "/".join(parts[:2]) if parts[0].startswith("@") else parts[0]


def _package_manifest(
    workspace: H2Workspace,
    lock_path: str,
    prefetched: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    cached = prefetched.get(lock_path)
    if cached is None:
        raise CandidateError(f"H2 package was not prefetched: {lock_path}")
    installed = workspace.frontend / lock_path / "package.json"
    if installed.is_file():
        observed = json.loads(installed.read_text(encoding="utf-8"))
        if observed != cached:
            raise CandidateError(
                f"H2 installed package differs from prefetched bytes: {lock_path}"
            )
    return cached


def _lifecycle_declarations(
    manifest: dict[str, Any], lock_path: str, source: str
) -> tuple[tuple[str, str], ...]:
    scripts = manifest.get("scripts", {})
    if not isinstance(scripts, dict):
        raise CandidateError(f"H2 {source} package scripts are malformed: {lock_path}")
    declarations: list[tuple[str, str]] = []
    for hook, command in scripts.items():
        if hook not in LIFECYCLE_NAMES:
            continue
        if not isinstance(command, str) or not command:
            raise CandidateError(
                f"H2 {source} lifecycle command is malformed: {lock_path} {hook}"
            )
        declarations.append((hook, command))
    declarations.sort(key=lambda item: (_utf8(item[0]), _utf8(item[1])))
    return tuple(declarations)


def _frontend_inputs(
    extracted: Path,
    workspace: H2Workspace,
    prefetched_manifests: tuple[tuple[str, dict[str, Any]], ...],
    prefetched_packuments: tuple[tuple[str, dict[str, Any]], ...],
) -> tuple[
    tuple[dict[str, object], ...],
    tuple[dict[str, object], ...],
    tuple[dict[str, object], ...],
    tuple[dict[str, object], ...],
]:
    root = extracted / FRONTEND
    prefetched = dict(prefetched_manifests)
    packuments = dict(prefetched_packuments)
    packument_names = [name for name, _ in prefetched_packuments]
    if (
        packument_names != sorted(packument_names, key=_utf8)
        or len(packument_names) != len(set(packument_names))
    ):
        raise CandidateError("H2 registry packument authority is unsorted or duplicate")
    inventory = _regular_tree_inventory(root)
    configs = tuple(
        {"relative_path": name, "sha256": file_sha256(root / name)}
        for name in CONFIG_NAMES
        if (root / name).is_file()
    )
    if {item["relative_path"] for item in configs} != set(CONFIG_NAMES):
        raise CandidateError("H2 frontend configuration inventory is incomplete")
    package = json.loads((root / "package.json").read_text(encoding="utf-8"))
    if package.get("packageManager") != f"npm@{NPM_VERSION}" or package.get(
        "engines"
    ) != {"node": "24.18.1"}:
        raise CandidateError("H2 frontend package manager identity differs")
    dependencies = package.get("dependencies", {})
    dev_dependencies = package.get("devDependencies")
    if (
        dependencies != {}
        or not isinstance(dev_dependencies, dict)
        or {**dependencies, **dev_dependencies} != DIRECT_PINS
    ):
        raise CandidateError("H2 frontend direct dependency pins differ")
    pins = tuple(
        {"name": name, "version": version}
        for name, version in sorted(
            DIRECT_PINS.items(), key=lambda item: (_utf8(item[0]), _utf8(item[1]))
        )
    )
    lock = json.loads((root / "package-lock.json").read_text(encoding="utf-8"))
    if lock.get("lockfileVersion") != 3 or not isinstance(lock.get("packages"), dict):
        raise CandidateError("H2 frontend lock is not npm lockfile version 3")
    root_lock = lock["packages"].get("")
    if (
        not isinstance(root_lock, dict)
        or root_lock.get("dependencies", {}) != dependencies
        or root_lock.get("devDependencies") != dev_dependencies
    ):
        raise CandidateError("H2 frontend lock root differs from package.json")
    expected_packument_names = {
        _package_name(lock_path, value)
        for lock_path, value in lock["packages"].items()
        if lock_path != "" and isinstance(value, dict)
    }
    if set(packuments) != expected_packument_names:
        raise CandidateError("H2 registry packument authority is incomplete")
    locked: list[dict[str, object]] = []
    for lock_path, value in sorted(
        lock["packages"].items(), key=lambda item: _utf8(item[0])
    ):
        if lock_path == "":
            continue
        if not isinstance(value, dict):
            raise CandidateError(f"H2 lock entry is not an object: {lock_path}")
        name = _package_name(lock_path, value)
        version = value.get("version")
        resolved = value.get("resolved")
        integrity = value.get("integrity")
        if (
            not isinstance(version, str)
            or not version
            or not isinstance(resolved, str)
            or not resolved.startswith("https://registry.npmjs.org/")
            or not isinstance(integrity, str)
            or not integrity.startswith("sha512-")
            or value.get("link") is True
        ):
            raise CandidateError(
                f"H2 lock entry is not registry-and-integrity bound: {lock_path}"
            )
        manifest = _package_manifest(workspace, lock_path, prefetched)
        if manifest.get("name") != name or manifest.get("version") != version:
            raise CandidateError(f"H2 installed package differs from lock: {lock_path}")
        packument = packuments[name]
        versions = packument.get("versions")
        packument_manifest = (
            versions.get(version) if isinstance(versions, dict) else None
        )
        if (
            not isinstance(packument_manifest, dict)
            or packument_manifest.get("name") != name
            or packument_manifest.get("version") != version
        ):
            raise CandidateError(
                f"H2 registry packument omits exact version authority: {lock_path}"
            )
        merged: dict[tuple[str, str], set[str]] = {}
        for source, source_manifest in (
            ("packument", packument_manifest),
            ("tarball", manifest),
        ):
            for identity in _lifecycle_declarations(
                source_manifest, lock_path, source
            ):
                merged.setdefault(identity, set()).add(source)
        has_install_script = value.get("hasInstallScript", False)
        if not isinstance(has_install_script, bool):
            raise CandidateError(
                f"H2 lockfile hasInstallScript is malformed: {lock_path}"
            )
        packument_install = tuple(
            identity
            for identity, sources in merged.items()
            if identity[0] in INSTALL_CLASS_LIFECYCLE_NAMES
            and "packument" in sources
        )
        if has_install_script and not packument_install:
            raise CandidateError(
                f"H2 lockfile install provenance lacks a packument command: {lock_path}"
            )
        if packument_install and not has_install_script:
            raise CandidateError(
                f"H2 packument install provenance lacks a lockfile marker: {lock_path}"
            )
        if has_install_script:
            for identity in packument_install:
                merged[identity].add("lockfile")
        lifecycle = tuple(
            {
                "name": hook,
                "command": command,
                "sources": sorted(sources, key=_utf8),
            }
            for (hook, command), sources in sorted(
                merged.items(), key=lambda item: (_utf8(item[0][0]), _utf8(item[0][1]))
            )
        )
        locked.append(
            {
                "lock_path": _relative_path(lock_path),
                "name": name,
                "version": version,
                "resolved": resolved,
                "integrity": integrity,
                "selected_optional": bool(
                    value.get("optional", False)
                    and (workspace.frontend / lock_path).is_dir()
                ),
                "lifecycle_scripts": list(lifecycle),
            }
        )
    return inventory, configs, pins, tuple(locked)


def _acquisition_member_map(
    field: str, value: object
) -> dict[str, object] | None:
    if not isinstance(value, tuple):
        return None
    members: dict[str, object] = {}
    for member in value:
        if field == "python_wheels":
            if not isinstance(member, dict) or not isinstance(
                member.get("filename"), str
            ):
                return None
            identity = member["filename"]
        else:
            if (
                not isinstance(member, tuple)
                or len(member) != 2
                or not isinstance(member[0], str)
            ):
                return None
            identity = member[0]
        if identity in members:
            return None
        members[identity] = member
    return members


def _acquisition_member_differences(
    field: str, first: object, second: object
) -> list[dict[str, str]]:
    first_members = _acquisition_member_map(field, first)
    second_members = _acquisition_member_map(field, second)
    if first_members is None or second_members is None:
        return []
    differences = []
    identities = sorted(first_members.keys() | second_members.keys(), key=_utf8)
    for identity in identities:
        first_member = first_members.get(identity, _ABSENT_MEMBER)
        second_member = second_members.get(identity, _ABSENT_MEMBER)
        if first_member == second_member:
            continue
        differences.append(
            {
                "identity": identity,
                "run_1_sha256": (
                    "absent"
                    if first_member is _ABSENT_MEMBER
                    else structured_sha256(first_member)
                ),
                "run_2_sha256": (
                    "absent"
                    if second_member is _ABSENT_MEMBER
                    else structured_sha256(second_member)
                ),
            }
        )
    return differences


def _acquisition_differences(
    first: AcquiredInputs, second: AcquiredInputs
) -> list[dict[str, object]]:
    differences = []
    for field in _ACQUISITION_IDENTITY_FIELDS:
        first_value = getattr(first, field)
        second_value = getattr(second, field)
        if first_value == second_value:
            continue
        difference: dict[str, object] = {
            "field": field,
            "run_1_sha256": structured_sha256(first_value),
            "run_2_sha256": structured_sha256(second_value),
        }
        if field in _ACQUISITION_COLLECTION_FIELDS:
            difference["members"] = _acquisition_member_differences(
                field, first_value, second_value
            )
        differences.append(difference)
    return differences


def _validate_abstract_resources(
    family: CandidateFamily,
    runs: tuple[RunObservation, RunObservation],
) -> None:
    source_member_count, source_bytes, source_largest_member_bytes = (
        _sdist_resource_usage(family.sdist)
    )
    acquired = runs[0].acquisition
    family_bytes = sum(int(record["size"]) for record in family.inventory)
    family_largest_member_bytes = max(
        (int(record["size"]) for record in family.inventory), default=0
    )
    locked_package_count = len(acquired.package_manifests)
    python_wheel_count = len(acquired.python_wheels)
    browser_records = _browser_archive_inventory(acquired.browser_archive)
    largest_browser_record = max(
        browser_records, key=lambda record: int(record["expanded_size"])
    )
    require_content_bound_resources(
        {
            "family_member_count": len(family.inventory),
            "family_largest_member_bytes": family_largest_member_bytes,
            "family_bytes": family_bytes,
            "source_member_count": source_member_count,
            "source_largest_member_bytes": source_largest_member_bytes,
            "source_bytes": source_bytes,
            "locked_package_count": locked_package_count,
            "locked_package_bytes": acquired.locked_package_bytes,
            "resolved_python_wheel_count": python_wheel_count,
            "resolved_python_wheel_bytes": acquired.python_wheel_bytes,
            "browser_archive_bytes": acquired.browser_archive.stat().st_size,
            "browser_archive_sha256": acquired.browser_archive_sha256,
            "browser_archive_member_count": acquired.browser_member_count,
            "browser_extracted_regular_bytes": (
                acquired.browser_expanded_regular_bytes
            ),
            "browser_largest_expanded_member_bytes": int(
                largest_browser_record["expanded_size"]
            ),
            "browser_largest_member": str(largest_browser_record["path"]),
            "browser_member_inventory_sha256": structured_sha256(browser_records),
            "browser_executable_sha256": acquired.browser_executable_sha256,
            "host_scenarios": len(runs),
        }
    )


def observe_h2(
    *,
    expected_commit: str,
    family: CandidateFamily,
    extracted: Path,
    workspaces: tuple[H2Workspace, H2Workspace],
    runs: tuple[RunObservation, RunObservation],
    source_date_epoch: int,
) -> dict[str, object]:
    acquired_identities = tuple(
        tuple(getattr(run.acquisition, field) for field in _ACQUISITION_IDENTITY_FIELDS)
        for run in runs
    )
    if acquired_identities[0] != acquired_identities[1]:
        document = {
            "differences": _acquisition_differences(
                runs[0].acquisition, runs[1].acquisition
            )
        }
        diagnostic = canonical_json_bytes(document).decode("utf-8")
        raise CandidateError(
            f"H2 isolated runs acquired different external inputs: {diagnostic}"
        )
    _validate_abstract_resources(family, runs)
    acquired = runs[0].acquisition
    source_inventory, configs, pins, locked = _frontend_inputs(
        extracted,
        workspaces[0],
        runs[0].acquisition.package_manifests,
        runs[0].acquisition.package_packuments,
    )
    libc_name, libc_version = platform.libc_ver()
    if (
        platform.system() != "Linux"
        or platform.machine().lower() not in {"x86_64", "amd64"}
        or libc_name != "glibc"
    ):
        raise CandidateError("H2 frozen host requires Linux x86-64 glibc")
    run_records = []
    for index, run in enumerate(runs, 1):
        run_records.append(
            {
                "isolated_directory_id": f"clean-run-{index}",
                "npm_ci_exit": 0,
                "validation_exit": 0,
                "external_request_count_after_npm_ci": run.external_request_count,
            }
        )
    receipt: dict[str, object] = {
        "probe": {
            "contract_sha256": CONTRACT_SHA256,
            "protected_base_sha": PROTECTED_BASE_SHA,
            "writer_revision": expected_commit,
            "verdict": "PASS",
        },
        "candidate": {
            "project": "eqiora",
            "version": family.version,
            "source_commit": expected_commit,
            "artifacts": list(family.inventory),
        },
        "environment": {
            "os": "Linux",
            "architecture": "x86_64",
            "libc": f"{libc_name}-{libc_version}",
            "node_version": NODE_VERSION,
            "node_executable_sha256": NODE_SHA256,
            "npm_version": NPM_VERSION,
            "npm_package_integrity": NPM_INTEGRITY,
            "locale": "C.UTF-8",
            "timezone": "UTC",
            "source_date_epoch": source_date_epoch,
            "environment_allowlist": list(ENVIRONMENT_ALLOWLIST),
        },
        "inputs": {
            "source_root_inventory": list(source_inventory),
            "package_json_sha256": file_sha256(extracted / FRONTEND / "package.json"),
            "package_lock_sha256": file_sha256(
                extracted / FRONTEND / "package-lock.json"
            ),
            "lockfile_version": 3,
            "config_inventory": list(configs),
            "direct_pins": list(pins),
            "locked_packages": list(locked),
        },
        "validation": {
            "npm_ci_command_argv": ["npm", "ci", "--ignore-scripts"],
            "offline_command_argv": [
                ["npm", "run", "typecheck"],
                ["npm", "run", "lint"],
            ],
            "network_policy": "registry-only-during-npm-ci;offline-after",
        },
        "clean_run_1": run_records[0],
        "clean_run_2": run_records[1],
        "comparison": {
            "acquired_inputs_equal": True,
            "diff": [],
        },
        "browser": {
            "playwright_test_integrity": acquired.playwright_test_integrity,
            "playwright_core_integrity": acquired.playwright_core_integrity,
            "browsers_json_sha256": BROWSERS_JSON_SHA256,
            "browser_name": "chromium",
            "revision": "1234",
            "browser_version": "151.0.7922.34",
            "platform": acquired.browser_platform,
            "downloaded_archive_sha256": acquired.browser_archive_sha256,
            "executable_sha256": acquired.browser_executable_sha256,
        },
        "python_host": {
            "python": "3.13",
            "resolved_environment_sha256": structured_sha256(acquired.python_wheels),
            "wheels": list(acquired.python_wheels),
        },
    }
    validate_h2_receipt(receipt)
    return receipt


def _keys(value: object, expected: Iterable[str], location: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != set(expected):
        raise CandidateError(f"H2 receipt {location} has the wrong closed keys")
    return value


def _array(value: object, location: str) -> list[Any]:
    if not isinstance(value, list):
        raise CandidateError(f"H2 receipt {location} is not an array")
    return value


def _json_integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _json_boolean(value: object) -> bool:
    return isinstance(value, bool)


def _sorted_unique(
    values: list[Any], key: Callable[[Any], object], location: str
) -> None:
    keys = [key(value) for value in values]
    if keys != sorted(keys) or len(keys) != len(set(keys)):
        raise CandidateError(f"H2 receipt {location} is unsorted or duplicate")


def _validate_file_records(values: object, location: str) -> list[dict[str, Any]]:
    records = _array(values, location)
    for record in records:
        item = _keys(record, ("relative_path", "mode", "size", "sha256"), location)
        _relative_path(item["relative_path"])
        if (
            any(
                not isinstance(item[name], int)
                or isinstance(item[name], bool)
                or item[name] < 0
                for name in ("mode", "size")
            )
            or SHA256_RE.fullmatch(item["sha256"]) is None
        ):
            raise CandidateError(f"H2 receipt {location} has an invalid file record")
    _sorted_unique(records, lambda item: _utf8(item["relative_path"]), location)
    return records


def validate_h2_receipt(receipt: object) -> None:
    root = _keys(
        receipt,
        (
            "probe",
            "candidate",
            "environment",
            "inputs",
            "validation",
            "clean_run_1",
            "clean_run_2",
            "comparison",
            "browser",
            "python_host",
        ),
        "root",
    )
    probe = _keys(
        root["probe"],
        ("contract_sha256", "protected_base_sha", "writer_revision", "verdict"),
        "probe",
    )
    if (
        probe["contract_sha256"] != CONTRACT_SHA256
        or probe["protected_base_sha"] != PROTECTED_BASE_SHA
        or probe["verdict"] != "PASS"
        or GIT_SHA_RE.fullmatch(probe["writer_revision"]) is None
    ):
        raise CandidateError("H2 receipt probe identity differs")
    candidate = _keys(
        root["candidate"],
        ("project", "version", "source_commit", "artifacts"),
        "candidate",
    )
    if (
        candidate["project"] != "eqiora"
        or candidate["source_commit"] != probe["writer_revision"]
        or not isinstance(candidate["version"], str)
    ):
        raise CandidateError("H2 receipt candidate identity differs")
    artifacts = _array(candidate["artifacts"], "candidate.artifacts")
    for artifact in artifacts:
        item = _keys(
            artifact, ("filename", "kind", "size", "sha256"), "candidate.artifacts[]"
        )
        _basename(item["filename"])
        if (
            item["kind"] not in {"sdist", "wheel"}
            or not isinstance(item["size"], int)
            or isinstance(item["size"], bool)
            or item["size"] <= 0
            or SHA256_RE.fullmatch(item["sha256"]) is None
        ):
            raise CandidateError("H2 receipt artifact record is invalid")
    _sorted_unique(
        artifacts, lambda item: _utf8(item["filename"]), "candidate.artifacts"
    )
    environment = _keys(
        root["environment"],
        (
            "os",
            "architecture",
            "libc",
            "node_version",
            "node_executable_sha256",
            "npm_version",
            "npm_package_integrity",
            "locale",
            "timezone",
            "source_date_epoch",
            "environment_allowlist",
        ),
        "environment",
    )
    if (
        environment["node_version"] != NODE_VERSION
        or environment["node_executable_sha256"] != NODE_SHA256
        or environment["npm_version"] != NPM_VERSION
        or environment["npm_package_integrity"] != NPM_INTEGRITY
        or environment["locale"] != "C.UTF-8"
        or environment["timezone"] != "UTC"
        or environment["environment_allowlist"] != list(ENVIRONMENT_ALLOWLIST)
        or not isinstance(environment["source_date_epoch"], int)
        or isinstance(environment["source_date_epoch"], bool)
    ):
        raise CandidateError("H2 receipt environment identity differs")
    inputs = _keys(
        root["inputs"],
        (
            "source_root_inventory",
            "package_json_sha256",
            "package_lock_sha256",
            "lockfile_version",
            "config_inventory",
            "direct_pins",
            "locked_packages",
        ),
        "inputs",
    )
    source_records = _validate_file_records(
        inputs["source_root_inventory"], "inputs.source_root_inventory"
    )
    if (
        not _json_integer(inputs["lockfile_version"])
        or inputs["lockfile_version"] != 3
        or any(
            SHA256_RE.fullmatch(inputs[name]) is None
            for name in ("package_json_sha256", "package_lock_sha256")
        )
    ):
        raise CandidateError("H2 receipt input identity differs")
    configs = _array(inputs["config_inventory"], "inputs.config_inventory")
    for item in configs:
        value = _keys(item, ("relative_path", "sha256"), "inputs.config_inventory[]")
        _relative_path(value["relative_path"])
        if SHA256_RE.fullmatch(value["sha256"]) is None:
            raise CandidateError("H2 receipt config hash is invalid")
    _sorted_unique(
        configs, lambda item: _utf8(item["relative_path"]), "inputs.config_inventory"
    )
    pins = _array(inputs["direct_pins"], "inputs.direct_pins")
    for item in pins:
        _keys(item, ("name", "version"), "inputs.direct_pins[]")
    _sorted_unique(
        pins,
        lambda item: (_utf8(item["name"]), _utf8(item["version"])),
        "inputs.direct_pins",
    )
    locked = _array(inputs["locked_packages"], "inputs.locked_packages")
    for item in locked:
        value = _keys(
            item,
            (
                "lock_path",
                "name",
                "version",
                "resolved",
                "integrity",
                "selected_optional",
                "lifecycle_scripts",
            ),
            "inputs.locked_packages[]",
        )
        _relative_path(value["lock_path"])
        if (
            not value["resolved"].startswith("https://registry.npmjs.org/")
            or not value["integrity"].startswith("sha512-")
            or not _json_boolean(value["selected_optional"])
        ):
            raise CandidateError("H2 receipt lock record is invalid")
        scripts = _array(value["lifecycle_scripts"], "lifecycle_scripts")
        for script in scripts:
            script_record = _keys(
                script, ("name", "command", "sources"), "lifecycle_scripts[]"
            )
            sources = _array(script_record["sources"], "lifecycle_scripts[].sources")
            if (
                not sources
                or any(
                    not isinstance(source, str) or source not in LIFECYCLE_SOURCES
                    for source in sources
                )
            ):
                raise CandidateError("H2 receipt lifecycle source is invalid")
            _sorted_unique(
                sources, _utf8, "lifecycle_scripts[].sources"
            )
            if script_record["name"] in INSTALL_CLASS_LIFECYCLE_NAMES:
                if "tarball" in sources:
                    raise CandidateError(
                        "H2 receipt contains a tarball install-class lifecycle script"
                    )
                if sources != ["lockfile", "packument"]:
                    raise CandidateError(
                        "H2 receipt install-class lifecycle provenance is partial"
                    )
            elif "lockfile" in sources:
                raise CandidateError(
                    "H2 receipt non-install lifecycle has lockfile provenance"
                )
        _sorted_unique(
            scripts,
            lambda script: (_utf8(script["name"]), _utf8(script["command"])),
            "lifecycle_scripts",
        )
    _sorted_unique(
        locked, lambda item: _utf8(item["lock_path"]), "inputs.locked_packages"
    )
    validation = _keys(
        root["validation"],
        (
            "npm_ci_command_argv",
            "offline_command_argv",
            "network_policy",
        ),
        "validation",
    )
    if (
        validation["npm_ci_command_argv"] != ["npm", "ci", "--ignore-scripts"]
        or validation["offline_command_argv"]
        != [["npm", "run", "typecheck"], ["npm", "run", "lint"]]
        or validation["network_policy"]
        != "registry-only-during-npm-ci;offline-after"
    ):
        raise CandidateError("H2 receipt validation predicate differs")
    for index, name in enumerate(("clean_run_1", "clean_run_2"), 1):
        run = _keys(
            root[name],
            (
                "isolated_directory_id",
                "npm_ci_exit",
                "validation_exit",
                "external_request_count_after_npm_ci",
            ),
            name,
        )
        if (
            run["isolated_directory_id"] != f"clean-run-{index}"
            or not _json_integer(run["npm_ci_exit"])
            or not _json_integer(run["validation_exit"])
            or not _json_integer(run["external_request_count_after_npm_ci"])
            or run["npm_ci_exit"] != 0
            or run["validation_exit"] != 0
            or run["external_request_count_after_npm_ci"] != 0
        ):
            raise CandidateError(f"H2 receipt {name} is not PASS")
    comparison = _keys(
        root["comparison"],
        (
            "acquired_inputs_equal",
            "diff",
        ),
        "comparison",
    )
    comparison_flags = (
        "acquired_inputs_equal",
    )
    if (
        any(
            not _json_boolean(comparison[name]) or comparison[name] is not True
            for name in comparison_flags
        )
        or comparison["diff"] != []
    ):
        raise CandidateError("H2 receipt comparison is not exact equality")
    browser = _keys(
        root["browser"],
        (
            "playwright_test_integrity",
            "playwright_core_integrity",
            "browsers_json_sha256",
            "browser_name",
            "revision",
            "browser_version",
            "platform",
            "downloaded_archive_sha256",
            "executable_sha256",
        ),
        "browser",
    )
    if (
        browser["playwright_test_integrity"] != PLAYWRIGHT_TEST_INTEGRITY
        or browser["playwright_core_integrity"] != PLAYWRIGHT_CORE_INTEGRITY
        or browser["browsers_json_sha256"] != BROWSERS_JSON_SHA256
        or browser["browser_name"] != "chromium"
        or browser["revision"] != "1234"
        or browser["browser_version"] != "151.0.7922.34"
        or browser["platform"] != "linux-x86_64"
        or browser["downloaded_archive_sha256"] != PLAYWRIGHT_BROWSER_ARCHIVE_SHA256
        or browser["executable_sha256"] != PLAYWRIGHT_BROWSER_EXECUTABLE_SHA256
    ):
        raise CandidateError("H2 receipt browser identity differs")
    host = _keys(
        root["python_host"],
        ("python", "resolved_environment_sha256", "wheels"),
        "python_host",
    )
    wheels = _array(host["wheels"], "python_host.wheels")
    for item in wheels:
        value = _keys(item, ("name", "version", "filename", "sha256"), "python_wheel")
        _basename(value["filename"])
        if SHA256_RE.fullmatch(value["sha256"]) is None:
            raise CandidateError("H2 receipt Python wheel hash is invalid")
    _sorted_unique(wheels, lambda item: _utf8(item["filename"]), "python_host.wheels")
    if (
        host["python"] != "3.13"
        or host["resolved_environment_sha256"] != structured_sha256(wheels)
        or structured_sha256(source_records)
        != structured_sha256(inputs["source_root_inventory"])
    ):
        raise CandidateError("H2 receipt Python or inventory framing differs")


def _require_empty_output(output: Path) -> None:
    resolved = output.resolve()
    if resolved == ROOT or resolved.is_relative_to(ROOT):
        raise CandidateError("H2 receipt output must remain outside the repository")
    if output.exists() and (
        not output.is_dir() or output.is_symlink() or any(output.iterdir())
    ):
        raise CandidateError("H2 receipt output directory must be initially empty")
    output.mkdir(parents=True, exist_ok=True)


def write_canonical_receipt(receipt: dict[str, object], output: Path) -> Path:
    if not output.is_dir() or output.is_symlink() or any(output.iterdir()):
        raise CandidateError("H2 receipt output directory must be empty")
    validate_h2_receipt(receipt)
    version = _basename(str(receipt["candidate"]["version"]))  # type: ignore[index]
    destination = output / f"eqiora-{version}-python-candidate-h2.json"
    payload = canonical_json_bytes(receipt)
    temporary: Path | None = None
    published = False
    try:
        with tempfile.NamedTemporaryFile(
            prefix=".h2-", dir=output, delete=False
        ) as staged:
            temporary = Path(staged.name)
            staged.write(payload)
            staged.flush()
            os.fsync(staged.fileno())
        os.replace(temporary, destination)
        published = True
        temporary = None
        descriptor = os.open(output, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    except BaseException:
        if published:
            destination.unlink(missing_ok=True)
            if destination.exists():
                raise CandidateError(
                    "H2 failed publication cleanup left final receipt bytes"
                )
        raise
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    if destination.read_bytes() != payload or tuple(output.iterdir()) != (destination,):
        raise CandidateError(
            "H2 canonical receipt publication was not atomic and exact"
        )
    return destination


def _current_revision() -> str:
    return checked_run(["git", "rev-parse", "HEAD"], capture=True)


def execute_h2(*, expected_commit: str, artifacts: Path, out: Path) -> Path:
    if GIT_SHA_RE.fullmatch(expected_commit) is None:
        raise CandidateError("H2 expected commit must be one full lowercase revision")
    current = _current_revision()
    try:
        source: SourceIdentity = source_identity()
    except CandidateError as error:
        if current != expected_commit:
            raise CandidateError(
                "H2 source revision differs from the expected commit"
            ) from error
        raise
    if source.commit != expected_commit:
        raise CandidateError(
            "H2 clean source commit differs from the expected revision"
        )
    family = admit_candidate_family(artifacts)
    entry_inventory = family_inventory(artifacts)
    if family.inventory and entry_inventory != family.inventory:
        raise CandidateError("H2 artifact family changed after admission")
    _require_empty_output(out)
    timestamp_revision = expected_commit if current == expected_commit else current
    source_date_epoch_text = checked_run(
        ["git", "show", "-s", "--format=%ct", timestamp_revision], capture=True
    )
    if not source_date_epoch_text.isascii() or not source_date_epoch_text.isdecimal():
        raise CandidateError("H2 source commit timestamp is malformed")
    source_date_epoch = int(source_date_epoch_text)
    scratch_parent = home_scratch_parent("python-candidate-h2")
    with tempfile.TemporaryDirectory(
        prefix="eqiora-h2-", dir=scratch_parent
    ) as temporary:
        scratch = Path(temporary)
        extracted = safe_extract_sdist(family.sdist, scratch / "source")
        if _retained_distribution_version(extracted) != family.version:
            raise CandidateError(
                "H2 artifact family version differs from retained Cargo authority"
            )
        workspaces = create_isolated_build_workspaces(scratch / "builds")
        for workspace in workspaces:
            stage_frontend(extracted, workspace)
        observations: list[RunObservation | None] = []
        for workspace in workspaces:
            acquired: AcquiredInputs | None = None
            tool_path: str | None = None
            external_requests = 0

            def run(argv: list[str], **kwargs: object) -> str:
                nonlocal acquired, tool_path, external_requests
                cwd = Path(kwargs["cwd"])
                environment = dict(kwargs["extra_environment"])  # type: ignore[arg-type]
                if argv == ["npm", "ci", "--ignore-scripts"]:
                    acquired = acquire_inputs(workspace)
                    tool_path = os.pathsep.join(
                        (
                            str(acquired.npm.parent),
                            str(acquired.node.parent),
                            environment["PATH"],
                        )
                    )
                    environment["PATH"] = tool_path
                    output, _ = _run_process(
                        argv, cwd=cwd, extra_environment=environment, offline=False
                    )
                    return output
                if acquired is None or tool_path is None:
                    raise CandidateError("H2 frontend command ran before exact npm ci")
                environment["PATH"] = tool_path
                output, observed = _run_process(
                    argv, cwd=cwd, extra_environment=environment, offline=True
                )
                external_requests += observed
                return output

            run_frontend_commands(
                workspace, source_date_epoch=source_date_epoch, run=run
            )
            if acquired is None:
                observations.append(None)
            else:
                observations.append(
                    _observe_output(workspace, acquired, external_requests)
                )
        receipt = observe_h2(
            expected_commit=expected_commit,
            family=family,
            extracted=extracted,
            workspaces=workspaces,
            runs=(observations[0], observations[1]),  # type: ignore[arg-type]
            source_date_epoch=source_date_epoch,
        )
        receipt_path = write_canonical_receipt(receipt, out)
    try:
        final_source = source_identity()
        if final_source.commit != expected_commit or _current_revision() != current:
            raise CandidateError("H2 source revision changed during execution")
        if family_inventory(artifacts) != entry_inventory:
            raise CandidateError("H2 artifact family changed during execution")
    except (CandidateError, OSError, subprocess.SubprocessError):
        receipt_path.unlink(missing_ok=True)
        raise
    return receipt_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-commit", required=True)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    arguments = parse_args()
    try:
        receipt = execute_h2(
            expected_commit=arguments.expected_commit,
            artifacts=arguments.artifacts,
            out=arguments.out,
        )
    except (
        CandidateError,
        OSError,
        ValueError,
        json.JSONDecodeError,
        subprocess.SubprocessError,
        tarfile.TarError,
        zipfile.BadZipFile,
    ) as error:
        print(f"H2 execution failed: {error}", file=sys.stderr)
        return 2
    print(
        json.dumps(
            {"receipt": str(receipt), "sha256": file_sha256(receipt)}, sort_keys=True
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
