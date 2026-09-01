#!/usr/bin/env python3
"""Build one Python candidate and verify every registered wheel profile."""

from __future__ import annotations

import hashlib
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/release"))

from candidate_manifest import (  # noqa: E402
    Candidate,
    REQUIRED_PROFILES,
    load_candidate_family,
    require_candidate_profile,
    verify_artifacts,
)
from python_candidate import source_identity  # noqa: E402
from python_candidate_common import checked_run, home_scratch_parent  # noqa: E402


def _family_inventory(directory: Path) -> tuple[tuple[str, int, str], ...]:
    """Capture the immutable distribution family without interpreting metadata."""

    if not directory.is_dir():
        raise RuntimeError("candidate family directory is missing")
    records: list[tuple[str, int, str]] = []
    for path in sorted(directory.iterdir(), key=lambda item: item.name.encode()):
        if path.is_symlink() or not path.is_file():
            raise RuntimeError("candidate family contains a non-regular entry")
        payload = path.read_bytes()
        records.append((path.name, len(payload), hashlib.sha256(payload).hexdigest()))
    return tuple(records)


def build_and_verify_candidate(root: Path) -> Candidate:
    """Replay preparation and profile finalization in disjoint roots."""

    revision = source_identity().commit
    artifacts = root / "family"
    metadata = root / "metadata"
    checked_run(
        [
            "python3",
            "tools/release/python_candidate.py",
            "prepare",
            "--expected-commit",
            revision,
            "--out",
            str(artifacts),
        ],
        cwd=ROOT,
    )
    entry_inventory = _family_inventory(artifacts)
    checked_run(
        [
            "python3",
            "tools/release/python_candidate.py",
            "finalize",
            "--expected-commit",
            revision,
            "--artifacts",
            str(artifacts),
            "--manifest-out",
            str(metadata),
        ],
        cwd=ROOT,
    )
    manifests = sorted(metadata.glob("*-python-candidate.json"))
    if len(manifests) != 1:
        raise RuntimeError("finalization did not retain exactly one candidate manifest")
    manifest_path = manifests[0]
    if _family_inventory(artifacts) != entry_inventory:
        raise RuntimeError("candidate family inventory changed during finalization")
    candidate = load_candidate_family(
        manifest_path,
        artifacts,
        requested_profiles=REQUIRED_PROFILES,
    )
    verify_artifacts(candidate, artifacts)
    for profile in REQUIRED_PROFILES:
        require_candidate_profile(candidate, profile)
    return candidate


def main() -> int:
    """Run the release-trust gate without retaining large local artifacts."""

    try:
        with tempfile.TemporaryDirectory(
            prefix="eqiora-python-distribution-gate-",
            dir=home_scratch_parent("python-distribution-gate"),
        ) as temporary:
            build_and_verify_candidate(Path(temporary))
    except (
        OSError,
        RuntimeError,
        ValueError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Python distribution gate failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
