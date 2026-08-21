from __future__ import annotations

import hashlib
import subprocess
import tempfile
import unittest
from pathlib import Path

from fixture import (
    REPOSITORY,
    _workflow,
    checker,
    git_object_authority,
    git_read_environment,
    historical_git,
)


BASIS_SHA = "68c9c2fe245ac52cc20dcf5a65a2455de507f0dc"
ARCHIVE_ERROR = "Pages archive must bind the tracked link after extraction"
BROWSER_ERROR = "Pages browser identity must precede execution and propagation"
C03_ERROR = "Pages path filters omit exact authorities: ['.gitattributes']"
ORDER_ERROR = "Pages archive/browser supply checks are out of causal order"
DEREFERENCE_ERROR = "Pages workflow uses forbidden supply substitution '--dereference'"
HISTORY_TREE = "20701fe8909295b980c1da7cf3eab366f8d5f27c"
HISTORY_BLOB = "6e685495bf6989e1ad902a7e88c199557285cbee"
HISTORY_DIGEST = "fc55c24da8b9b58a7e997a22d5ebc26a87cdb52b73b2513a2cc91b8347432f16"
HISTORY_ERRORS = [C03_ERROR, ARCHIVE_ERROR, BROWSER_ERROR]
UNRELATED_ERROR = "UNRELATED: retained deployment syntax rejected"
observe = checker.check_workflow_text


def omitted(token: str) -> str:
    return f"Pages workflow omits offline/supply boundary {token!r}"


def historical_workflow() -> str:
    tree = historical_git("rev-parse", f"{BASIS_SHA}^{{tree}}").decode().strip()
    blob = (
        historical_git("rev-parse", f"{BASIS_SHA}:.github/workflows/pages.yml")
        .decode()
        .strip()
    )
    data = historical_git("cat-file", "blob", HISTORY_BLOB)
    digest = hashlib.sha256(data).hexdigest()
    actual = (tree, blob, len(data), data.count(b"\n"), digest)
    if actual != (HISTORY_TREE, HISTORY_BLOB, 30_202, 642, HISTORY_DIGEST):
        raise AssertionError(f"historical workflow identity changed: {actual!r}")
    return data.decode("utf-8")


def workflow(pull: list[str], push: list[str]) -> str:
    def block(values: list[str]) -> str:
        return "\n".join(f'      - "{value}"' for value in values)

    complete = block(sorted({*checker.REQUIRED_TRIGGER_PATTERNS, ".gitattributes"}))
    before, between, after = _workflow().split(complete)
    return before + block(pull) + between + block(push) + after


def archive_case(root: Path, repo: Path, revision: str, mutation: str = ":") -> Path:
    root.mkdir(parents=True, exist_ok=True)
    source, sentinel = root / "source", root / "source-used"
    script = r"""set -euo pipefail
source_links="$(git -C "$GIT_REPOSITORY" ls-tree -r "$REVISION" | awk '$1 == "120000" { print $4 }')"
case "$source_links" in ''|'CLAUDE.md') ;; *) exit 1 ;; esac
if test -n "$source_links"; then
  test "$(git -C "$GIT_REPOSITORY" cat-file blob "$REVISION:CLAUDE.md" | sha256sum | cut -d ' ' -f 1)" = "a54ff182c7e8acf56acfd6e4b9c3ff41e2c41a31c9b211b2deb9df75d9a478f9"
  git -C "$GIT_REPOSITORY" ls-tree "$REVISION" -- AGENTS.md | grep -F '100644 blob'
  git -C "$GIT_REPOSITORY" cat-file blob "$REVISION:AGENTS.md" > "$EXPECTED"
fi
mkdir -p "$SOURCE"
git -C "$GIT_REPOSITORY" archive --format=tar "$REVISION" | tar -xf - -C "$SOURCE"
eval "$MUTATION"
if test -n "$source_links"; then
  test -L "$SOURCE/CLAUDE.md"
  test "$(readlink -n "$SOURCE/CLAUDE.md" | sha256sum | cut -d ' ' -f 1)" = "a54ff182c7e8acf56acfd6e4b9c3ff41e2c41a31c9b211b2deb9df75d9a478f9"
  cmp "$SOURCE/AGENTS.md" "$EXPECTED"
elif test -e "$SOURCE/CLAUDE.md" || test -L "$SOURCE/CLAUDE.md"; then
  exit 1
fi
printf source-used > "$SENTINEL"
"""
    environment = {
        **git_read_environment(),
        "GIT_REPOSITORY": str(repo),
        "REVISION": revision,
        "SOURCE": str(source),
        "EXPECTED": str(root / "expected-AGENTS.md"),
        "SENTINEL": str(sentinel),
        "MUTATION": mutation,
    }
    command = ["bash", "-c", script]
    subprocess.run(command, cwd=root, env=environment, capture_output=True)
    return sentinel


