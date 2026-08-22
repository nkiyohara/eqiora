from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from fixtures import PublicationFixture  # noqa: E402

CHECKER = HERE.parents[1] / "check_gallery_publication.py"


class GalleryPublicationCliTests(unittest.TestCase):
    def _receipt_command(self, fixture: PublicationFixture) -> list[str]:
        return [
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

    def _assert_exact_failure(self, command: list[str], code: str) -> None:
        rejected = subprocess.run(command, check=False, text=True, capture_output=True)
        self.assertEqual(rejected.returncode, 1)
        self.assertEqual(rejected.stdout, "")
        self.assertRegex(rejected.stderr, rf"^gallery publication check: {code}: [^\n]+\n$")

    def test_missing_record_uses_exact_failure_protocol(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = PublicationFixture(Path(temporary) / "repository")
            command = self._receipt_command(fixture)
            fixture.external_record.unlink()
            self._assert_exact_failure(command, "path")

    def test_control_character_path_cannot_split_failure_protocol(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = PublicationFixture(Path(temporary) / "repository")
            command = self._receipt_command(fixture)
            command[command.index("--record") + 1] = str(fixture.external_record) + "\nmutant"
            self._assert_exact_failure(command, "path")

    def test_dotless_case_id_stays_inside_failure_protocol(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = PublicationFixture(Path(temporary) / "repository")
            fixture.payload["evidence_cases"][0]["id"] = "dotless"
            fixture.refresh_and_write_external()
            self._assert_exact_failure(self._receipt_command(fixture), "case-set")

    def test_finite_integer_overflow_stays_inside_failure_protocol(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = PublicationFixture(Path(temporary) / "repository")
            fixture.payload["lineage"]["pressure"]["value_range"]["minimum"] = 10**4000
            fixture.refresh_and_write_external()
            self._assert_exact_failure(self._receipt_command(fixture), "shape")

    def test_installed_mode_rejects_record_path_substitution_with_exact_protocol(self):
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
            self.assertRegex(result.stderr, r"^gallery publication check: path: .+\n$")


if __name__ == "__main__":
    unittest.main()
