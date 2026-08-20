from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from fixture import REPOSITORY, _workflow, checker


BASIS_SHA = "68c9c2fe245ac52cc20dcf5a65a2455de507f0dc"
ARCHIVE_ERROR = "Pages archive must bind the tracked link after extraction"
BROWSER_ERROR = "Pages browser identity must precede execution and propagation"


def workflow(pull: list[str], push: list[str]) -> str:
    def block(values: list[str]) -> str:
        return "\n".join(f'      - "{value}"' for value in values)

    ordinary = _workflow()
    complete = block(sorted({*checker.REQUIRED_TRIGGER_PATTERNS, ".gitattributes"}))
    return ordinary.replace(complete, block(pull), 1).replace(complete, block(push), 1)


def archive_case(
    root: Path, repository: Path, revision: str, mutation: str = ":"
) -> tuple[subprocess.CompletedProcess[str], Path, Path]:
    source = root / "source"
    sentinel = root / "source-used"
    script = r"""set -euo pipefail
source_links="$(git ls-tree -r "$REVISION" | awk '$1 == "120000" { print $4 }')"
case "$source_links" in ''|'CLAUDE.md') ;; *) exit 1 ;; esac
if test -n "$source_links"; then
  test "$(git cat-file blob "$REVISION:CLAUDE.md" | sha256sum | cut -d ' ' -f 1)" = "a54ff182c7e8acf56acfd6e4b9c3ff41e2c41a31c9b211b2deb9df75d9a478f9"
  git ls-tree "$REVISION" -- AGENTS.md | grep -F '100644 blob'
  git cat-file blob "$REVISION:AGENTS.md" > "$EXPECTED"
fi
mkdir -p "$SOURCE"
git archive --format=tar "$REVISION" | tar -xf - -C "$SOURCE"
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
        **os.environ,
        "REVISION": revision,
        "SOURCE": str(source),
        "EXPECTED": str(root / "expected-AGENTS.md"),
        "SENTINEL": str(sentinel),
        "MUTATION": mutation,
    }
    result = subprocess.run(
        ["bash", "-c", script],
        cwd=repository,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return result, source, sentinel


class TriggerContractTests(unittest.TestCase):
    def assert_category(self, text: str, category: str) -> None:
        errors = checker.check_workflow_text(text)
        self.assertIn(category, errors, "\n".join(errors))

    def test_00_real_linked_and_genuine_no_link_archives_reach_source_use(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home() / ".cache/eqiora") as value:
            _, _, sentinel = archive_case(Path(value), REPOSITORY, BASIS_SHA)
            self.assertEqual(sentinel.read_bytes(), b"source-used")
        with tempfile.TemporaryDirectory(dir=Path.home() / ".cache/eqiora") as value:
            root = Path(value)
            repository = root / "no-link"
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            (repository / "AGENTS.md").write_text("no link\n", encoding="utf-8")
            subprocess.run(
                ["git", "-C", str(repository), "add", "AGENTS.md"], check=True
            )
            identity = {
                **os.environ,
                "GIT_AUTHOR_NAME": "oracle",
                "GIT_AUTHOR_EMAIL": "oracle@example.invalid",
                "GIT_COMMITTER_NAME": "oracle",
                "GIT_COMMITTER_EMAIL": "oracle@example.invalid",
            }
            subprocess.run(
                ["git", "-C", str(repository), "commit", "-qm", "no link"],
                check=True,
                env=identity,
            )
            revision = subprocess.check_output(
                ["git", "-C", str(repository), "rev-parse", "HEAD"], text=True
            ).strip()
            _, _, sentinel = archive_case(root / "case", repository, revision)
            self.assertEqual(sentinel.read_bytes(), b"source-used")

    def test_01_corrected_workflow_and_exact_trigger_selection_pass_first(self) -> None:
        patterns = sorted({*checker.REQUIRED_TRIGGER_PATTERNS, ".gitattributes"})
        self.assertEqual(checker.check_workflow_text(workflow(patterns, patterns)), [])
        for changed in checker.TRIGGER_REPRESENTATIVES.values():
            self.assertTrue(checker.selected_by_paths(patterns, changed))
        self.assertTrue(checker.selected_by_paths(patterns, ".gitattributes"))
        self.assertFalse(checker.selected_by_paths(patterns, "notes/unrelated.txt"))

    def test_02_repository_workflow_is_red_at_all_three_owned_boundaries(self) -> None:
        text = (REPOSITORY / ".github/workflows/pages.yml").read_text(encoding="utf-8")
        errors = checker.check_workflow_text(text)
        self.assertLessEqual(
            {
                ARCHIVE_ERROR,
                BROWSER_ERROR,
                "Pages path filters omit exact authorities: ['.gitattributes']",
            },
            set(errors),
        )

    def test_c01_archive_mutants_stop_before_source_use(self) -> None:
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
                _, _, sentinel = archive_case(
                    Path(value), REPOSITORY, BASIS_SHA, mutation
                )
                self.assertFalse(sentinel.exists())

    def test_c01_workflow_mutants_fail_at_archive_binding(self) -> None:
        ordinary = _workflow()
        archive = checker.DIRECT_SOURCE_ARCHIVE_COMMAND
        mandatory = '            test -L "$scratch/source/CLAUDE.md"\n'
        payload = f'              test "$({checker.EXACT_EXTRACTED_LINK_COMMAND})" = {checker.EXACT_LINK_PAYLOAD_SHA256}\n'
        target = '              cmp "$scratch/source/AGENTS.md" "$scratch/expected-AGENTS.md"\n'
        post_start = ordinary.index(
            '          if test -n "$source_links"; then', ordinary.index(archive)
        )
        post_end = ordinary.index(
            '          echo "EQIORA_SITE_SOURCE_ROOT=', post_start
        )
        post = ordinary[post_start:post_end]
        moved_post = ordinary[:post_start] + ordinary[post_end:]
        moved_post = moved_post.replace(archive, post + archive, 1)
        mutants = [
            ordinary.replace(mandatory, "", 1),
            ordinary.replace(
                archive, f'{archive}\n          unlink "$scratch/source/CLAUDE.md"', 1
            ),
            ordinary.replace(
                archive, f'{archive}\n          rm -f "$scratch/source/CLAUDE.md"', 1
            ),
            ordinary.replace(
                archive,
                f'{archive}\n          rm -f "$scratch/source/CLAUDE.md"; printf AGENTS.md > "$scratch/source/CLAUDE.md"',
                1,
            ),
            moved_post,
            ordinary.replace(
                archive, archive.replace("tar -xf", "tar --dereference -xf"), 1
            ),
            ordinary.replace(
                archive,
                'git archive --format=tar "$GITHUB_SHA" -- . \':(exclude)CLAUDE.md\' | tar -xf - -C "$scratch/source"',
                1,
            ),
            ordinary.replace(archive, "ARCHIVE-MOVED", 1).replace(
                "unshare --net", f"{archive}\nunshare --net", 1
            ),
            ordinary.replace(payload, "", 1),
            ordinary.replace(target, "", 1),
        ]
        for mutant in mutants:
            self.assert_category(mutant, ARCHIVE_ERROR)

    def test_c02_workflow_mutants_fail_at_browser_admission(self) -> None:
        ordinary = _workflow()
        checker_call = "          python3 tools/site/check_site.py browser-supply"
        version = '          version_hex="$("$browser_path" --version | od -An -tx1 | tr -d \'[:space:]\')"\n'
        propagation = '          EQIORA_SITE_BROWSER_SHA256="$expected_browser_sha256"\n          EQIORA_SITE_BROWSER_BYTES="$expected_browser_bytes"\n          export EQIORA_SITE_BROWSER_SHA256 EQIORA_SITE_BROWSER_BYTES\n'
        mutants = [
            ordinary.replace(version, "", 1).replace(
                checker_call, version + checker_call, 1
            ),
            ordinary.replace(
                '          test "$browser_sha256" = "$expected_browser_sha256"\n', "", 1
            ),
            ordinary.replace(
                '          test "$browser_bytes" = "$expected_browser_bytes"\n', "", 1
            ),
            ordinary.replace(
                "0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71",
                "1" * 64,
                1,
            ),
            ordinary.replace(
                'expected_browser_bytes="290614600"',
                'expected_browser_bytes="290614601"',
                1,
            ),
            ordinary.replace(
                'EQIORA_SITE_BROWSER_SHA256="$expected_browser_sha256"',
                'EQIORA_SITE_BROWSER_SHA256="$browser_sha256"',
                1,
            ),
            ordinary.replace(
                'EQIORA_SITE_BROWSER_BYTES="$expected_browser_bytes"',
                'EQIORA_SITE_BROWSER_BYTES="$browser_bytes"',
                1,
            ),
            ordinary.replace(propagation, "", 1).replace(
                '          test "$browser_sha256"',
                propagation + '          test "$browser_sha256"',
                1,
            ),
        ]
        for mutant in mutants:
            self.assert_category(mutant, BROWSER_ERROR)
        for token in checker.OFFLINE_WORKFLOW_TOKENS:
            self.assertTrue(
                checker.check_workflow_text(ordinary.replace(token, "removed"))
            )
        for token in checker.FORBIDDEN_WORKFLOW_TOKENS:
            self.assertTrue(checker.check_workflow_text(f"{ordinary}\n# {token}\n"))

    def test_c03_exact_attribute_trigger_mutants_fail(self) -> None:
        patterns = sorted({*checker.REQUIRED_TRIGGER_PATTERNS, ".gitattributes"})
        without = [item for item in patterns if item != ".gitattributes"]
        mutants = [
            workflow(without, without),
            workflow(patterns, without),
            workflow(without, patterns),
            workflow(without, without) + "\n# .gitattributes\n",
            workflow([*without, ".git*"], [*without, ".git*"]),
            workflow([*without, ".*"], [*without, ".*"]),
            workflow(patterns, list(reversed(patterns))),
            workflow([*patterns, ".gitattributes"], [*patterns, ".gitattributes"]),
        ]
        for removed in checker.REQUIRED_TRIGGER_PATTERNS:
            reduced = [item for item in patterns if item != removed]
            mutants.append(workflow(reduced, reduced))
        for mutant in mutants:
            errors = checker.check_workflow_text(mutant)
            self.assertTrue(errors)


if __name__ == "__main__":
    unittest.main()
