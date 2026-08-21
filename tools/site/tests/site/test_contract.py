from __future__ import annotations

import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from fixture import REPOSITORY, SOURCE_SHA, checker, make_fixture


class CompleteContractTests(unittest.TestCase):
    PUBLICATION_RELATIVE = Path(
        "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json"
    )

    def test_00_publication_provenance_positives_then_mutants(self) -> None:
        # The real, fixed-production record in the post-B source tree is the
        # first positive. All source checks pass before any provenance mutant runs.
        self.assertEqual(checker.check_source(REPOSITORY), [])

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact, identities = make_fixture(root)
            self.assertEqual(
                checker.check_site(root, artifact, SOURCE_SHA, identities), []
            )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            ordinary = root / "docs/site/src/content/docs/current.mdx"
            ordinary.parent.mkdir(parents=True, exist_ok=True)
            ordinary.write_text(
                "This ordinary source contains no Eqiora release literal.\n",
                encoding="utf-8",
            )
            self.assertEqual(checker.check_source(root, identities), [])

        self.assertEqual(
            checker.CURRENT_VERSION_SOURCE_EXCEPTIONS,
            {
                "docs/site/src/content/docs/reference/cli/index.mdx",
                "docs/site/src/content/docs/reference/mcp/index.mdx",
            },
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            classified_sources = {
                "docs/site/src/content/docs/release-notes/alpha-1.mdx": (
                    "Historical Cargo 0.1.0-alpha.1 and Python 0.1.0a1.\n"
                ),
                "docs/site/src/content/docs/reference/cli/index.mdx": (
                    "Generated CLI release 0.1.0-alpha.1.\n"
                ),
                "docs/site/src/content/docs/reference/mcp/index.mdx": (
                    "Generated MCP release 0.1.0a1.\n"
                ),
            }
            for relative, content in classified_sources.items():
                source = root / relative
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_text(content, encoding="utf-8")
            self.assertEqual(checker.check_source(root, identities), [])

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities, publication = self._real_publication_fixture(
                root, "0.1.0-alpha.2"
            )
            before = publication.read_bytes()
            release_identity, release_errors = checker.derive_release_identity(root)
            self.assertEqual(release_errors, [])
            self.assertEqual(
                release_identity,
                checker.ReleaseIdentity(cargo="0.1.0-alpha.2", python="0.1.0a2"),
            )
            self.assertEqual(checker.check_source(root, identities), [])
            self.assertEqual(publication.read_bytes(), before)
            self.assertEqual(checker.sha256(publication), checker.PUBLICATION_SHA256)
            self.assertEqual(
                self._eqiora_input(self._read_publication(publication))["version"],
                "0.1.0a1",
            )

        # Causal mutants execute only after every ordinary positive above.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            ordinary = root / "docs/site/src/content/docs/current.mdx"
            ordinary.parent.mkdir(parents=True, exist_ok=True)
            ordinary.write_text("Alpha 0.1.0a1\n", encoding="utf-8")
            errors = checker.check_source(root, identities)
            self.assertTrue(
                any(
                    "hard-codes product version '0.1.0a1': "
                    "docs/site/src/content/docs/current.mdx" in error
                    for error in errors
                ),
                errors,
            )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            copied = (
                root / "docs/site/src/data/copied/"
                "exact-cylinder-steady-stokes.publication.json"
            )
            copied.parent.mkdir(parents=True, exist_ok=True)
            copied.write_bytes((REPOSITORY / self.PUBLICATION_RELATIVE).read_bytes())
            errors = checker.check_source(root, identities)
            self.assertTrue(
                any(
                    "hard-codes product version '0.1.0a1': "
                    "docs/site/src/data/copied/"
                    "exact-cylinder-steady-stokes.publication.json" in error
                    for error in errors
                ),
                errors,
            )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities, publication = self._real_publication_fixture(root)
            document = self._read_publication(publication)
            self._eqiora_input(document)["version"] = "0.1.0a2"
            self._write_publication(publication, document)
            self._assert_publication_mutant(root, identities, publication, "0.1.0a2")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities, publication = self._real_publication_fixture(root)
            document = self._read_publication(publication)
            resolved_inputs = self._resolved_inputs(document)
            eqiora_input = resolved_inputs.pop(4)
            resolved_inputs.insert(5, eqiora_input)
            self._write_publication(publication, document)
            self._assert_publication_mutant(root, identities, publication, "0.1.0a1")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities, publication = self._real_publication_fixture(root)
            document = self._read_publication(publication)
            document["unexpected_eqiora_version"] = "0.1.0-alpha.9"
            self._write_publication(publication, document)
            self._assert_publication_mutant(
                root, identities, publication, "0.1.0a1", "0.1.0-alpha.9"
            )

        object_mutations = {
            "kind": "archive",
            "name": "not-eqiora",
            "sha256": "0" * 64,
        }
        for field, replacement in object_mutations.items():
            with (
                self.subTest(publication_object_field=field),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                _, identities, publication = self._real_publication_fixture(root)
                document = self._read_publication(publication)
                self._eqiora_input(document)[field] = replacement
                self._write_publication(publication, document)
                self._assert_publication_mutant(
                    root, identities, publication, "0.1.0a1"
                )

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

    def test_02_execution_control_visible_and_accessible_labels_agree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact, identities = make_fixture(root)
            case = artifact / "gallery/exact-cylinder-steady-stokes/index.html"
            self._replace(
                case,
                "</main>",
                "<p>Documentation may discuss Start computation, Evaluate model, "
                "and Begin processing without making prose interactive.</p></main>",
            )
            self.assertEqual(
                checker.check_site(root, artifact, SOURCE_SHA, identities), []
            )

        pairs = {
            "native rendered value with benign ARIA override": (
                '<input type="button" value="Documentation" '
                'aria-label="Documentation">',
                '<input type="button" value="Start computation" '
                'aria-label="Documentation">',
            ),
            "descendant image alternative": (
                '<button><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" '
                'alt="Documentation"></button>',
                '<button><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" '
                'alt="Start computation"></button>',
            ),
            "ASCII-case-insensitive native type": (
                '<input type="text" value="Start computation">',
                '<input type="BUTTON" value="Start computation">',
            ),
        }
        for label, (ordinary, mutant) in pairs.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifact, identities = make_fixture(root)
                case = artifact / "gallery/exact-cylinder-steady-stokes/index.html"
                self._replace(case, "</main>", f"{ordinary}</main>")
                self.assertEqual(
                    checker.check_site(root, artifact, SOURCE_SHA, identities), []
                )
                self._replace(case, ordinary, mutant)
                errors = checker.check_site(root, artifact, SOURCE_SHA, identities)
                self.assertTrue(
                    any("uncontracted execution control" in error for error in errors),
                    errors,
                )

        controls = {
            "accessible anchor label": (
                '<a href="/get-started/" aria-label="Run simulation">Documentation</a>'
            ),
            "visible anchor label overriding benign accessibility label": (
                '<a href="/get-started/" aria-label="Documentation">Run simulation</a>'
            ),
            "explicit button role": (
                '<div role="button" aria-label="Execute calculation">Details</div>'
            ),
            "native input button": '<input type="button" value="Launch computation">',
            "aria-labelledby anchor": (
                '<a href="/get-started/" aria-labelledby="execution-label">Docs</a>'
                '<span id="execution-label">Start computation</span>'
            ),
            "evaluate synonym": '<a href="/get-started/">Evaluate model</a>',
            "processing synonym": '<a href="/get-started/">Begin processing</a>',
            "generate synonym": "<button>Generate result</button>",
            "analyse synonym": "<button>Analyse case</button>",
            "predict synonym": "<button>Predict flow</button>",
        }
        for label, control in controls.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifact, identities = make_fixture(root)
                case = artifact / "gallery/exact-cylinder-steady-stokes/index.html"
                self._replace(case, "</main>", f"{control}</main>")
                errors = checker.check_site(root, artifact, SOURCE_SHA, identities)
                self.assertTrue(
                    any("uncontracted execution control" in error for error in errors),
                    errors,
                )

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
            "fake run link": lambda root, artifact: self._replace(
                artifact / "gallery/exact-cylinder-steady-stokes/index.html",
                "</main>",
                '<a href="/get-started/">Run now</a></main>',
            ),
            "fake simulation link": lambda root, artifact: self._replace(
                artifact / "gallery/exact-cylinder-steady-stokes/index.html",
                "</main>",
                '<a href="/get-started/">Run simulation</a></main>',
            ),
            "fake calculation link": lambda root, artifact: self._replace(
                artifact / "gallery/exact-cylinder-steady-stokes/index.html",
                "</main>",
                '<a href="/get-started/">Execute calculation</a></main>',
            ),
            "fake stage-label link": lambda root, artifact: self._replace(
                artifact / "gallery/exact-cylinder-steady-stokes/index.html",
                "</main>",
                '<a href="/get-started/">Submit and result</a></main>',
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

    def test_provider_dependency_and_release_identity_mutants_fail(self) -> None:
        for relative in checker.PROVIDER_PATHS:
            with (
                self.subTest(provider=relative),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                _, identities = make_fixture(root)
                (root / relative).unlink()
                errors = checker.check_source(root, identities)
                self.assertTrue(any("accepted provider" in error for error in errors))

        provider_mutations = (
            (
                "docs/site/src/components/site/ExactSourceLink.astro",
                "EQIORA_SITE_SOURCE_SHA",
            ),
            (
                "docs/site/src/components/site/ReleaseIdentity.astro",
                "EQIORA_SITE_PYTHON_VERSION",
            ),
            (
                "docs/site/src/content/docs/index.mdx",
                "@components/site/ReleaseIdentity.astro",
            ),
            (
                "docs/site/astro.config.mjs",
                "src/components/site/ExactSourceLink.astro",
            ),
        )
        for relative, token in provider_mutations:
            with (
                self.subTest(provider_token=f"{relative}:{token}"),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                _, identities = make_fixture(root)
                source = root / relative
                source.write_text(
                    source.read_text(encoding="utf-8").replace(token, "removed"),
                    encoding="utf-8",
                )
                errors = checker.check_source(root, identities)
                self.assertTrue(any("provider" in error for error in errors))

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, identities = make_fixture(root)
            package = root / "docs/site/package.json"
            document = json.loads(package.read_text(encoding="utf-8"))
            document["dependencies"]["react"] = "19.2.4"
            package.write_text(json.dumps(document), encoding="utf-8")
            lock = root / "docs/site/package-lock.json"
            lock_document = json.loads(lock.read_text(encoding="utf-8"))
            lock_document["packages"][""]["dependencies"]["react"] = "19.2.4"
            lock_document["packages"]["node_modules/react"] = {
                "version": "19.2.4",
                "integrity": "sha512-fixture",
            }
            lock.write_text(json.dumps(lock_document), encoding="utf-8")
            errors = checker.check_source(root, identities)
            self.assertTrue(any("exact direct set" in error for error in errors))

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact, identities = make_fixture(root, "0.1.0-alpha.2")
            self.assertEqual(
                checker.check_site(root, artifact, SOURCE_SHA, identities), []
            )
            source = root / "docs/site/src/content/docs/current.mdx"
            source.write_text("0.1.0-alpha.2", encoding="utf-8")
            errors = checker.check_source(root, identities)
            self.assertTrue(
                any("hard-codes product version" in error for error in errors)
            )

    @staticmethod
    def _replace(path: Path, old: str, new: str) -> None:
        path.write_text(
            path.read_text(encoding="utf-8").replace(old, new), encoding="utf-8"
        )

    @classmethod
    def _real_publication_fixture(
        cls, root: Path, cargo_version: str = "0.1.0-alpha.1"
    ) -> tuple[Path, checker.SiteIdentities, Path]:
        artifact, identities = make_fixture(root, cargo_version)
        publication = root / cls.PUBLICATION_RELATIVE
        publication.write_bytes((REPOSITORY / cls.PUBLICATION_RELATIVE).read_bytes())
        return (
            artifact,
            replace(identities, publication=checker.PUBLICATION_SHA256),
            publication,
        )

    @staticmethod
    def _read_publication(path: Path) -> dict:
        return json.loads(path.read_text(encoding="utf-8"))

    @staticmethod
    def _write_publication(path: Path, document: dict) -> None:
        path.write_text(
            json.dumps(
                document, ensure_ascii=False, separators=(",", ":"), sort_keys=True
            )
            + "\n",
            encoding="utf-8",
        )

    @staticmethod
    def _resolved_inputs(document: dict) -> list[dict]:
        return document["publication_payload"]["renderer"]["environment"][
            "resolved_inputs"
        ]

    @classmethod
    def _eqiora_input(cls, document: dict) -> dict:
        return cls._resolved_inputs(document)[4]

    def _assert_publication_mutant(
        self,
        root: Path,
        identities: checker.SiteIdentities,
        publication: Path,
        *rejected_versions: str,
    ) -> None:
        errors = checker.check_source(root, identities)
        self.assertTrue(
            any("publication record digest mismatch" in error for error in errors),
            errors,
        )
        for rejected_version in rejected_versions:
            self.assertTrue(
                any(
                    f"hard-codes product version {rejected_version!r}: "
                    f"{self.PUBLICATION_RELATIVE.as_posix()}" in error
                    for error in errors
                ),
                errors,
            )

        caller_identity = replace(identities, publication=checker.sha256(publication))
        caller_errors = checker.check_source(root, caller_identity)
        for rejected_version in rejected_versions:
            self.assertTrue(
                any(
                    f"hard-codes product version {rejected_version!r}: "
                    f"{self.PUBLICATION_RELATIVE.as_posix()}" in error
                    for error in caller_errors
                ),
                caller_errors,
            )


if __name__ == "__main__":
    unittest.main()
