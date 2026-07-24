#!/usr/bin/env python3
"""Build one Python candidate and verify every registered host profile."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/release"))

from candidate_manifest import (  # noqa: E402
    Candidate,
    REQUIRED_PROFILES,
    load_candidate,
    require_candidate_profile,
    verify_artifacts,
)
from python_candidate import build_candidate  # noqa: E402


def build_and_verify_candidate(root: Path) -> Candidate:
    """Build once, then project every required profile from one manifest."""

    artifacts = root / "artifacts"
    manifest_path = build_candidate(
        artifacts,
        require_tag=False,
        skip_extras=False,
    )
    candidate = load_candidate(manifest_path)
    retained_manifest = root / "manifest" / manifest_path.name
    retained_manifest.parent.mkdir()
    manifest_path.replace(retained_manifest)
    verify_artifacts(candidate, artifacts)
    for profile in REQUIRED_PROFILES:
        require_candidate_profile(candidate, profile)
    return candidate


def main() -> int:
    """Run the release-trust gate without retaining large local artifacts."""

    try:
        with tempfile.TemporaryDirectory(
            prefix="eqiora-python-distribution-gate-"
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
