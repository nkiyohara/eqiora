from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from fixture import REPOSITORY, SOURCE_SHA


class OfflineRunnerLayoutTests(unittest.TestCase):
    def _layout(self, root: Path) -> tuple[Path, dict[str, str]]:
        scratch = root / "scratch"
        source = scratch / "source"
        (scratch / "build").mkdir(parents=True)
        (scratch / "uv-cache").mkdir()
        (source / "docs/site/node_modules").mkdir(parents=True)
        (source / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        (source / "docs/site/package.json").write_text("{}\n", encoding="utf-8")
        runner = source / "tools/site/run_offline_site_checks.sh"
        runner.parent.mkdir(parents=True)
        shutil.copy2(REPOSITORY / "tools/site/run_offline_site_checks.sh", runner)
        runner.chmod(0o755)
        scratch = scratch.resolve()
        environment = os.environ.copy()
        environment.update(
            {
                "LC_ALL": "C",
                "TZ": "UTC",
                "EQIORA_API_SCRATCH": str(scratch),
                "EQIORA_SITE_SOURCE_ROOT": str(scratch / "source"),
                "EQIORA_SITE_ASTRO_OUT_DIR": str(scratch / "astro"),
                "EQIORA_SITE_RUSTDOC_TARGET": str(scratch / "rustdoc-target"),
                "EQIORA_SITE_RUSTDOC_STAGE": str(scratch / "rustdoc-stage"),
                "EQIORA_SITE_ARTIFACT": str(scratch / "build/site"),
                "EQIORA_SITE_SOURCE_SHA": SOURCE_SHA,
                "PLAYWRIGHT_BROWSERS_PATH": str(
                    root / "browser-supply/eqiora-pw-1.62.1-r1234"
                ),
            }
        )
        return runner, environment

    @staticmethod
    def _run(runner: Path, environment: dict[str, str]) -> subprocess.CompletedProcess:
        return subprocess.run(
            [str(runner), "--preflight-only"],
            check=False,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

    def test_00_archived_workflow_layout_passes_before_mutants(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            runner, environment = self._layout(Path(temporary))
            result = self._run(runner, environment)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_layout_mutants_fail_closed(self) -> None:
        def extra_entry(runner: Path, environment: dict[str, str]) -> None:
            Path(environment["EQIORA_API_SCRATCH"], "unexpected").touch()

        def missing_build(runner: Path, environment: dict[str, str]) -> None:
            Path(environment["EQIORA_API_SCRATCH"], "build").rmdir()

        def linked_source(runner: Path, environment: dict[str, str]) -> None:
            source = Path(environment["EQIORA_SITE_SOURCE_ROOT"])
            backing = source.parent.parent / "source-backing"
            source.rename(backing)
            source.symlink_to(backing, target_is_directory=True)

        for label, mutate in (
            ("extra top-level entry", extra_entry),
            ("missing build directory", missing_build),
            ("linked source", linked_source),
        ):
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                runner, environment = self._layout(Path(temporary))
                mutate(runner, environment)
                self.assertNotEqual(self._run(runner, environment).returncode, 0)


if __name__ == "__main__":
    unittest.main()
