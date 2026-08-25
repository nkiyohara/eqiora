from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[4]
BUILDER_PATH = REPOSITORY / "tools/site/build_rust_reference.py"
SPEC = importlib.util.spec_from_file_location("rustdoc_builder_order", BUILDER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("cannot load Rust-reference builder")
BUILDER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = BUILDER
SPEC.loader.exec_module(BUILDER)


def _implementation(section_id: str, href: str) -> str:
    return (
        '<details class="toggle implementors-toggle">'
        f'<summary><section id="{section_id}"><a href="{href}">{section_id}</a>'
        "</section></summary><p>Ordinary implementation documentation.</p></details>"
    )


def _fixed_implementation(section_id: str, href: str) -> str:
    return (
        f'<section class="impl" id="{section_id}">'
        f'<a href="{href}">{section_id}</a></section>'
    )


def _document(implementations: list[str]) -> str:
    return (
        "<!doctype html><html><body><main>"
        '<div id="trait-implementations-list">\n'
        + "\n".join(implementations)
        + "\n</div></main></body></html>"
    )


class RustdocBuilderOrderTests(unittest.TestCase):
    def test_reversed_compiler_order_converges_without_losing_no_js_links(self) -> None:
        first = _implementation("impl-Alpha-for-Value", "alpha.html")
        second = _implementation("impl-Beta-for-Value", "beta.html")
        fixed_first = _fixed_implementation(
            "impl-FixedAlpha-for-Value", "fixed-alpha.html"
        )
        fixed_second = _fixed_implementation(
            "impl-FixedBeta-for-Value", "fixed-beta.html"
        )

        canonical, _ = BUILDER._project_document(
            _document([first, second, fixed_first, fixed_second]),
            "ordinary compiler order",
        )
        reversed_output, _ = BUILDER._project_document(
            _document([second, first, fixed_second, fixed_first]),
            "reversed compiler order",
        )

        self.assertEqual(reversed_output, canonical)
        parsed = BUILDER._parse_document(canonical, "canonical output")
        self.assertEqual(
            BUILDER._active_hrefs(parsed),
            ["alpha.html", "beta.html", "fixed-alpha.html", "fixed-beta.html"],
        )
        self.assertEqual(canonical.count("data-eqiora-href="), 2)
        self.assertEqual(canonical.count('class="eqiora-signature-links"'), 2)

    def test_duplicate_stable_implementation_ids_fail_closed(self) -> None:
        duplicate = _implementation("impl-Alpha-for-Value", "alpha.html")
        with self.assertRaisesRegex(
            BUILDER.RustReferenceError, "duplicate stable section ids"
        ):
            BUILDER._project_document(
                _document([duplicate, duplicate]), "duplicate implementation order"
            )


if __name__ == "__main__":
    unittest.main()
