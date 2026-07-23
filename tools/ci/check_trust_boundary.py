#!/usr/bin/env python3
"""Reject pull requests that alter the definitions of public trust gates.

This program is intended to run from the protected base revision under
``pull_request_target``. It reads only GitHub's changed-file metadata; it never
checks out, imports, or executes code from the pull-request head.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable, Iterable
from typing import Any


PROTECTED_PREFIXES = (
    ".github/actions/",
    ".github/workflows/",
    "crates/eqiora-verify/",
    "tools/ci/",
    "tools/release/",
    "tools/xtask/",
)
PROTECTED_PATHS = frozenset({"CODEOWNERS", ".github/CODEOWNERS", "deny.toml"})
MAX_VISIBLE_FILES = 3_000
PAGE_SIZE = 100
MAX_PAGES = MAX_VISIBLE_FILES // PAGE_SIZE + 1


def protected_path(path: str) -> bool:
    """Return whether *path* defines merge or release trust."""
    normalized = path.replace("\\", "/").removeprefix("./")
    return normalized in PROTECTED_PATHS or normalized.startswith(PROTECTED_PREFIXES)


def changed_file_names(payload: Any) -> list[str]:
    """Validate one GitHub pull-files response and return its filenames."""
    if not isinstance(payload, list):
        raise ValueError("GitHub pull-files response must be a JSON array")

    names: list[str] = []
    for index, entry in enumerate(payload):
        if not isinstance(entry, dict):
            raise ValueError(f"changed-file entry {index} must be an object")
        filename = entry.get("filename")
        if not isinstance(filename, str) or not filename:
            raise ValueError(f"changed-file entry {index} has no filename")
        names.append(filename)
        previous = entry.get("previous_filename")
        if previous is not None:
            if not isinstance(previous, str) or not previous:
                raise ValueError(
                    f"changed-file entry {index} has invalid previous_filename"
                )
            names.append(previous)
    return names


def fetch_changed_files(
    *,
    api_url: str,
    repository: str,
    pull_number: int,
    expected_file_count: int,
    token: str,
    opener: Callable[..., Any] = urllib.request.urlopen,
) -> list[str]:
    """Read every changed filename through the read-only GitHub REST API."""
    if not token:
        raise ValueError("GITHUB_TOKEN is required")
    if (
        "/" not in repository
        or repository.startswith("/")
        or repository.endswith("/")
    ):
        raise ValueError("repository must be an owner/name pair")
    if pull_number <= 0:
        raise ValueError("pull number must be positive")
    if expected_file_count <= 0:
        raise ValueError("expected changed-file count must be positive")
    if expected_file_count > MAX_VISIBLE_FILES:
        raise ValueError(
            f"pull request exceeds the {MAX_VISIBLE_FILES}-file API trust boundary"
        )

    encoded_repository = "/".join(
        urllib.parse.quote(component, safe="") for component in repository.split("/")
    )
    root = api_url.rstrip("/")
    names: list[str] = []
    observed_file_count = 0
    for page in range(1, MAX_PAGES + 1):
        url = (
            f"{root}/repos/{encoded_repository}/pulls/{pull_number}/files"
            f"?per_page={PAGE_SIZE}&page={page}"
        )
        request = urllib.request.Request(
            url,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {token}",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        with opener(request, timeout=30) as response:
            payload = json.load(response)
        page_names = changed_file_names(payload)
        observed_file_count += len(payload)
        names.extend(page_names)
        if len(payload) < PAGE_SIZE:
            if observed_file_count != expected_file_count:
                raise ValueError(
                    "GitHub changed-file metadata count differs: "
                    f"expected {expected_file_count}, observed {observed_file_count}"
                )
            return names
    raise ValueError("pull-file metadata did not terminate within the API boundary")


def protected_changes(paths: Iterable[str]) -> list[str]:
    """Return unique protected paths in stable order."""
    return sorted({path for path in paths if protected_path(path)})


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--pull-number", required=True, type=int)
    parser.add_argument("--expected-file-count", required=True, type=int)
    parser.add_argument(
        "--api-url", default=os.environ.get("GITHUB_API_URL", "https://api.github.com")
    )
    arguments = parser.parse_args()

    try:
        paths = fetch_changed_files(
            api_url=arguments.api_url,
            repository=arguments.repository,
            pull_number=arguments.pull_number,
            expected_file_count=arguments.expected_file_count,
            token=os.environ.get("GITHUB_TOKEN", ""),
        )
        rejected = protected_changes(paths)
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        urllib.error.HTTPError,
        urllib.error.URLError,
    ) as error:
        print(f"CI definition trust check failed closed: {error}", file=sys.stderr)
        return 2

    if rejected:
        print(
            "This pull request changes protected merge or release definitions. "
            "It requires an explicit maintainer ruleset bypass after independent "
            "local verification; this check cannot approve its own replacement.",
            file=sys.stderr,
        )
        for path in rejected:
            print(f"- {path}", file=sys.stderr)
        return 1

    print("Pull request does not change protected merge or release definitions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
