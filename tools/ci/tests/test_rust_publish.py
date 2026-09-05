from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))
import rust_publish as release


def package(name: str, *dependencies: str, publish=None) -> dict:
    return {"name": name, "version": "0.1.0-alpha.1", "publish": publish,
            "dependencies": [{"name": dep, "path": f"/source/{dep}"} for dep in dependencies]}


class RustPublicationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.package = package("eqiora")
        self.archive = self.root / release.archive_name(self.package)
        self.archive.write_bytes(b"accepted archive")

    def test_closure_orders_optional_local_dependencies_and_excludes_workspace_tools(self) -> None:
        facade = package("eqiora", "api", "gpu")
        facade["dependencies"][1]["optional"] = True
        facade["dependencies"].append({"name": "serde", "path": None})
        metadata = {"packages": [facade, package("gpu", "core"), package("core"),
                                 package("api", "core"), package("unrelated", publish=[])]}
        self.assertEqual([p["name"] for p in release.publication_order(metadata)],
                         ["core", "api", "gpu", "eqiora"])

    def test_unpublishable_dependency_and_cycle_fail_closed(self) -> None:
        for dependency in (package("core", publish=[]), package("core", "eqiora")):
            with self.subTest(dependency=dependency), self.assertRaises(release.PublicationError):
                release.publication_order({"packages": [package("eqiora", "core"), dependency]})

    def test_source_rejects_wrong_commit_and_dirty_tree(self) -> None:
        with mock.patch.object(release.subprocess, "check_output", return_value="b" * 40):
            with self.assertRaisesRegex(release.PublicationError, "differs"):
                release.require_source(self.root, "a" * 40)
        with mock.patch.object(release.subprocess, "check_output", side_effect=["a" * 40, " M source"]):
            with self.assertRaisesRegex(release.PublicationError, "clean"):
                release.require_source(self.root, "a" * 40)
        with mock.patch.object(release.subprocess, "check_output") as query:
            with self.assertRaisesRegex(release.PublicationError, "full lowercase"):
                release.require_source(self.root, "main")
            query.assert_not_called()

    def test_archive_source_rejects_stale_and_dirty_artifacts(self) -> None:
        for commit, dirty, accepted in (("a" * 40, False, True), ("b" * 40, False, False),
                                        ("a" * 40, True, False)):
            data = json.dumps({"git": {"sha1": commit, "dirty": dirty}}).encode()
            with tarfile.open(self.archive, "w:gz") as archive:
                member = tarfile.TarInfo("eqiora-0.1.0-alpha.1/.cargo_vcs_info.json")
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
            if accepted:
                release.require_archive_source([self.package], self.root, "a" * 40)
            else:
                with self.assertRaisesRegex(release.PublicationError, "clean release commit"):
                    release.require_archive_source([self.package], self.root, "a" * 40)

    def test_inventory_and_archive_mismatch_fail_before_publication(self) -> None:
        actual = self.root / "actual"
        actual.mkdir()
        (actual / self.archive.name).write_bytes(self.archive.read_bytes())
        expected = self.root / "expected"
        expected.mkdir()
        (expected / self.archive.name).write_bytes(self.archive.read_bytes())
        release.compare_archives([self.package], expected, actual)
        (actual / self.archive.name).write_bytes(b"different archive")
        with self.assertRaisesRegex(release.PublicationError, "differs"):
            release.compare_archives([self.package], expected, actual)
        (expected / "unexpected.crate").touch()
        with self.assertRaisesRegex(release.PublicationError, "inventory"):
            release.compare_archives([self.package], expected, actual)

    def test_existing_versions_require_identical_unyanked_bytes(self) -> None:
        good = {"checksum": hashlib.sha256(b"accepted archive").hexdigest(), "yanked": False}
        with mock.patch.object(release, "registry_version", return_value=good):
            self.assertEqual(release.pending_publications([self.package], self.root), [])
        for bad in ({**good, "yanked": True}, {**good, "checksum": "0" * 64}):
            with mock.patch.object(release, "registry_version", return_value=bad):
                with self.assertRaisesRegex(release.PublicationError, "differs"):
                    release.pending_publications([self.package], self.root)

    def test_all_existing_versions_are_checked_before_any_upload(self) -> None:
        pending = package("core")
        with mock.patch.object(release, "registry_version", side_effect=[None, {"checksum": "wrong"}]), \
             mock.patch.object(release.subprocess, "run") as upload:
            with self.assertRaises(release.PublicationError):
                release.publish([pending, self.package], self.root, self.root, ["cargo"], {})
            upload.assert_not_called()

    def test_failed_upload_is_not_retried_without_explicit_rate_limit(self) -> None:
        failure = subprocess.CompletedProcess([], 1, "authentication failed")
        with mock.patch.object(release, "registry_version", return_value=None), \
             mock.patch.object(release.subprocess, "run", return_value=failure) as upload:
            with self.assertRaisesRegex(release.PublicationError, "cargo publish failed"):
                release.publish([self.package], self.root, self.root, ["cargo"], {})
            self.assertEqual(upload.call_count, 1)

    def test_rate_limit_uses_server_date_and_upload_checks_final_checksum(self) -> None:
        rate_limit = "status 429: Please try again after Sat, 05 Sep 2026 01:56:52 GMT and see https://crates.io/docs/rate-limits"
        self.assertGreater(release.retry_at(rate_limit), 0)
        self.assertIsNone(release.retry_at("status 403"))
        good = {"checksum": release.checksum(self.archive)}
        responses = [subprocess.CompletedProcess([], 1, rate_limit), subprocess.CompletedProcess([], 0, "published")]
        with mock.patch.object(release, "registry_version", side_effect=[None, None, good]), \
             mock.patch.object(release.subprocess, "run", side_effect=responses) as upload, \
             mock.patch.object(release.time, "time", return_value=release.retry_at(rate_limit) + 1):
            release.publish([self.package], self.root, self.root, ["cargo"], {})
            self.assertEqual(upload.call_count, 2)
            self.assertIn("--no-verify", upload.call_args.args[0])
            self.assertIn("--locked", upload.call_args.args[0])

