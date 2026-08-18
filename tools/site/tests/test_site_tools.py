from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


catalog = load_module("generate_evidence_catalog", TOOLS / "generate_evidence_catalog.py")
site_check = load_module("check_site", TOOLS / "check_site.py")


def index(entries):
    return {
        "schema": "eqiora.capability-evidence-index/v3",
        "selected_capability": None,
        "success": True,
        "entries": entries,
        "errors": [],
    }


def entry(capability="spatial.solve", case="numerics.example"):
    return {
        "capability": capability,
        "case": case,
        "manifest": "verify/numerics/example/case.toml",
        "status": "verified",
        "reference_kind": "independent-analytic",
        "conformance_kits": ["linear-v1"],
        "evidence": {
            "package": "eqiora-numerics",
            "test": "example",
            "features": ["evidence-runtime"],
        },
    }


class EvidenceCatalogTests(unittest.TestCase):
    def test_render_requires_canonical_order_and_is_stable(self):
        canonical = index(
            [
                entry("a.capability", "case-a"),
                entry("a.capability", "case-z"),
            ]
        )
        first = catalog.render_catalog(canonical)
        second = catalog.render_catalog(json.loads(json.dumps(canonical)))
        self.assertEqual(first, second)
        self.assertLess(first.index("case-a"), first.index("case-z"))
        self.assertIn("verify/numerics/example/case.toml", first)

        reversed_entries = index(list(reversed(canonical["entries"])))
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(reversed_entries)

        retargeted = json.loads(json.dumps(canonical))
        retargeted["entries"][0]["case"] = "case-zz"
        retargeted["entries"][0]["manifest"] = "verify/retargeted/case.toml"
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(retargeted)

    def test_failed_or_filtered_index_is_rejected(self):
        failed = index([])
        failed["success"] = False
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(failed)
        filtered = index([])
        filtered["selected_capability"] = "one"
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(filtered)

    def test_duplicate_identity_and_escaping_manifest_are_rejected(self):
        duplicate = entry()
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(index([duplicate, duplicate]))
        escaping = entry()
        escaping["manifest"] = "../case.toml"
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(index([escaping]))

    def test_physical_environment_is_visible_and_closed(self):
        physical = entry()
        physical["evidence"]["environment"] = "physical-mpi-cuda"
        rendered = catalog.render_catalog(index([physical]))
        self.assertIn("environment: physical-mpi-cuda", rendered)

        physical["evidence"]["environment"] = "unreviewed-runner"
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(index([physical]))

    def test_cargo_library_target_is_visible_and_closed(self):
        library = entry()
        library["evidence"] = {
            "runner": "cargo-library-test",
            "package": "eqiora-api",
            "test": "parameter_study::tests::registered_evidence",
        }
        rendered = catalog.render_catalog(index([library]))
        self.assertIn(
            "cargo-library-test: eqiora-api/parameter_study::tests::registered_evidence",
            rendered,
        )

        library["evidence"]["runner"] = "unreviewed-runner"
        with self.assertRaises(catalog.CatalogError):
            catalog.render_catalog(index([library]))


class SiteCheckTests(unittest.TestCase):
    def test_local_link_checker_accepts_existing_and_rejects_escape(self):
        with tempfile.TemporaryDirectory() as temporary:
            site = Path(temporary) / "docs/site"
            site.mkdir(parents=True)
            (site / "target.md").write_text("# Target\n", encoding="utf-8")
            source = site / "index.md"
            source.write_text("[Target](target.md)\n", encoding="utf-8")
            self.assertEqual(site_check.check_markdown_links(site), [])
            source.write_text("[Private](../../secret.md)\n", encoding="utf-8")
            errors = site_check.check_markdown_links(site)
            self.assertTrue(any("escapes docs/site" in error for error in errors))

    def test_action_regex_requires_full_sha(self):
        pinned = "uses: actions/checkout@" + "a" * 40
        floating = "uses: actions/checkout@v7"
        self.assertEqual(site_check.ACTION_USE.findall(pinned)[0][1], "a" * 40)
        self.assertNotRegex(site_check.ACTION_USE.findall(floating)[0][1], r"^[0-9a-f]{40}$")


if __name__ == "__main__":
    unittest.main()
