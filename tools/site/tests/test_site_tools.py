from __future__ import annotations

import importlib.util
import subprocess
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


site_check = load_module("check_site", TOOLS / "check_site.py")


def render_markdown(source):
    script = """
import { markdownToHtml } from 'satteri';
import { katexMathPlugin } from './src/plugins/katex.ts';
let source = '';
process.stdin.setEncoding('utf8');
for await (const chunk of process.stdin) source += chunk;
const { html } = markdownToHtml(source, {
  features: { math: true },
  mdastPlugins: [katexMathPlugin],
});
process.stdout.write(html);
"""
    return subprocess.run(
        ["node", "--input-type=module", "--eval", script],
        cwd=TOOLS.parents[1] / "docs/site",
        input=source,
        text=True,
        capture_output=True,
        check=True,
    ).stdout


class MathRenderingTests(unittest.TestCase):
    def test_block_math_renders_accessible_html_and_mathml(self):
        html = render_markdown("$$\n\\frac{1}{2}\n$$\n")
        for marker in (
            'class="katex-display"', 'class="katex-mathml"',
            'class="katex-html"', '<math', 'display="block"',
            'role="region"', 'aria-label="Displayed equation"', 'tabindex="0"',
        ):
            self.assertIn(marker, html)

    def test_inline_math_renders_without_a_display_region(self):
        html = render_markdown("Inline $\\frac{1}{2}$ equation.\n")
        for marker in ('class="katex-mathml"', 'class="katex-html"', '<math'):
            self.assertIn(marker, html)
        self.assertNotIn('class="katex-display"', html)
        self.assertNotIn('role="region"', html)

    def test_malformed_math_stops_rendering(self):
        for source in ("$$\n\\frac{\n$$\n", "Inline $\\frac{$ equation.\n"):
            with self.subTest(source=source):
                with self.assertRaises(subprocess.CalledProcessError) as raised:
                    render_markdown(source)
                self.assertIn("KaTeX parse error", raised.exception.stderr)
                self.assertEqual(raised.exception.stdout, "")


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
