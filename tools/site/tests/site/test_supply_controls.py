from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from fixture import checker, make_fixture


class DependencySupplyTests(unittest.TestCase):
    @staticmethod
    def _documents(root: Path) -> tuple[Path, dict, Path, dict]:
        package_path = root / "docs/site/package.json"
        lock_path = root / "docs/site/package-lock.json"
        package = json.loads(package_path.read_text(encoding="utf-8"))
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
        return package_path, package, lock_path, lock

    @staticmethod
    def _write(path: Path, document: dict) -> None:
        path.write_text(json.dumps(document), encoding="utf-8")

    def test_00_exact_dependency_supply_passes_before_mutants(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            self.assertEqual(checker.check_source(root, identities), [])

    def test_peer_dependency_and_realized_install_mutants_fail(self) -> None:
        for label in (
            "root peer dependency",
            "realized nested package",
            "transitive peer edge",
        ):
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                _, identities = make_fixture(root)
                package_path, package, lock_path, lock = self._documents(root)
                if label == "root peer dependency":
                    package["peerDependencies"] = {"react": "19.2.4"}
                    lock["packages"][""]["peerDependencies"] = {"react": "19.2.4"}
                    lock["packages"]["node_modules/react"] = {
                        "version": "19.2.4",
                        "integrity": "sha512-fixture",
                    }
                elif label == "realized nested package":
                    lock["packages"]["node_modules/example/node_modules/react"] = {
                        "version": "19.2.4",
                        "integrity": "sha512-fixture",
                    }
                else:
                    lock["packages"]["node_modules/example"] = {
                        "version": "1.0.0",
                        "integrity": "sha512-fixture",
                        "peerDependencies": {"react": "19.2.4"},
                    }
                self._write(package_path, package)
                self._write(lock_path, lock)
                errors = checker.check_source(root, identities)
                self.assertTrue(
                    any(
                        "exact direct set" in error
                        or "client framework" in error
                        or "forbidden" in error
                        for error in errors
                    ),
                    errors,
                )


if __name__ == "__main__":
    unittest.main()
