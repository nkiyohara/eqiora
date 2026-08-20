from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPOSITORY = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY / "tools/release"))
import python_candidate_h2 as h2  # noqa: E402

VERSION = b"v24.18.1\n"
URL = "https://registry.npmjs.org/npm/-/npm-11.16.0.tgz"
REJECTIONS = (h2.CandidateError, OSError, subprocess.SubprocessError)
MODES = ("extra-before", "extra-after", "version-on-stderr", "stderr", "wrong-version", "nonzero", "no-newline")  # fmt: skip
ENV_MUTATIONS = ("keep-force", "drop-no-color", "blank-no-color", "rewrite-no-color", "drop-sentinel", "rewrite-sentinel")  # fmt: skip
EXPECTED_CHILD = {"argv": ["--version"], "FORCE_COLOR": None, "sentinel": "unchanged"}  # fmt: skip
CUSTOM_COLORS = ("custom-no-color-雪", "custom-force-color-ß")
NODE = r"""#!/usr/bin/env python3
import json, os, sys
from pathlib import Path

record = {"argv0": sys.argv[0], "argv": sys.argv[1:], "NO_COLOR": os.environ.get("NO_COLOR"),
          "FORCE_COLOR": os.environ.get("FORCE_COLOR"),
          "sentinel": os.environ.get("H2_IDENTITY_SENTINEL")}
with Path(os.environ["H2_IDENTITY_OBSERVATION"]).open("a", encoding="utf-8") as out:
    out.write(json.dumps(record, sort_keys=True) + "\n")
expected = {"argv0": os.environ["H2_IDENTITY_EXPECTED_NODE"],
            "argv": ["--version"], "NO_COLOR": os.environ["H2_IDENTITY_EXPECTED_NO_COLOR"], "FORCE_COLOR": None,
            "sentinel": "unchanged"}
if record != expected:
    sys.stderr.write("identity environment differs\n"); raise SystemExit(11)
mode = os.environ.get("H2_IDENTITY_MODE", "exact")
stdout, stderr, status = {
    "exact": ("v24.18.1\n", "", 0),
    "extra-before": ("extra\nv24.18.1\n", "", 0),
    "extra-after": ("v24.18.1\nextra\n", "", 0),
    "version-on-stderr": ("v0.0.0\n", "v24.18.1\n", 0),
    "stderr": ("v24.18.1\n", "diagnostic\n", 0),
    "wrong-version": ("v0.0.0\n", "", 0),
    "nonzero": ("v24.18.1\n", "", 7),
    "no-newline": ("v24.18.1", "", 0),
}[mode]
sys.stdout.write(stdout); sys.stderr.write(stderr); raise SystemExit(status)
"""


NetworkBoundary = type("NetworkBoundary", (RuntimeError,), {})


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reference_identity(
    node: Path,
    expected_digest: str,
    download: mock.Mock,
    *,
    child_mutation: str = "none",
    parent_mutation: bool = False,
) -> None:
    environment = os.environ.copy()
    environment.pop("FORCE_COLOR", None)
    key, value = {
        "none": (None, None),
        "keep-force": ("FORCE_COLOR", "1"),
        "drop-no-color": ("NO_COLOR", None),
        "blank-no-color": ("NO_COLOR", ""),
        "rewrite-no-color": ("NO_COLOR", "0"),
        "drop-sentinel": ("H2_IDENTITY_SENTINEL", None),
        "rewrite-sentinel": ("H2_IDENTITY_SENTINEL", "changed"),
    }[child_mutation]
    if key is not None:
        environment.pop(key, None) if value is None else environment.__setitem__(
            key, value
        )
    try:
        completed = subprocess.run(
            [str(node.resolve()), "--version"],
            env=environment,
            check=False,
            capture_output=True,
        )
    except OSError as error:
        raise h2.CandidateError("Node invocation failed") from error
    if completed.returncode or completed.stdout != VERSION or completed.stderr:
        raise h2.CandidateError("Node version observation differs")
    if digest(node.resolve()) != expected_digest:
        raise h2.CandidateError("Node executable identity differs")
    if parent_mutation:
        os.environ["H2_IDENTITY_SENTINEL"] = "changed"
    download(URL, node.parent / "npm-11.16.0.tgz")


class NodeIdentityContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=Path.home() / ".cache/eqiora")
        self.root = Path(self.temporary.name)
        self.node = self.root / "node"
        self.node.write_text(NODE, encoding="utf-8")
        self.node.chmod(0o755)
        self.selected = self.root / "selected-node"
        self.selected.symlink_to(self.node)
        self.decoy = self.root / "decoy"
        self.decoy.write_text(NODE + "# distinct bytes\n", encoding="utf-8")
        self.decoy.chmod(0o755)
        self.observation = self.root / "observation.jsonl"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def environment(self, no_color: str = "1", force_color: str | None = "1") -> mock._patch_dict:  # fmt: skip
        values = {
            "PATH": str(Path(sys.executable).resolve().parent),
            "NO_COLOR": no_color,
            "H2_IDENTITY_SENTINEL": "unchanged",
            "H2_IDENTITY_OBSERVATION": str(self.observation),
            "H2_IDENTITY_EXPECTED_NODE": str(self.node.resolve()),
            "H2_IDENTITY_EXPECTED_NO_COLOR": no_color,
        }
        if force_color is not None:
            values["FORCE_COLOR"] = force_color
        return mock.patch.dict(os.environ, values, clear=True)

    def run_product(self, node: Path | None, expected_digest: str, network: mock.Mock) -> None:  # fmt: skip
        selected = None if node is None else str(node)
        with (
            mock.patch.object(h2.shutil, "which", return_value=selected) as which,
            mock.patch.object(h2, "NODE_SHA256", expected_digest),
            mock.patch.object(h2, "_download", network),
        ):
            try:
                h2._node_and_npm_identity(h2.H2Workspace(*([self.root] * 8)))
            finally:
                self.assertEqual(which.call_args_list, [mock.call("node")])

    def assert_exact_child(self, no_color: str = "1") -> None:
        records = [
            json.loads(line) for line in self.observation.read_text().splitlines()
        ]
        expected = {**EXPECTED_CHILD, "argv0": str(self.node.resolve()), "NO_COLOR": no_color}  # fmt: skip
        self.assertEqual(records, [expected])

    def assert_parent_and_sibling(self, no_color: str = "1", force_color: str = "1") -> None:  # fmt: skip
        keys = ["NO_COLOR", "FORCE_COLOR", "H2_IDENTITY_SENTINEL"]
        expected = [no_color, force_color, "unchanged"]
        self.assertEqual([os.environ.get(key) for key in keys], expected)
        code = "import json,os; print(json.dumps([os.environ.get(k) for k in "
        child = subprocess.run([sys.executable, "-c", code + repr(keys) + "]))"], check=True, text=True, capture_output=True)  # fmt: skip
        self.assertEqual(json.loads(child.stdout), expected)

    def assert_positive(self, product: bool, no_color: str = "1", force_color: str = "1") -> None:  # fmt: skip
        self.observation.unlink(missing_ok=True)
        network = mock.Mock(side_effect=NetworkBoundary)
        with self.environment(no_color, force_color):
            with self.assertRaises(NetworkBoundary):
                if product:
                    self.run_product(self.selected, digest(self.node), network)
                else:
                    reference_identity(self.selected, digest(self.node), network)
            self.assert_exact_child(no_color)
            self.assert_parent_and_sibling(no_color, force_color)
        network.assert_called_once_with(URL, self.root / "npm-11.16.0.tgz")

    def assert_product_rejected(self, node: Path | None, expected: str, mode: str = "exact") -> None:  # fmt: skip
        self.observation.unlink(missing_ok=True)
        network = mock.Mock(side_effect=NetworkBoundary)
        with self.environment(force_color=None):
            os.environ["H2_IDENTITY_MODE"] = mode
            with self.assertRaises(REJECTIONS):
                self.run_product(node, expected, network)
        if node is not None and node.exists():
            self.assert_exact_child()
        network.assert_not_called()

    def test_00_reference_conflicting_parent_positive_reaches_network(self) -> None:
        self.assert_positive(False)
        self.assert_positive(False, *CUSTOM_COLORS)

    def test_01_product_conflicting_parent_positive_reaches_network(self) -> None:
        self.assert_positive(True)
        self.assert_positive(True, *CUSTOM_COLORS)
        self.assert_product_rejected(self.selected, digest(self.node), "no-newline")

    def test_02_reference_rejects_environment_and_parent_mutants(self) -> None:
        for mutation in ENV_MUTATIONS:
            self.observation.unlink(missing_ok=True)
            network = mock.Mock(side_effect=NetworkBoundary)
            with self.subTest(mutation=mutation), self.environment():
                with self.assertRaises(h2.CandidateError):
                    reference_identity(self.selected, digest(self.node), network, child_mutation=mutation)  # fmt: skip
            network.assert_not_called()
        network = mock.Mock(side_effect=NetworkBoundary)
        with self.environment():
            with self.assertRaises(NetworkBoundary):
                reference_identity(
                    self.selected, digest(self.node), network, parent_mutation=True
                )
            with self.assertRaises(AssertionError):
                self.assert_parent_and_sibling()
        network.assert_called_once_with(URL, self.root / "npm-11.16.0.tgz")

    def test_03_reference_rejects_identity_mutants_before_network(self) -> None:
        for mode in MODES:
            self.observation.unlink(missing_ok=True)
            network = mock.Mock(side_effect=NetworkBoundary)
            with self.subTest(mode=mode), self.environment(force_color=None):
                os.environ["H2_IDENTITY_MODE"] = mode
                with self.assertRaises(h2.CandidateError):
                    reference_identity(self.selected, digest(self.node), network)
            network.assert_not_called()
        for node, expected in ((self.selected, digest(self.decoy)), (self.root / "missing", "0" * 64)):  # fmt: skip
            network = mock.Mock(side_effect=NetworkBoundary)
            with self.environment(force_color=None), self.assertRaises(h2.CandidateError):  # fmt: skip
                reference_identity(node, expected, network)
            network.assert_not_called()

    def test_04_product_rejects_identity_mutants_before_network(self) -> None:
        cases = (*((mode, digest(self.node)) for mode in MODES[:-1]), ("exact", digest(self.decoy)))  # fmt: skip
        for mode, expected in cases:
            with self.subTest(mode=mode):
                self.assert_product_rejected(self.selected, expected, mode)
        for node in (self.root / "missing", None):
            with self.subTest(node=node):
                self.assert_product_rejected(node, "0" * 64)


if __name__ == "__main__":
    unittest.main()
