from __future__ import annotations

import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

import fixture
from fixture import (
    GIT_OBJECT_REPOSITORY_VARIABLE,
    REPOSITORY,
    SOURCE_SHA_VARIABLE,
    GitObjectAuthorityError,
    git_object_authority,
    git_object_authority_status,
    historical_git,
)


SCRATCH_ROOT = Path.home() / ".cache/eqiora/site-oracle-tests"
CHECKOUT_PIN = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
DESIGNATION = 'echo "EQIORA_SITE_GIT_OBJECT_REPOSITORY=$GITHUB_WORKSPACE"'
SUPPLY_MARKERS = (
    "npm ci --engine-strict --prefix docs/site",
    'mv docs/site/node_modules "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules"',
    'test -z "$(git status --porcelain=v1 --untracked-files=all)"',
)
FIXED_OBJECTS = {
    "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f": (
        "3237f739098498ac46bfdd6a993c00b0575900f3",
        "57f8b9b476c04b8103b5a43c8a30504c0e2fa1fb",
        "47dc3e3d863cfb5727b87d785d09abf9743c0a72",
        "61c1bbede492aef4a9c85fa364d031e012621809",
    ),
    "68c9c2fe245ac52cc20dcf5a65a2455de507f0dc": (
        "20701fe8909295b980c1da7cf3eab366f8d5f27c",
        "6e685495bf6989e1ad902a7e88c199557285cbee",
    ),
    "19968da984c16e718baeb9faa5aae04260896c29": (
        "1d19473c487b8035608cc88cbd99757f2b95865a",
        "21d5f0bc5213bca02336040f1085c7d52c63588f",
        "47dc3e3d863cfb5727b87d785d09abf9743c0a72",
        "61c1bbede492aef4a9c85fa364d031e012621809",
    ),
}
FORBIDDEN_AFTER_DESIGNATION = (
    "git fetch",
    "git checkout",
    "git reset",
    "git clean",
    "git gc",
    "git maintenance",
    "git commit-graph",
    "git update-ref",
    "git hash-object -w",
    "npm ci",
    "mv docs/site/node_modules",
    'printf mutation > "$GITHUB_WORKSPACE/',
    'cat "$GITHUB_WORKSPACE/',
)


def _run_git(repository: Path, *arguments: str) -> bytes:
    return subprocess.check_output(
        ["git", "-c", "commit.gpgsign=false", "-C", str(repository), *arguments],
        stderr=subprocess.PIPE,
    )


def _make_repository(root: Path, payload: str = "ordinary\n") -> tuple[Path, str]:
    repository = root / "repository"
    repository.mkdir()
    _run_git(repository, "init", "-q")
    (repository / "current.txt").write_text(payload, encoding="utf-8")
    _run_git(repository, "add", "current.txt")
    _run_git(
        repository,
        "-c",
        "user.name=authority-oracle",
        "-c",
        "user.email=authority-oracle@example.invalid",
        "commit",
        "-qm",
        "ordinary",
    )
    return repository.resolve(), _run_git(
        repository, "rev-parse", "HEAD"
    ).decode().strip()


def _archive_environment(archive: Path, authority: Path, head: str) -> dict[str, str]:
    return {
        GIT_OBJECT_REPOSITORY_VARIABLE: str(authority),
        SOURCE_SHA_VARIABLE: head,
    }


def _authority_manifest(
    repository: Path, environment: dict[str, str]
) -> tuple[bytes, ...]:
    values = [
        historical_git(
            "rev-parse",
            "--verify",
            "HEAD^{commit}",
            repository=repository,
            environment=environment,
        ),
        historical_git(
            "rev-parse",
            "--verify",
            "HEAD^{tree}",
            repository=repository,
            environment=environment,
        ),
    ]
    for commit, objects in FIXED_OBJECTS.items():
        values.append(
            historical_git(
                "cat-file",
                "-e",
                f"{commit}^{{commit}}",
                repository=repository,
                environment=environment,
            )
        )
        for object_id in objects:
            values.append(
                historical_git(
                    "cat-file",
                    "-e",
                    object_id,
                    repository=repository,
                    environment=environment,
                )
            )
    return tuple(values)


