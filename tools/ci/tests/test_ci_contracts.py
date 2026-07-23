from __future__ import annotations

import os
import sys
import tomllib
import unittest
from pathlib import Path
from unittest import mock


CI_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = CI_ROOT.parents[1]
sys.path.insert(0, str(CI_ROOT))

from check_gate import evaluate, parse_relevance  # noqa: E402
from classify_changes import SURFACES, changed_paths, classify, render_outputs  # noqa: E402
from python_jax_gate import uv_gate_command as jax_uv_gate_command  # noqa: E402
from python_package_gate import (  # noqa: E402
    uv_gate_command,
    venv_environment,
    venv_python,
)
from python_torch_gate import uv_gate_command as torch_uv_gate_command  # noqa: E402


class HostedTriggerTests(unittest.TestCase):
    def test_public_workflow_runs_for_pull_requests_and_exact_sha_dispatch(self) -> None:
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

    def test_studio_checks_its_independent_manifest_at_the_same_msrv(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        studio = workflow.split("  studio:\n", maxsplit=1)[1].split(
            "\n  gate:", maxsplit=1
        )[0]
        self.assertIn("rustup toolchain install 1.89.0", studio)
        self.assertIn(
            "cargo +1.89.0 check --manifest-path studio/src-tauri/Cargo.toml --locked --all-targets",
            studio,
        )

    def test_dependency_policy_checks_both_independent_cargo_workspaces(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        dependency = workflow.split(
            "  dependency_policy:\n", maxsplit=1
        )[1].split("\n  cubecl_experiment:", maxsplit=1)[0]
        action = (
            "EmbarkStudios/cargo-deny-action@"
            "3c6349835b2b7b196a839186cb8b78e02f7b5f25"
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
            "arguments: --all-features --locked --config "
            "studio/src-tauri/deny.toml",
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
            "crates/eqiora-fabric",
            "crates/eqiora-fabric/src/lib.rs",
            "RAYON_VERSION",
        )


class PythonPackageGateTests(unittest.TestCase):
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
                self.assertEqual(classify([path]), {surface: True for surface in SURFACES})

        python_requirements = classify(["bindings/python/requirements.txt"])
        self.assertTrue(python_requirements["python"])

    def test_rename_classifies_source_and_destination(self) -> None:
        completed = mock.Mock(
            stdout=b"crates/eqiora/src/old.rs\0docs/architecture/old.md\0"
        )
        with mock.patch("classify_changes.subprocess.run", return_value=completed) as run:
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


class AggregateGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.relevance = {surface: False for surface in SURFACES}
        self.results = {
            "changes": "success",
            "documentation": "success",
            "quality": "skipped",
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


if __name__ == "__main__":
    unittest.main()
