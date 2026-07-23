#!/usr/bin/env python3
"""Authenticate the workflow run that supplied a Python release candidate."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable
from typing import Any


FULL_SHA = re.compile(r"[0-9a-f]{40}")
EXPECTED_EVENT = "workflow_dispatch"
EXPECTED_HEAD_BRANCH = "main"
EXPECTED_WORKFLOW_PATH = ".github/workflows/python-release-candidate.yml"
EXPECTED_TITLE_PREFIX = "Python candidate / "


class CandidateRunError(RuntimeError):
    """The selected Actions run is not an accepted candidate run."""


def _object(value: Any, location: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise CandidateRunError(f"{location} must be an object")
    return value


def validate_candidate_run(
    payload: Any,
    *,
    repository: str,
    run_id: int,
    expected_commit: str,
) -> None:
    """Require one successful candidate run from the protected release commit."""

    run = _object(payload, "workflow run")
    if run.get("id") != run_id:
        raise CandidateRunError("workflow run ID differs from the request")
    if run.get("event") != EXPECTED_EVENT:
        raise CandidateRunError("candidate run was not manually dispatched")
    workflow_path = run.get("path")
    if workflow_path not in {
        EXPECTED_WORKFLOW_PATH,
        f"{EXPECTED_WORKFLOW_PATH}@{EXPECTED_HEAD_BRANCH}",
    }:
        raise CandidateRunError("candidate run used an unexpected workflow")
    if run.get("head_branch") != EXPECTED_HEAD_BRANCH:
        raise CandidateRunError(
            "candidate workflow was not dispatched from protected main"
        )
    if run.get("head_sha") != expected_commit:
        raise CandidateRunError(
            "candidate workflow definition is not bound to the release commit"
        )
    if run.get("display_title") != f"{EXPECTED_TITLE_PREFIX}{expected_commit}":
        raise CandidateRunError("candidate dispatch input is not bound to the release commit")
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        raise CandidateRunError(
            "candidate workflow did not complete successfully, including every replay job"
        )

    source_repository = _object(run.get("repository"), "workflow run repository")
    head_repository = _object(
        run.get("head_repository"), "workflow run head repository"
    )
    for location, value in (
        ("workflow run repository", source_repository),
        ("workflow run head repository", head_repository),
    ):
        if value.get("full_name") != repository:
            raise CandidateRunError(
                f"{location} differs from the release repository"
            )


def fetch_candidate_run(
    *,
    api_url: str,
    repository: str,
    run_id: int,
    token: str,
    opener: Callable[..., Any] = urllib.request.urlopen,
) -> Any:
    """Fetch one workflow run through the read-only GitHub Actions API."""

    if not token:
        raise CandidateRunError("GITHUB_TOKEN is required")
    if (
        repository.count("/") != 1
        or repository.startswith("/")
        or repository.endswith("/")
    ):
        raise CandidateRunError("repository must be an owner/name pair")
    if run_id <= 0:
        raise CandidateRunError("candidate run ID must be positive")

    encoded_repository = "/".join(
        urllib.parse.quote(component, safe="") for component in repository.split("/")
    )
    url = (
        f"{api_url.rstrip('/')}/repos/{encoded_repository}/actions/runs/{run_id}"
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
        return json.load(response)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--run-id", required=True, type=int)
    parser.add_argument("--expected-commit", required=True)
    parser.add_argument(
        "--api-url",
        default=os.environ.get("GITHUB_API_URL", "https://api.github.com"),
    )
    arguments = parser.parse_args()

    try:
        if FULL_SHA.fullmatch(arguments.expected_commit) is None:
            raise CandidateRunError(
                "expected release commit must be a full lowercase SHA"
            )
        payload = fetch_candidate_run(
            api_url=arguments.api_url,
            repository=arguments.repository,
            run_id=arguments.run_id,
            token=os.environ.get("GITHUB_TOKEN", ""),
        )
        validate_candidate_run(
            payload,
            repository=arguments.repository,
            run_id=arguments.run_id,
            expected_commit=arguments.expected_commit,
        )
    except (
        CandidateRunError,
        OSError,
        json.JSONDecodeError,
        urllib.error.HTTPError,
        urllib.error.URLError,
    ) as error:
        print(f"candidate workflow run rejected: {error}", file=sys.stderr)
        return 2

    print(
        f"candidate workflow run {arguments.run_id} is complete, successful, "
        f"and bound to {arguments.expected_commit}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
