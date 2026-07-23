#!/usr/bin/env python3
"""Build and verify the complete declared Python distribution candidate."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/release"))

from python_candidate import MANIFEST_FORMAT, build_candidate  # noqa: E402


def main() -> int:
    """Run the release-trust gate without retaining large local artifacts."""

    try:
        with tempfile.TemporaryDirectory(
            prefix="eqiora-python-distribution-gate-"
        ) as temporary:
            manifest_path = build_candidate(
                Path(temporary) / "candidate",
                require_tag=False,
                skip_extras=False,
            )
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            assert manifest["format"] == MANIFEST_FORMAT
            assert manifest["acceptance"] == "complete"
            assert manifest["source"]["tree"] == "clean"
            assert manifest["build"]["sdist_rebuilt"] is True
            assert len(manifest["artifacts"]) == 5
    except (
        AssertionError,
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
