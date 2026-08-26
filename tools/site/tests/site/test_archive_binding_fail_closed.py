from __future__ import annotations

import hashlib
import os
import stat
import subprocess
import tempfile
import textwrap
import unittest
from functools import cache
from pathlib import Path

from fixture import (
    REPOSITORY,
    checker,
    git_object_authority,
    git_read_environment,
    historical_git,
)


BASIS = "5d2a9bef58c2df32cd6b14c5b6dd876beac7144f"
BASIS_TREE = "3237f739098498ac46bfdd6a993c00b0575900f3"
WORKFLOW_BLOB = "57f8b9b476c04b8103b5a43c8a30504c0e2fa1fb"
WORKFLOW_SHA = "e458ccefde0d3940805ddfc42d3d947996ca72d68dc115089d2400cffb18f17d"
LINK_BLOB = "47dc3e3d863cfb5727b87d785d09abf9743c0a72"
TARGET_BLOB = "61c1bbede492aef4a9c85fa364d031e012621809"
TARGET_SHA = "ffc9b0381a01c16b3d72389ef777842215c48b65d6eda6881f5e75bfa5d531c0"
FAIL_CLOSED = "Pages linked archive admission is not one fail-closed sequence"
EXECUTION_FAILURE = "C-01 ordinary did not reach authenticated source use"
START = 'git ls-tree -r "$GITHUB_SHA" > "$scratch/source-tree"'
ARCHIVE = checker.DIRECT_SOURCE_ARCHIVE_COMMAND
FIRST_LINK = 'test -L "$scratch/source/CLAUDE.md"'
GUARD = 'if test -L "$scratch/source/CLAUDE.md"; then'
PAYLOAD = f'test "$({checker.EXACT_EXTRACTED_LINK_COMMAND})" = "{checker.EXACT_LINK_PAYLOAD_SHA256}"'
TARGET = 'cmp "$scratch/source/AGENTS.md" "$scratch/expected-AGENTS.md"'
SOURCE_EXPORT = 'echo "EQIORA_SITE_SOURCE_ROOT=$scratch/source"'
END = '} >> "$GITHUB_ENV"'
COPY = 'cp --remove-destination "$scratch/source/AGENTS.md" "$scratch/source/CLAUDE.md"'

OLD_OPTIONAL = f"""            {GUARD}
              {PAYLOAD}
              {TARGET}
            fi
"""
SAFE_SEQUENCE = f"""            {GUARD}
              :
            else
              exit 1
            fi
            {PAYLOAD}
            {TARGET}
"""


def _fixture_scratch_parent() -> Path:
    home = Path.home()
    root = home / ".cache/eqiora"
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    if root.is_symlink() or not root.is_dir():
        raise AssertionError("archive fixture scratch root is not a real directory")
    try:
        resolved_home = home.resolve(strict=True)
        resolved_root = root.resolve(strict=True)
    except OSError as error:
        raise AssertionError("archive fixture scratch root is unavailable") from error
    if resolved_root != resolved_home / ".cache/eqiora":
        raise AssertionError("archive fixture scratch root is not home-backed")
    details = root.stat()
    if details.st_uid != os.geteuid() or details.st_mode & (
        stat.S_IWGRP | stat.S_IWOTH
    ):
        raise AssertionError("archive fixture scratch root is unsafe")
    return root


@cache
def workflows() -> tuple[str, str, bytes]:
    def require(actual, expected) -> None:
        if actual != expected:
            raise AssertionError(f"historical C-01 identity changed: {actual!r}")

    def identity(data: bytes) -> tuple[int, int, str]:
        return len(data), data.count(b"\n"), hashlib.sha256(data).hexdigest()

    def resolve(path: str) -> bytes:
        return historical_git("rev-parse", f"{BASIS}:{path}").strip()

    workflow = historical_git("cat-file", "blob", WORKFLOW_BLOB)
    target = historical_git("cat-file", "blob", TARGET_BLOB)
    require(
        historical_git("rev-parse", f"{BASIS}^{{tree}}").strip(),
        BASIS_TREE.encode(),
    )
    require(resolve(".github/workflows/pages.yml"), WORKFLOW_BLOB.encode())
    require(identity(workflow), (30_567, 651, WORKFLOW_SHA))
    require(resolve("CLAUDE.md"), LINK_BLOB.encode())
    require(resolve("AGENTS.md"), TARGET_BLOB.encode())
    entries = historical_git("ls-tree", BASIS, "--", "AGENTS.md", "CLAUDE.md").split()
    require((entries[0], entries[4]), (b"100644", b"120000"))
    require(historical_git("cat-file", "blob", LINK_BLOB), b"AGENTS.md")
    require(identity(target), (12_408, 200, TARGET_SHA))
    historical = workflow.decode("utf-8")
    current = (REPOSITORY / ".github/workflows/pages.yml").read_text(encoding="utf-8")
    return historical, current, target


