from __future__ import annotations

import unittest

from tools.editor.check_syntax_bundle import main, missing_parser_keywords


class SyntaxBundleTests(unittest.TestCase):
    def test_current_bundle_matches_parser_and_site(self) -> None:
        self.assertEqual(main(), 0)

    def test_missing_parser_keyword_is_reported(self) -> None:
        parser = 'parser.expect_keyword("component"); parser.at_keyword("future_word");'
        grammar = {"match": "component"}
        self.assertEqual(missing_parser_keywords(parser, grammar), ["future_word"])


if __name__ == "__main__":
    unittest.main()
