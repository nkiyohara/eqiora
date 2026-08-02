from __future__ import annotations

import os
import re
import sys
import tomllib
import unittest
from pathlib import Path
from unittest import mock


CI_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = CI_ROOT.parents[1]
sys.path.insert(0, str(CI_ROOT))

from check_gate import JOB_SURFACES, evaluate, parse_relevance, parse_results  # noqa: E402
from classify_changes import SURFACES, changed_paths, classify, render_outputs  # noqa: E402
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
        self.assertNotIn("schedule:", trigger)
        self.assertNotIn("push:", trigger)
        self.assertIn("github.event.pull_request.head.sha || inputs.commit", workflow)
        self.assertIn("persist-credentials: false", workflow)

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

        aggregate = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        ).split("\n  gate:\n", maxsplit=1)[1]
        self.assertNotIn("windows", aggregate.lower())

    def test_base_owned_trust_workflow_never_checks_out_head_code(self) -> None:
        workflow = (
            REPOSITORY_ROOT / ".github/workflows/ci-definition-trust.yml"
        ).read_text(encoding="utf-8")
        self.assertIn("pull_request_target:", workflow)
        self.assertIn("github.event.pull_request.base.sha", workflow)
        self.assertNotIn("github.event.pull_request.head.sha", workflow)
        self.assertIn("pull-requests: read", workflow)
        self.assertNotIn("contents: write", workflow)
        self.assertNotIn("id-token: write", workflow)

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
        self.assertIn('["uv"]', python_evidence)
        self.assertIn("python -m pip install --only-binary=:all:", python_evidence)
        self.assertIn("uv --version", python_evidence)
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

    def test_studio_checks_its_independent_manifest_at_the_same_msrv(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        studio = workflow.split("  studio:\n", maxsplit=1)[1].split(
            "\n  gate:", maxsplit=1
        )[0]
        formatting = (
            "cargo +stable fmt --manifest-path "
            "studio/src-tauri/Cargo.toml -- --check"
        )
        self.assertEqual(workflow.count(formatting), 1)
        self.assertIn("rustup toolchain install 1.89.0", studio)
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

    def test_uv_rebuilds_the_current_noneditable_project(self) -> None:
        command = uv_gate_command("uv", "/usr/bin/python3")

        self.assertIn("--no-editable", command)
        index = command.index("--reinstall-package")
        self.assertEqual(command[index + 1], "eqiora")
        self.assertEqual(command[command.index("--python") + 1], "/usr/bin/python3")

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

        self.assertEqual(command[command.index("--extra") + 1], "matplotlib")
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
        self.assertEqual(
            classify(
                [
                    "docs/architecture.md",
                    "README.md",
                    "bindings/python/README.md",
                    "verify/numerics/linear-backends/README.md",
                ]
            ),
            {surface: False for surface in SURFACES},
        )

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
                    classify([path]), {surface: True for surface in SURFACES}
                )

        python_requirements = classify(["bindings/python/requirements.txt"])
        self.assertTrue(python_requirements["python"])

    def test_rename_classifies_source_and_destination(self) -> None:
        completed = mock.Mock(
            stdout=b"crates/eqiora/src/old.rs\0docs/architecture/old.md\0"
        )
        with mock.patch(
            "classify_changes.subprocess.run", return_value=completed
        ) as run:
            paths = changed_paths("base", "head")

        self.assertEqual(
            run.call_args.args[0],
            ["git", "diff", "--no-renames", "--name-only", "-z", "base...head"],
        )
        selected = classify(paths)
        self.assertTrue(selected["rust"])
        self.assertTrue(selected["msrv"])

    def test_full_run_selects_compatibility_matrix(self) -> None:
        selected = classify([], full=True)
        rendered = render_outputs("a" * 40, selected, full=True)
        self.assertIn('python_versions=["3.11","3.12","3.13","3.14"]', rendered)

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


if __name__ == "__main__":
    unittest.main()


class HostedTestProfileTests(unittest.TestCase):
    """The local gate must build test targets the way the hosted one does."""

    def _hosted_profile_blocks(self) -> list[dict[str, str]]:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
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
        self.assertGreater(len(blocks), 0, "ci.yml declares no CARGO_PROFILE_TEST_* block")
        for index, block in enumerate(blocks):
            with self.subTest(block=index):
                self.assertEqual(block, HOSTED_TEST_PROFILE)
