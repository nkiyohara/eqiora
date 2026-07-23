#!/usr/bin/env python3
"""Validate optional implementation-agent provenance against a protected base."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[2]
REGISTRY_PATH = "governance/implementation-agent-qualifications.toml"
ID_PREFIX = "agent-config-v1:"
CONFIGURATION_FIELDS = (
    "model_provider",
    "model_id",
    "model_revision",
    "reasoning_effort",
    "agent_harness",
    "harness_revision",
    "tools_profile",
    "execution_budget",
    "evaluation_protocol",
)
ENTRY_FIELDS = frozenset(
    {
        "id",
        *CONFIGURATION_FIELDS,
        "evidence_url",
        "evidence_sha256",
        "score_basis_points",
        "accepted_by",
        "accepted_at",
        "valid_until",
        "status",
    }
)
HEADER_FIELDS = frozenset(
    {
        "schema_version",
        "benchmark",
        "benchmark_version",
        "minimum_score_basis_points",
        "configuration",
    }
)
CONFIGURATION_LINE = re.compile(
    r"^Implementation-agent configuration:[ \t]*(.*?)[ \t]*$", re.MULTILINE
)
LOWER_SHA256 = re.compile(r"[0-9a-f]{64}")


class AttestationError(ValueError):
    """A supplied provenance input is ambiguous, malformed, or untrusted."""


@dataclass(frozen=True)
class QualificationRegistry:
    benchmark: str
    benchmark_version: str
    minimum_score_basis_points: int
    configurations: Mapping[str, Mapping[str, object]]


def _exact_fields(value: Mapping[str, object], expected: frozenset[str], label: str) -> None:
    actual = frozenset(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        raise AttestationError(
            f"{label} fields differ; missing={missing or 'none'}, unknown={unknown or 'none'}"
        )


def configuration_id(entry: Mapping[str, object]) -> str:
    """Derive the stable identifier for one complete evaluated configuration."""
    configuration: dict[str, str] = {}
    for field in CONFIGURATION_FIELDS:
        value = entry.get(field)
        if (
            not isinstance(value, str)
            or not value.strip()
            or value != value.strip()
            or any(ord(character) < 0x20 for character in value)
        ):
            raise AttestationError(
                f"configuration field {field!r} must be a nonempty exact string"
            )
        configuration[field] = value
    canonical = json.dumps(
        configuration,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return f"{ID_PREFIX}{hashlib.sha256(canonical).hexdigest()}"


def _utc_datetime(value: object, label: str) -> dt.datetime:
    if not isinstance(value, str) or not value.endswith("Z"):
        raise AttestationError(f"{label} must be an RFC 3339 UTC string ending in Z")
    try:
        parsed = dt.datetime.fromisoformat(f"{value[:-1]}+00:00")
    except ValueError as error:
        raise AttestationError(f"{label} is not a valid RFC 3339 timestamp") from error
    if parsed.tzinfo != dt.timezone.utc:
        raise AttestationError(f"{label} must use UTC")
    return parsed


def _https_url(value: object, label: str) -> str:
    if not isinstance(value, str) or value != value.strip():
        raise AttestationError(f"{label} must be an exact HTTPS URL")
    parsed = urlparse(value)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise AttestationError(f"{label} must be an unauthenticated HTTPS URL")
    return value


def _utc_date(value: object, label: str) -> dt.date:
    if not isinstance(value, str):
        raise AttestationError(f"{label} must be an ISO date string")
    try:
        parsed = dt.date.fromisoformat(value)
    except ValueError as error:
        raise AttestationError(f"{label} is not a valid ISO date") from error
    if parsed.isoformat() != value:
        raise AttestationError(f"{label} must use canonical YYYY-MM-DD form")
    return parsed


def load_registry(data: bytes) -> QualificationRegistry:
    """Parse and exhaustively validate one protected-base registry."""
    try:
        raw = tomllib.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise AttestationError(f"qualification registry is invalid: {error}") from error
    if not isinstance(raw, dict):
        raise AttestationError("qualification registry root must be a table")
    _exact_fields(raw, HEADER_FIELDS, "registry")
    if raw["schema_version"] != 1:
        raise AttestationError("qualification registry schema_version must be 1")
    if raw["benchmark"] != "DeepSWE" or raw["benchmark_version"] != "1.1":
        raise AttestationError("qualification registry must target DeepSWE v1.1 exactly")
    threshold = raw["minimum_score_basis_points"]
    if not isinstance(threshold, int) or isinstance(threshold, bool) or threshold != 7000:
        raise AttestationError("qualification threshold must be exactly 7000 basis points")
    entries = raw["configuration"]
    if not isinstance(entries, list):
        raise AttestationError("registry configuration must be an array of tables")

    configurations: dict[str, Mapping[str, object]] = {}
    for index, value in enumerate(entries):
        label = f"configuration[{index}]"
        if not isinstance(value, dict):
            raise AttestationError(f"{label} must be a table")
        _exact_fields(value, ENTRY_FIELDS, label)
        identifier = value["id"]
        if not isinstance(identifier, str) or LOWER_SHA256.fullmatch(
            identifier.removeprefix(ID_PREFIX)
        ) is None or not identifier.startswith(ID_PREFIX):
            raise AttestationError(
                f"{label}.id is not a canonical agent-config-v1 identifier"
            )
        if identifier != configuration_id(value):
            raise AttestationError(f"{label}.id does not match its exact configuration")
        if identifier in configurations:
            raise AttestationError(f"duplicate qualification identifier {identifier}")
        score = value["score_basis_points"]
        if not isinstance(score, int) or isinstance(score, bool) or not threshold <= score <= 10_000:
            raise AttestationError(
                f"{label} is below threshold or outside exact score bounds"
            )
        if value["status"] != "accepted":
            raise AttestationError(f"{label}.status must be accepted")
        _https_url(value["evidence_url"], f"{label}.evidence_url")
        evidence_sha256 = value["evidence_sha256"]
        if not isinstance(evidence_sha256, str) or LOWER_SHA256.fullmatch(evidence_sha256) is None:
            raise AttestationError(
                f"{label}.evidence_sha256 must be lowercase SHA-256"
            )
        accepted_by = value["accepted_by"]
        if not isinstance(accepted_by, str) or not accepted_by.strip():
            raise AttestationError(f"{label}.accepted_by must identify the reviewer")
        accepted_at = _utc_datetime(value["accepted_at"], f"{label}.accepted_at")
        valid_until = _utc_date(value["valid_until"], f"{label}.valid_until")
        if valid_until < accepted_at.date():
            raise AttestationError(f"{label}.valid_until precedes maintainer acceptance")
        configurations[identifier] = value

    return QualificationRegistry(
        benchmark=str(raw["benchmark"]),
        benchmark_version=str(raw["benchmark_version"]),
        minimum_score_basis_points=threshold,
        configurations=configurations,
    )


def configuration_claim(body: str) -> str | None:
    """Extract at most one exact identifier; normalize genuine absence."""
    matches = CONFIGURATION_LINE.findall(body)
    if len(matches) > 1:
        raise AttestationError(
            "pull request must not contain multiple implementation-agent configuration fields"
        )
    if not matches:
        return None
    claim = matches[0].strip().strip("`")
    if not claim or claim == "not-supplied":
        return None
    return claim


def validate_claim(
    body: str,
    registry: QualificationRegistry | None,
    *,
    today: dt.date | None = None,
) -> str:
    """Validate a supplied claim, while accepting genuinely absent provenance."""
    claim = configuration_claim(body)
    if claim is None:
        return "not-supplied"

    if not claim.startswith(ID_PREFIX) or LOWER_SHA256.fullmatch(
        claim.removeprefix(ID_PREFIX)
    ) is None:
        raise AttestationError("implementation-agent configuration identifier is malformed")
    if registry is None:
        raise AttestationError("protected base has no qualification registry")
    entry = registry.configurations.get(claim)
    if entry is None:
        raise AttestationError(f"unknown implementation-agent configuration {claim}")
    current_date = today or dt.datetime.now(dt.timezone.utc).date()
    if _utc_date(entry["valid_until"], "valid_until") < current_date:
        raise AttestationError(f"implementation-agent configuration {claim} is stale")
    return claim


def registry_from_base(base: str, root: Path = ROOT) -> QualificationRegistry | None:
    merge_base = subprocess.run(
        ["git", "merge-base", base, "HEAD"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    result = subprocess.run(
        ["git", "show", f"{merge_base}:{REGISTRY_PATH}"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        return None
    return load_registry(result.stdout)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="origin/main", help="protected integration ref")
    parser.add_argument("--pr-body-file", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        body = arguments.pr_body_file.read_text(encoding="utf-8")
        registry = (
            None
            if configuration_claim(body) is None
            else registry_from_base(arguments.base)
        )
        outcome = validate_claim(body, registry)
        print(f"implementation-agent provenance: {outcome}")
    except (
        AttestationError,
        OSError,
        subprocess.CalledProcessError,
        UnicodeError,
    ) as error:
        print(f"implementation-agent provenance failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
