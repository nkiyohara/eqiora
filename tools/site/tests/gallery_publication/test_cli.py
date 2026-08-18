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


class GalleryPublicationCliTests(unittest.TestCase):
    def test_exact_success_and_failure_protocol(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = PublicationFixture(Path(temporary) / "repository")
            command = [
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
            ]
            accepted = subprocess.run(command, check=False, text=True, capture_output=True)
            self.assertEqual(accepted.returncode, 0, accepted.stderr)
            self.assertEqual(accepted.stderr, "")
            result = json.loads(accepted.stdout)
            self.assertEqual(accepted.stdout.encode(), canonical_value(result) + b"\n")
            self.assertEqual(
                set(result),
                {
                    "entry_id",
                    "media_sha256",
                    "mode",
                    "predicate",
                    "publication_payload_sha256",
                    "receipt_sha256",
                    "schema",
                    "source_revision",
                    "status",
                },
            )
            self.assertEqual(result["status"], "accepted")

            fixture.external_record.unlink()
            rejected = subprocess.run(command, check=False, text=True, capture_output=True)
            self.assertEqual(rejected.returncode, 1)
            self.assertEqual(rejected.stdout, "")
            self.assertRegex(rejected.stderr, r"^gallery publication check: path: .+\n$")

    def test_installed_mode_rejects_record_path_substitution(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = PublicationFixture(Path(temporary) / "repository")
            result = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER),
                    "verify-installed",
                    "--repository-root",
                    str(fixture.root),
                    "--record",
                    str(fixture.external_record),
                ],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertEqual(result.stdout, "")
            self.assertIn("gallery publication check: path:", result.stderr)


if __name__ == "__main__":
    unittest.main()
