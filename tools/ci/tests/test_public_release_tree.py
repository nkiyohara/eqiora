from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

from check_public_release_tree import check  # noqa: E402


class PublicReleaseTreeTests(unittest.TestCase):
    def check_tree(self, files: dict[str, str]) -> list[str]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative, source in files.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(source, encoding="utf-8")
            return [finding.rule for finding in check(root)]

    def test_accepts_spec_numbers_and_markdown_anchors(self) -> None:
        rules = self.check_tree(
            {
                "docs/reference.md": (
                    "The importer preserves STEP #16.\n"
                    "See [the first contract](architecture.md#1-contract).\n"
                )
            }
        )

        self.assertEqual(rules, [])

    def test_rejects_private_tracking_state_and_personal_paths(self) -> None:
        issue_url = (
            "https://"
            + "github.com/"
            + "nkiyohara/"
            + "eqiora/"
            + "issues/42"
        )
        stale_state = "private " + "bootstrap"
        personal_path = "/" + "home" + "/developer/work/eqiora"
        numbered_history = "issue " + "#42"

        rules = self.check_tree(
            {
                "docs/release.md": (
                    f"Tracked at {issue_url} during {stale_state}.\n"
                    f"Built from {personal_path}.\n"
                    f"Historical {numbered_history}.\n"
                )
            }
        )

        self.assertCountEqual(
            rules,
            [
                "bare-tracking-reference",
                "personal-path",
                "stale-release-state",
                "private-issue-url",
            ],
        )

        hyphenated = self.check_tree(
            {"docs/release.md": "This is a private-" + "bootstrap state.\n"}
        )
        self.assertEqual(hyphenated, ["stale-release-state"])

    def test_rejects_machine_identity_in_registered_observations(self) -> None:
        device = "GPU-" + "12345678-1234-1234-1234-123456789abc"
        observation = json.dumps(
            {
                "host" + "name": "builder.example.invalid",
                "load_" + "average_before": "0.1 0.2 0.3 1/20 4321",
                "visible_device": device,
                "gpu_snapshot": "00000000:31:00.0, P0",
                "process_" + "id": 4321,
            }
        )

        rules = self.check_tree(
            {
                "verify/example/observations/environment.json": observation,
            }
        )

        self.assertCountEqual(
            rules,
            [
                "gpu-uuid",
                "host-identity",
                "host-identity",
                "pci-address",
                "process-identity",
            ],
        )

    def test_does_not_treat_runtime_uuid_examples_as_machine_identity(self) -> None:
        run_id = "00000000-0000-4000-8000-000000000001"

        rules = self.check_tree(
            {
                "studio/state.test.ts": f'const runId = "{run_id}";\n',
                "verify/example/expected/result.json": json.dumps(
                    {"run_id": run_id}
                ),
            }
        )

        self.assertEqual(rules, [])

    def test_ignores_only_repository_root_git_worktree_metadata(self) -> None:
        personal_path = "/" + "home" + "/developer/work/eqiora"

        rules = self.check_tree(
            {
                ".git": f"gitdir: {personal_path}/.git/worktrees/release\n",
                "metadata.txt": "public source\n",
            }
        )

        self.assertEqual(rules, [])

        nested = self.check_tree(
            {"fixture/.git": f"gitdir: {personal_path}/private\n"}
        )
        self.assertEqual(nested, ["personal-path"])


if __name__ == "__main__":
    unittest.main()
