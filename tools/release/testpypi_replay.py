#!/usr/bin/env python3
"""Download an exact TestPyPI version and verify it against a candidate."""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable

from candidate_manifest import (
    Candidate,
    ManifestError,
    file_sha256,
    load_candidate,
    verify_artifacts,
    verify_manifest_hash,
)


TEST_PYPI_JSON = "https://test.pypi.org/pypi/eqiora/{version}/json"
ALLOWED_FILE_HOSTS = {"test-files.pythonhosted.org"}


def release_files(payload: Any, candidate: Candidate) -> dict[str, str]:
    """Validate TestPyPI JSON and return exact filename-to-URL mappings."""
    if not isinstance(payload, dict):
        raise ManifestError("TestPyPI response must be an object")
    info = payload.get("info")
    urls = payload.get("urls")
    if not isinstance(info, dict) or not isinstance(urls, list):
        raise ManifestError("TestPyPI response omits info or urls")
    if str(info.get("name", "")).lower() != "eqiora":
        raise ManifestError("TestPyPI response has the wrong project")
    if info.get("version") != candidate.version:
        raise ManifestError("TestPyPI response has the wrong version")

    expected = {artifact.filename: artifact for artifact in candidate.artifacts}
    observed: dict[str, str] = {}
    for index, entry in enumerate(urls):
        if not isinstance(entry, dict):
            raise ManifestError(f"TestPyPI file {index} is not an object")
        filename = entry.get("filename")
        url = entry.get("url")
        digests = entry.get("digests")
        size = entry.get("size")
        if (
            not isinstance(filename, str)
            or filename not in expected
            or filename in observed
        ):
            raise ManifestError("TestPyPI contains an unexpected or duplicate file")
        if not isinstance(url, str) or not url.startswith("https://"):
            raise ManifestError(f"TestPyPI file {filename} has an unsafe URL")
        hostname = urllib.parse.urlsplit(url).hostname
        if hostname not in ALLOWED_FILE_HOSTS:
            raise ManifestError(f"TestPyPI file {filename} has an unexpected host")
        if not isinstance(digests, dict) or digests.get("sha256") != expected[
            filename
        ].sha256:
            raise ManifestError(f"TestPyPI metadata hash differs for {filename}")
        if size != expected[filename].size:
            raise ManifestError(f"TestPyPI metadata size differs for {filename}")
        observed[filename] = url
    if set(observed) != set(expected):
        raise ManifestError(
            f"TestPyPI file set differs: missing={sorted(set(expected) - set(observed))}"
        )
    return observed


def fetch_json(
    url: str,
    *,
    attempts: int,
    wait_seconds: float,
    opener: Callable[..., Any] = urllib.request.urlopen,
) -> Any:
    """Fetch JSON with bounded retries for index propagation."""
    error: Exception | None = None
    for attempt in range(attempts):
        try:
            request = urllib.request.Request(
                url,
                headers={"Accept": "application/json", "User-Agent": "Eqiora-release"},
            )
            with opener(request, timeout=30) as response:
                return json.load(response)
        except (
            OSError,
            UnicodeDecodeError,
            json.JSONDecodeError,
            urllib.error.HTTPError,
            urllib.error.URLError,
        ) as caught:
            error = caught
            if attempt + 1 < attempts:
                time.sleep(wait_seconds)
    raise ManifestError(f"TestPyPI metadata remained unavailable: {error}")


def download_files(
    candidate: Candidate,
    urls: dict[str, str],
    output: Path,
    *,
    opener: Callable[..., Any] = urllib.request.urlopen,
) -> None:
    """Download each exact file with a strict size ceiling."""
    if output.exists():
        raise ManifestError("replay output path must not already exist")
    output.mkdir(parents=True)
    artifacts = {artifact.filename: artifact for artifact in candidate.artifacts}
    for filename in sorted(urls):
        artifact = artifacts[filename]
        request = urllib.request.Request(
            urls[filename],
            headers={"User-Agent": "Eqiora-release"},
        )
        target = output / filename
        with opener(request, timeout=120) as response, target.open("wb") as sink:
            remaining = artifact.size
            while chunk := response.read(min(1024 * 1024, remaining + 1)):
                remaining -= len(chunk)
                if remaining < 0:
                    raise ManifestError(f"TestPyPI file is too large: {filename}")
                sink.write(chunk)
        if remaining != 0:
            raise ManifestError(f"TestPyPI file is truncated: {filename}")
    verify_artifacts(candidate, output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--attempts", type=int, default=12)
    parser.add_argument("--wait-seconds", type=float, default=10.0)
    arguments = parser.parse_args()

    try:
        if arguments.attempts <= 0 or not (0.0 <= arguments.wait_seconds <= 60.0):
            raise ManifestError("retry bounds are invalid")
        verify_manifest_hash(arguments.manifest, arguments.manifest_sha256)
        candidate = load_candidate(arguments.manifest)
        payload = fetch_json(
            TEST_PYPI_JSON.format(version=candidate.version),
            attempts=arguments.attempts,
            wait_seconds=arguments.wait_seconds,
        )
        urls = release_files(payload, candidate)
        download_files(candidate, urls, arguments.out)
    except (ManifestError, OSError) as error:
        if arguments.out.is_dir():
            for path in arguments.out.iterdir():
                if path.is_file() and not path.is_symlink():
                    path.unlink()
            try:
                arguments.out.rmdir()
            except OSError:
                pass
        print(f"TestPyPI replay failed: {error}", file=sys.stderr)
        return 2

    print(f"TestPyPI replay accepted {candidate.version} from {candidate.commit}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
