#!/usr/bin/env python3
"""Fail closed over conditional GitHub Actions job results."""

from __future__ import annotations

import json
import os
import sys
from typing import Mapping


JOB_SURFACES = {
    "quality": ("rust",),
    "host_evidence": ("rust",),
    "python_host_evidence": ("rust", "python"),
    "msrv": ("msrv",),
    "dependency_policy": ("dependency_policy",),
    "cubecl_experiment": ("cubecl_experiment",),
    "python_wheel": ("python",),
    "studio": ("studio",),
}


def evaluate(relevance: Mapping[str, bool], results: Mapping[str, str]) -> list[str]:
    """Return every reason the aggregate gate must reject the run."""
    failures: list[str] = []
    for always_job in ("changes", "documentation"):
        if results.get(always_job) != "success":
            failures.append(f"always-required job {always_job} was {results.get(always_job)!r}")

    for job, surfaces in JOB_SURFACES.items():
        result = results.get(job)
        if result not in {"success", "skipped"}:
            failures.append(f"conditional job {job} was {result!r}")
        for surface in surfaces:
            if relevance.get(surface) and result != "success":
                failures.append(
                    f"required surface {surface} did not succeed in job {job}"
                )
    return failures


def load_object(name: str) -> dict[str, object]:
    value = json.loads(os.environ[name])
    if not isinstance(value, dict):
        raise ValueError(f"{name} must contain a JSON object")
    return value


def parse_relevance(raw: Mapping[str, object]) -> dict[str, bool]:
    """Require the complete, exact change-classifier output vocabulary."""
    expected = {
        surface for surfaces in JOB_SURFACES.values() for surface in surfaces
    }
    actual = set(raw)
    if actual != expected:
        missing = sorted(expected - actual)
        unexpected = sorted(actual - expected)
        raise ValueError(
            f"CI_RELEVANCE keys differ: missing={missing}, unexpected={unexpected}"
        )

    relevance: dict[str, bool] = {}
    for key, value in raw.items():
        if value is True or value == "true":
            relevance[key] = True
        elif value is False or value == "false":
            relevance[key] = False
        else:
            raise ValueError(f"CI_RELEVANCE[{key!r}] must be exactly true or false")
    return relevance


def parse_results(raw: Mapping[str, object]) -> dict[str, str]:
    """Require one result for every always-required and conditional job."""
    expected = {"changes", "documentation", *JOB_SURFACES}
    actual = set(raw)
    if actual != expected:
        missing = sorted(expected - actual)
        unexpected = sorted(actual - expected)
        raise ValueError(
            f"CI_RESULTS keys differ: missing={missing}, unexpected={unexpected}"
        )
    return {key: str(value) for key, value in raw.items()}


def main() -> int:
    try:
        raw_relevance = load_object("CI_RELEVANCE")
        raw_results = load_object("CI_RESULTS")
        relevance = parse_relevance(raw_relevance)
        results = parse_results(raw_results)
        failures = evaluate(relevance, results)
    except (KeyError, json.JSONDecodeError, ValueError) as error:
        print(f"CI aggregate input is invalid: {error}", file=sys.stderr)
        return 2

    if failures:
        for failure in failures:
            print(f"CI gate rejected: {failure}", file=sys.stderr)
        return 1
    print("CI gate accepted every relevant job")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