def _workflow_errors(workflow: str, runner: str) -> list[str]:
    errors: list[str] = []
    checkout = workflow.find(CHECKOUT_PIN)
    fetch = workflow.find("fetch-depth: 0", checkout)
    credentials = workflow.find("persist-credentials: false", checkout)
    if (
        checkout < 0
        or fetch < 0
        or credentials < 0
        or workflow.count("fetch-depth:") != 1
        or not credentials < fetch
    ):
        errors.append("Git object authority checkout is not exact full history")

    designation = workflow.find(DESIGNATION)
    if designation < 0 or workflow.count(DESIGNATION) != 1:
        errors.append("Git object authority designation is missing or ambiguous")
    else:
        for marker in SUPPLY_MARKERS:
            position = workflow.find(marker)
            if position < 0 or position > designation:
                errors.append(
                    "Git object authority was designated before complete Phase A supply"
                )
                break
        later = workflow[designation:]
        forbidden = [token for token in FORBIDDEN_AFTER_DESIGNATION if token in later]
        if forbidden:
            errors.append(
                f"Git object authority Phase B contains a forbidden operation: {forbidden}"
            )

    fixed = [
        item for commit, objects in FIXED_OBJECTS.items() for item in (commit, *objects)
    ]
    if any(workflow.count(object_id) < 2 for object_id in fixed):
        errors.append(
            "Git object authority pre/post fixed-object authentication is incomplete"
        )
    required_identity = (
        "git rev-parse --show-toplevel",
        "git rev-parse HEAD",
        "HEAD^{tree}",
        "git status --porcelain=v1 --untracked-files=all",
    )
    if any(workflow.count(token) < 2 for token in required_identity):
        errors.append("Git object authority pre/post checkout identity is incomplete")
    if workflow.count(GIT_OBJECT_REPOSITORY_VARIABLE) < 4:
        errors.append(
            "Git object authority does not propagate through the cleared environment"
        )

    required_runner = (
        GIT_OBJECT_REPOSITORY_VARIABLE,
        'authority_real="$(realpath "$EQIORA_SITE_GIT_OBJECT_REPOSITORY")"',
        'test "$authority_real" = "$EQIORA_SITE_GIT_OBJECT_REPOSITORY"',
        'test ! -L "$EQIORA_SITE_GIT_OBJECT_REPOSITORY"',
        'test "$authority_real" != "$source_real"',
    )
    if any(token not in runner for token in required_runner):
        errors.append("offline runner omits canonical archive/authority separation")
    return errors


