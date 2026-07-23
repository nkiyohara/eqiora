from __future__ import annotations

import datetime as dt
import sys
import textwrap
import unittest
from pathlib import Path
from unittest import mock


CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

from check_implementation_agent import (  # noqa: E402
    AttestationError,
    configuration_claim,
    configuration_id,
    load_registry,
    registry_from_base,
    validate_claim,
)


CONFIGURATION = {
    "model_provider": "example-provider",
    "model_id": "example-model",
    "model_revision": "sha256:0123456789abcdef",
    "reasoning_effort": "exact-evaluated-setting",
    "agent_harness": "example-harness",
    "harness_revision": "0123456789abcdef0123456789abcdef01234567",
    "tools_profile": "deepswe-v1.1-tools",
    "execution_budget": "1200 weighted tokens; 60 minutes",
    "evaluation_protocol": "DeepSWE v1.1 official pass@1",
}


def registry_text(*, score: int = 7000, valid_until: str = "2030-01-01") -> bytes:
    identifier = configuration_id(CONFIGURATION)
    fields = "\n".join(f'{name} = "{value}"' for name, value in CONFIGURATION.items())
    return textwrap.dedent(
        f"""
        schema_version = 1
        benchmark = "DeepSWE"
        benchmark_version = "1.1"
        minimum_score_basis_points = 7000

        [[configuration]]
        id = "{identifier}"
        {fields}
        evidence_url = "https://example.invalid/evidence.json"
        evidence_sha256 = "{'a' * 64}"
        score_basis_points = {score}
        accepted_by = "bootstrap-maintainer"
        accepted_at = "2026-07-21T00:00:00Z"
        valid_until = "{valid_until}"
        status = "accepted"
        """
    ).encode()


def body(claim: str) -> str:
    return textwrap.dedent(
        f"""
        ## Optional implementation-agent provenance

        Implementation-agent configuration: {claim}
        """
    )


class RegistryTests(unittest.TestCase):
    def test_exact_configuration_is_content_addressed_and_accepted(self) -> None:
        registry = load_registry(registry_text())
        identifier = configuration_id(CONFIGURATION)

        self.assertIn(identifier, registry.configurations)
        self.assertEqual(registry.minimum_score_basis_points, 7000)

    def test_below_threshold_and_digest_mismatch_fail_closed(self) -> None:
        with self.assertRaisesRegex(AttestationError, "below threshold"):
            load_registry(registry_text(score=6999))

        corrupted = registry_text().replace(
            configuration_id(CONFIGURATION).encode(),
            f"agent-config-v1:{'0' * 64}".encode(),
        )
        with self.assertRaisesRegex(AttestationError, "does not match"):
            load_registry(corrupted)

        malformed_url = registry_text().replace(
            b"https://example.invalid/evidence.json",
            b"https:evidence-without-an-authority",
        )
        with self.assertRaisesRegex(AttestationError, "unauthenticated HTTPS"):
            load_registry(malformed_url)

    def test_empty_registry_is_valid_and_authorizes_nothing(self) -> None:
        registry = load_registry(
            b'schema_version=1\nbenchmark="DeepSWE"\nbenchmark_version="1.1"\nminimum_score_basis_points=7000\nconfiguration=[]\n'
        )
        self.assertEqual(registry.configurations, {})

    def test_registry_is_loaded_from_the_merge_base_not_the_candidate_tree(self) -> None:
        merge_base = "1" * 40
        with mock.patch(
            "check_implementation_agent.subprocess.run",
            side_effect=[
                mock.Mock(stdout=f"{merge_base}\n"),
                mock.Mock(returncode=0, stdout=registry_text()),
            ],
        ) as run:
            registry = registry_from_base("origin/main", Path("/candidate"))

        self.assertIsNotNone(registry)
        self.assertEqual(
            run.call_args_list[1].args[0],
            [
                "git",
                "show",
                f"{merge_base}:governance/implementation-agent-qualifications.toml",
            ],
        )


class ClaimTests(unittest.TestCase):
    def test_known_current_configuration_is_accepted(self) -> None:
        registry = load_registry(registry_text())
        identifier = configuration_id(CONFIGURATION)
        self.assertEqual(
            validate_claim(
                body(identifier),
                registry,
                today=dt.date(2029, 12, 31),
            ),
            identifier,
        )

    def test_unknown_and_stale_configurations_are_rejected(self) -> None:
        registry = load_registry(registry_text(valid_until="2026-07-21"))
        with self.assertRaisesRegex(AttestationError, "unknown"):
            validate_claim(
                body(f"agent-config-v1:{'f' * 64}"),
                registry,
            )
        with self.assertRaisesRegex(AttestationError, "stale"):
            validate_claim(
                body(configuration_id(CONFIGURATION)),
                registry,
                today=dt.date(2026, 7, 22),
            )

    def test_absent_empty_and_explicit_not_supplied_all_pass(self) -> None:
        self.assertEqual(validate_claim("no provenance field", None), "not-supplied")
        self.assertEqual(validate_claim(body(""), None), "not-supplied")
        self.assertEqual(validate_claim(body("not-supplied"), None), "not-supplied")
        self.assertIsNone(configuration_claim(body("not-supplied")))

    def test_malformed_or_duplicate_supplied_claim_fails_closed(self) -> None:
        with self.assertRaisesRegex(AttestationError, "malformed"):
            validate_claim(body("GPT-5"), None)
        with self.assertRaisesRegex(AttestationError, "malformed"):
            validate_claim(body("GPT 5"), None)
        duplicated = body("not-supplied") + body("not-supplied")
        with self.assertRaisesRegex(AttestationError, "multiple"):
            validate_claim(duplicated, None)


if __name__ == "__main__":
    unittest.main()
