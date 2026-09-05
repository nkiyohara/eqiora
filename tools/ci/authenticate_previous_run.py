#!/usr/bin/env python3
"""Authenticate heavy CI work completed on the immediately previous PR head.

This helper deliberately makes no lane-selection decision.  It only emits an
ephemeral attestation for exact successful jobs and witness steps from the
same pull request.  The repository-owned classifier separately proves that a
lane's complete input closure is unchanged before consuming the attestation.
Any unavailable or ambiguous GitHub state produces an empty attestation.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable


FULL_SHA = re.compile(r"[0-9a-f]{40}")

CI_WITNESSES = {
    "rust": (("Stable quality gate", "Tests"),),
    "msrv": (("MSRV 1.89", "Check every production feature and target"),),
    "python": (
        ("Python 3.11 installed wheel", "Test installed wheel"),
        ("Python 3.14 installed wheel", "Test installed wheel"),
    ),
    "studio": (("Studio projection and native boundary", "Production shell build"),),
    "dependency_policy": (
        ("Dependency policy", "Check root dependency policy"),
        ("Dependency policy", "Check Studio dependency policy"),
    ),
    "cubecl_experiment": (
        ("Isolated CubeCL contract experiment", "Device-independent contract tests"),
    ),
}

PAGES_WITNESSES = {
    "site": (
        (
            "Build and verify static documentation",
            "Build and verify with only loopback networking",
        ),
    ),
}


def _request_json(url: str, token: str) -> Any:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "eqiora-ci-reuse",
        },
    )
    with urllib.request.urlopen(request, timeout=20) as response:
        if response.status != 200:
            raise ValueError(f"GitHub API returned HTTP {response.status}")
        payload = json.load(response)
    return payload


def authenticate(
    *,
    repository: str,
    pull_request: int,
    previous_sha: str,
    workflow: str,
    fetch: Callable[[str], Any],
) -> dict[str, Any]:
    """Return an exact prior-run attestation or raise on uncertain state."""
    if FULL_SHA.fullmatch(previous_sha) is None:
        raise ValueError("previous head is not one exact commit SHA")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("repository identity is malformed")
    if pull_request <= 0:
        raise ValueError("pull-request number is invalid")
    if workflow not in {"ci.yml", "pages.yml"}:
        raise ValueError("workflow is not eligible for reuse")

    associated_pulls = fetch(
        f"https://api.github.com/repos/{repository}/commits/{previous_sha}/pulls"
        "?per_page=100"
    )
    if not isinstance(associated_pulls, list) or len(associated_pulls) >= 100:
        raise ValueError(
            "commit pull-request associations are unavailable or incomplete"
        )
    if not any(
        isinstance(item, dict) and item.get("number") == pull_request
        for item in associated_pulls
    ):
        raise ValueError("previous head is not associated with this pull request")

    encoded_workflow = urllib.parse.quote(workflow, safe="")
    query = urllib.parse.urlencode(
        {
            "event": "pull_request",
            "head_sha": previous_sha,
            "status": "completed",
            "per_page": "100",
        }
    )
    runs_url = (
        f"https://api.github.com/repos/{repository}/actions/workflows/"
        f"{encoded_workflow}/runs?{query}"
    )
    runs_payload = fetch(runs_url)
    runs = runs_payload.get("workflow_runs")
    if not isinstance(runs, list):
        raise ValueError("workflow-run response omits its run list")
    if runs_payload.get("total_count") != len(runs):
        raise ValueError("workflow-run response is incomplete")
    candidates = []
    for run in runs:
        if not isinstance(run, dict):
            continue
        if (
            run.get("head_sha") == previous_sha
            and run.get("event") == "pull_request"
            and run.get("status") == "completed"
            and run.get("conclusion") == "success"
            and isinstance(run.get("id"), int)
        ):
            candidates.append(run)
    if len(candidates) != 1:
        raise ValueError("did not find exactly one successful prior workflow run")

    run = candidates[0]
    run_id = run["id"]
    expected_run_url = f"https://github.com/{repository}/actions/runs/{run_id}"
    if run.get("html_url") != expected_run_url:
        raise ValueError("prior workflow run URL is not its canonical repository URL")
    jobs_payload = fetch(
        f"https://api.github.com/repos/{repository}/actions/runs/{run_id}/jobs"
        "?filter=latest&per_page=100"
    )
    jobs = jobs_payload.get("jobs")
    if not isinstance(jobs, list):
        raise ValueError("workflow-job response omits its job list")
    if jobs_payload.get("total_count") != len(jobs):
        raise ValueError("workflow-job response is incomplete")

    successful_steps: set[tuple[str, str]] = set()
    for job in jobs:
        if not isinstance(job, dict) or job.get("conclusion") != "success":
            continue
        job_name = job.get("name")
        steps = job.get("steps")
        if not isinstance(job_name, str) or not isinstance(steps, list):
            continue
        for step in steps:
            if (
                isinstance(step, dict)
                and isinstance(step.get("name"), str)
                and step.get("status") == "completed"
                and step.get("conclusion") == "success"
            ):
                successful_steps.add((job_name, step["name"]))

    witnesses = CI_WITNESSES if workflow == "ci.yml" else PAGES_WITNESSES
    lanes = {
        lane: all(witness in successful_steps for witness in required)
        for lane, required in witnesses.items()
    }
    return {
        "version": 1,
        "repository": repository,
        "pull_request": pull_request,
        "workflow": workflow,
        "previous_sha": previous_sha,
        "run_id": run_id,
        "run_url": expected_run_url,
        "lanes": lanes,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--pull-request", required=True, type=int)
    parser.add_argument("--previous-sha", required=True)
    parser.add_argument("--workflow", required=True)
    parser.add_argument("--output", required=True)
    arguments = parser.parse_args()
    output = Path(arguments.output)
    if not output.is_absolute() or (os.path.lexists(output) and output.is_symlink()):
        print(
            "reuse attestation output must be an absolute non-symlink path",
            file=sys.stderr,
        )
        return 2
    token = os.environ.get("GITHUB_TOKEN", "")
    attestation: dict[str, Any] = {}
    try:
        if not token:
            raise ValueError("GitHub token is unavailable")
        attestation = authenticate(
            repository=arguments.repository,
            pull_request=arguments.pull_request,
            previous_sha=arguments.previous_sha,
            workflow=arguments.workflow,
            fetch=lambda url: _request_json(url, token),
        )
    except (OSError, ValueError, urllib.error.URLError, json.JSONDecodeError) as error:
        print(
            f"Previous-run reuse unavailable; running current lanes: {error}",
            file=sys.stderr,
        )
    output.write_text(json.dumps(attestation, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
