#!/usr/bin/env python3
"""Validate one complete Eqiora Python candidate artifact set.

The manifest is the authority for filenames and hashes. This checker is used
both after an Actions artifact download and immediately before production
publication; it never rebuilds a candidate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


MANIFEST_FORMAT = "eqiora.python-distribution-candidate/v2"
FULL_SHA = re.compile(r"[0-9a-f]{40}")
SHA256 = re.compile(r"[0-9a-f]{64}")
REQUIRED_PROFILES = ("base", "jax", "matplotlib", "torch", "typing")


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
                f"cp{python}:packaged-fixed-reference-fsi-demo",
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
            "cp313:packaged-fixed-reference-fsi-still",
        }
    ),
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
    for profile in REQUIRED_PROFILES:
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
    parser.add_argument("--expected-commit")
    parser.add_argument("--expected-tag")
    arguments = parser.parse_args()

    try:
        verify_manifest_hash(arguments.manifest, arguments.manifest_sha256)
        candidate = load_candidate(arguments.manifest)
        verify_artifacts(candidate, arguments.artifacts)
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
