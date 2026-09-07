"""Mutable Reference pages use only the asserted documentation commit."""

from pathlib import Path
from types import SimpleNamespace
import unittest

from tools.site.check_site_references import _check_html


CURRENT_SHA = "a" * 40
PREFIX = "https://github.com/nkiyohara/eqiora"


class CurrentSourceLinkTests(unittest.TestCase):
    def errors(self, url, *, page="reference/language/index.html", tag="a", attribute="href"):
        artifact = Path("/artifact")
        path = artifact / page
        return _check_html(
            artifact, {path: ("", SimpleNamespace(visible_text="Reference"))},
            [(path, tag, attribute, url, True)], CURRENT_SHA,
        )

    def test_exact_current_commit_accepts_blob_and_tree(self):
        for kind in ("blob", "tree"):
            self.assertEqual(self.errors(f"{PREFIX}/{kind}/{CURRENT_SHA}/crates/eqiora-lang/src"), [])

    def test_no_page_gets_an_old_release_or_moving_revision_exception(self):
        for page in ("index.html", "reference/language/index.html",
                     "reference/standard-packages/index.html", "reference/python/index.html"):
            for revision in ("main", "v0.1.0a7", "b" * 40, CURRENT_SHA[:-1], CURRENT_SHA + "0"):
                with self.subTest(page=page, revision=revision):
                    self.assertTrue(self.errors(f"{PREFIX}/blob/{revision}/README.md", page=page))

    def test_source_links_never_allow_external_runtime_or_wrong_origin(self):
        url = f"{PREFIX}/blob/{CURRENT_SHA}/README.md"
        for tag, attribute in (("script", "src"), ("img", "src"), ("link", "href")):
            self.assertTrue(any("external runtime" in error for error in self.errors(url, tag=tag, attribute=attribute)))
        for changed in (url.replace("https:", "http:"), url.replace("github.com", "example.com"),
                        url.replace("github.com/", "github.com@evil.example/")):
            self.assertTrue(self.errors(changed))


if __name__ == "__main__":
    unittest.main()
