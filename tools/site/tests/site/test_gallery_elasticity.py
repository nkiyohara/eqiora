from __future__ import annotations

import ast
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[4]
CHECKER_PATH = ROOT / "tools/site/check_gallery_publication.py"
SPEC = importlib.util.spec_from_file_location("elasticity_gallery_checker", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)

RECORD = ROOT / checker.ELASTICITY_RECORD
MEDIA = ROOT / checker.ELASTICITY_MEDIA
MARIMO = ROOT / "examples/python/mixed_boundary_elasticity_marimo.py"
JUPYTER = ROOT / "examples/python/mixed_boundary_elasticity_jupyter.ipynb"
SHARED = ROOT / "examples/python/mixed_boundary_elasticity.py"


def canonical_file(value: object) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        + b"\n"
    )


def calls(source: str, name: str) -> int:
    tree = ast.parse(source)
    return sum(
        isinstance(node, ast.Call)
        and (
            (isinstance(node.func, ast.Name) and node.func.id == name)
            or (isinstance(node.func, ast.Attribute) and node.func.attr == name)
        )
        for node in ast.walk(tree)
    )


class MixedBoundaryElasticityGalleryTests(unittest.TestCase):
    def _check(self, record: Path = RECORD):
        return checker.check_elasticity_publication(
            repository_root=ROOT,
            record_path=record,
            media_path=MEDIA,
        )

    def _mutated_record(self, mutate) -> tuple[tempfile.TemporaryDirectory, Path]:
        temporary = tempfile.TemporaryDirectory(dir=Path.home())
        value = json.loads(RECORD.read_text(encoding="utf-8"))
        mutate(value)
        value["publication_payload_sha256"] = checker._sha(
            checker._canonical_value(value["publication_payload"])
        )
        path = Path(temporary.name) / "record.json"
        path.write_bytes(canonical_file(value))
        return temporary, path

    def test_checked_in_concrete_second_profile_is_admitted(self) -> None:
        result = self._check()
        self.assertEqual(result["entry_id"], "mixed-boundary-elasticity")
        self.assertEqual(result["status"], "accepted")
        self.assertEqual(result["mode"], "verify-installed")

    def test_git_absent_archive_uses_authenticated_object_repository(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            archive = Path(temporary) / "source"
            record = archive / checker.ELASTICITY_RECORD
            media = archive / checker.ELASTICITY_MEDIA
            record.parent.mkdir(parents=True)
            media.parent.mkdir(parents=True)
            shutil.copyfile(RECORD, record)
            shutil.copyfile(MEDIA, media)
            self.assertFalse((archive / ".git").exists())
            authority = Path(
                os.environ.get(checker.GIT_OBJECT_REPOSITORY_VARIABLE, ROOT)
            ).resolve(strict=True)
            head = (
                checker._git(authority, "rev-parse", "--verify", "HEAD^{commit}")
                .decode("ascii")
                .strip()
            )
            environment = {
                checker.GIT_OBJECT_REPOSITORY_VARIABLE: str(authority),
                checker.SOURCE_SHA_VARIABLE: head,
            }
            with mock.patch.dict(os.environ, environment, clear=False):
                result = checker.check_elasticity_publication(
                    repository_root=archive,
                    record_path=record,
                    media_path=media,
                )
            self.assertEqual(result["status"], "accepted")

            environment[checker.SOURCE_SHA_VARIABLE] = "0" * 40
            with mock.patch.dict(os.environ, environment, clear=False):
                with self.assertRaises(checker.AdmissionError) as raised:
                    checker.check_elasticity_publication(
                        repository_root=archive,
                        record_path=record,
                        media_path=media,
                    )
            self.assertEqual(raised.exception.code, "source-git")

    def test_claim_widening_and_foreign_lineage_are_rejected(self) -> None:
        for label, mutate, code in (
            (
                "claim",
                lambda value: value["publication_payload"]["claim"].update(
                    public_claim="general linear elasticity"
                ),
                "claim",
            ),
            (
                "lineage",
                lambda value: value["publication_payload"]["lineage"]["identities"].update(
                    result_plan_key="0" * 64
                ),
                "lineage",
            ),
        ):
            with self.subTest(label=label):
                temporary, path = self._mutated_record(mutate)
                self.addCleanup(temporary.cleanup)
                with self.assertRaises(checker.AdmissionError) as raised:
                    self._check(path)
                self.assertEqual(raised.exception.code, code)

    def test_profile_stays_concrete(self) -> None:
        source = CHECKER_PATH.read_text(encoding="utf-8")
        self.assertNotIn("GALLERY_PROFILES", source)
        record = json.loads(RECORD.read_text(encoding="utf-8"))
        self.assertEqual(
            set(record["publication_payload"]["lineage"]),
            {"field", "identities", "methods"},
        )

    def test_marimo_and_jupyter_share_one_workflow_without_reauthoring_it(self) -> None:
        marimo = MARIMO.read_text(encoding="utf-8")
        notebook = json.loads(JUPYTER.read_text(encoding="utf-8"))
        self.assertEqual(notebook["nbformat"], 4)
        self.assertTrue(all(cell.get("outputs", []) == [] for cell in notebook["cells"]))
        jupyter = "\n".join(
            "".join(cell["source"])
            for cell in notebook["cells"]
            if cell["cell_type"] == "code"
        )
        for source in (marimo, jupyter):
            self.assertIn("from mixed_boundary_elasticity import solve", source)
            self.assertEqual(calls(source, "solve"), 1)
            self.assertEqual(calls(source, "plot_deformed_field"), 1)
            for duplicated_owner in (
                "GeometryGraph",
                "CartesianMesher",
                "eqiora.compile",
                "eqiora.resolve",
                "eqiora.run",
            ):
                self.assertNotIn(duplicated_owner, source)

    def test_shared_workflow_owns_one_direct_common_run(self) -> None:
        source = SHARED.read_text(encoding="utf-8")
        self.assertEqual(source.count("eqiora.run(plan)"), 1)
        self.assertEqual(source.count("def solve()"), 1)
        self.assertEqual(source.count("plot_deformed_field("), 1)


if __name__ == "__main__":
    unittest.main()
