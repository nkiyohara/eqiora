from __future__ import annotations

import contextlib
import copy
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import classify_changes
from authenticate_previous_run import authenticate_deployed_pages


class PagesDeploymentReuseTests(unittest.TestCase):
    def fixture(self):
        sha = "a" * 40
        base = "https://api.github.com/repos/example/project"
        steps = lambda name: [
            {"name": name, "status": "completed", "conclusion": "success"}
        ]
        return {
            f"{base}/deployments?environment=github-pages&per_page=1": [
                {"id": 1, "sha": sha, "environment": "github-pages", "ref": "main"}
            ],
            f"{base}/deployments/1/statuses?per_page=1": [
                {
                    "state": "success",
                    "environment": "github-pages",
                    "log_url": "https://github.com/example/project/actions/runs/2/job/4",
                }
            ],
            f"{base}/actions/runs/2": {
                "id": 2,
                "event": "push",
                "head_branch": "main",
                "head_sha": sha,
                "path": ".github/workflows/pages.yml",
                "status": "completed",
                "conclusion": "success",
            },
            f"{base}/actions/runs/2/jobs?filter=latest&per_page=100": {
                "total_count": 2,
                "jobs": [
                    {
                        "id": 3,
                        "name": "Build and verify static documentation",
                        "conclusion": "success",
                        "steps": steps(
                            "Build and verify with only loopback networking"
                        ),
                    },
                    {
                        "id": 4,
                        "name": "Deploy static documentation",
                        "conclusion": "success",
                        "steps": steps("Deploy GitHub Pages artifact"),
                    },
                ],
            },
        }

    def test_success_binds_the_deployed_commit_and_publishing_job(self):
        result = authenticate_deployed_pages(
            repository="example/project", fetch=self.fixture().__getitem__
        )
        self.assertEqual(result["previous_sha"], "a" * 40)
        self.assertEqual(result["lanes"], {"site": True})
        self.assertEqual(result["event"], "push")

    def test_unpublished_or_unrelated_runs_cannot_skip_a_build(self):
        original = self.fixture()
        urls = list(original)
        for index, field, value in (
            (0, "ref", "feature"),
            (0, "environment", "preview"),
            (1, "state", "failure"),
            (1, "state", "in_progress"),
            (1, "log_url", "https://github.com/foreign/project/actions/runs/2/job/4"),
            (2, "head_sha", "b" * 40),
            (2, "event", "pull_request"),
            (2, "head_branch", "feature"),
            (2, "path", ".github/workflows/ci.yml"),
            (2, "conclusion", "failure"),
            (3, "total_count", 3),
        ):
            with self.subTest(field=field, value=value):
                payload = copy.deepcopy(original)
                target = payload[urls[index]]
                (target[0] if isinstance(target, list) else target)[field] = value
                with self.assertRaises(ValueError):
                    authenticate_deployed_pages(
                        repository="example/project", fetch=payload.__getitem__
                    )
        for job_index in (0, 1):
            payload = copy.deepcopy(original)
            payload[urls[3]]["jobs"][job_index]["steps"][0]["conclusion"] = "skipped"
            with self.assertRaises(ValueError):
                authenticate_deployed_pages(
                    repository="example/project", fetch=payload.__getitem__
                )

    def test_main_compares_with_deployment_not_the_previous_push(self):
        attestation = authenticate_deployed_pages(
            repository="example/project", fetch=self.fixture().__getitem__
        )
        for paths, unsafe, event, ref, expected in (
            (["docs/language/core.md"], False, "push", "refs/heads/main", False),
            (
                ["docs/site/src/content/docs/index.mdx"],
                False,
                "push",
                "refs/heads/main",
                True,
            ),
            (["tools/site/check_site.py"], False, "push", "refs/heads/main", True),
            (["unknown/file"], False, "push", "refs/heads/main", True),
            (["docs/language/core.md"], True, "push", "refs/heads/main", True),
            ([], False, "workflow_dispatch", "refs/heads/main", True),
            ([], False, "push", "refs/heads/feature", True),
        ):
            with (
                self.subTest(paths=paths, event=event, ref=ref),
                tempfile.TemporaryDirectory() as directory,
            ):
                receipt = Path(directory) / "previous.json"
                receipt.write_text(json.dumps(attestation))
                argv = [
                    "classify",
                    "--event",
                    event,
                    "--workflow",
                    "pages.yml",
                    "--previous",
                    "a" * 40,
                    "--reuse-attestation",
                    str(receipt),
                ]
                with (
                    mock.patch.object(sys, "argv", argv),
                    mock.patch.dict("os.environ", {"GITHUB_REF": ref}, clear=True),
                    mock.patch.object(
                        classify_changes, "exact_head", return_value="b" * 40
                    ),
                    mock.patch.object(
                        classify_changes, "exact_commit", side_effect=lambda sha, _: sha
                    ),
                    mock.patch.object(
                        classify_changes,
                        "snapshot_changed_paths",
                        return_value=(paths, unsafe),
                    ) as diff,
                    contextlib.redirect_stdout(io.StringIO()) as output,
                ):
                    self.assertEqual(classify_changes.main(), 0)
                values = dict(
                    line.split("=", 1) for line in output.getvalue().splitlines()
                )
                self.assertEqual(values["site"], str(expected).lower())
                if not expected:
                    diff.assert_called_once_with("a" * 40, "b" * 40)
                    self.assertEqual(values["site_source_sha"], "a" * 40)
                    classify_changes.append_github_outputs(
                        Path(directory) / "output", output.getvalue().strip()
                    )