class RustPublicationWorkflowTests(unittest.TestCase):
    def test_candidate_authentication_builds_and_secret_are_separated(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/python-production-publish.yml").read_text()
        prepare = workflow.split("  prepare_rust:\n", 1)[1].split("  publish_rust:\n", 1)[0]
        publish = workflow.split("  publish_rust:\n", 1)[1]
        self.assertIn("    needs: verify\n", prepare)
        self.assertIn("    needs: prepare_rust\n", publish)
        self.assertIn("      name: crates-io\n", publish)
        self.assertNotIn("secrets.", prepare)
        self.assertNotIn("id-token: write", prepare + publish)
        self.assertIn("rust_publish.py prepare", prepare)
        self.assertIn("actions/upload-artifact@", prepare)
        self.assertIn("actions/download-artifact@", publish)
        for section in (prepare, publish):
            self.assertIn("ref: ${{ inputs.commit }}", section)
            self.assertIn("persist-credentials: false", section)
            self.assertIn("locked_toolchain", section)
            self.assertIn('name: eqiora-rust-crates', section)
        steps = publish.split("      - name:")
        secret_steps = [step for step in steps if "secrets.CARGO_REGISTRY_TOKEN" in step]
        self.assertEqual(len(secret_steps), 1)
        self.assertIn("rust_publish.py publish", secret_steps[0])
        self.assertNotIn("rust_publish.py validate", secret_steps[0])
        self.assertLess(publish.index("rust_publish.py validate"), publish.index("secrets.CARGO_REGISTRY_TOKEN"))


if __name__ == "__main__":
    unittest.main()