def span(text: str) -> tuple[str, ...]:
    lines = [" ".join(line.split()) for line in text.splitlines() if line.strip()]
    start, export = lines.index(START), lines.index(SOURCE_EXPORT)
    end = lines.index(END, export)
    return tuple(lines[start : end + 1])


def run_case(text: str, linked: bool = True, enforce: bool = True):
    try:
        admitted = span(text) == span(workflows()[1])
    except (ValueError, UnicodeError):
        admitted = False
    if enforce and not admitted:
        return [FAIL_CLOSED], None, (False, False, False, "", b"", b"")
    with tempfile.TemporaryDirectory(dir=_fixture_scratch_parent()) as value:
        root, run = Path(value), subprocess.run
        repo = root / "repository"
        run(["git", "init", "-q", str(repo)], check=True)
        command = ["git", "-C", str(repo)]
        if linked:
            authority = git_object_authority()
            fetch = [*command, "fetch", "-q", "--no-tags", str(authority.root), BASIS]
            git_environment = git_read_environment()
            run(fetch, check=True, env=git_environment)
            run(
                [*command, "checkout", "-q", "--detach", "FETCH_HEAD"],
                check=True,
                env=git_environment,
            )
        else:
            (repo / "AGENTS.md").write_text("genuine no-link tree\n", encoding="utf-8")
            run([*command, "add", "AGENTS.md"], check=True)
            command += ["-c", "user.name=oracle"]
            command += ["-c", "user.email=oracle@example.invalid"]
            run([*command, "commit", "-qm", "no link"], check=True)
        revision = (
            subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"])
            .decode()
            .strip()
        )
        lines = text.splitlines(keepends=True)
        values = [" ".join(line.split()) for line in lines]
        start = values.index(START)
        end = values.index(END, start)
        block = textwrap.dedent("".join(lines[start : end + 1]))
        case, sentinel = root / "case", root / "case/source-used"
        script = f"""set -euo pipefail
scratch="$CASE_ROOT"
mkdir -p "$scratch/source"
scratch_device="$(stat -c %d -- "$scratch")"
scratch_inode="$(stat -c %i -- "$scratch")"
{block}
grep -Fx "EQIORA_SITE_SOURCE_ROOT=$scratch/source" "$GITHUB_ENV"
printf source-used > "$scratch/source-used"
"""
        environment = os.environ.copy()
        environment["CASE_ROOT"] = str(case)
        environment["GITHUB_ENV"] = str(root / "github-env")
        environment["GITHUB_SHA"] = revision
        shell = ["bash", "-c", script]
        result = run(shell, cwd=repo, env=environment, capture_output=True)
        claude = case / "source/CLAUDE.md"
        exists = os.path.lexists(claude)
        state = [sentinel.exists(), claude.is_symlink(), exists]
        state += [os.readlink(claude) if claude.is_symlink() else ""]
        agents = (case / "source/AGENTS.md").read_bytes()
        state += [agents, claude.read_bytes() if exists else b""]
        errors = [] if result.returncode == 0 and state[0] else [EXECUTION_FAILURE]
        return errors, result, tuple(state)


def replace(text: str, old: str, new: str) -> str:
    if text.count(old) != 1:
        raise AssertionError(f"mutation target is not unique: {old!r}")
    return text.replace(old, new, 1)


