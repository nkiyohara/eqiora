from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from fixtures import PublicationFixture, canonical_value  # noqa: E402

CHECKER = HERE.parents[1] / "check_gallery_publication.py"
RESULT_KEYS = {
    "entry_id",
    "media_sha256",
    "mode",
    "predicate",
    "publication_payload_sha256",
    "receipt_sha256",
    "schema",
    "source_revision",
    "status",
}


class GalleryPublicationPositiveFirst(unittest.TestCase):
    def test_00_complete_receipt_then_receipt_free_installed_cli_lifecycle(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = PublicationFixture(Path(temporary) / "repository")
            cases = {item["id"] for item in fixture.payload["evidence_cases"]}
            self.assertIn("fluid.exact-circular-hole-stokes-2d-gmsh", cases)
            self.assertIn("interfaces.python-circular-hole-chordal-mesh", cases)
            self.assertNotIn("fluid.exact-circular-hole-stokes-2d", cases)
            self.assertNotIn("geometry.circular-hole-chordal-reference-mesh", cases)
            self.assertIn("1,210-triangle affine mesh", fixture.payload["text"]["alt"])
            self.assertIn("548 interior vertices", fixture.payload["claim"]["public_claim"])
            external = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER),
                    "verify-receipt",
                    "--repository-root",
                    str(fixture.root),
                    "--record",
                    str(fixture.external_record),
                    "--receipt",
                    str(fixture.receipt_path),
                    "--media",
                    str(fixture.candidate),
                ],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(external.returncode, 0, external.stderr)
            self.assertEqual(external.stderr, "")
            external_result = json.loads(external.stdout)
            self.assertEqual(external.stdout.encode(), canonical_value(external_result) + b"\n")
            self.assertEqual(set(external_result), RESULT_KEYS)
            self.assertEqual(external_result["mode"], "verify-receipt")
            self.assertEqual(external_result["status"], "accepted")

            fixture.install(include_receipt=False)
            self.assertFalse(fixture.receipt_path.exists())
            installed = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER),
                    "verify-installed",
                    "--repository-root",
                    str(fixture.root),
                    "--record",
                    str(fixture.installed_record),
                ],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(installed.returncode, 0, installed.stderr)
            self.assertEqual(installed.stderr, "")
            installed_result = json.loads(installed.stdout)
            self.assertEqual(installed.stdout.encode(), canonical_value(installed_result) + b"\n")
            self.assertEqual(set(installed_result), RESULT_KEYS)
            self.assertEqual(installed_result["mode"], "verify-installed")
            self.assertEqual(installed_result["status"], "accepted")
            self.assertEqual(installed_result["receipt_sha256"], external_result["receipt_sha256"])


if __name__ == "__main__":
    unittest.main()
