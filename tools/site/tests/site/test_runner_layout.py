from __future__ import annotations

import hashlib
import os
import shutil
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

from fixture import REPOSITORY, SOURCE_SHA, checker, pinned_node_path

SCRATCH_ROOT = Path.home() / ".cache/eqiora/site-oracle-tests"
BASIS_SHA = "19968da984c16e718baeb9faa5aae04260896c29"
BASIS_TREE = "1d19473c487b8035608cc88cbd99757f2b95865a"


class OfflineRunnerLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        SCRATCH_ROOT.mkdir(parents=True, exist_ok=True)

    def _environment(self, root: Path, scratch: Path) -> dict[str, str]:
        scratch = scratch.resolve()
        environment = os.environ.copy()
        environment.update(
            {
                "LC_ALL": "C",
                "TZ": "UTC",
                "npm_config_offline": "true",
                "CARGO_NET_OFFLINE": "true",
                "UV_OFFLINE": "1",
                "PATH": pinned_node_path(root),
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
        return environment

    def _layout(
        self, root: Path, *, admitted_link: bool = True
    ) -> tuple[Path, dict[str, str]]:
        scratch = root / "scratch"
        source = scratch / "source"
        (scratch / "build").mkdir(parents=True)
        (scratch / "uv-cache").mkdir()
        (source / "docs/site/node_modules").mkdir(parents=True)
        (source / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        (source / "docs/site/package.json").write_text("{}\n", encoding="utf-8")
        (source / "AGENTS.md").write_bytes(b"fixture governance\n")
        if admitted_link:
            (source / "CLAUDE.md").symlink_to("AGENTS.md")
        runner = source / "tools/site/run_offline_site_checks.sh"
        runner.parent.mkdir(parents=True)
        shutil.copy2(REPOSITORY / "tools/site/run_offline_site_checks.sh", runner)
        runner.chmod(0o755)
        return runner, self._environment(root, scratch)

    def _archive_layout(self, root: Path) -> tuple[Path, dict[str, str]]:
        scratch = root / "scratch"
        source = scratch / "source"
        (scratch / "build").mkdir(parents=True)
        (scratch / "uv-cache").mkdir()
        source.mkdir()
        archive = root / "source.tar"
        with archive.open("wb") as target:
            subprocess.run(
                ["git", "archive", "--format=tar", BASIS_SHA],
                cwd=REPOSITORY,
                check=True,
                stdout=target,
            )
        subprocess.run(["tar", "-xf", str(archive), "-C", str(source)], check=True)
        self.assertTrue(stat.S_ISLNK((source / "CLAUDE.md").lstat().st_mode))
        self.assertFalse((source / "AGENTS.md").is_symlink())
        self.assertTrue((source / "AGENTS.md").is_file())
        (source / "docs/site/node_modules").mkdir(exist_ok=True)
        runner = source / "tools/site/run_offline_site_checks.sh"
        return runner, self._environment(root, scratch)

    def _assert_frozen_basis_archive(self, root: Path) -> None:
        archive = root / "basis.tar"
        with archive.open("wb") as target:
            subprocess.run(
                ["git", "archive", "--format=tar", BASIS_SHA],
                cwd=REPOSITORY,
                check=True,
                stdout=target,
            )
        self.assertEqual(archive.stat().st_size, 34_088_960)
        self.assertEqual(
            hashlib.sha256(archive.read_bytes()).hexdigest(),
            "69b560bac187303143a1a53bf82f73b8129c84a31fb1df4bfd109cee33f29252",
        )

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

    def test_00_s01_exact_archive_and_optional_link_pass_before_mutants(self) -> None:
        tree_identity = subprocess.run(
            ["git", "rev-parse", f"{BASIS_SHA}^{{tree}}"],
            cwd=REPOSITORY,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout.strip()
        self.assertEqual(tree_identity, BASIS_TREE)
        tree = subprocess.run(
            ["git", "ls-tree", "-r", "-z", BASIS_SHA],
            cwd=REPOSITORY,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout.split(b"\0")
        links = [entry for entry in tree if entry.startswith(b"120000 ")]
        self.assertEqual(
            links,
            [b"120000 blob 47dc3e3d863cfb5727b87d785d09abf9743c0a72\tCLAUDE.md"],
        )
        self.assertIn(
            b"100644 blob 61c1bbede492aef4a9c85fa364d031e012621809\tAGENTS.md",
            tree,
        )
        payload = subprocess.run(
            ["git", "show", f"{BASIS_SHA}:CLAUDE.md"],
            cwd=REPOSITORY,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout
        target = subprocess.run(
            ["git", "show", f"{BASIS_SHA}:AGENTS.md"],
            cwd=REPOSITORY,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout
        self.assertEqual(payload, b"AGENTS.md")
        self.assertEqual(
            hashlib.sha256(payload).hexdigest(),
            "a54ff182c7e8acf56acfd6e4b9c3ff41e2c41a31c9b211b2deb9df75d9a478f9",
        )
        self.assertEqual(len(target), 12_408)
        self.assertEqual(
            hashlib.sha256(target).hexdigest(),
            "ffc9b0381a01c16b3d72389ef777842215c48b65d6eda6881f5e75bfa5d531c0",
        )

        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as temporary:
            self._assert_frozen_basis_archive(Path(temporary))

        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as temporary:
            _, environment = self._archive_layout(Path(temporary))
            source = Path(environment["EQIORA_SITE_SOURCE_ROOT"])
            before = (
                os.readlink(source / "CLAUDE.md"),
                hashlib.sha256((source / "AGENTS.md").read_bytes()).hexdigest(),
            )
            self.assertEqual(checker.check_source_topology(source, before[1]), [])
            self.assertEqual(
                (
                    os.readlink(source / "CLAUDE.md"),
                    hashlib.sha256((source / "AGENTS.md").read_bytes()).hexdigest(),
                ),
                before,
            )

        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as temporary:
            _, environment = self._layout(Path(temporary), admitted_link=False)
            source = Path(environment["EQIORA_SITE_SOURCE_ROOT"])
            self.assertEqual(checker.check_source_topology(source), [])

        def extra_link(runner: Path, environment: dict[str, str]) -> None:
            Path(environment["EQIORA_SITE_SOURCE_ROOT"], "extra-link").symlink_to(
                "AGENTS.md"
            )

        def link_target(value: str):
            def mutate(runner: Path, environment: dict[str, str]) -> None:
                link = Path(environment["EQIORA_SITE_SOURCE_ROOT"], "CLAUDE.md")
                link.unlink()
                link.symlink_to(value)

            return mutate

        def missing_target(runner: Path, environment: dict[str, str]) -> None:
            Path(environment["EQIORA_SITE_SOURCE_ROOT"], "AGENTS.md").unlink()

        def directory_target(runner: Path, environment: dict[str, str]) -> None:
            target = Path(environment["EQIORA_SITE_SOURCE_ROOT"], "AGENTS.md")
            target.unlink()
            target.mkdir()

        def fifo_target(runner: Path, environment: dict[str, str]) -> None:
            target = Path(environment["EQIORA_SITE_SOURCE_ROOT"], "AGENTS.md")
            target.unlink()
            os.mkfifo(target)

        def symlink_target(runner: Path, environment: dict[str, str]) -> None:
            source = Path(environment["EQIORA_SITE_SOURCE_ROOT"])
            (source / "AGENTS.md").rename(source / "governance")
            (source / "AGENTS.md").symlink_to("governance")

        def target_byte_drift(runner: Path, environment: dict[str, str]) -> None:
            source = Path(environment["EQIORA_SITE_SOURCE_ROOT"])
            (source / "AGENTS.md").write_bytes(b"drifted governance\n")

        for label, mutate, expected_error in (
            ("additional source symlink", extra_link, "unadmitted symlinks"),
            ("absolute CLAUDE target", link_target("/AGENTS.md"), "exact link payload"),
            ("dot CLAUDE target", link_target("./AGENTS.md"), "exact link payload"),
            ("parent CLAUDE target", link_target("../AGENTS.md"), "exact link payload"),
            (
                "nested CLAUDE target",
                link_target("docs/AGENTS.md"),
                "exact link payload",
            ),
            (
                "alternate CLAUDE spelling",
                link_target("agents.md"),
                "exact link payload",
            ),
            (
                "CLAUDE target with final LF",
                link_target("AGENTS.md\n"),
                "exact link payload",
            ),
            ("missing AGENTS target", missing_target, "target is unavailable"),
            ("directory AGENTS target", directory_target, "regular non-symlink"),
            ("FIFO AGENTS target", fifo_target, "regular non-symlink"),
            ("symlink AGENTS target", symlink_target, "unadmitted symlinks"),
            (
                "same-commit AGENTS byte drift",
                target_byte_drift,
                "differs from the same-commit Git blob",
            ),
        ):
            with (
                self.subTest(label=label),
                tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as temporary,
            ):
                runner, environment = self._layout(Path(temporary))
                source = Path(environment["EQIORA_SITE_SOURCE_ROOT"])
                expected = hashlib.sha256(
                    (source / "AGENTS.md").read_bytes()
                ).hexdigest()
                mutate(runner, environment)
                errors = checker.check_source_topology(source, expected)
                self.assertTrue(
                    any(expected_error in error for error in errors),
                    f"S-01 mutant missed its causal gate: {label}: {errors}",
                )

        runner_text = (REPOSITORY / "tools/site/run_offline_site_checks.sh").read_text(
            encoding="utf-8"
        )
        errors = checker.check_runner_source_topology_text(runner_text)
        self.assertEqual(errors, [], "\n".join(errors))

    def test_existing_layout_mutants_fail_closed(self) -> None:
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
            with (
                self.subTest(label=label),
                tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as temporary,
            ):
                runner, environment = self._layout(Path(temporary), admitted_link=False)
                mutate(runner, environment)
                self.assertNotEqual(self._run(runner, environment).returncode, 0)


if __name__ == "__main__":
    unittest.main()
