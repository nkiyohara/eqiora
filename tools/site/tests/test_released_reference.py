"""Focused content and opt-in installed-release checks for the Reference examples.

The executable check consumes a separately qualified PyPI installation and sdist;
it never installs packages or treats a current development wheel as release proof.
"""

from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
REFERENCE = ROOT / "docs/site/src/content/docs/reference"
PAGES = sorted((REFERENCE / "language").glob("*.mdx")) + sorted(
    (REFERENCE / "standard-packages").glob("*.mdx")
)
RAW_IMPORT = re.compile(r"import (\w+) from '([^']+\.eqi)\?raw';")
PYTHON_BLOCK = re.compile(r"```python\n(.*?)```", re.DOTALL)
RELEASE_SOURCE = re.compile(
    r"https://github\.com/nkiyohara/eqiora/blob/v0\.1\.0a7/(packages/[^)#\s]+\.eqi)"
)


def examples() -> dict[Path, str]:
    result = {}
    for page in PAGES:
        content = page.read_text(encoding="utf-8")
        for name, relative in RAW_IMPORT.findall(content):
            path = (page.parent / relative).resolve()
            if not path.is_relative_to(REFERENCE):
                raise AssertionError(f"example outside Reference: {path}")
            if not re.search(rf"<Code\s+code=\{{{name}\}}", content):
                raise AssertionError(f"unrendered example import: {page}: {name}")
            result[path] = path.read_text(encoding="utf-8")
    return result


class ReleasedReferenceContentTests(unittest.TestCase):
    def test_rendered_examples_have_one_maintained_source(self):
        self.assertEqual(len(PAGES), 8)
        self.assertEqual(set(examples()), set(REFERENCE.glob("*/_examples/*.eqi")))
        for page in PAGES:
            with self.subTest(page=page.name):
                content = page.read_text(encoding="utf-8")
                self.assertIn("0.1.0a7", content)
                self.assertIn("editUrl: https://github.com/nkiyohara/eqiora/edit/main/", content)
                # Eqiora programs are imported, not maintained as a second manual copy.
                self.assertNotIn("```eqiora", content)

    def test_reference_links_resolve_to_existing_content_or_release_sources(self):
        content_root = ROOT / "docs/site/src/content/docs"
        for page in [REFERENCE / "index.mdx", *PAGES]:
            content = page.read_text(encoding="utf-8")
            links = re.findall(r"\]\((/[^)#]*)(?:#[^)]+)?\)", content)
            links.extend(re.findall(r'href="(/[^"#]+)"', content))
            for link in links:
                route = content_root / link.strip("/")
                candidates = [route / "index.mdx", route / "index.md", route.with_suffix(".mdx"), route.with_suffix(".md")]
                self.assertTrue(any(path.is_file() for path in candidates), f"{page}: {link}")
        index = (REFERENCE / "standard-packages/index.mdx").read_text(encoding="utf-8")
        self.assertEqual(len(set(RELEASE_SOURCE.findall(index))), 8)
        self.assertNotIn("pip install", index)


@unittest.skipUnless(
    os.environ.get("EQIORA_REFERENCE_RELEASE_PYTHON")
    and os.environ.get("EQIORA_REFERENCE_RELEASE_SDIST"),
    "requires explicitly qualified installed-release Python and downloaded release sdist",
)
class InstalledReleasedReferenceTests(unittest.TestCase):
    def test_published_examples_and_commands_against_installed_a7(self):
        python = os.environ["EQIORA_REFERENCE_RELEASE_PYTHON"]
        archive_path = Path(os.environ["EQIORA_REFERENCE_RELEASE_SDIST"])
        with tempfile.TemporaryDirectory(prefix="eqiora-reference-") as directory:
            scratch = Path(directory)
            sources = examples()
            for path, content in sources.items():
                (scratch / path.name).write_text(content, encoding="utf-8")
            (scratch / "model.eqi").write_text(
                (REFERENCE / "language/_examples/declarations.eqi").read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            # Read only the named source members, never extract an unbounded archive tree.
            declared = set(RELEASE_SOURCE.findall(
                (REFERENCE / "standard-packages/index.mdx").read_text(encoding="utf-8")
            ))
            with tarfile.open(archive_path, "r:gz") as archive:
                for relative in sorted(declared):
                    name = "eqiora-0.1.0a7/" + relative
                    member = archive.getmember(name)
                    self.assertTrue(member.isfile())
                    self.assertLess(member.size, 1_048_576)
                    stream = archive.extractfile(member)
                    self.assertIsNotNone(stream)
                    data = stream.read()
                    target = scratch / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(data)

            def execute(code: str) -> None:
                prelude = (
                    "import importlib.metadata\n"
                    "assert importlib.metadata.version('eqiora') == '0.1.0a7'\n"
                )
                completed = subprocess.run(
                    [python, "-c", prelude + code], cwd=scratch,
                    capture_output=True, text=True, timeout=30, check=False,
                )
                self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

            for page in PAGES:
                for code in PYTHON_BLOCK.findall(page.read_text(encoding="utf-8")):
                    with self.subTest(command=page.name):
                        execute(code + "\nassert isinstance(model, eqiora.Model)\n")
            for path in sorted((REFERENCE / "language/_examples").glob("*.eqi")):
                with self.subTest(construct=path.name):
                    execute(
                        "import eqiora\n"
                        f"model = eqiora.compile(path={path.name!r})\n"
                        "assert model.field_ids\n"
                    )
            execute("""
from pathlib import Path
import eqiora
declaration = Path('declarations.eqi').read_text()
composition = Path('composition.eqi').read_text()
bad = [
    declaration.replace('field current: A;', 'field current: m;'),
    declaration.replace('= 12;', '= 12 [V];'),
    composition.replace('input = 2', 'offset = 2'),
]
for source in bad:
    try:
        eqiora.compile(source=source, filename='rejected.eqi')
    except eqiora.ValidationError:
        pass
    else:
        raise AssertionError('incorrect dimensions, unsupported quantities or missing binding accepted')
""")


if __name__ == "__main__":
    unittest.main()