def regularized(text: str):
    lines = text.splitlines(keepends=True)
    values = [" ".join(line.split()) for line in lines]
    start = values.index(ARCHIVE)
    end = values.index(END, start)
    source, target = '"$scratch/source/AGENTS.md"', '"$scratch/source/CLAUDE.md"'
    alias = '"$scratch/source/./CLAUDE.md"'
    variants = {
        "copy": f"cp --remove-destination {source} {target}",
        "move-copy": f'mv {target} "$scratch/source/CLAUDE.link"\ncp {source} {target}',
        "copy-variable": f'destination={target}\ncp --remove-destination {source} "$destination"',
        "move-copy-variable": f'destination={target}\nmv "$destination" "$scratch/source/CLAUDE.link"\ncp {source} "$destination"',
        "copy-path-alias": f"cp --remove-destination {source} {alias}",
        "move-copy-path-alias": f'mv {alias} "$scratch/source/CLAUDE.link"\ncp {source} {alias}',
    }
    for name, commands in variants.items():
        insertion = "".join(f"          {line}\n" for line in commands.splitlines())
        for boundary in range(start, end):
            mutant = lines[: boundary + 1] + [insertion] + lines[boundary + 1 :]
            yield name, boundary - start, "".join(mutant)


class ArchiveBindingFailClosedTests(unittest.TestCase):
    def assert_admitted(self, text: str, linked: bool = True) -> None:
        errors, result, state = run_case(text, linked)
        self.assertEqual(errors, [], getattr(result, "stderr", b""))
        if linked:
            self.assertEqual(state[:4], (True, True, True, "AGENTS.md"))
            self.assertEqual(state[4:], (workflows()[2], workflows()[2]))
        else:
            self.assertEqual(state[:3], (True, False, False))

    def assert_rejected(self, text: str) -> None:
        errors, result, state = run_case(text)
        self.assertEqual((errors, result, state[0]), ([FAIL_CLOSED], None, False))

    def test_00_exact_linked_archive_reaches_source_use(self) -> None:
        self.assert_admitted(workflows()[1])

    def test_01_genuine_no_link_archive_reaches_source_use(self) -> None:
        self.assert_admitted(workflows()[1], linked=False)

    def test_02_corrected_reference_is_structural_and_executed(self) -> None:
        self.assertEqual(checker.check_workflow_text(workflows()[1]), [])
        self.assert_admitted(workflows()[1])

    def test_03_historical_optional_branch_is_causal_reject(self) -> None:
        text = workflows()[0]
        self.assert_rejected(text)
        first = f"            {FIRST_LINK}\n"
        unsafe = replace(text, first, first + f"          {COPY}\n")
        result, state = run_case(unsafe, enforce=False)[1:]
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(state[:3], (True, False, True))

    def test_04_current_workflow_is_fail_closed(self) -> None:
        path = REPOSITORY / ".github/workflows/pages.yml"
        self.assert_admitted(path.read_text(encoding="utf-8"))

    def test_05_regularizers_fail_at_every_boundary(self) -> None:
        for name, boundary, mutant in regularized(workflows()[1]):
            with self.subTest(regularizer=name, boundary=boundary):
                self.assert_rejected(mutant)

    def test_06_named_control_flow_and_order_mutants_fail(self) -> None:
        text = workflows()[1]
        first = f"            {FIRST_LINK}\n"
        payload, target = f"            {PAYLOAD}\n", f"            {TARGET}\n"
        identities = payload + target
        archive, export = f"          {ARCHIVE}\n", f"            {SOURCE_EXPORT}\n"
        empty = f"            {GUARD}\n              :\n            fi\n"
        condition = f"if {PAYLOAD}; then"
        conditional = f"            {condition}\n              :\n            fi\n"
        branches = (
            OLD_OPTIONAL,
            f"            {GUARD}\n              {PAYLOAD}\n            fi\n{target}",
            f"{payload}            {GUARD}\n              {TARGET}\n            fi\n",
        )
        mutants = [replace(text, SAFE_SEQUENCE, branch) for branch in branches]
        mutants += [replace(text, item, "") for item in (first, payload, target)]
        without = replace(text, identities, "")
        mutants += [
            replace(without, archive, identities + archive),
            replace(without, export, export + identities),
            replace(text, identities, target + payload),
            replace(text, first, first + "          fi\n"),
            replace(text, identities, empty),
            replace(text, payload, payload.rstrip() + " || true\n"),
            replace(text, payload, conditional),
        ]
        for index, mutant in enumerate(mutants):
            with self.subTest(mutant=index):
                self.assert_rejected(mutant)
