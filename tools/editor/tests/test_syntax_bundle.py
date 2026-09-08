from __future__ import annotations

import re
import unittest

from tools.editor.check_syntax_bundle import load, main, missing_parser_keywords


class SyntaxBundleTests(unittest.TestCase):
    def test_current_bundle_matches_parser_and_site(self) -> None:
        self.assertEqual(main(), 0)

    def test_missing_parser_keyword_is_reported(self) -> None:
        parser = 'parser.expect_keyword("component"); parser.at_keyword("future_word");'
        grammar = {"match": "component"}
        self.assertEqual(missing_parser_keywords(parser, grammar), ["future_word"])

    def test_enum_declaration_highlights_keyword_and_type_name(self) -> None:
        grammar = load("syntaxes/eqiora.tmLanguage.json")
        patterns = grammar["repository"]["declarations"]["patterns"]
        for prefix in ("", "public ", "private "):
            with self.subTest(prefix=prefix):
                source = f"{prefix}enum Mode {{ Heating, Cooling }}"
                matches = [
                    (pattern, match)
                    for pattern in patterns
                    if (match := re.match(pattern["match"], source))
                ]
                self.assertEqual(len(matches), 1)
                pattern, match = matches[0]
                captures = {
                    match.group(int(index)): capture["name"]
                    for index, capture in pattern["captures"].items()
                    if match.group(int(index)) is not None
                }
                self.assertEqual(captures["enum"], "keyword.declaration.eqiora")
                self.assertEqual(captures["Mode"], "entity.name.type.eqiora")
                if prefix:
                    self.assertEqual(captures[prefix.strip()], "keyword.declaration.eqiora")

    def test_case_keywords_and_arrow_preserve_token_boundaries(self) -> None:
        grammar = load("syntaxes/eqiora.tmLanguage.json")
        keywords = grammar["repository"]["keywords"]["patterns"][0]["match"]
        source = "case mode { Mode.Heating => 1, Mode.Cooling => 0 }"
        self.assertEqual(re.findall(keywords, source), ["case"])
        self.assertEqual(re.findall(keywords, "enum Mode { Case, Enum }"), ["enum"])
        self.assertEqual(re.findall(keywords, "showcase case_value enumerate enum_value"), [])
        operators = grammar["repository"]["operators"]["patterns"][0]["match"]
        self.assertEqual(re.findall(operators, source), ["=>", "=>"])


if __name__ == "__main__":
    unittest.main()
