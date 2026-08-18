from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from fixture import SOURCE_SHA, checker, make_fixture


class CompleteContractTests(unittest.TestCase):
    def test_00_synthetic_ordinary_site_passes_before_mutants(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact, identities = make_fixture(root)
            self.assertEqual(
                checker.check_site(root, artifact, SOURCE_SHA, identities), []
            )

    def test_01_foundation_shape_is_red_for_only_missing_downstream_inputs(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            old = root / "docs/site/assets/social-card.svg"
            old.parent.mkdir(parents=True, exist_ok=True)
            old.write_text(checker.OLD_SOCIAL_LINE, encoding="utf-8")
            for relative in (
                "docs/site/src/assets/gallery/exact-cylinder-pressure.png",
                "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json",
                "docs/site/public/social-card.svg",
            ):
                (root / relative).unlink()
            errors = checker.check_source(root, identities)
            joined = "\n".join(errors)
            self.assertIn(
                "obsolete successor source remains: docs/site/assets/social-card.svg",
                joined,
            )
            self.assertIn("missing exact admitted pressure media", joined)
            self.assertIn("missing exact admitted publication record", joined)
            self.assertIn("missing exact timeless social card", joined)
            self.assertNotIn("site package must pin", joined)
            self.assertNotIn("Pages path filters", joined)

    def test_route_canonical_media_and_claim_mutants_fail(self) -> None:
        mutations = {
            "missing route": lambda root, artifact: (
                artifact / "reference/mcp/index.html"
            ).unlink(),
            "duplicate canonical": lambda root, artifact: self._replace(
                artifact / "index.html",
                "</head>",
                '<link rel="canonical" href="https://eqiora.org/"></head>',
            ),
            "wrong pressure alt": lambda root, artifact: self._replace(
                artifact / "gallery/exact-cylinder-steady-stokes/index.html",
                checker.PRESSURE_ALT,
                "Pressure plot",
            ),
            "missing featured pressure": lambda root, artifact: self._replace(
                artifact / "index.html",
                checker.PRESSURE_ALT,
                "Pressure plot",
            ),
            "unlinked brand": lambda root, artifact: self._replace(
                artifact / "index.html",
                '<a href="/"><img src="/assets/brand.svg" alt="">Eqiora</a>',
                '<img src="/assets/brand.svg" alt="">Eqiora',
            ),
            "widened featured claim": lambda root, artifact: self._replace(
                artifact / "index.html",
                "Featured walkthrough",
                "Featured walkthrough production ready",
            ),
            "fake run button": lambda root, artifact: self._replace(
                artifact / "gallery/exact-cylinder-steady-stokes/index.html",
                "</main>",
                "<button>Run now</button></main>",
            ),
            "missing social": lambda root, artifact: (
                artifact / "social-card.svg"
            ).unlink(),
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifact, identities = make_fixture(root)
                mutate(root, artifact)
                self.assertTrue(
                    checker.check_site(root, artifact, SOURCE_SHA, identities)
                )

    def test_source_identity_and_stale_social_mutants_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            publication = (
                root
                / "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json"
            )
            publication.write_text('{"tuned":true}\n', encoding="utf-8")
            errors = checker.check_source(root, identities)
            self.assertTrue(
                any("publication record digest mismatch" in error for error in errors)
            )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            old = root / "docs/site/assets/social-card.svg"
            old.parent.mkdir(parents=True, exist_ok=True)
            old.write_text(checker.OLD_SOCIAL_LINE, encoding="utf-8")
            errors = checker.check_source(root, identities)
            self.assertTrue(
                any("obsolete successor source remains" in error for error in errors)
            )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            source = root / "docs/site/src/content/docs/current.mdx"
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("Alpha 0.1.0a1", encoding="utf-8")
            errors = checker.check_source(root, identities)
            self.assertTrue(
                any("hard-codes product version" in error for error in errors)
            )

    @staticmethod
    def _replace(path: Path, old: str, new: str) -> None:
        path.write_text(
            path.read_text(encoding="utf-8").replace(old, new), encoding="utf-8"
        )


if __name__ == "__main__":
    unittest.main()