class TriggerContractTests(unittest.TestCase):
    def assert_transition(self, *actual: list[str]) -> None:
        self.assertEqual(list(actual), [HISTORY_ERRORS, [], []])

    def assert_diagnostics(self, text: str, expected: list[str]) -> None:
        self.assertCountEqual(observe(text), expected)

    def test_00_real_linked_and_genuine_no_link_archives_reach_source_use(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home() / ".cache/eqiora") as value:
            root = Path(value)
            sentinel = archive_case(root, git_object_authority().root, BASIS_SHA)
            self.assertEqual(sentinel.read_bytes(), b"source-used")
            repository = root / "no-link"
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            git = ["git", "-C", str(repository)]
            (repository / "AGENTS.md").write_text("no link\n", encoding="utf-8")
            subprocess.run([*git, "add", "AGENTS.md"], check=True)
            git += ["-c", "user.name=oracle"]
            git += ["-c", "user.email=oracle@example.invalid"]
            subprocess.run([*git, "commit", "-qm", "no link"], check=True)
            revision = subprocess.check_output([*git, "rev-parse", "HEAD"])
            revision = revision.decode().strip()
            sentinel = archive_case(root / "case", repository, revision)
            self.assertEqual(sentinel.read_bytes(), b"source-used")

    def test_01_historical_and_corrected_workflow_pass_first(self) -> None:
        patterns = sorted({*checker.REQUIRED_TRIGGER_PATTERNS, ".gitattributes"})
        corrected = workflow(patterns, patterns)
        self.assert_transition(observe(historical_workflow()), observe(corrected), [])
        for changed in checker.TRIGGER_REPRESENTATIVES.values():
            self.assertTrue(checker.selected_by_paths(patterns, changed))
        self.assertTrue(checker.selected_by_paths(patterns, ".gitattributes"))
        self.assertFalse(checker.selected_by_paths(patterns, "notes/unrelated.txt"))

    def test_02_current_workflow_is_exactly_green(self) -> None:
        text = (REPOSITORY / ".github/workflows/pages.yml").read_text(encoding="utf-8")
        self.assert_transition(
            observe(historical_workflow()), observe(_workflow()), observe(text)
        )

    def test_03_transition_diagnostic_mutants_fail_exactly(self) -> None:
        historical = HISTORY_ERRORS

        def reject(*mutant: list[str]) -> None:
            with self.assertRaises(AssertionError):
                self.assert_transition(*mutant)

        for index, error in enumerate(historical):
            missing = historical.copy()
            missing.pop(index)
            renamed = historical.copy()
            renamed[index] = f"{error} (legacy alias)"
            reject(missing, [], [])
            reject(renamed, [], [])
            reject(historical, [], [error])
        reject([*historical, UNRELATED_ERROR], [], [])
        reject([historical[1], historical[0], historical[2]], [], [])
        reject(historical, [], historical)
        reject(historical, [UNRELATED_ERROR], [])
        reject(historical, [], [UNRELATED_ERROR])

    def test_c01_archive_and_workflow_mutants_fail(self) -> None:
        mutations = (
            'unlink "$SOURCE/CLAUDE.md"',
            'rm -f "$SOURCE/CLAUDE.md"',
            'rm -f "$SOURCE/CLAUDE.md"; printf AGENTS.md > "$SOURCE/CLAUDE.md"',
        )
        for mutation in mutations:
            with (
                self.subTest(mutation=mutation),
                tempfile.TemporaryDirectory(dir=Path.home() / ".cache/eqiora") as value,
            ):
                sentinel = archive_case(
                    Path(value), git_object_authority().root, BASIS_SHA, mutation
                )
                self.assertFalse(sentinel.exists())
        ordinary = _workflow()
        swap = ordinary.replace
        assert_errors = self.assert_diagnostics
        archive = checker.DIRECT_SOURCE_ARCHIVE_COMMAND
        mandatory = '            test -L "$scratch/source/CLAUDE.md"\n'
        payload = f'              test "$({checker.EXACT_EXTRACTED_LINK_COMMAND})" = {checker.EXACT_LINK_PAYLOAD_SHA256}\n'
        target = '              cmp "$scratch/source/AGENTS.md" "$scratch/expected-AGENTS.md"\n'
        post_marker = '          if test -n "$source_links"; then'
        export_marker = '          echo "EQIORA_SITE_SOURCE_ROOT='
        post_start = ordinary.index(post_marker, ordinary.index(archive))
        post_end = ordinary.index(export_marker, post_start)
        post = ordinary[post_start:post_end]
        moved_post = ordinary[:post_start] + ordinary[post_end:]
        moved_post = moved_post.replace(archive, post + archive, 1)
        assert_errors(swap(mandatory, "", 1), [ARCHIVE_ERROR])
        for command in mutations:
            command = command.replace("$SOURCE", "$scratch/source")
            mutant = swap(archive, f"{archive}\n          {command}", 1)
            assert_errors(mutant, [ARCHIVE_ERROR])
        assert_errors(moved_post, [ORDER_ERROR, ARCHIVE_ERROR])
        dereference = swap(
            archive, archive.replace("tar -xf", "tar --dereference -xf"), 1
        )
        assert_errors(dereference, [omitted(archive), DEREFERENCE_ERROR, ARCHIVE_ERROR])
        exclusion = 'git archive --format=tar "$GITHUB_SHA" -- . \':(exclude)CLAUDE.md\' | tar -xf - -C "$scratch/source"'
        assert_errors(swap(archive, exclusion, 1), [omitted(archive), ARCHIVE_ERROR])
        moved_archive = swap(archive, "ARCHIVE-MOVED", 1).replace(
            "unshare --net", f"{archive}\nunshare --net", 1
        )
        assert_errors(moved_archive, [ORDER_ERROR, ARCHIVE_ERROR])
        comparisons = (
            (payload, checker.EXACT_EXTRACTED_LINK_COMMAND),
            (target, target.strip()),
        )
        for statement, token in comparisons:
            assert_errors(swap(statement, "", 1), [omitted(token), ARCHIVE_ERROR])

    def test_c02_and_c03_workflow_mutants_fail(self) -> None:
        ordinary = _workflow()
        swap = ordinary.replace
        assert_errors = self.assert_diagnostics
        c02 = [BROWSER_ERROR]
        checker_call = "          python3 tools/site/check_site.py browser-supply"
        version = '          version_hex="$("$browser_path" --version | od -An -tx1 | tr -d \'[:space:]\')"\n'
        digest = "0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71"
        bytes_constant = 'expected_browser_bytes="290614600"'
        propagation = '          EQIORA_SITE_BROWSER_SHA256="$expected_browser_sha256"\n          EQIORA_SITE_BROWSER_BYTES="$expected_browser_bytes"\n          export EQIORA_SITE_BROWSER_SHA256 EQIORA_SITE_BROWSER_BYTES\n'
        replacements = (
            (digest, "1" * 64),
            (bytes_constant, 'expected_browser_bytes="290614601"'),
        )
        for old, new in replacements:
            assert_errors(swap(old, new, 1), c02)
        for name in ("sha256", "bytes"):
            equality = (
                f'          test "$browser_{name}" = "$expected_browser_{name}"\n'
            )
            export = f'EQIORA_SITE_BROWSER_{name.upper()}="$expected_browser_{name}"'
            assert_errors(swap(equality, "", 1), c02)
            assert_errors(swap(export, export.replace("expected_", ""), 1), c02)
        early_version = swap(version, "", 1).replace(
            checker_call, version + checker_call, 1
        )
        assert_errors(early_version, c02)
        early_identity = swap(propagation, "", 1).replace(
            '          test "$browser_sha256"',
            propagation + '          test "$browser_sha256"',
            1,
        )
        assert_errors(early_identity, c02)
        for token in checker.OFFLINE_WORKFLOW_TOKENS:
            self.assertTrue(observe(ordinary.replace(token, "removed")))
        for token in checker.FORBIDDEN_WORKFLOW_TOKENS:
            self.assertTrue(observe(f"{ordinary}\n# {token}\n"))

        patterns = sorted({*checker.REQUIRED_TRIGGER_PATTERNS, ".gitattributes"})
        without = [item for item in patterns if item != ".gitattributes"]
        for removed in checker.REQUIRED_TRIGGER_PATTERNS:
            reduced = [item for item in patterns if item != removed]
            self.assertTrue(observe(workflow(reduced, reduced)))
        unequal = "Pages PR and push path filters differ"
        assert_errors(workflow(without, without), [C03_ERROR])
        assert_errors(workflow(patterns, without), [unequal])
        assert_errors(workflow(without, patterns), [unequal, C03_ERROR])
        assert_errors(workflow(without, without) + "\n# .gitattributes\n", [C03_ERROR])
        for broad in (".git*", ".*"):
            replacement = [*without, broad]
            assert_errors(workflow(replacement, replacement), [C03_ERROR])
        assert_errors(workflow(patterns, list(reversed(patterns))), [unequal])
        duplicate = [*patterns, ".gitattributes"]
        duplicate_error = "Pages path filters contain duplicates"
        assert_errors(workflow(duplicate, duplicate), [duplicate_error])


if __name__ == "__main__":
    unittest.main()
