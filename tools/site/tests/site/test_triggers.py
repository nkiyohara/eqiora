from __future__ import annotations

import unittest

from fixture import checker


def workflow(pull: list[str], push: list[str]) -> str:
    def block(values: list[str]) -> str:
        return "\n".join(f'      - "{value}"' for value in values)

    return f"""on:
  pull_request:
    paths:
{block(pull)}
  push:
    paths:
{block(push)}
jobs:
  build:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@{"1" * 40}
      - run: |
          echo ubuntu-24.04 eqiora-pw-1.62.1-r1234
          npx playwright install --with-deps --only-shell chromium
          echo 'HeadlessChrome 151.0.7922.34'
          unshare --net sh -c 'ip link set lo up; setpriv true'
          export npm_config_offline=true CARGO_NET_OFFLINE=true UV_OFFLINE=1
"""


class TriggerContractTests(unittest.TestCase):
    def test_00_complete_equal_filters_select_every_representative(self) -> None:
        patterns = sorted(checker.REQUIRED_TRIGGER_PATTERNS)
        self.assertEqual(checker.check_workflow_text(workflow(patterns, patterns)), [])
        for changed in checker.TRIGGER_REPRESENTATIVES.values():
            self.assertTrue(checker.selected_by_paths(patterns, changed))
        self.assertFalse(checker.selected_by_paths(patterns, "notes/unrelated.txt"))

    def test_each_removed_authority_and_unequal_event_filter_fails(self) -> None:
        patterns = sorted(checker.REQUIRED_TRIGGER_PATTERNS)
        for removed in patterns:
            with self.subTest(removed=removed):
                mutant = [item for item in patterns if item != removed]
                errors = checker.check_workflow_text(workflow(mutant, mutant))
                self.assertTrue(
                    any("omit exact authorities" in error for error in errors)
                )
        errors = checker.check_workflow_text(workflow(patterns, patterns[:-1]))
        self.assertTrue(any("filters differ" in error for error in errors))

    def test_toolchain_supply_and_namespace_mutants_fail(self) -> None:
        patterns = sorted(checker.REQUIRED_TRIGGER_PATTERNS)
        ordinary = workflow(patterns, patterns)
        for token in checker.OFFLINE_WORKFLOW_TOKENS:
            with self.subTest(token=token):
                errors = checker.check_workflow_text(
                    ordinary.replace(token, "removed-token")
                )
                self.assertTrue(
                    any("offline/supply boundary" in error for error in errors)
                )


if __name__ == "__main__":
    unittest.main()
