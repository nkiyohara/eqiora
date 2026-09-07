"""Named-release source links cannot widen the current-head link boundary."""

from pathlib import Path
from types import SimpleNamespace
import unittest

from tools.site.check_site_references import _check_html


RELEASE_SHA = "e72f744cf6956e0e3c1f03aad65868d726f8ebbf"
CURRENT_SHA = "a" * 40
PREFIX = "https://github.com/nkiyohara/eqiora"
RELEASE_PAGE = "reference/language/index.html"


class ReleasedSourceLinkTests(unittest.TestCase):
    def errors(self, url, *, page=RELEASE_PAGE, text="Eqiora 0.1.0a7", tag="a", attribute="href"):
        artifact = Path("/artifact")
        path = artifact / page
        return _check_html(
            artifact,
            {path: ("", SimpleNamespace(visible_text=text))},
            [(path, tag, attribute, url, True)],
            CURRENT_SHA,
        )

    def test_named_release_accepts_exact_commit_blob_and_tree(self):
        for kind in ("blob", "tree"):
            with self.subTest(kind=kind):
                self.assertEqual(self.errors(f"{PREFIX}/{kind}/{RELEASE_SHA}/crates/eqiora-lang/src"), [])
        self.assertEqual(self.errors(f"{PREFIX}/blob/{CURRENT_SHA}/README.md", page="index.html", text="Home"), [])

    def test_tag_other_commit_and_unlabelled_release_reject(self):
        for revision in ("v0.1.0a7", "main", "b" * 40, RELEASE_SHA[:-1], RELEASE_SHA + "0"):
            with self.subTest(revision=revision):
                self.assertTrue(self.errors(f"{PREFIX}/blob/{revision}/README.md"))
        for text in ("Reference", "Eqiora 0.1.0a8", "Eqiora 0.1.0a70"):
            with self.subTest(text=text):
                self.assertTrue(self.errors(f"{PREFIX}/blob/{RELEASE_SHA}/README.md", text=text))

    def test_release_permission_does_not_escape_its_pages(self):
        for page in ("index.html", "reference/python/index.html", "reference/language/unknown/index.html"):
            with self.subTest(page=page):
                self.assertTrue(self.errors(f"{PREFIX}/blob/{RELEASE_SHA}/README.md", page=page))

    def test_release_path_cannot_normalize_to_another_revision(self):
        for tail in ("../main/README.md", "%2e%2e/main/README.md", "./README.md", "README.md?ref=main"):
            with self.subTest(tail=tail):
                self.assertTrue(self.errors(f"{PREFIX}/blob/{RELEASE_SHA}/{tail}"))

    def test_release_link_never_allows_external_runtime_or_wrong_origin(self):
        url = f"{PREFIX}/blob/{RELEASE_SHA}/README.md"
        for tag, attribute in (("script", "src"), ("img", "src"), ("link", "href")):
            with self.subTest(tag=tag):
                errors = self.errors(url, tag=tag, attribute=attribute)
                self.assertTrue(any("external runtime" in error for error in errors))
        for changed in (url.replace("https:", "http:"), url.replace("github.com", "example.com"), url.replace("github.com/", "github.com@evil.example/")):
            with self.subTest(url=changed):
                self.assertTrue(self.errors(changed))


if __name__ == "__main__":
    unittest.main()
