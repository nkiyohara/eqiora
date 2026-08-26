from __future__ import annotations

import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock


CI_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = CI_ROOT.parents[1]
sys.path.insert(0, str(CI_ROOT))

import python_package_gate as python_package_gate_module  # noqa: E402

from check_gate import JOB_SURFACES, evaluate, parse_relevance, parse_results  # noqa: E402
from classify_changes import (  # noqa: E402
    CLASSIFIED_SURFACES,
    SURFACES,
    append_github_outputs,
    changed_paths,
    classify,
    render_outputs,
)
from local_verify import HOSTED_TEST_PROFILE  # noqa: E402
from python_jax_gate import uv_gate_command as jax_uv_gate_command  # noqa: E402
from python_matplotlib_gate import (  # noqa: E402
    run as run_matplotlib_gate_command,
    uv_gate_command as matplotlib_uv_gate_command,
)
from python_package_gate import (  # noqa: E402
    run as run_python_package_gate_command,
    uv_gate_command,
    venv_environment,
    venv_python,
)
from python_torch_gate import uv_gate_command as torch_uv_gate_command  # noqa: E402


class HostedTriggerTests(unittest.TestCase):
    EXPECTED_PULL_REQUEST_ACTIONS = ("opened", "reopened", "synchronize")

    def assert_exact_pull_request_actions(
        self,
        workflow: str,
        event: str,
        expected: tuple[str, ...] | None = None,
    ) -> None:
        match = re.search(
            rf"(?m)^  {re.escape(event)}:\n    types: \[([a-z_, ]+)\]$",
            workflow,
        )
        self.assertIsNotNone(match, f"{event} must declare explicit action types")
        assert match is not None
        actions = tuple(action.strip() for action in match.group(1).split(","))
        self.assertEqual(actions, expected or self.EXPECTED_PULL_REQUEST_ACTIONS)

    def test_public_workflow_runs_for_pull_requests_and_exact_sha_dispatch(
        self,
    ) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        trigger = workflow.split("permissions:", maxsplit=1)[0]
        self.assertIn("workflow_dispatch:", trigger)
        self.assertIn("required: true", trigger)
        self.assertIn("pull_request:", trigger)
        self.assertNotIn("pull_request_target:", trigger)
        self.assertNotIn("schedule:", trigger)
        self.assertNotIn("push:", trigger)
        self.assertIn("github.event.pull_request.head.sha || inputs.commit", workflow)
        self.assertIn("persist-credentials: false", workflow)

    def test_public_pull_request_lifecycle_runs_once_per_head(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        self.assert_exact_pull_request_actions(workflow, "pull_request")
        self.assertNotIn("github.event.pull_request.draft", workflow)

        concurrency = workflow.split("concurrency:\n", maxsplit=1)[1].split(
            "\nenv:", maxsplit=1
        )[0]
        self.assertEqual(
            concurrency,
            "  group: ci-${{ github.event.pull_request.number || inputs.commit }}\n"
            "  cancel-in-progress: true\n",
        )

    def test_change_ownership_stages_pinned_mise_without_reading_config(
        self,
    ) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        changes = workflow.split("  changes:\n", maxsplit=1)[1].split(
            "\n  documentation:\n", maxsplit=1
        )[0]
        setup = """      - name: Stage the checksum-pinned mise CLI
        shell: bash
        run: |
          mise_bin_dir="${RUNNER_TEMP:?}/eqiora-mise/bin"
          mise_bin="$mise_bin_dir/mise"
          mkdir -p "$mise_bin_dir"
          curl --fail --location --proto '=https' --tlsv1.2 --silent --show-error \\
            --output "$mise_bin" \\
            https://github.com/jdx/mise/releases/download/v2026.5.10/mise-v2026.5.10-linux-x64
          printf '%s  %s\\n' \\
            568e6074262804788f138fb8749865738e47dff739ebaa0d428134c45957b569 \\
            "$mise_bin" | sha256sum --check --strict
          chmod 0755 "$mise_bin"
          printf '%s\\n' "$mise_bin_dir" >> "$GITHUB_PATH"
"""

        test_step = "      - name: Test CI ownership and aggregate contracts"
        stage_step = "      - name: Stage the checksum-pinned mise CLI"
        stage_start = changes.index(stage_step)
        test_start = changes.index(test_step)

        self.assertNotIn("jdx/mise-action@", workflow)
        self.assertNotIn("github.token", changes)
        self.assertNotIn("secrets.", changes)
        self.assertEqual(
            changes[stage_start:test_start],
            setup,
        )
        contract_step = changes[test_start:].split(
            "      - name: Classify exact change surface", maxsplit=1
        )[0]
        self.assertIn('EQIORA_CI_CONTRACT_ONLY: "1"', contract_step)
        self.assertEqual(workflow.count("EQIORA_CI_CONTRACT_ONLY"), 1)

    def test_windows_compile_probe_is_visible_complete_and_non_gating(self) -> None:
        workflow = (
            REPOSITORY_ROOT / ".github/workflows/windows-compile-probe.yml"
        ).read_text(encoding="utf-8")
        trigger = workflow.split("permissions:", maxsplit=1)[0]
        command = (
            "cargo +stable check --workspace --all-targets --all-features "
            "--locked --keep-going"
        )

        self.assertIn("workflow_dispatch:", trigger)
        self.assertNotIn("schedule:", trigger)
        self.assertNotIn("pull_request:", trigger)
        self.assertNotIn("push:", trigger)
        self.assertIn("runs-on: windows-latest", workflow)
        self.assertIn("continue-on-error: true", workflow)
        self.assertIn("MSMPISDK", workflow)
        self.assertIn("MSMPI_INC", workflow)
        self.assertIn("MSMPI_LIB64", workflow)
        self.assertEqual(workflow.count(f"run: {command}"), 1)
        self.assertNotIn("--exclude", workflow)
        self.assertIn("GITHUB_STEP_SUMMARY", workflow)

        aggregate = (
            (REPOSITORY_ROOT / ".github/workflows/ci.yml")
            .read_text(encoding="utf-8")
            .split("\n  gate:\n", maxsplit=1)[1]
        )
        self.assertNotIn("windows", aggregate.lower())

    def test_base_owned_trust_workflow_never_checks_out_head_code(self) -> None:
        workflow = (
            REPOSITORY_ROOT / ".github/workflows/ci-definition-trust.yml"
        ).read_text(encoding="utf-8")
        self.assertIn("pull_request_target:", workflow)
        self.assert_exact_pull_request_actions(
            workflow,
            "pull_request_target",
            (*self.EXPECTED_PULL_REQUEST_ACTIONS, "edited"),
        )
        self.assertNotIn("github.event.pull_request.draft", workflow)
        self.assertIn("github.event.pull_request.base.sha", workflow)
        self.assertEqual(workflow.count("github.event.pull_request.head.sha"), 1)
        self.assertIn(
            '--head-sha "${{ github.event.pull_request.head.sha }}"', workflow
        )
        self.assertNotIn("ref: ${{ github.event.pull_request.head.sha }}", workflow)
        head_sha_lines = [
            line.strip()
            for line in workflow.splitlines()
            if "github.event.pull_request.head.sha" in line
        ]
        self.assertEqual(
            head_sha_lines,
            ['--head-sha "${{ github.event.pull_request.head.sha }}"'],
        )
        self.assertEqual(workflow.count("uses: actions/checkout@"), 1)
        self.assertIn("pull-requests: read", workflow)
        self.assertNotIn("contents: write", workflow)
        self.assertNotIn("id-token: write", workflow)

        concurrency = workflow.split("concurrency:\n", maxsplit=1)[1].split(
            "\njobs:", maxsplit=1
        )[0]
        self.assertEqual(
            concurrency,
            "  group: ci-definition-trust-${{ github.event.pull_request.number }}\n"
            "  cancel-in-progress: true\n",
        )

    def test_msrv_gate_covers_optional_production_features(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        msrv = workflow.split("  msrv:\n", maxsplit=1)[1].split(
            "\n  dependency_policy:", maxsplit=1
        )[0]
        self.assertIn("MSRV 1.89", msrv)
        self.assertIn("cargo +1.89.0 check", msrv)
        self.assertIn("--workspace --all-targets --all-features --locked", msrv)
        self.assertIn("libopenmpi-dev", msrv)

    def test_host_evidence_has_an_independent_declared_environment(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        quality = workflow.split("  quality:\n", maxsplit=1)[1].split(
            "\n  host_evidence:", maxsplit=1
        )[0]
        evidence = workflow.split("  host_evidence:\n", maxsplit=1)[1].split(
            "\n  python_host_evidence:", maxsplit=1
        )[0]
        python_evidence = workflow.split("  python_host_evidence:\n", maxsplit=1)[
            1
        ].split("\n  msrv:", maxsplit=1)[0]
        action = "actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97"

        self.assertIn(action, quality)
        self.assertIn('python-version: "3.12"', quality)
        self.assertIn('["tested-numpy-floor"]', quality)
        self.assertIn("python -m pip install --only-binary=:all:", quality)
        self.assertNotIn('["uv"]', quality)
        self.assertNotIn("uv --version", quality)
        self.assertNotIn("eqiora-verify -- run --environment host-cpu", quality)
        self.assertIn("name: Host-CPU verification evidence", evidence)
        self.assertIn("runs-on: ubuntu-latest", evidence)
        self.assertIn("libopenmpi-dev", evidence)
        # The Cargo evidence job needs an interpreter and the NumPy floor even
        # though neither reads as Cargo work: `eqiora-python`'s Cargo tests
        # import NumPy through the embedded pyo3 interpreter. An earlier split
        # moved both to the Python job on the strength of their names, and
        # `interfaces.python-array-transport` failed closed on a missing module.
        self.assertIn(action, evidence)
        self.assertIn('["tested-numpy-floor"]', evidence)
        # It does not need the candidate builder; only wheel construction does.
        self.assertNotIn('["uv"]', evidence)
        self.assertNotIn("uv --version", evidence)
        self.assertIn(
            "eqiora-verify -- run --environment host-cpu --runner-kind cargo",
            evidence,
        )
        self.assertIn("name: Host-CPU Python installed-wheel evidence", python_evidence)
        self.assertIn("runs-on: ubuntu-latest", python_evidence)
        self.assertIn(action, python_evidence)
        self.assertIn('python-version: "3.12"', python_evidence)
        self.assertIn('["tested-numpy-floor"]', python_evidence)
        self.assertNotIn('["uv"]', python_evidence)
        self.assertIn("python -m pip install --only-binary=:all:", python_evidence)
        self.assertNotIn("uv --version", python_evidence)
        self.assertIn(
            "sudo apt-get install --no-install-recommends --yes ffmpeg",
            python_evidence,
        )
        self.assertIn("ffprobe -version", python_evidence)
        self.assertNotIn("openmpi", python_evidence)
        self.assertIn(
            "eqiora-verify -- run --environment host-cpu "
            "--runner-kind python-installed-wheel",
            python_evidence,
        )

        release = (
            REPOSITORY_ROOT / ".github/workflows/python-release-candidate.yml"
        ).read_text(encoding="utf-8")
        self.assertNotIn("uv==", release)
        self.assertIn("tools/release/python_candidate.py", release)

    def test_python_candidate_hosts_pin_n1_node_npm_and_detached_h2_receipt(
        self,
    ) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        python_evidence = workflow.split("  python_host_evidence:\n", maxsplit=1)[
            1
        ].split("\n  msrv:", maxsplit=1)[0]
        release = (
            REPOSITORY_ROOT / ".github/workflows/python-release-candidate.yml"
        ).read_text(encoding="utf-8")
        setup_node = "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38"

        for surface in (python_evidence, release):
            self.assertIn(setup_node, surface)
            self.assertIn("node-version: 24.18.1", surface)
            self.assertIn("npm@11.16.0", surface)
            self.assertIn('test "$(npm --version)" = "11.16.0"', surface)

        self.assertIn("*-python-candidate-h2.json", release)
        self.assertIn("--h2-receipt", release)
        self.assertIn("candidate-manifest", release)
        self.assertNotIn(
            '*-python-candidate-h2.json" -exec cp {} candidate-dist',
            release,
        )

    def test_python_release_has_three_revision_bound_trust_jobs_before_publish(
        self,
    ) -> None:
        release = (
            REPOSITORY_ROOT / ".github/workflows/python-release-candidate.yml"
        ).read_text(encoding="utf-8")

        def job(name: str) -> str:
            match = re.search(
                rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9_-]*:\n|\Z)",
                release.split("jobs:\n", maxsplit=1)[1],
            )
            self.assertIsNotNone(match, f"missing release job {name}")
            assert match is not None
            return match.group(1)

        def action_inputs(section: str, action: str) -> list[dict[str, str]]:
            matches = re.finditer(
                rf"(?m)^        uses: {re.escape(action)}@[^\n]+\n"
                rf"        with:\n(?P<body>(?:^          [^\n]+\n)+)",
                section,
            )
            parsed: list[dict[str, str]] = []
            for match in matches:
                values: dict[str, str] = {}
                for line in match.group("body").splitlines():
                    key, separator, value = line.strip().partition(":")
                    self.assertEqual(separator, ":")
                    values[key] = value.strip()
                parsed.append(values)
            return parsed

        def action_path(shell_path: str) -> str:
            self.assertTrue(shell_path.startswith("$RUNNER_TEMP/"))
            relative = shell_path.removeprefix("$RUNNER_TEMP/")
            return "${{ runner.temp }}/" + relative

        def compact_shell(section: str) -> str:
            without_continuations = re.sub(r"\\\n[ \t]*", "", section)
            return " ".join(without_continuations.split())

        prepare = job("prepare-family")
        h2 = job("h2")
        finalize = job("finalize-candidate")
        publish = job("publish_testpypi")
        compact_prepare = compact_shell(prepare)
        compact_h2 = compact_shell(h2)
        compact_finalize = compact_shell(finalize)

        self.assertRegex(h2, r"(?m)^    needs: prepare-family$")
        self.assertRegex(
            finalize,
            r"(?m)^    needs: \[(?:prepare-family, h2|h2, prepare-family)\]$",
        )
        self.assertRegex(publish, r"(?m)^    needs: finalize-candidate$")
        self.assertNotIn("needs: prepare-family", publish)
        self.assertNotIn("needs: h2", publish)

        prepare_command = re.search(
            r"python3 tools/release/python_candidate\.py prepare "
            r"--expected-commit \"\$CANDIDATE_COMMIT\" "
            r"(?P<tag>--require-tag )?--out \"(?P<family>[^\"]+)\"",
            compact_prepare,
        )
        self.assertIsNotNone(prepare_command)
        assert prepare_command is not None
        self.assertEqual(prepare_command.group("tag"), "--require-tag ")
        family = prepare_command.group("family")

        h2_command = re.search(
            r"python3 tools/release/python_candidate_h2\.py "
            r"--expected-commit \"\$CANDIDATE_COMMIT\" "
            r"--artifacts \"(?P<family>[^\"]+)\" "
            r"--out \"(?P<h2_out>[^\"]+)\"",
            compact_h2,
        )
        self.assertIsNotNone(h2_command)
        assert h2_command is not None
        self.assertEqual(h2_command.group("family"), family)
        h2_out = h2_command.group("h2_out")

        receipt_discovery = re.search(
            r"(?m)^          mapfile -t receipts < <\(find "
            r'"(?P<root>\$RUNNER_TEMP/candidate-h2)" '
            r"-maxdepth 1 -type f -name '\*-python-candidate-h2\.json'\)$",
            finalize,
        )
        self.assertIsNotNone(receipt_discovery)
        assert receipt_discovery is not None
        self.assertEqual(receipt_discovery.group("root"), h2_out)
        self.assertRegex(
            finalize,
            r'(?m)^          test "\$\{#receipts\[@\]\}" -eq 1$',
        )

        finalize_command = re.search(
            r"python3 tools/release/python_candidate\.py finalize "
            r"--expected-commit \"\$CANDIDATE_COMMIT\" "
            r"--artifacts \"(?P<family>[^\"]+)\" "
            r"--h2-receipt \"(?P<receipt>\$\{receipts\[0\]\})\" "
            r"--manifest-out \"(?P<metadata>[^\"]+)\"",
            compact_finalize,
        )
        self.assertIsNotNone(finalize_command)
        assert finalize_command is not None
        self.assertEqual(finalize_command.group("family"), family)
        self.assertEqual(finalize_command.group("receipt"), "${receipts[0]}")
        metadata = finalize_command.group("metadata")
        self.assertEqual(len({family, h2_out, metadata}), 3)

        prepare_downloads = action_inputs(prepare, "actions/download-artifact")
        prepare_uploads = action_inputs(prepare, "actions/upload-artifact")
        h2_downloads = action_inputs(h2, "actions/download-artifact")
        h2_uploads = action_inputs(h2, "actions/upload-artifact")
        finalize_downloads = action_inputs(finalize, "actions/download-artifact")
        finalize_uploads = action_inputs(finalize, "actions/upload-artifact")
        publish_downloads = action_inputs(publish, "actions/download-artifact")
        publish_uploads = action_inputs(publish, "actions/upload-artifact")

        self.assertEqual(prepare_downloads, [])
        self.assertEqual(len(prepare_uploads), 1)
        self.assertEqual(len(h2_downloads), 1)
        self.assertEqual(len(h2_uploads), 1)
        self.assertEqual(len(finalize_downloads), 2)
        self.assertEqual(len(finalize_uploads), 2)
        self.assertEqual(len(publish_downloads), 1)
        self.assertEqual(publish_uploads, [])

        prepared_family = prepare_uploads[0]
        self.assertEqual(prepared_family["path"], action_path(family) + "/*")
        self.assertEqual(prepared_family["compression-level"], "0")

        h2_family = h2_downloads[0]
        self.assertEqual(h2_family["name"], prepared_family["name"])
        self.assertEqual(h2_family["path"], action_path(family))
        h2_receipt = h2_uploads[0]
        self.assertEqual(h2_receipt["path"], action_path(h2_out) + "/*")

        finalize_downloads_by_name = {
            transfer["name"]: transfer for transfer in finalize_downloads
        }
        self.assertEqual(len(finalize_downloads_by_name), 2)
        self.assertEqual(
            finalize_downloads_by_name[prepared_family["name"]]["path"],
            action_path(family),
        )
        self.assertEqual(
            finalize_downloads_by_name[h2_receipt["name"]]["path"],
            action_path(h2_out),
        )

        finalize_uploads_by_path = {
            transfer["path"]: transfer for transfer in finalize_uploads
        }
        finalized_family = finalize_uploads_by_path[action_path(family) + "/*"]
        finalized_metadata = finalize_uploads_by_path[action_path(metadata) + "/*"]
        self.assertEqual(finalized_family["compression-level"], "0")
        upload_names = {
            prepared_family["name"],
            h2_receipt["name"],
            finalized_family["name"],
            finalized_metadata["name"],
        }
        self.assertEqual(len(upload_names), 4)

        published_family = publish_downloads[0]
        self.assertEqual(published_family["name"], finalized_family["name"])
        publish_packages = re.search(
            r"(?m)^          packages-dir: (?P<path>\S+)$", publish
        )
        self.assertIsNotNone(publish_packages)
        assert publish_packages is not None
        self.assertEqual(published_family["path"], publish_packages.group("path"))

        for upload in prepare_uploads + h2_uploads + finalize_uploads:
            self.assertNotIn(upload["path"], {".", "./"})

        for stage in (prepare, h2, finalize):
            self.assertIn("ref: ${{ inputs.commit }}", stage)
            self.assertIn('test "$(git rev-parse HEAD)" = "$CANDIDATE_COMMIT"', stage)
        self.assertIn("compression-level: 0", prepare)
        self.assertIn("compression-level: 0", finalize)
        self.assertNotIn("*-python-candidate.json", prepare)
        self.assertNotIn("*.whl", h2)
        self.assertNotIn("*.tar.gz", h2)

        self.assertIn("packages-dir:", publish)
        self.assertNotIn("candidate-h2", publish)
        self.assertNotIn("candidate-metadata", publish)
        self.assertLess(
            release.index("  finalize-candidate:"),
            release.index("  publish_testpypi:"),
        )

    def test_candidate_replay_and_production_publish_share_one_verified_family(
        self,
    ) -> None:
        candidate = (
            REPOSITORY_ROOT / ".github/workflows/python-release-candidate.yml"
        ).read_text(encoding="utf-8")
        production = (
            REPOSITORY_ROOT / ".github/workflows/python-production-publish.yml"
        ).read_text(encoding="utf-8")

        def job(workflow: str, name: str) -> str:
            match = re.search(
                rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9_-]*:\n|\Z)",
                workflow.split("jobs:\n", maxsplit=1)[1],
            )
            self.assertIsNotNone(match, f"missing release job {name}")
            assert match is not None
            return match.group(1)

        def transfers(section: str, action: str) -> list[dict[str, str]]:
            observed = []
            for match in re.finditer(
                rf"(?m)^        uses: {re.escape(action)}@[^\n]+\n"
                rf"        with:\n(?P<body>(?:^          [^\n]+\n)+)",
                section,
            ):
                values = {}
                for line in match.group("body").splitlines():
                    key, separator, value = line.strip().partition(":")
                    self.assertEqual(separator, ":")
                    values[key] = value.strip()
                observed.append(values)
            return observed

        finalized_family = "eqiora-python-finalized-family"
        candidate_metadata = "eqiora-python-candidate-metadata"
        finalize = job(candidate, "finalize-candidate")
        replay = job(candidate, "replay_download")
        verify = job(production, "verify")
        publish = job(production, "publish")

        uploads = {
            transfer["name"]: transfer
            for transfer in transfers(finalize, "actions/upload-artifact")
        }
        self.assertEqual(set(uploads), {finalized_family, candidate_metadata})

        with self.subTest(stage="candidate-replay"):
            replay_downloads = {
                transfer["name"]: transfer
                for transfer in transfers(replay, "actions/download-artifact")
            }
            self.assertEqual(
                set(replay_downloads),
                {finalized_family, candidate_metadata},
            )
            replay_family_path = replay_downloads[finalized_family]["path"]
            replay_metadata_path = replay_downloads[candidate_metadata]["path"]
            self.assertNotIn(replay_family_path, {"", ".", "./"})
            self.assertNotIn(replay_metadata_path, {"", ".", "./"})
            self.assertIn("*-python-candidate.json", replay)
            self.assertIn("*-python-candidate-h2.json", replay)
            self.assertIn('test "${#manifests[@]}" -eq 1', replay)
            self.assertIn('test "${#receipts[@]}" -eq 1', replay)
            replay_command = " ".join(replay.split())
            self.assertIn("python3 tools/release/testpypi_replay.py", replay_command)
            self.assertRegex(
                replay_command,
                rf"--artifacts [\"']?{re.escape(replay_family_path)}[\"']?(?:\s|$)",
            )
            self.assertRegex(
                replay,
                rf"find\s+[\"']?{re.escape(replay_metadata_path)}[\"']?\s+"
                r"-maxdepth 1\s+-type f\s+-name\s+[\"']\*-python-candidate\.json[\"']",
            )
            self.assertRegex(
                replay,
                rf"find\s+[\"']?{re.escape(replay_metadata_path)}[\"']?\s+"
                r"-maxdepth 1\s+-type f\s+-name\s+"
                r"[\"']\*-python-candidate-h2\.json[\"']",
            )
            self.assertIn('--h2-receipt "${receipts[0]}"', replay_command)
            self.assertIn('--manifest-sha256 "$MANIFEST_SHA256"', replay_command)

        with self.subTest(stage="production-verification"):
            verify_downloads = {
                transfer["name"]: transfer
                for transfer in transfers(verify, "actions/download-artifact")
            }
            self.assertEqual(
                set(verify_downloads),
                {finalized_family, candidate_metadata},
            )
            verify_family_path = verify_downloads[finalized_family]["path"]
            verify_metadata_path = verify_downloads[candidate_metadata]["path"]
            self.assertNotIn(verify_family_path, {"", ".", "./"})
            self.assertNotIn(verify_metadata_path, {"", ".", "./"})
            for transfer in verify_downloads.values():
                self.assertEqual(transfer["run-id"], "${{ inputs.candidate_run_id }}")
                self.assertEqual(transfer["github-token"], "${{ github.token }}")

            self.assertIn("*-python-candidate.json", verify)
            self.assertIn("*-python-candidate-h2.json", verify)
            self.assertIn('test "${#manifests[@]}" -eq 1', verify)
            self.assertIn('test "${#receipts[@]}" -eq 1', verify)
            verify_command = " ".join(verify.split())
            self.assertIn("python3 tools/release/candidate_manifest.py", verify_command)
            self.assertRegex(
                verify_command,
                rf"--artifacts [\"']?{re.escape(verify_family_path)}[\"']?(?:\s|$)",
            )
            self.assertRegex(
                verify,
                rf"find\s+[\"']?{re.escape(verify_metadata_path)}[\"']?\s+"
                r"-maxdepth 1\s+-type f\s+-name\s+[\"']\*-python-candidate\.json[\"']",
            )
            self.assertRegex(
                verify,
                rf"find\s+[\"']?{re.escape(verify_metadata_path)}[\"']?\s+"
                r"-maxdepth 1\s+-type f\s+-name\s+"
                r"[\"']\*-python-candidate-h2\.json[\"']",
            )
            self.assertIn('--h2-receipt "${receipts[0]}"', verify_command)
            self.assertIn('--manifest-sha256 "$MANIFEST_SHA256"', verify_command)
            self.assertIn('--expected-commit "$RELEASE_COMMIT"', verify_command)
            self.assertIn('--expected-tag "$RELEASE_TAG"', verify_command)

        with self.subTest(stage="production-publication"):
            publish_downloads = transfers(publish, "actions/download-artifact")
            self.assertEqual(len(publish_downloads), 1)
            self.assertEqual(publish_downloads[0]["name"], finalized_family)
            publish_family_path = publish_downloads[0]["path"]
            self.assertNotIn(publish_family_path, {"", ".", "./"})
            self.assertEqual(
                publish_downloads[0]["run-id"], "${{ inputs.candidate_run_id }}"
            )
            self.assertEqual(
                publish_downloads[0]["github-token"], "${{ github.token }}"
            )
            self.assertRegex(publish, r"(?m)^    needs: verify$")
            self.assertIn(f"packages-dir: {publish_family_path}", publish)

    def test_role_d_production_workflow_binds_its_dispatch_revision(self) -> None:
        equality = 'test "$GITHUB_SHA" = "$RELEASE_COMMIT"'
        acquisition = "uses: actions/download-artifact@"

        def job(workflow: str, name: str) -> str:
            match = re.search(
                rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9_-]*:\n|\Z)",
                workflow.split("jobs:\n", maxsplit=1)[1],
            )
            self.assertIsNotNone(match, f"missing production job {name}")
            assert match is not None
            return match.group(1)

        def require_definition_binding(workflow: str) -> None:
            verify = job(workflow, "verify")
            publish = job(workflow, "publish")
            self.assertEqual(
                verify.count(equality),
                1,
                "production workflow definition is not bound to RELEASE_COMMIT",
            )
            self.assertIn(acquisition, verify)
            self.assertLess(verify.index(equality), verify.index(acquisition))
            self.assertRegex(publish, r"(?m)^    needs: verify$")

        reference = f"""\
jobs:
  verify:
    steps:
      - name: Bind workflow definition
        env:
          RELEASE_COMMIT: ${{{{ inputs.commit }}}}
        run: {equality}
      - name: Acquire candidate
        {acquisition}pinned
  publish:
    needs: verify
    steps:
      - run: publish
"""
        require_definition_binding(reference)
        mutants = (
            reference.replace("$GITHUB_SHA", "$(git rev-parse HEAD)"),
            reference.replace(f"        run: {equality}\n", ""),
            reference.replace(
                f"        run: {equality}\n      - name: Acquire candidate\n"
                f"        {acquisition}pinned\n",
                f"      - name: Acquire candidate\n        {acquisition}pinned\n"
                f"      - name: Late binding\n        run: {equality}\n",
            ),
            reference.replace("    needs: verify\n", ""),
        )
        for mutant in mutants:
            with self.assertRaises(AssertionError):
                require_definition_binding(mutant)

        current = (
            REPOSITORY_ROOT / ".github/workflows/python-production-publish.yml"
        ).read_text(encoding="utf-8")
        require_definition_binding(current)

    def test_rich_display_claim_names_candidate_level_bounded_host_teardown(
        self,
    ) -> None:
        rich_case = tomllib.loads(
            (
                REPOSITORY_ROOT / "verify/interfaces/python-rich-mesh-display/case.toml"
            ).read_text(encoding="utf-8")
        )
        distribution_case = tomllib.loads(
            (
                REPOSITORY_ROOT
                / "verify/interfaces/python-distribution-candidate/case.toml"
            ).read_text(encoding="utf-8")
        )
        claim = rich_case["acceptance"]
        self.assertNotIn(
            "host_shutdown_and_kernel_or_wrapper_finalization_exit_zero",
            claim,
        )
        host_status = (
            "within timeout; accepts status 0 or exactly -SIGTERM only when the "
            "candidate runner sent SIGTERM; unsolicited signals, other nonzero "
            "statuses, timeout, and forced kill reject"
        )
        teardown = (
            f"{host_status}; success additionally requires bounded cleanup "
            "without forced escalation and a "
            "complete-empty owned notebook, kernel, browser, and profile-helper "
            "observation before the absolute 35.0-second decision deadline; "
            "failure still performs bounded cleanup and rejects with stable "
            "survivor or incomplete-observation diagnostics; no fixed-time "
            "survivor-disappearance claim"
        )
        self.assertEqual(
            claim.get("candidate_host_teardown"),
            teardown,
        )
        self.assertTrue(teardown.startswith(f"{host_status}; "))
        distribution = distribution_case["claim_boundary"]
        self.assertEqual(distribution.get("notebook_host_teardown"), teardown)
        self.assertEqual(
            distribution.get("notebook_cleanup_graceful_seconds"),
            30.0,
        )
        self.assertEqual(
            distribution.get("notebook_cleanup_decision_seconds"),
            35.0,
        )
        self.assertEqual(distribution.get("notebook_cleanup_identity_limit"), 256)
        self.assertEqual(
            distribution.get("notebook_cleanup_diagnostic_bytes"),
            65_536,
        )

    def test_hosted_test_profile_is_compact_and_test_scoped(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        quality = workflow.split("  quality:\n", maxsplit=1)[1].split(
            "\n  host_evidence:", maxsplit=1
        )[0]
        evidence = workflow.split("  host_evidence:\n", maxsplit=1)[1].split(
            "\n  python_host_evidence:", maxsplit=1
        )[0]
        python_evidence = workflow.split("  python_host_evidence:\n", maxsplit=1)[
            1
        ].split("\n  msrv:", maxsplit=1)[0]
        studio = workflow.split("  studio:\n", maxsplit=1)[1].split(
            "\n  gate:", maxsplit=1
        )[0]
        tests = quality.split("- name: Tests\n", maxsplit=1)[1].split(
            "- name: Full feature tests\n", maxsplit=1
        )[0]
        full_feature_tests = quality.split("- name: Full feature tests\n", maxsplit=1)[
            1
        ].split("- name: Dependency layers\n", maxsplit=1)[0]
        host_evidence = evidence.split(
            "- name: Run registered Cargo host evidence\n", maxsplit=1
        )[1]
        python_host_evidence = python_evidence.split(
            "- name: Run registered Python installed-wheel host evidence\n",
            maxsplit=1,
        )[1]
        profile = (
            'CARGO_PROFILE_TEST_DEBUG: "0"',
            'CARGO_PROFILE_TEST_DEBUG_ASSERTIONS: "true"',
            'CARGO_PROFILE_TEST_INCREMENTAL: "false"',
            'CARGO_PROFILE_TEST_OPT_LEVEL: "1"',
            'CARGO_PROFILE_TEST_OVERFLOW_CHECKS: "true"',
        )

        for step in (
            tests,
            full_feature_tests,
            host_evidence,
            python_host_evidence,
        ):
            for setting in profile:
                self.assertIn(setting, step)
            self.assertNotIn("RUSTFLAGS", step)
        for setting in profile:
            self.assertEqual(workflow.count(setting), 4)
        self.assertNotIn("CARGO_PROFILE_TEST_", studio)
        self.assertNotIn("fast-math", workflow.lower())

    def test_quality_workspace_tests_use_step_scoped_runner_temp(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        quality = workflow.split("  quality:\n", maxsplit=1)[1].split(
            "\n  host_evidence:", maxsplit=1
        )[0]
        expected_job_contract = (
            "    name: Stable quality gate\n"
            "    needs: changes\n"
            "    if: needs.changes.outputs.rust == 'true'\n"
            "    runs-on: ubuntu-latest\n"
            "    timeout-minutes: 180\n"
        )
        steps = (
            (
                "      - name: Tests\n",
                "      - name: Full feature tests\n",
                (),
                "cargo +stable test --workspace --all-targets --locked",
            ),
            (
                "      - name: Full feature tests\n",
                "      - name: Dependency layers\n",
                ("needs.changes.outputs.full == 'true'",),
                "cargo +stable test --workspace --all-targets --all-features --locked",
            ),
        )

        self.assertEqual(
            quality.split("    steps:\n", maxsplit=1)[0],
            expected_job_contract,
        )
        self.assertEqual(workflow.count("TMPDIR"), 3)
        self.assertEqual(
            re.findall(r"(?m)^[ \t]*TMPDIR:[ \t]*(.*?)[ \t]*$", workflow),
            ["${{ runner.temp }}"] * 3,
        )
        self.assertLess(quality.index(steps[0][0]), quality.index(steps[1][0]))
        self.assertNotRegex(
            workflow.split("jobs:\n", maxsplit=1)[0],
            r"(?m)^\s*TMPDIR:",
        )
        self.assertNotRegex(quality, r"(?m)^ {0,8}TMPDIR:")

        for marker, next_marker, conditions, command in steps:
            self.assertEqual(quality.count(marker), 1)
            step = quality.split(marker, maxsplit=1)[1].split(next_marker, maxsplit=1)[
                0
            ]
            self.assertEqual(
                re.findall(r"(?m)^        if: (.+)$", step),
                list(conditions),
            )
            environment = step.split("        env:\n", maxsplit=1)[1].split(
                "        run:", maxsplit=1
            )[0]
            self.assertEqual(
                re.findall(r"(?m)^          TMPDIR:[ \t]*(.*?)[ \t]*$", environment),
                ["${{ runner.temp }}"],
            )
            self.assertEqual(
                re.findall(r"(?m)^        run: (.+)$", step),
                [command],
            )

    def test_host_cpu_cargo_evidence_uses_step_scoped_runner_temp(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        host_evidence = workflow.split("  host_evidence:\n", maxsplit=1)[1].split(
            "\n  python_host_evidence:", maxsplit=1
        )[0]
        expected_job_contract = (
            "    name: Host-CPU verification evidence\n"
            "    needs: changes\n"
            "    if: needs.changes.outputs.rust == 'true'\n"
            "    runs-on: ubuntu-latest\n"
            "    timeout-minutes: 120\n"
        )
        previous_marker = "      - name: Install stable Rust\n"
        marker = "      - name: Run registered Cargo host evidence\n"
        command = (
            "cargo +stable run --locked -p eqiora-verify -- run "
            "--environment host-cpu --runner-kind cargo"
        )

        self.assertEqual(
            host_evidence.split("    steps:\n", maxsplit=1)[0],
            expected_job_contract,
        )
        self.assertEqual(host_evidence.count(marker), 1)
        self.assertLess(
            host_evidence.index(previous_marker), host_evidence.index(marker)
        )
        self.assertNotRegex(
            workflow.split("jobs:\n", maxsplit=1)[0],
            r"(?m)^\s*TMPDIR:",
        )
        self.assertNotRegex(host_evidence, r"(?m)^ {0,8}TMPDIR:")

        step = host_evidence.split(marker, maxsplit=1)[1]
        self.assertNotRegex(step, r"(?m)^      - ")
        self.assertNotRegex(step, r"(?m)^        if:")
        environment = step.split("        env:\n", maxsplit=1)[1].split(
            "        run:", maxsplit=1
        )[0]
        self.assertEqual(
            re.findall(r"(?m)^          TMPDIR:[ \t]*(.*?)[ \t]*$", environment),
            ["${{ runner.temp }}"],
        )
        self.assertEqual(
            re.findall(r"(?m)^        run: (.+)$", step),
            [command],
        )

    def test_studio_checks_its_independent_manifest_at_the_same_msrv(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        studio = workflow.split("  studio:\n", maxsplit=1)[1].split(
            "\n  gate:", maxsplit=1
        )[0]
        formatting = (
            "cargo +stable fmt --manifest-path studio/src-tauri/Cargo.toml -- --check"
        )
        self.assertEqual(workflow.count(formatting), 1)
        self.assertIn("rustup toolchain install 1.89.0", studio)
        self.assertIn("node-version: 24.18.1", studio)
        self.assertIn(
            "cargo +1.89.0 check --manifest-path studio/src-tauri/Cargo.toml --locked --all-targets",
            studio,
        )

    def test_dependency_policy_checks_both_independent_cargo_workspaces(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        dependency = workflow.split("  dependency_policy:\n", maxsplit=1)[1].split(
            "\n  cubecl_experiment:", maxsplit=1
        )[0]
        action = (
            "EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25"
        )
        self.assertEqual(dependency.count(action), 2)
        self.assertIn("name: Check root dependency policy", dependency)
        self.assertIn("name: Check Studio dependency policy", dependency)
        self.assertIn("arguments: --all-features --locked", dependency)
        self.assertIn(
            "manifest-path: studio/src-tauri/Cargo.toml",
            dependency,
        )
        self.assertIn(
            "arguments: --all-features --locked --config studio/src-tauri/deny.toml",
            dependency,
        )

        dependabot = (REPOSITORY_ROOT / ".github/dependabot.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("package-ecosystem: uv", dependabot)
        self.assertIn("package-ecosystem: npm", dependabot)


class DependencyIdentityTests(unittest.TestCase):
    def assert_exact_adapter_dependency(
        self,
        dependency: str,
        owner: str,
        source: str,
        constant: str,
    ) -> None:
        manifest = tomllib.loads(
            (REPOSITORY_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        )
        declared = manifest["workspace"]["dependencies"][dependency]
        version = declared["version"] if isinstance(declared, dict) else declared
        self.assertRegex(version, r"^=\d+\.\d+\.\d+$")
        release = version.removeprefix("=")

        owner_manifest = tomllib.loads(
            (REPOSITORY_ROOT / owner / "Cargo.toml").read_text(encoding="utf-8")
        )
        self.assertIs(
            owner_manifest["dependencies"][dependency]["workspace"],
            True,
        )

        lock = tomllib.loads(
            (REPOSITORY_ROOT / "Cargo.lock").read_text(encoding="utf-8")
        )
        resolved = [
            package["version"]
            for package in lock["package"]
            if package["name"] == dependency
        ]
        self.assertEqual(resolved, [release])

        adapter = (REPOSITORY_ROOT / source).read_text(encoding="utf-8")
        self.assertIn(
            f'pub const {constant}: &str = "{release}";',
            adapter,
        )

    def test_production_rust_manifests_share_one_msrv(self) -> None:
        root = tomllib.loads(
            (REPOSITORY_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        )
        studio = tomllib.loads(
            (REPOSITORY_ROOT / "studio/src-tauri/Cargo.toml").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(root["workspace"]["package"]["rust-version"], "1.89")
        self.assertEqual(studio["package"]["rust-version"], "1.89")

    def test_diffsol_backend_release_matches_exact_manifest_and_lock(self) -> None:
        manifest = tomllib.loads(
            (REPOSITORY_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        )
        declared = manifest["workspace"]["dependencies"]["diffsol"]["version"]
        self.assertRegex(declared, r"^=\d+\.\d+\.\d+$")
        release = declared.removeprefix("=")

        lock = tomllib.loads(
            (REPOSITORY_ROOT / "Cargo.lock").read_text(encoding="utf-8")
        )
        resolved = [
            package["version"]
            for package in lock["package"]
            if package["name"] == "diffsol"
        ]
        self.assertEqual(resolved, [release])

        adapter = (
            REPOSITORY_ROOT / "crates/eqiora-backend-diffsol/src/runtime.rs"
        ).read_text(encoding="utf-8")
        self.assertIn(
            f'TimeBackendIdentity::new("eqiora.time.diffsol", "{release}")',
            adapter,
        )

    def test_faer_backend_release_matches_exact_manifest_and_lock(self) -> None:
        self.assert_exact_adapter_dependency(
            "faer",
            "crates/eqiora-backend-faer",
            "crates/eqiora-backend-faer/src/lib.rs",
            "FAER_VERSION",
        )

    def test_rayon_adapter_release_matches_exact_manifest_and_lock(self) -> None:
        self.assert_exact_adapter_dependency(
            "rayon",
            "crates/eqiora-backend-rayon",
            "crates/eqiora-backend-rayon/src/lib.rs",
            "RAYON_VERSION",
        )


class PythonPackageGateTests(unittest.TestCase):
    def test_sdist_remains_git_backed_for_explicit_out_dir_includes(self) -> None:
        document = tomllib.loads(
            (REPOSITORY_ROOT / "pyproject.toml").read_text(encoding="utf-8")
        )
        maturin = document["tool"]["maturin"]

        self.assertEqual(maturin["sdist-generator"], "git")
        self.assertIn(
            {
                "path": "steady-flow-past-cylinder.model.json",
                "from": "out-dir",
                "to": "eqiora/examples/",
            },
            maturin["include"],
        )
        self.assertTrue(
            {
                "target/**",
                "/target/**",
                "**/target/**",
            }.isdisjoint(maturin["exclude"])
        )

    def test_fallback_activates_venv_for_pep517_backend_tools(self) -> None:
        virtual_environment = Path("/tmp/eqiora-test-venv")
        environment = venv_environment(
            virtual_environment,
            base={"PATH": "/usr/bin"},
        )

        self.assertEqual(environment["VIRTUAL_ENV"], str(virtual_environment))
        self.assertEqual(
            environment["PATH"],
            f"{venv_python(virtual_environment).parent}{os.pathsep}/usr/bin",
        )

    def test_uv_rebuilds_the_current_noneditable_project_without_gmsh(self) -> None:
        command = uv_gate_command("uv", "/usr/bin/python3")

        self.assertIn("--no-editable", command)
        index = command.index("--reinstall-package")
        self.assertEqual(command[index + 1], "eqiora")
        self.assertNotIn("--extra", command)
        self.assertNotIn("gmsh", command)
        self.assertEqual(command[command.index("--python") + 1], "/usr/bin/python3")

    def test_package_gate_runs_base_only_before_exact_gmsh_evidence(self) -> None:
        tests = REPOSITORY_ROOT / "bindings/python/tests"
        gmsh_evidence = tuple(
            str(tests / name)
            for name in (
                "test_circular_hole_chordal_mesh.py",
                "test_exact_cylinder_stokes_result.py",
                "test_rich_mesh_display.py",
            )
        )
        temporary = mock.MagicMock()
        temporary.__enter__.return_value = "/reviewed/python-package-gate"
        with (
            mock.patch.object(
                python_package_gate_module.shutil,
                "which",
                side_effect=("/reviewed/uv", None),
            ),
            mock.patch.object(
                python_package_gate_module.tempfile,
                "TemporaryDirectory",
                return_value=temporary,
            ),
            mock.patch.object(python_package_gate_module, "run") as run,
        ):
            self.assertEqual(python_package_gate_module.main(), 0)
            uv_calls = tuple(run.call_args_list)
            run.reset_mock()
            self.assertEqual(python_package_gate_module.main(), 0)
            pip_calls = tuple(run.call_args_list)

        expected_base_tail = [
            str(tests),
            "--ignore",
            gmsh_evidence[0],
            "--ignore",
            gmsh_evidence[1],
            "--ignore",
            gmsh_evidence[2],
        ]
        with self.subTest(path="uv"):
            commands = [call.args[0] for call in uv_calls]
            self.assertEqual(len(commands), 2)
            base, gmsh = commands
            self.assertNotIn("--extra", base)
            self.assertEqual(
                base[base.index(expected_base_tail[0]) :], expected_base_tail
            )
            self.assertEqual(gmsh[gmsh.index("--extra") + 1], "gmsh")
            self.assertEqual(gmsh[gmsh.index("-q") + 1 :], list(gmsh_evidence))
            self.assertEqual(
                [
                    base[base.index("--python") + 1],
                    gmsh[gmsh.index("--python") + 1],
                ],
                [sys.executable, sys.executable],
            )

        with self.subTest(path="pip"):
            self.assertEqual(len(pip_calls), 6)
            environment = Path(temporary.__enter__.return_value)
            python = str(venv_python(environment))
            self.assertEqual(
                pip_calls[3:],
                (
                    mock.call(
                        [python, "-m", "pytest", "-q", *expected_base_tail],
                        cwd=python_package_gate_module.PACKAGE,
                        virtual_environment=environment,
                    ),
                    mock.call(
                        [
                            python,
                            "-m",
                            "pip",
                            "install",
                            "--no-build-isolation",
                            ".[gmsh]",
                        ],
                        cwd=python_package_gate_module.PACKAGE,
                        virtual_environment=environment,
                    ),
                    mock.call(
                        [python, "-m", "pytest", "-q", *gmsh_evidence],
                        cwd=python_package_gate_module.PACKAGE,
                        virtual_environment=environment,
                    ),
                ),
            )
            self.assertEqual(
                [call.kwargs.get("virtual_environment") for call in pip_calls[1:]],
                [environment] * 5,
            )

    @mock.patch("python_package_gate.subprocess.run")
    def test_package_gate_removes_host_python_path(self, run: mock.Mock) -> None:
        with mock.patch.dict(os.environ, {"PYTHONPATH": "/host/cpython312"}):
            run_python_package_gate_command(["python", "-V"])

        environment = run.call_args.kwargs["env"]
        self.assertNotIn("PYTHONPATH", environment)
        self.assertEqual(environment["PYTHONNOUSERSITE"], "1")

    def test_torch_gate_installs_the_extra_and_exact_verified_release(self) -> None:
        command = torch_uv_gate_command("uv", "/usr/bin/python3")

        self.assertEqual(command[command.index("--extra") + 1], "torch")
        self.assertIn("torch==2.13.0", command)
        self.assertTrue(command[-1].endswith("bindings/python/tests/test_torch.py"))

    def test_jax_gate_installs_the_exact_verified_environment(self) -> None:
        command = jax_uv_gate_command("uv")

        self.assertIn("--no-editable", command)
        self.assertEqual(command[command.index("--extra") + 1], "jax")
        self.assertIn("jax==0.11.0", command)
        self.assertIn("jaxlib==0.11.0", command)
        self.assertEqual(command[command.index("--python") + 1], "3.13")
        self.assertTrue(command[-1].endswith("bindings/python/tests/test_jax.py"))

    def test_matplotlib_gate_installs_the_extra_and_exact_renderer(self) -> None:
        command = matplotlib_uv_gate_command("uv")

        extras = [
            command[index + 1]
            for index, value in enumerate(command)
            if value == "--extra"
        ]
        self.assertEqual(extras, ["gmsh", "matplotlib"])
        self.assertIn("matplotlib==3.11.1", command)
        self.assertEqual(command[command.index("--python") + 1], "3.13")
        self.assertTrue(
            command[-1].endswith("bindings/python/tests/test_matplotlib.py")
        )

    @mock.patch("python_matplotlib_gate.subprocess.run")
    def test_matplotlib_gate_isolates_host_configuration(
        self,
        run: mock.Mock,
    ) -> None:
        inherited = {
            "MATPLOTLIBRC": "/host/matplotlibrc",
            "MPLCONFIGDIR": "/host/matplotlib-config",
            "PYTHONPATH": "/host/cpython312",
        }
        with mock.patch.dict(os.environ, inherited):
            run_matplotlib_gate_command(["python", "-V"])

        environment = run.call_args.kwargs["env"]
        self.assertNotIn("MATPLOTLIBRC", environment)
        self.assertNotIn("PYTHONPATH", environment)
        self.assertNotEqual(environment["MPLCONFIGDIR"], inherited["MPLCONFIGDIR"])
        self.assertEqual(environment["MPLBACKEND"], "Agg")


class ChangeClassificationTests(unittest.TestCase):
    def test_documentation_only_selects_no_heavy_surface(self) -> None:
        selected = classify(
            ["docs/architecture.md", "README.md", "bindings/python/README.md"]
        )
        self.assertEqual(selected, {surface: False for surface in CLASSIFIED_SURFACES})

        evidence_projection = classify(["verify/numerics/linear-backends/README.md"])
        self.assertTrue(evidence_projection["site"])
        self.assertFalse(evidence_projection["msrv"])

    def test_site_input_closure_selects_only_real_artifact_inputs(self) -> None:
        relevant = (
            ".github/workflows/pages.yml",
            "crates/eqiora/src/lib.rs",
            "bindings/python/python/eqiora/fluid.pyi",
            "docs/site/src/content/docs/index.mdx",
            "tools/docs/generate_python_api.py",
            "tools/xtask/src/main.rs",
            "verify/fluid/example/case.toml",
        )
        for path in relevant:
            with self.subTest(path=path):
                self.assertTrue(classify([path])["site"])

        irrelevant = (
            "README.md",
            "docs/architecture.md",
            "bindings/python/README.md",
            "studio/src/state.ts",
        )
        for path in irrelevant:
            with self.subTest(path=path):
                self.assertFalse(classify([path])["site"])

    def test_numerics_change_selects_rust_and_msrv_only(self) -> None:
        selected = classify(["crates/eqiora-numerics/src/lib.rs"])
        self.assertTrue(selected["rust"])
        self.assertTrue(selected["msrv"])
        self.assertFalse(selected["python"])
        self.assertFalse(selected["studio"])

    def test_public_facade_selects_installed_clients(self) -> None:
        for path in ("crates/eqiora/src/lib.rs", "api/eqiora-facade-v1.json"):
            with self.subTest(path=path):
                selected = classify([path])
                self.assertTrue(selected["rust"])
                self.assertTrue(selected["python"])
                self.assertTrue(selected["studio"])

    def test_python_and_studio_own_their_surfaces(self) -> None:
        python = classify(["bindings/python/python/eqiora/__init__.pyi"])
        self.assertTrue(python["python"])
        self.assertFalse(python["rust"])
        studio = classify(["studio/src/state.ts"])
        self.assertTrue(studio["studio"])
        self.assertFalse(studio["rust"])
        self.assertFalse(studio["dependency_policy"])

    def test_private_python_frontend_selects_python_not_studio(self) -> None:
        for path in (
            "bindings/python/frontend/package-lock.json",
            "bindings/python/frontend/src/mesh-view.ts",
            "bindings/python/python/eqiora/_presentation/static/mesh-view.mjs",
        ):
            with self.subTest(path=path):
                selected = classify([path])
                self.assertTrue(selected["python"])
                self.assertIn(
                    "python_host_evidence=true",
                    render_outputs("a" * 40, selected, full=False),
                )
                self.assertFalse(selected["studio"])
                self.assertFalse(selected["rust"])

    def test_studio_dependency_inputs_select_both_owned_gates(self) -> None:
        for path in (
            "studio/src-tauri/Cargo.toml",
            "studio/src-tauri/Cargo.lock",
            "studio/src-tauri/deny.toml",
        ):
            with self.subTest(path=path):
                selected = classify([path])
                self.assertTrue(selected["studio"])
                self.assertTrue(selected["dependency_policy"])
                self.assertFalse(selected["rust"])

    def test_dependency_and_experiment_inputs_are_independent(self) -> None:
        dependency = classify(["crates/eqiora-core/Cargo.toml"])
        self.assertTrue(dependency["dependency_policy"])
        dependabot = classify([".github/dependabot.yml"])
        self.assertTrue(dependabot["dependency_policy"])
        self.assertFalse(dependabot["rust"])
        deny = classify(["deny.toml"])
        self.assertTrue(deny["dependency_policy"])
        self.assertFalse(deny["rust"])
        self.assertFalse(deny["msrv"])
        cubecl = classify(["experiments/cubecl-local-action/src/lib.rs"])
        self.assertTrue(cubecl["cubecl_experiment"])
        self.assertFalse(cubecl["rust"])

    def test_verification_data_does_not_select_msrv(self) -> None:
        selected = classify(["verify/numerics/linear-backends/case.toml"])
        self.assertTrue(selected["rust"])
        self.assertFalse(selected["msrv"])

    def test_governance_and_unknown_paths_fail_closed(self) -> None:
        for path in (
            ".github/workflows/ci.yml",
            ".github/actions/check/action.yml",
            "tools/ci/check_gate.py",
            "assets/requirements.txt",
            "new-area/file.bin",
        ):
            with self.subTest(path=path):
                self.assertEqual(
                    classify([path]), {surface: True for surface in CLASSIFIED_SURFACES}
                )

        python_requirements = classify(["bindings/python/requirements.txt"])
        self.assertTrue(python_requirements["python"])

    def test_rename_classifies_source_and_destination(self) -> None:
        zero = b"0" * 40
        blob = b"1" * 40
        completed = mock.Mock(
            stdout=(
                b":100644 000000 " + blob + b" " + zero + b" D\0"
                b"tools/xtask/src/old.rs\0"
                b":000000 100644 " + zero + b" " + blob + b" A\0"
                b"docs/architecture/old.md\0"
            )
        )
        with mock.patch(
            "classify_changes.subprocess.run", return_value=completed
        ) as run:
            paths, unsafe_mode = changed_paths("base", "head")

        self.assertEqual(
            run.call_args.args[0],
            ["git", "diff", "--raw", "--no-renames", "-z", "base...head"],
        )
        self.assertFalse(unsafe_mode)
        selected = classify(paths)
        self.assertTrue(selected["site"])

    def test_deleted_site_input_still_selects_full_build(self) -> None:
        completed = mock.Mock(
            stdout=(
                b":100644 000000 " + b"1" * 40 + b" " + b"0" * 40 + b" D\0"
                b"tools/xtask/src/removed.rs\0"
            )
        )
        with mock.patch("classify_changes.subprocess.run", return_value=completed):
            paths, unsafe_mode = changed_paths("base", "head")
        self.assertFalse(unsafe_mode)
        self.assertTrue(classify(paths)["site"])

    def test_site_classifier_failure_falls_back_to_full_build(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(CI_ROOT / "classify_changes.py"),
                "--event",
                "pull_request",
                "--base",
                "missing-base",
                "--head",
                "HEAD",
            ],
            cwd=REPOSITORY_ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("site=true", result.stdout)
        self.assertIn("site_reason=classification failure: full build", result.stdout)
        self.assertIn("failed closed", result.stderr)

    @staticmethod
    def _commit(repository: Path, message: str) -> str:
        subprocess.run(["git", "add", "-A"], cwd=repository, check=True)
        subprocess.run(["git", "commit", "-qm", message], cwd=repository, check=True)
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=repository, text=True
        ).strip()

    @staticmethod
    def _classify_repository(repository: Path, base: str, head: str) -> str:
        completed = subprocess.run(
            [
                sys.executable,
                str(CI_ROOT / "classify_changes.py"),
                "--event",
                "pull_request",
                "--base",
                base,
                "--head",
                head,
            ],
            cwd=repository,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return completed.stdout

    def test_advanced_base_is_the_unchanged_site_content_authority(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as value:
            repository = Path(value)
            subprocess.run(["git", "init", "-q", "-b", "main"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.name", "oracle"], cwd=repository, check=True)
            subprocess.run(
                ["git", "config", "user.email", "oracle@example.invalid"],
                cwd=repository,
                check=True,
            )
            (repository / "README.md").write_text("base\n", encoding="utf-8")
            branch_point = self._commit(repository, "branch point")
            subprocess.run(["git", "switch", "-qc", "head"], cwd=repository, check=True)
            (repository / "README.md").write_text("head\n", encoding="utf-8")
            head = self._commit(repository, "irrelevant head")
            subprocess.run(["git", "switch", "-q", "main"], cwd=repository, check=True)
            site = repository / "docs/site"
            site.mkdir(parents=True)
            (site / "base.mdx").write_text("advanced base\n", encoding="utf-8")
            base = self._commit(repository, "advance base")
            subprocess.run(["git", "switch", "-q", "head"], cwd=repository, check=True)

            rendered = self._classify_repository(repository, base, head)
            self.assertNotEqual(base, branch_point)
            self.assertIn("site=false", rendered)
            self.assertIn(f"site_source_sha={base}", rendered)
            self.assertNotIn(f"site_source_sha={branch_point}", rendered)
            self.assertIn("site_reason=unchanged input closure", rendered)

    def test_irrelevant_regular_change_skips_but_symlink_changes_build(self) -> None:
        for mutation in ("regular", "symlink-add", "symlink-conversion"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory(
                dir=Path.home()
            ) as value:
                repository = Path(value)
                subprocess.run(["git", "init", "-q", "-b", "main"], cwd=repository, check=True)
                subprocess.run(["git", "config", "user.name", "oracle"], cwd=repository, check=True)
                subprocess.run(
                    ["git", "config", "user.email", "oracle@example.invalid"],
                    cwd=repository,
                    check=True,
                )
                studio = repository / "studio/src"
                studio.mkdir(parents=True)
                state = studio / "state.ts"
                state.write_text("base\n", encoding="utf-8")
                base = self._commit(repository, "base")
                if mutation == "regular":
                    state.write_text("head\n", encoding="utf-8")
                elif mutation == "symlink-add":
                    (studio / "link.ts").symlink_to("state.ts")
                else:
                    state.unlink()
                    state.symlink_to("target.ts")
                head = self._commit(repository, mutation)

                rendered = self._classify_repository(repository, base, head)
                if mutation == "regular":
                    self.assertIn("site=false", rendered)
                else:
                    self.assertIn("site=true", rendered)
                    self.assertIn(
                        "site_reason=file mode or type change: full build", rendered
                    )

    def test_full_run_selects_compatibility_matrix(self) -> None:
        selected = classify([], full=True)
        rendered = render_outputs("a" * 40, selected, full=True)
        self.assertIn('python_versions=["3.11","3.12","3.13","3.14"]', rendered)

    def test_github_output_rejects_malformed_or_inconsistent_quick_decisions(
        self,
    ) -> None:
        valid = render_outputs(
            "a" * 40,
            classify(["README.md"]),
            full=False,
            site_source_sha="b" * 40,
            site_reason="unchanged input closure",
        )
        mutations = (
            valid.replace("site_source_sha=" + "b" * 40, "site_source_sha=forged"),
            valid.replace("site_reason=unchanged input closure", "site_reason=forged"),
            valid + "\nsite=false",
        )
        with tempfile.TemporaryDirectory(dir=Path.home()) as value:
            output = Path(value) / "github-output"
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    with self.assertRaises(ValueError):
                        append_github_outputs(output, mutation)
                    self.assertFalse(output.exists())
            append_github_outputs(output, valid)
            self.assertEqual(output.read_text(encoding="utf-8"), valid + "\n")

    def test_python_host_evidence_is_selected_by_rust_or_python(self) -> None:
        for path in (
            "crates/eqiora-numerics/src/lib.rs",
            "bindings/python/python/eqiora/__init__.pyi",
        ):
            with self.subTest(path=path):
                selected = classify([path])
                rendered = render_outputs("a" * 40, selected, full=False)
                self.assertIn("python_host_evidence=true", rendered)

        selected = classify(["docs/architecture.md"])
        rendered = render_outputs("a" * 40, selected, full=False)
        self.assertIn("python_host_evidence=false", rendered)


class AggregateGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.relevance = {surface: False for surface in SURFACES}
        self.results = {
            "changes": "success",
            "documentation": "success",
            "quality": "skipped",
            "host_evidence": "skipped",
            "python_host_evidence": "skipped",
            "msrv": "skipped",
            "dependency_policy": "skipped",
            "cubecl_experiment": "skipped",
            "python_wheel": "skipped",
            "studio": "skipped",
        }

    def test_documentation_only_run_is_accepted(self) -> None:
        self.assertEqual(evaluate(self.relevance, self.results), [])

    def test_relevant_skip_and_any_failure_are_rejected(self) -> None:
        self.relevance["rust"] = True
        self.assertTrue(evaluate(self.relevance, self.results))
        self.relevance["rust"] = False
        self.results["quality"] = "failure"
        self.assertTrue(evaluate(self.relevance, self.results))

    def test_relevant_success_is_accepted(self) -> None:
        self.relevance["python"] = True
        self.results["python_wheel"] = "success"
        self.results["python_host_evidence"] = "success"
        self.assertEqual(evaluate(self.relevance, self.results), [])

    def test_rust_surface_requires_quality_and_registered_evidence(self) -> None:
        self.relevance["rust"] = True
        self.results["quality"] = "success"
        self.assertTrue(evaluate(self.relevance, self.results))
        self.results["host_evidence"] = "success"
        self.assertTrue(evaluate(self.relevance, self.results))
        self.results["python_host_evidence"] = "success"
        self.assertEqual(evaluate(self.relevance, self.results), [])

    def test_relevance_contract_rejects_missing_and_malformed_values(self) -> None:
        complete = {surface: "false" for surface in SURFACES}
        self.assertEqual(parse_relevance(complete), self.relevance)

        for malformed in ({**complete, "rust": ""}, {**complete, "rust": "typo"}):
            with self.subTest(malformed=malformed["rust"]):
                with self.assertRaises(ValueError):
                    parse_relevance(malformed)

        missing = dict(complete)
        del missing["rust"]
        with self.assertRaises(ValueError):
            parse_relevance(missing)

    def test_result_vocabulary_is_complete_and_exact(self) -> None:
        self.assertEqual(parse_results(self.results), self.results)
        for malformed in (
            {
                key: value
                for key, value in self.results.items()
                if key != "host_evidence"
            },
            {**self.results, "unregistered_job": "success"},
        ):
            with self.assertRaises(ValueError):
                parse_results(malformed)

    def test_every_workflow_job_is_admitted_by_the_gate(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        jobs = set(
            re.findall(r"^  ([a-z][a-z0-9_]*):$", workflow.split("jobs:\n", 1)[1], re.M)
        )
        conditional = jobs - {"changes", "documentation", "gate"}
        self.assertEqual(conditional, set(JOB_SURFACES))

        gate = workflow.split("\n  gate:\n", maxsplit=1)[1]
        for job in conditional:
            self.assertIn(f"      - {job}\n", gate)
            self.assertIn(f'"{job}":"${{{{ needs.{job}.result }}}}"', gate)


class MiseTaskContractTests(unittest.TestCase):
    AUTHORITY_DOCS = (
        "AGENTS.md",
        "docs/development/ai-authored-platform-strategy.md",
        "docs/development/local-verification.md",
        "docs/development/vertical-slice-development.md",
    )
    DIRECT_GATE = re.compile(
        r"(?:/usr/bin/)?python3\s+tools/ci/local_verify\.py\s+"
        r"(?:pr|fast|affected|periodic)(?=[\s`\\;&]|\|\||$)"
    )
    PLANNER_VALUE_OPTIONS = frozenset(
        (
            "--base",
            "--case",
            "--cpu-slots",
            "--memory-mib",
            "--gpu-slots",
            "--scratch-root",
        )
    )
    PLANNER_VALUE = re.compile(r"[A-Za-z0-9_./:@+~^-]+")
    SETUP_FREE_DIAGNOSTIC_MARKER = "# eqiora: setup-free-local-verify-diagnostic-only"

    @classmethod
    def setUpClass(cls) -> None:
        cls.tasks = tomllib.loads(
            (REPOSITORY_ROOT / "mise.toml").read_text(encoding="utf-8")
        )["tasks"]

    def test_executable_gates_and_studio_tasks_require_locked_setup(self) -> None:
        setup = self.tasks["setup"]
        self.assertEqual(setup["dir"], "{{config_root}}/studio")
        self.assertEqual(setup["run"], "npm ci")
        self.assertIn(
            "{{config_root}}/studio/package-lock.json",
            setup["sources"],
        )
        for task in (
            "pr",
            "fast",
            "affected",
            "periodic",
            "studio:check",
            "studio:test",
            "studio:dev",
        ):
            with self.subTest(task=task):
                self.assertEqual(self.tasks[task]["depends"], ["setup"])

    def test_standard_gates_wrap_exact_planner_commands(self) -> None:
        expected = {
            "pr": "python3 tools/ci/local_verify.py pr --base origin/main",
            "fast": "python3 tools/ci/local_verify.py fast --base origin/main",
            "affected": (
                "python3 tools/ci/local_verify.py affected --base origin/main"
            ),
            "periodic": "python3 tools/ci/local_verify.py periodic",
        }
        for task, command in expected.items():
            with self.subTest(task=task):
                self.assertEqual(self.tasks[task]["run"], command)

    def test_mise_forwards_arguments_after_setup_to_every_executable_gate(self) -> None:
        mise = self._required_mise_executable()

        environment = dict(os.environ)
        environment["COLUMNS"] = "240"
        environment.pop("FORCE_COLOR", None)
        environment["NO_COLOR"] = "1"
        for task, marker in (
            ("pr", "r"),
            ("fast", "f"),
            ("affected", "a"),
            ("periodic", "p"),
        ):
            with self.subTest(task=task):
                completed = subprocess.run(
                    [
                        mise,
                        "--locked",
                        "run",
                        "--dry-run",
                        "--force",
                        task,
                        "--",
                        f"--x={marker}",
                    ],
                    cwd=REPOSITORY_ROOT,
                    env=environment,
                    check=False,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                )
                self.assertEqual(completed.returncode, 0, completed.stdout)
                setup = "[setup] $ npm ci"
                invocation = f"[{task}] $ {self.tasks[task]['run']} --x={marker}"
                self.assertIn(setup, completed.stdout)
                self.assertIn(invocation, completed.stdout)
                self.assertLess(
                    completed.stdout.index(setup),
                    completed.stdout.index(invocation),
                )

    @staticmethod
    def _required_mise_executable() -> str:
        mise = os.environ.get("MISE_EXE") or shutil.which("mise")
        if mise is None:
            raise AssertionError(
                "mise is required to prove setup ordering and gate argument forwarding"
            )
        return mise

    def test_missing_mise_fails_the_forwarding_contract(self) -> None:
        with (
            mock.patch.dict(os.environ, {}, clear=True),
            mock.patch.object(shutil, "which", return_value=None),
        ):
            with self.assertRaisesRegex(AssertionError, "mise is required"):
                self._required_mise_executable()

    def test_plan_is_setup_free_and_nonexecuting(self) -> None:
        plan = self.tasks["plan"]
        self.assertNotIn("depends", plan)
        self.assertEqual(
            plan["run"],
            "python3 tools/ci/local_verify.py affected --base origin/main --plan",
        )

    @classmethod
    def _direct_gate_commands(cls, markdown: str) -> list[tuple[int, str]]:
        lines = markdown.splitlines()
        commands: list[tuple[int, str]] = []
        index = 0
        while index < len(lines):
            first_line = index + 1
            logical_line = lines[index]
            while logical_line.rstrip().endswith("\\") and index + 1 < len(lines):
                index += 1
                logical_line = f"{logical_line}\n{lines[index]}"

            inline_spans = tuple(
                (match.start("body"), match.end("body"))
                for match in re.finditer(
                    r"(?P<delimiter>`+)(?P<body>.*?)(?P=delimiter)",
                    logical_line,
                    re.DOTALL,
                )
            )
            for match in cls.DIRECT_GATE.finditer(logical_line):
                command_end = len(logical_line)
                for body_start, body_end in inline_spans:
                    if body_start <= match.start() and match.end() <= body_end:
                        command_end = body_end
                        break
                commands.append((first_line, logical_line[match.start() : command_end]))
            index += 1
        return commands

    @classmethod
    def _direct_gate_tokens(cls, command: str) -> list[str] | None:
        lexer = shlex.shlex(
            command.replace("\\\n", " "),
            posix=True,
            punctuation_chars=";&|",
        )
        lexer.whitespace_split = True
        lexer.commenters = ""
        try:
            tokens = list(lexer)
        except ValueError:
            return None
        if len(tokens) < 3:
            return None
        if tokens[0] not in ("python3", "/usr/bin/python3"):
            return None
        if tokens[1] != "tools/ci/local_verify.py":
            return None
        if tokens[2] not in ("fast", "affected", "periodic"):
            return None
        return tokens

    @classmethod
    def _only_known_value_options(cls, arguments: list[str]) -> bool:
        index = 0
        while index < len(arguments):
            argument = arguments[index]
            if "=" in argument:
                option, value = argument.split("=", maxsplit=1)
                if option not in cls.PLANNER_VALUE_OPTIONS:
                    return False
                if cls.PLANNER_VALUE.fullmatch(value) is None:
                    return False
                index += 1
                continue
            if argument not in cls.PLANNER_VALUE_OPTIONS:
                return False
            if index + 1 >= len(arguments):
                return False
            value = arguments[index + 1]
            if value.startswith("--") or value in (";", "&", "&&", "|", "||"):
                return False
            if cls.PLANNER_VALUE.fullmatch(value) is None:
                return False
            index += 2
        return True

    @classmethod
    def _is_bounded_plan_command(cls, command: str) -> bool:
        tokens = cls._direct_gate_tokens(command)
        if tokens is None:
            return False
        arguments = tokens[3:]
        if arguments.count("--plan") != 1:
            return False
        return cls._only_known_value_options(
            [argument for argument in arguments if argument != "--plan"]
        )

    @classmethod
    def _is_bounded_diagnostic_command(cls, command: str) -> bool:
        stripped = command.rstrip()
        marker = cls.SETUP_FREE_DIAGNOSTIC_MARKER
        if not stripped.endswith(marker):
            return False
        prefix = stripped[: -len(marker)]
        if not prefix or not prefix[-1].isspace():
            return False
        tokens = cls._direct_gate_tokens(prefix.rstrip())
        return tokens is not None and cls._only_known_value_options(tokens[3:])

    @classmethod
    def _direct_gate_violations(cls, markdown: str) -> list[tuple[int, str]]:
        violations = []
        for line, command in cls._direct_gate_commands(markdown):
            if cls._is_bounded_plan_command(command):
                continue
            if cls._is_bounded_diagnostic_command(command):
                continue
            violations.append((line, command))
        return violations

    @staticmethod
    def _tracked_markdown_documents() -> list[Path]:
        completed = subprocess.run(
            ["git", "ls-files", "-z", "--", "*.md"],
            cwd=REPOSITORY_ROOT,
            check=True,
            stdout=subprocess.PIPE,
        )
        return [
            REPOSITORY_ROOT / os.fsdecode(path)
            for path in completed.stdout.split(b"\0")
            if path
        ]

    def test_direct_gate_markdown_presentation_mutations_fail_closed(self) -> None:
        forbidden = (
            "python3 tools/ci/local_verify.py fast",
            "```bash\npython3 tools/ci/local_verify.py periodic\n```",
            "- `python3 tools/ci/local_verify.py affected --case example.case`",
            "$ python3 tools/ci/local_verify.py fast --case example.case",
            "Run `python3 tools/ci/local_verify.py periodic`.",
            "python3 tools/ci/local_verify.py affected \\\n  --case example.case",
            "python3 tools/ci/local_verify.py fast; echo executed",
            "python3 tools/ci/local_verify.py fast&& echo executed",
            "python3 tools/ci/local_verify.py fast|| echo executed",
        )
        for markdown in forbidden:
            with self.subTest(markdown=markdown):
                self.assertEqual(len(self._direct_gate_violations(markdown)), 1)

    def test_only_bounded_direct_gate_exceptions_pass(self) -> None:
        marker = self.SETUP_FREE_DIAGNOSTIC_MARKER
        allowed = (
            "`python3 tools/ci/local_verify.py fast --plan` derives the case id",
            "python3 tools/ci/local_verify.py affected \\\n  --base origin/main --plan",
            "python3 tools/ci/local_verify.py fast --plan --case example.case",
            "python3 tools/ci/local_verify.py affected --case=example.case "
            "--plan --cpu-slots 4",
            f"python3 tools/ci/local_verify.py periodic {marker}",
            "python3 tools/ci/local_verify.py affected --case example.case "
            f"--base=origin/main {marker}",
        )
        for markdown in allowed:
            with self.subTest(markdown=markdown):
                self.assertEqual(self._direct_gate_violations(markdown), [])

        forbidden_near_misses = (
            "`python3 tools/ci/local_verify.py fast` is not `--plan`",
            "Run python3 tools/ci/local_verify.py fast to compare with --plan",
            "python3 tools/ci/local_verify.py affected --planet",
            "python3 tools/ci/local_verify.py fast --plan --unknown x",
            "python3 tools/ci/local_verify.py fast --plan && echo executable-suffix",
            "python3 tools/ci/local_verify.py fast --plan --plan",
            "python3 tools/ci/local_verify.py fast --plan trailing-token",
            "python3 tools/ci/local_verify.py fast --plan --base=$(echo-executed)",
            f"python3 tools/ci/local_verify.py periodic {marker}-extended",
            f"`python3 tools/ci/local_verify.py periodic` uses {marker}",
            "python3 tools/ci/local_verify.py periodic --unknown x && echo "
            f"diagnostic {marker}",
            "python3 tools/ci/local_verify.py affected --base origin/main && echo "
            f"diagnostic {marker}",
            "python3 tools/ci/local_verify.py affected --base=$(echo-executed) "
            f"{marker}",
        )
        for markdown in forbidden_near_misses:
            with self.subTest(markdown=markdown):
                self.assertEqual(len(self._direct_gate_violations(markdown)), 1)

        historical_notation = (
            "python3 tools/ci/local_verify.py fast|affected cannot execute"
        )
        self.assertEqual(self._direct_gate_commands(historical_notation), [])

    def test_tracked_markdown_inventory_fails_without_git(self) -> None:
        with mock.patch.object(
            subprocess,
            "run",
            side_effect=FileNotFoundError("git executable not found"),
        ):
            with self.assertRaisesRegex(FileNotFoundError, "git executable"):
                self._tracked_markdown_documents()

    def test_authority_and_evidence_docs_reject_direct_executable_gates(self) -> None:
        authorities = {
            path: (REPOSITORY_ROOT / path).read_text(encoding="utf-8")
            for path in self.AUTHORITY_DOCS
        }
        instruction_documents = self._tracked_markdown_documents()
        relative_documents = {
            document.relative_to(REPOSITORY_ROOT) for document in instruction_documents
        }
        for expected in (
            Path("README.md"),
            Path("CONTRIBUTING.md"),
            Path("experiments/cubecl-local-action/README.md"),
            Path("studio/README.md"),
        ):
            self.assertIn(expected, relative_documents)

        for document in instruction_documents:
            contents = document.read_text(encoding="utf-8")
            for line, command in self._direct_gate_violations(contents):
                with self.subTest(
                    path=document.relative_to(REPOSITORY_ROOT),
                    line=line,
                ):
                    self.fail(
                        "executable gates in tracked Markdown must use mise or an "
                        f"exact bounded exception: {command}"
                    )
        combined = "\n".join(authorities.values())
        self.assertIn("mise run fast", combined)
        self.assertIn("mise run affected", combined)


if __name__ == "__main__":
    unittest.main()


class HostedTestProfileTests(unittest.TestCase):
    """The local gate must build test targets the way the hosted one does."""

    def _hosted_profile_blocks(self) -> list[dict[str, str]]:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        blocks: list[dict[str, str]] = []
        current: dict[str, str] = {}
        for line in workflow.splitlines():
            matched = re.match(r'\s+(CARGO_PROFILE_TEST_[A-Z_]+):\s*"(.*)"\s*$', line)
            if matched:
                current[matched.group(1)] = matched.group(2)
                continue
            if current:
                blocks.append(current)
                current = {}
        if current:
            blocks.append(current)
        return blocks

    def test_local_verify_reproduces_every_hosted_cargo_test_profile(self) -> None:
        blocks = self._hosted_profile_blocks()
        # A renamed or deleted workflow key must fail here rather than leave the
        # comparison vacuously true.
        self.assertGreater(
            len(blocks), 0, "ci.yml declares no CARGO_PROFILE_TEST_* block"
        )
        for index, block in enumerate(blocks):
            with self.subTest(block=index):
                self.assertEqual(block, HOSTED_TEST_PROFILE)
