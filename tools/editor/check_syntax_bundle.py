#!/usr/bin/env python3
"""Check the distributable editor syntax bundle against parser vocabulary."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BUNDLE = ROOT / "editor/eqiora"
PARSER_FILES = (ROOT / "crates/eqiora-lang/src/parser.rs", *sorted((ROOT / "crates/eqiora-lang/src/parser").glob("*.rs")))


def load(relative: str) -> object:
    return json.loads((BUNDLE / relative).read_text(encoding="utf-8"))


def missing_parser_keywords(parser_text: str, grammar: object) -> list[str]:
    parser_keywords = set(
        re.findall(r'(?:at|expect)_keyword\("([a-z_]+)"\)', parser_text)
    )
    grammar_text = json.dumps(grammar, sort_keys=True)
    return sorted(
        word
        for word in parser_keywords
        if not re.search(rf"\b{re.escape(word)}\b", grammar_text)
    )


def main() -> int:
    inventory = sorted(
        path.relative_to(BUNDLE).as_posix()
        for path in BUNDLE.rglob("*")
        if path.is_file()
    )
    assert inventory == [
        "README.md",
        "bundle.json",
        "language-configuration.json",
        "snippets/eqiora.json",
        "syntaxes/eqiora.tmLanguage.json",
    ]
    manifest = load("bundle.json")
    assert isinstance(manifest, dict)
    assert manifest == {
        "schema": "eqiora.editor-syntax-bundle.v1",
        "bundleVersion": "0.1.0",
        "languageVersion": "0",
        "languageId": "eqiora",
        "extensions": [".eqi"],
        "grammar": "syntaxes/eqiora.tmLanguage.json",
        "configuration": "language-configuration.json",
        "snippets": "snippets/eqiora.json",
    }
    grammar = load(str(manifest["grammar"]))
    configuration = load(str(manifest["configuration"]))
    snippets = load(str(manifest["snippets"]))
    assert isinstance(grammar, dict) and grammar["name"] == manifest["languageId"]
    assert grammar["scopeName"] == "source.eqiora"
    assert grammar["fileTypes"] == ["eqi"]
    assert isinstance(configuration, dict) and configuration["comments"] == {"lineComment": "//"}
    assert isinstance(snippets, dict) and set(snippets) == {"Component", "Model", "Relation"}
    for entry in grammar["repository"].values():
        for pattern in entry["patterns"]:
            re.compile(pattern["match"])

    parser_text = "\n".join(path.read_text(encoding="utf-8") for path in PARSER_FILES)
    parser_keywords = set(
        re.findall(r'(?:at|expect)_keyword\("([a-z_]+)"\)', parser_text)
    )
    missing = missing_parser_keywords(parser_text, grammar)
    if missing:
        raise SystemExit(f"syntax bundle misses parser keywords: {', '.join(missing)}")

    site_config = (ROOT / "docs/site/astro.config.mjs").read_text(encoding="utf-8")
    if "../../editor/eqiora/syntaxes/eqiora.tmLanguage.json" not in site_config:
        raise SystemExit("documentation site does not consume the canonical grammar")
    print(f"editor syntax bundle: {len(parser_keywords)} parser keywords covered")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
