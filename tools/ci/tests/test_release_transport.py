from __future__ import annotations

import hashlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))

from candidate_manifest import (  # noqa: E402
    ManifestError,
    file_sha256,
    load_candidate,
    verify_artifacts,
)
from testpypi_replay import release_files  # noqa: E402


def candidate_document(root: Path) -> tuple[Path, Path, dict]:
    artifacts = root / "artifacts"
    artifacts.mkdir()
    records = []
    for filename, kind, python in (
        ("eqiora-0.1.0a1.tar.gz", "sdist", None),
        (
            "eqiora-0.1.0a1-cp311-cp311-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.11",
        ),
        (
            "eqiora-0.1.0a1-cp312-cp312-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.12",
        ),
        (
            "eqiora-0.1.0a1-cp313-cp313-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.13",
        ),
        (
            "eqiora-0.1.0a1-cp314-cp314-manylinux_2_17_x86_64.whl",
            "wheel",
            "3.14",
        ),
    ):
        data = f"bytes:{filename}".encode()
        (artifacts / filename).write_bytes(data)
        record = {
            "filename": filename,
            "kind": kind,
            "size": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }
        if python is not None:
            record.update(
                {
                    "python": python,
                    "abi": f"cp{python.replace('.', '')}",
                    "platform": "manylinux_2_17_x86_64",
                }
            )
        records.append(record)
    document = {
        "format": "eqiora.python-distribution-candidate/v1",
        "project": "eqiora",
        "version": "0.1.0a1",
        "acceptance": "complete",
        "source": {
            "commit": "1" * 40,
            "expected_tag": "v0.1.0a1",
            "tags": [],
            "tree": "clean",
        },
        "build": {
            "sdist_rebuilt": True,
            "wheel_family": {
                "implementation": "CPython",
                "ordinary_gil": True,
                "versions": ["3.11", "3.12", "3.13", "3.14"],
                "platform": "manylinux_2_17_x86_64",
                "abi3": False,
            },
            "dependency_profiles": {
                "numpy_floor": {
                    "python": "3.12",
                    "requirement": "numpy==2.1.0",
                    "observed": "2.1.0",
                    "profile": "cp312:numpy-2.1.0-floor",
                },
            },
        },
        "artifacts": records,
        "checks": [
            "generated-public-api",
            "sdist-to-wheel-rebuild",
            "twine-strict",
            "cp311:installed-wheel",
            "cp311:base-and-numpy",
            "cp311:async-and-cancellation",
            "cp311:strict-base-typing",
            "cp311:public-smoke-base",
            "cp312:installed-wheel",
            "cp312:base-and-numpy",
            "cp312:async-and-cancellation",
            "cp312:strict-base-typing",
            "cp312:public-smoke-base",
            "cp312:numpy-2.1.0-floor",
            "cp313:installed-wheel",
            "cp313:base-and-numpy",
            "cp313:async-and-cancellation",
            "cp313:strict-base-typing",
            "cp313:public-smoke-base",
            "cp314:installed-wheel",
            "cp314:base-and-numpy",
            "cp314:async-and-cancellation",
            "cp314:strict-base-typing",
            "cp314:public-smoke-base",
            "cp313:torch",
            "cp313:jax",
            "cp313:public-smoke-torch",
            "cp313:public-smoke-jax",
            "cp313:complete-public-typing",
        ],
    }
    manifest = root / "candidate.json"
    manifest.write_text(json.dumps(document), encoding="utf-8")
    return manifest, artifacts, document


class CandidateManifestTests(unittest.TestCase):
    def test_exact_artifact_set_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, artifacts, document = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            verify_artifacts(candidate, artifacts)
            self.assertEqual(candidate.version, "0.1.0a1")
            self.assertEqual(candidate.commit, "1" * 40)
            self.assertEqual(len(candidate.artifacts), 5)
            self.assertEqual(file_sha256(manifest), hashlib.sha256(manifest.read_bytes()).hexdigest())

            payload = {
                "info": {"name": "eqiora", "version": "0.1.0a1"},
                "urls": [
                    {
                        "filename": record["filename"],
                        "url": f"https://test-files.pythonhosted.org/{record['filename']}",
                        "size": record["size"],
                        "digests": {"sha256": record["sha256"]},
                    }
                    for record in document["artifacts"]
                ],
            }
            self.assertEqual(
                set(release_files(payload, candidate)),
                {record["filename"] for record in document["artifacts"]},
            )

    def test_substitution_and_extra_file_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, artifacts, _ = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            first = artifacts / candidate.artifacts[0].filename
            first.write_bytes(b"substituted")
            with self.assertRaisesRegex(ManifestError, "size differs"):
                verify_artifacts(candidate, artifacts)

        with tempfile.TemporaryDirectory() as temporary:
            manifest, artifacts, _ = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            (artifacts / "unreviewed.whl").write_bytes(b"x")
            with self.assertRaisesRegex(ManifestError, "unexpected"):
                verify_artifacts(candidate, artifacts)

    def test_numpy_floor_profile_drift_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, _, document = candidate_document(Path(temporary))
            document["build"]["dependency_profiles"]["numpy_floor"][
                "observed"
            ] = "2.2.0"
            manifest.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(ManifestError, "NumPy floor profile drifted"):
                load_candidate(manifest)

    def test_testpypi_metadata_hash_and_host_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest, _, document = candidate_document(Path(temporary))
            candidate = load_candidate(manifest)
            payload = {
                "info": {"name": "eqiora", "version": "0.1.0a1"},
                "urls": [
                    {
                        "filename": record["filename"],
                        "url": f"https://test-files.pythonhosted.org/{record['filename']}",
                        "size": record["size"],
                        "digests": {"sha256": record["sha256"]},
                    }
                    for record in document["artifacts"]
                ],
            }
            payload["urls"][0]["digests"]["sha256"] = "0" * 64
            with self.assertRaisesRegex(ManifestError, "metadata hash"):
                release_files(payload, candidate)
            payload["urls"][0]["digests"]["sha256"] = document["artifacts"][0][
                "sha256"
            ]
            payload["urls"][0]["url"] = "https://example.invalid/file"
            with self.assertRaisesRegex(ManifestError, "unexpected host"):
                release_files(payload, candidate)


if __name__ == "__main__":
    unittest.main()