def _admitted_text() -> tuple[str, str]:
    workflow = (REPOSITORY / ".github/workflows/pages.yml").read_text(encoding="utf-8")
    workflow = workflow.replace(
        "          persist-credentials: false\n",
        "          persist-credentials: false\n          fetch-depth: 0\n",
        1,
    )
    identities = " ".join(
        item for commit, objects in FIXED_OBJECTS.items() for item in (commit, *objects)
    )
    boundary = textwrap.dedent(
        f"""\
          test "$(git rev-parse --show-toplevel)" = "$GITHUB_WORKSPACE"
          test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
          test "$(git rev-parse 'HEAD^{{tree}}')" = "$(git rev-parse "$GITHUB_SHA^{{tree}}")"
          test -z "$(git status --porcelain=v1 --untracked-files=all)"
          for object in {identities}; do git cat-file -e "$object"; done
"""
    )
    supply_status = (
        '          test -z "$(git status --porcelain=v1 --untracked-files=all)"\n'
    )
    supply_position = workflow.index(
        supply_status, workflow.index("mv docs/site/node_modules")
    )
    supply_end = supply_position + len(supply_status)
    workflow = (
        workflow[:supply_end]
        + boundary
        + f'          {{ {DESIGNATION}; }} >> "$GITHUB_ENV"\n'
        + workflow[supply_end:]
    )
    workflow = workflow.replace(
        "EQIORA_SITE_SOURCE_ROOT,EQIORA_SITE_ASTRO_OUT_DIR",
        "EQIORA_SITE_SOURCE_ROOT,EQIORA_SITE_GIT_OBJECT_REPOSITORY,EQIORA_SITE_ASTRO_OUT_DIR",
        1,
    )
    workflow = workflow.replace(
        '                  EQIORA_SITE_SOURCE_ROOT="$EQIORA_SITE_SOURCE_ROOT" \\\n',
        '                  EQIORA_SITE_SOURCE_ROOT="$EQIORA_SITE_SOURCE_ROOT" \\\n'
        '                  EQIORA_SITE_GIT_OBJECT_REPOSITORY="$EQIORA_SITE_GIT_OBJECT_REPOSITORY" \\\n',
        1,
    )
    post_status = (
        '          test -z "$(git status --porcelain=v1 --untracked-files=all)"\n'
    )
    post_position = workflow.index(post_status, workflow.index("Recheck exact inputs"))
    workflow = workflow[:post_position] + boundary + workflow[post_position:]

    runner = (REPOSITORY / "tools/site/run_offline_site_checks.sh").read_text(
        encoding="utf-8"
    )
    runner = runner.replace(
        "  EQIORA_SITE_SOURCE_ROOT \\\n",
        "  EQIORA_SITE_SOURCE_ROOT \\\n  EQIORA_SITE_GIT_OBJECT_REPOSITORY \\\n",
        1,
    )
    source_real = 'source_real="$(realpath "$EQIORA_SITE_SOURCE_ROOT")"\n'
    runner = runner.replace(
        source_real,
        source_real
        + 'authority_real="$(realpath "$EQIORA_SITE_GIT_OBJECT_REPOSITORY")"\n'
        + 'test "$authority_real" = "$EQIORA_SITE_GIT_OBJECT_REPOSITORY"\n'
        + 'test ! -L "$EQIORA_SITE_GIT_OBJECT_REPOSITORY"\n'
        + 'test "$authority_real" != "$source_real"\n',
        1,
    )
    return workflow, runner


class GitObjectAuthorityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        SCRATCH_ROOT.mkdir(parents=True, exist_ok=True)

    def test_00_repository_fallback_and_fixed_history_are_ordinary_positive(
        self,
    ) -> None:
        authority = git_object_authority()
        if os.path.lexists(REPOSITORY / ".git"):
            self.assertEqual(authority.root, REPOSITORY)
        else:
            self.assertNotEqual(authority.root, REPOSITORY)
            self.assertEqual(
                str(authority.root), os.environ[GIT_OBJECT_REPOSITORY_VARIABLE]
            )
        self.assertRegex(authority.head, r"^[0-9a-f]{40}$")
        self.assertRegex(authority.tree, r"^[0-9a-f]{40}$")
        environment = dict(os.environ)
        before = _authority_manifest(REPOSITORY, environment)
        self.assertEqual(git_object_authority_status(), b"")
        self.assertEqual(_authority_manifest(REPOSITORY, environment), before)

    def test_01_git_absent_archive_uses_only_named_historical_authority(self) -> None:
        authority = git_object_authority()
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            archive = Path(value).resolve() / "archive"
            current = archive / ".github/workflows/pages.yml"
            current.parent.mkdir(parents=True)
            expected = (REPOSITORY / ".github/workflows/pages.yml").read_bytes()
            current.write_bytes(expected)
            environment = _archive_environment(archive, authority.root, authority.head)
            admitted = git_object_authority(repository=archive, environment=environment)
            self.assertEqual(admitted, authority)
            self.assertFalse(os.path.lexists(archive / ".git"))
            self.assertEqual(current.read_bytes(), expected)
            self.assertEqual(
                _authority_manifest(archive, environment)[0],
                f"{authority.head}\n".encode(),
            )
            self.assertEqual(current.read_bytes(), expected)

    def test_02_o1_transition_browser_and_runner_consumers_are_split(self) -> None:
        files = {
            name: (REPOSITORY / "tools/site/tests/site" / name).read_text(
                encoding="utf-8"
            )
            for name in (
                "test_archive_binding_fail_closed.py",
                "test_triggers.py",
                "test_browser_supply.py",
                "test_runner_layout.py",
            )
        }
        for name, text in files.items():
            with self.subTest(name=name):
                self.assertIn("historical_git", text)
                self.assertNotIn("cwd=REPOSITORY", text)
        self.assertIn(
            "git_object_authority", files["test_archive_binding_fail_closed.py"]
        )
        for current_path in (
            '.github/workflows/pages.yml").read_text',
            'tools/site/run_offline_site_checks.sh").read_text',
            'REPOSITORY / "docs/site/package.json"',
        ):
            self.assertTrue(
                any(current_path in text for text in files.values()), current_path
            )

    def test_03_admitted_workflow_and_runner_lifecycle_is_positive(self) -> None:
        workflow, runner = _admitted_text()
        self.assertEqual(_workflow_errors(workflow, runner), [])

    def test_10_missing_and_ambiguous_authority_paths_fail_closed(self) -> None:
        authority = git_object_authority()
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            root = Path(value).resolve()
            archive = root / "archive"
            archive.mkdir()
            current = archive / "current-sentinel"
            current.write_bytes(b"current archive is usable\n")
            candidates = {
                "missing": {},
                "empty": {GIT_OBJECT_REPOSITORY_VARIABLE: ""},
                "relative": {GIT_OBJECT_REPOSITORY_VARIABLE: "relative"},
                "nonexistent": {GIT_OBJECT_REPOSITORY_VARIABLE: str(root / "missing")},
            }
            regular = root / "regular"
            regular.write_text("not a repository\n", encoding="utf-8")
            candidates["non-directory"] = {GIT_OBJECT_REPOSITORY_VARIABLE: str(regular)}
            link = root / "authority-link"
            link.symlink_to(authority.root, target_is_directory=True)
            candidates["symlink"] = {GIT_OBJECT_REPOSITORY_VARIABLE: str(link)}
            candidates["noncanonical"] = {
                GIT_OBJECT_REPOSITORY_VARIABLE: str(root / "archive" / ".." / "archive")
            }
            candidates["equal"] = {GIT_OBJECT_REPOSITORY_VARIABLE: str(archive)}
            nested = archive / "nested"
            nested.mkdir()
            candidates["inside archive"] = {GIT_OBJECT_REPOSITORY_VARIABLE: str(nested)}
            for label, environment in candidates.items():
                environment[SOURCE_SHA_VARIABLE] = authority.head
                with (
                    self.subTest(label=label),
                    self.assertRaises(GitObjectAuthorityError),
                ):
                    git_object_authority(repository=archive, environment=environment)
                self.assertEqual(current.read_bytes(), b"current archive is usable\n")

    def test_11_repository_identity_and_history_mutants_reach_their_gates(self) -> None:
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            root = Path(value).resolve()
            archive = root / "archive"
            archive.mkdir()
            repository, head = _make_repository(root)
            environment = _archive_environment(archive, repository, head)
            self.assertEqual(
                git_object_authority(repository=archive, environment=environment).head,
                head,
            )

            subdirectory = repository / "subdirectory"
            subdirectory.mkdir()
            wrong_top = _archive_environment(archive, subdirectory.resolve(), head)
            with self.assertRaisesRegex(GitObjectAuthorityError, "top level differs"):
                git_object_authority(repository=archive, environment=wrong_top)

            wrong_head = dict(environment)
            wrong_head[SOURCE_SHA_VARIABLE] = "f" * 40
            with self.assertRaisesRegex(GitObjectAuthorityError, "HEAD differs"):
                git_object_authority(repository=archive, environment=wrong_head)

            (repository / "current.txt").write_text("changed head\n", encoding="utf-8")
            _run_git(repository, "add", "current.txt")
            _run_git(
                repository,
                "-c",
                "user.name=authority-oracle",
                "-c",
                "user.email=authority-oracle@example.invalid",
                "commit",
                "-qm",
                "changed",
            )
            with self.assertRaisesRegex(GitObjectAuthorityError, "HEAD differs"):
                git_object_authority(repository=archive, environment=environment)

            unborn = root / "unborn"
            unborn.mkdir()
            _run_git(unborn, "init", "-q")
            unborn_environment = _archive_environment(archive, unborn.resolve(), head)
            with self.assertRaises(GitObjectAuthorityError):
                git_object_authority(repository=archive, environment=unborn_environment)

    def test_12_exact_head_without_history_fails_at_object_availability(self) -> None:
        authority = git_object_authority()
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            root = Path(value).resolve()
            archive = root / "archive"
            shallow = root / "shallow"
            archive.mkdir()
            shallow.mkdir()
            _run_git(shallow, "init", "-q")
            _run_git(
                shallow,
                "fetch",
                "-q",
                "--depth=1",
                "--no-tags",
                f"file://{authority.root}",
                authority.head,
            )
            _run_git(shallow, "checkout", "-q", "--detach", "FETCH_HEAD")
            environment = _archive_environment(
                archive, shallow.resolve(), authority.head
            )
            self.assertEqual(
                git_object_authority(repository=archive, environment=environment).head,
                authority.head,
            )
            for commit in FIXED_OBJECTS:
                with (
                    self.subTest(commit=commit),
                    self.assertRaisesRegex(GitObjectAuthorityError, "failed"),
                ):
                    historical_git(
                        "cat-file",
                        "-e",
                        f"{commit}^{{commit}}",
                        repository=archive,
                        environment=environment,
                    )

    def test_13_ambient_git_redirection_and_configuration_are_inert(self) -> None:
        authority = git_object_authority()
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            root = Path(value).resolve()
            archive = root / "archive"
            archive.mkdir()
            marker = root / "ambient-git-used"
            fake_bin = root / "fake-bin"
            fake_bin.mkdir()
            fake_git = fake_bin / "git"
            fake_git.write_text(
                f"#!/bin/sh\ntouch '{marker}'\nexit 99\n", encoding="utf-8"
            )
            fake_git.chmod(0o755)
            environment = _archive_environment(archive, authority.root, authority.head)
            environment.update(
                {
                    "PATH": str(fake_bin),
                    "GIT_DIR": "missing-git-dir",
                    "GIT_WORK_TREE": "missing-work-tree",
                    "GIT_COMMON_DIR": "missing-common-dir",
                    "GIT_INDEX_FILE": "missing-index",
                    "GIT_OBJECT_DIRECTORY": "missing-objects",
                    "GIT_ALTERNATE_OBJECT_DIRECTORIES": "missing-alternates",
                    "GIT_NAMESPACE": "foreign-namespace",
                    "GIT_CEILING_DIRECTORIES": "/",
                    "GIT_DISCOVERY_ACROSS_FILESYSTEM": "1",
                    "GIT_REPLACE_REF_BASE": "refs/replace-attacker/",
                    "GIT_CONFIG_COUNT": "1",
                    "GIT_CONFIG_KEY_0": "core.worktree",
                    "GIT_CONFIG_VALUE_0": str(root / "foreign"),
                }
            )
            admitted = git_object_authority(repository=archive, environment=environment)
            self.assertEqual(admitted, authority)
            self.assertFalse(marker.exists())

    def test_14_identity_failure_stderr_shape_size_and_timeout_fail_closed(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            root = Path(value).resolve()
            archive = root / "archive"
            authority, head = _make_repository(root)
            archive.mkdir()
            environment = _archive_environment(archive, authority, head)

            def executable(name: str, head_action: str) -> Path:
                path = root / name
                path.write_text(
                    "#!/usr/bin/env python3\n"
                    "import sys, time\n"
                    f"root = {str(authority)!r}\n"
                    "args = sys.argv[1:]\n"
                    "if args[-2:] == ['rev-parse', '--show-toplevel']:\n"
                    " print(root)\n"
                    "elif args[-3:] == ['rev-parse', '--verify', 'HEAD^{commit}']:\n"
                    f" {head_action}\n"
                    "elif args[-3:] == ['rev-parse', '--verify', 'HEAD^{tree}']:\n"
                    " print('1' * 40)\n"
                    "else:\n"
                    " raise SystemExit(2)\n",
                    encoding="utf-8",
                )
                path.chmod(0o755)
                return path

            cases = {
                "failure": "raise SystemExit(7)",
                "stderr": "print('noise', file=sys.stderr); print('a' * 40)",
                "empty": "pass",
                "non-hex": "print('g' * 40)",
                "trailing": "print('a' * 40 + ' trailing')",
                "multiple": "print('a' * 40); print('b' * 40)",
                "oversize": "print('a' * 70000)",
                "timeout": "time.sleep(2); print('a' * 40)",
            }
            for label, action in cases.items():
                with (
                    self.subTest(label=label),
                    mock.patch.object(fixture, "GIT_TIMEOUT_SECONDS", 0.1),
                    self.assertRaises(GitObjectAuthorityError),
                ):
                    git_object_authority(
                        repository=archive,
                        environment=environment,
                        executable=executable(label, action),
                    )

    def test_15_archive_current_bytes_cannot_be_redirected_or_masked(self) -> None:
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            root = Path(value).resolve()
            archive = root / "archive"
            current = archive / ".github/workflows/pages.yml"
            current.parent.mkdir(parents=True)
            current.write_bytes(b"archive-current\n")
            authority, head = _make_repository(root, "authority-current\n")
            authority_workflow = authority / ".github/workflows/pages.yml"
            authority_workflow.parent.mkdir(parents=True)
            authority_workflow.write_bytes(b"checkout-current\n")
            checkout_current = b"checkout-current\n"
            environment = _archive_environment(archive, authority, head)
            self.assertEqual(
                git_object_authority(repository=archive, environment=environment).head,
                head,
            )
            self.assertEqual(current.read_bytes(), b"archive-current\n")
            self.assertNotEqual(current.read_bytes(), checkout_current)
            current.write_bytes(b"archive-mutant\n")
            self.assertEqual(current.read_bytes(), b"archive-mutant\n")

    def test_16_git_admission_and_checkout_in_place_are_rejected(self) -> None:
        authority = git_object_authority()
        with tempfile.TemporaryDirectory(dir=SCRATCH_ROOT) as value:
            root = Path(value).resolve()
            for kind in ("directory", "file", "pointer"):
                archive = root / kind
                archive.mkdir()
                marker = archive / ".git"
                if kind == "directory":
                    marker.mkdir()
                elif kind == "file":
                    marker.write_bytes(b"object bytes\n")
                else:
                    marker.symlink_to(authority.root / ".git")
                environment = _archive_environment(
                    archive, authority.root, authority.head
                )
                with (
                    self.subTest(kind=kind),
                    self.assertRaisesRegex(GitObjectAuthorityError, "contains .git"),
                ):
                    git_object_authority(repository=archive, environment=environment)
            environment = _archive_environment(
                authority.root, authority.root, authority.head
            )
            with self.assertRaises(GitObjectAuthorityError):
                git_object_authority(repository=authority.root, environment=environment)

    def test_17_authority_write_and_current_read_commands_are_not_executable(
        self,
    ) -> None:
        authority = git_object_authority()
        before = _authority_manifest(REPOSITORY, dict(os.environ))
        forbidden = (
            ("show", "HEAD:Cargo.toml"),
            ("fetch", "origin"),
            ("checkout", "HEAD"),
            ("reset", "--hard", "HEAD"),
            ("clean", "-fdx"),
            ("gc",),
            ("maintenance", "run"),
            ("commit-graph", "write"),
            ("update-ref", "refs/heads/mutant", "HEAD"),
            ("hash-object", "-w", "Cargo.toml"),
        )
        for command in forbidden:
            with (
                self.subTest(command=command),
                self.assertRaisesRegex(
                    GitObjectAuthorityError, "not a frozen read-only object query"
                ),
            ):
                historical_git(*command)
        self.assertEqual(git_object_authority_status(), b"")
        self.assertEqual(_authority_manifest(REPOSITORY, dict(os.environ)), before)
        self.assertEqual(git_object_authority(), authority)

    def test_20_lifecycle_mutants_fail_at_the_named_boundary(self) -> None:
        workflow, runner = _admitted_text()
        self.assertEqual(_workflow_errors(workflow, runner), [])

        mutants = {
            "default shallow checkout": workflow.replace(
                "          fetch-depth: 0\n", "", 1
            ),
            "changed fetch depth": workflow.replace(
                "fetch-depth: 0", "fetch-depth: 1", 1
            ),
            "missing object": workflow.replace(next(iter(FIXED_OBJECTS)), "0" * 40, 1),
            "post-designation npm": workflow.replace(
                DESIGNATION, DESIGNATION + "\n          npm ci", 1
            ),
            "post-designation move": workflow.replace(
                DESIGNATION,
                DESIGNATION
                + '\n          mv docs/site/node_modules "$EQIORA_SITE_SOURCE_ROOT/docs/site/node_modules"',
                1,
            ),
            "post-designation fetch": workflow.replace(
                DESIGNATION, DESIGNATION + "\n          git fetch origin", 1
            ),
            "post-designation ref mutation": workflow.replace(
                DESIGNATION,
                DESIGNATION + "\n          git update-ref refs/heads/mutant HEAD",
                1,
            ),
            "post-designation current mutation": workflow.replace(
                DESIGNATION,
                DESIGNATION
                + '\n          printf mutation > "$GITHUB_WORKSPACE/current"',
                1,
            ),
            "post-designation current read": workflow.replace(
                DESIGNATION,
                DESIGNATION + '\n          cat "$GITHUB_WORKSPACE/Cargo.toml"',
                1,
            ),
        }
        for label, mutant in mutants.items():
            with self.subTest(label=label):
                self.assertTrue(_workflow_errors(mutant, runner), label)

        designation_line = f'          {{ {DESIGNATION}; }} >> "$GITHUB_ENV"\n'
        without_designation = workflow.replace(designation_line, "", 1)
        early = without_designation.replace(
            "          npm ci --engine-strict --prefix docs/site\n",
            designation_line + "          npm ci --engine-strict --prefix docs/site\n",
            1,
        )
        self.assertIn(
            "Git object authority was designated before complete Phase A supply",
            _workflow_errors(early, runner),
        )
        masked = mutants["post-designation fetch"].replace(
            next(iter(FIXED_OBJECTS)), "", 1
        )
        errors = _workflow_errors(masked, runner)
        self.assertTrue(any("fixed-object" in error for error in errors))
        self.assertTrue(any("forbidden operation" in error for error in errors))

    def test_99_current_product_is_red_at_full_history_supply(self) -> None:
        workflow = (REPOSITORY / ".github/workflows/pages.yml").read_text(
            encoding="utf-8"
        )
        runner = (REPOSITORY / "tools/site/run_offline_site_checks.sh").read_text(
            encoding="utf-8"
        )
        self.assertEqual(_workflow_errors(workflow, runner), [])


if __name__ == "__main__":
    unittest.main()
