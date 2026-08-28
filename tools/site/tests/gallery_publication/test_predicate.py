from __future__ import annotations

import copy
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from fixtures import PublicationFixture, canonical_file, canonical_value, case_path, png_bytes, sha  # noqa: E402

CHECKER_PATH = HERE.parents[1] / "check_gallery_publication.py"
SPEC = importlib.util.spec_from_file_location("gallery_publication_checker", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


class GalleryPublicationPredicateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.fixture = PublicationFixture(Path(self.temporary.name) / "repository")

    def _verify_receipt(self):
        return checker.check_publication(
            repository_root=self.fixture.root,
            record_path=self.fixture.external_record,
            media_path=self.fixture.candidate,
            receipt_path=self.fixture.receipt_path,
        )

    def _assert_rejected(self, code: str):
        with self.assertRaises(checker.AdmissionError) as raised:
            self._verify_receipt()
        self.assertEqual(raised.exception.code, code)

    def test_09_stale_104_triangle_alt_is_rejected_at_the_text_gate(self):
        stale = self.fixture.clone_payload()
        stale["text"]["alt"] = (
            "Pressure in pascals for the frozen 2D steady-Stokes exact-cylinder "
            "demonstration, shown with a viridis color scale and the 104-triangle "
            "affine mesh overlaid. Presentation image only; linked Result evidence "
            "carries the numerical claim."
        )
        stale["text"]["alt_sha256"] = sha(stale["text"]["alt"].encode())
        with self.assertRaises(checker.AdmissionError) as raised:
            checker._check_text(stale, self.fixture.revision)
        self.assertEqual(raised.exception.code, "text")

    def test_10_one_claim_field_mutant_is_rejected(self):
        self.fixture.payload["claim"]["pixels_are_validation"] = True
        self.fixture.refresh_and_write_external()
        self._assert_rejected("claim")

    def test_11_missing_and_extra_payload_keys_are_rejected(self):
        original = copy.deepcopy(self.fixture.payload)
        del self.fixture.wrapper["publication_payload"]["scene_profile"]
        self.fixture.wrapper["publication_payload_sha256"] = sha(
            canonical_value(self.fixture.wrapper["publication_payload"])
        )
        self.fixture.external_record.write_bytes(canonical_file(self.fixture.wrapper))
        self._assert_rejected("shape")

        self.fixture.payload = original
        self.fixture.payload["unexpected"] = "closed schema"
        self.fixture.wrapper["publication_payload"] = self.fixture.payload
        self.fixture.wrapper["publication_payload_sha256"] = sha(canonical_value(self.fixture.payload))
        self.fixture.external_record.write_bytes(canonical_file(self.fixture.wrapper))
        self._assert_rejected("shape")

    def test_12_noncanonical_object_order_or_whitespace_is_rejected(self):
        raw = json.dumps(self.fixture.wrapper, ensure_ascii=False, sort_keys=True, indent=2).encode() + b"\n"
        self.fixture.external_record.write_bytes(raw)
        self._assert_rejected("json-canonical")

    def test_13_source_array_order_mutant_is_rejected(self):
        self.fixture.payload["source_files"].reverse()
        self.fixture.refresh_and_write_external()
        self._assert_rejected("source-set")

    def test_14_media_digest_mutant_is_rejected(self):
        self.fixture.payload["media"]["sha256"] = "0" * 64
        self.fixture.refresh_and_write_external()
        self._assert_rejected("media-digest")

    def test_15_model_to_result_lineage_mutant_is_rejected(self):
        self.fixture.payload["lineage"]["chain"][-1]["to"] = "1" * 64
        self.fixture.refresh_and_write_external()
        self._assert_rejected("lineage")

    def test_15b_result_binding_and_pressure_field_mutants_are_rejected(self):
        self.fixture.payload["lineage"]["result_binding"]["identity"] = "2" * 64
        self.fixture.refresh_and_write_external()
        self._assert_rejected("lineage")

    def test_15c_only_existing_result_rooted_method_owners_are_admitted(self):
        accepted = copy.deepcopy(self.fixture.payload["lineage"]["methods"])
        mutants = {
            "direct-owner": {"model_digest": "Result.digest"},
            "foreign-plan": {"plan_identity": "Result.plan_key"},
            "foreign-output": {"pressure_output": "Result.field(FieldRef)"},
            "invented": {key: "invented.not_an_owner" for key in accepted},
        }
        for label, changes in mutants.items():
            with self.subTest(label=label):
                methods = copy.deepcopy(accepted)
                methods.update(changes)
                self.fixture.payload["lineage"]["methods"] = methods
                self.fixture.refresh_and_write_external()
                self._assert_rejected("lineage-method")

    def test_15d_existing_result_identity_equalities_are_required(self):
        accepted = copy.deepcopy(self.fixture.payload["lineage"])
        for label in ("evidence-plan", "output-shape", "fieldref-model"):
            with self.subTest(label=label):
                lineage = copy.deepcopy(accepted)
                if label == "evidence-plan":
                    lineage["identities"]["evidence_plan_key"] = "3" * 64
                    lineage["chain"][6]["to"] = "3" * 64
                elif label == "output-shape":
                    lineage["pressure"]["components"] = 2
                else:
                    lineage["pressure"]["model_digest"] = "5" * 64
                self.fixture.payload["lineage"] = lineage
                self.fixture.refresh_and_write_external()
                self._assert_rejected("lineage")

        self.fixture.payload["lineage"]["result_binding"]["identity"] = self.fixture.payload["lineage"]["identities"][
            "result_plan_key"
        ]
        self.fixture.payload["lineage"]["pressure"]["field"] = "velocity"
        self.fixture.refresh_and_write_external()
        self._assert_rejected("lineage")

    def test_16_png_crc_mutant_is_rejected_even_when_file_digest_is_rebound(self):
        raw = bytearray(self.fixture.candidate.read_bytes())
        data_start = raw.index(b"IDAT") + 4
        raw[data_start] ^= 0x01
        self.fixture.candidate.write_bytes(raw)
        self.fixture.payload["media"]["byte_size"] = len(raw)
        self.fixture.payload["media"]["sha256"] = sha(bytes(raw))
        self.fixture.refresh_and_write_external()
        self._assert_rejected("png")

    def test_16b_fully_transparent_varying_rgb_is_visually_blank(self):
        transparent, decoded, chunk_types = png_bytes(0)
        self.fixture.candidate.write_bytes(transparent)
        media = self.fixture.payload["media"]
        media["byte_size"] = len(transparent)
        media["chunk_types"] = list(chunk_types)
        media["decoded_pixel_sha256"] = sha(decoded)
        media["sha256"] = sha(transparent)
        self.fixture.refresh_and_write_external()
        self._assert_rejected("png")

    def test_17_renderer_environment_mutant_is_rejected(self):
        inputs = self.fixture.payload["renderer"]["environment"]["resolved_inputs"]
        next(item for item in inputs if item["name"] == "Python")["version"] = "3.13.13"
        self.fixture.refresh_and_write_external()
        self._assert_rejected("environment")

    def test_17b_text_and_scene_profile_mutants_are_rejected(self):
        self.fixture.payload["text"]["alt"] += " Widened."
        self.fixture.payload["text"]["alt_sha256"] = sha(self.fixture.payload["text"]["alt"].encode())
        self.fixture.refresh_and_write_external()
        self._assert_rejected("text")

        self.fixture.payload = self.fixture.clone_payload()
        self.fixture.payload["text"]["alt"] = checker.ALT_TEXT
        self.fixture.payload["text"]["alt_sha256"] = sha(checker.ALT_TEXT.encode())
        self.fixture.payload["scene_profile"]["dpi"] = 161
        self.fixture.refresh_and_write_external()
        self._assert_rejected("scene-profile")

    def test_18_escaping_source_path_mutant_is_rejected(self):
        self.fixture.payload["source_files"][0]["path"] = "../matplotlib.py"
        self.fixture.refresh_and_write_external()
        self._assert_rejected("path")

    def test_19_case_role_and_route_mutants_are_rejected(self):
        case = next(
            item
            for item in self.fixture.payload["evidence_cases"]
            if item["id"] == "interfaces.python-exact-cylinder-stokes-marimo"
        )
        case["role"] = "media-admission"
        self.fixture.refresh_and_write_external()
        self._assert_rejected("case-set")

    def test_19c_unregistered_case_cannot_replace_a_current_case(self):
        case = next(
            item
            for item in self.fixture.payload["evidence_cases"]
            if item["id"] == "interfaces.python-exact-cylinder-stokes-marimo"
        )
        case["id"] = "fluid.unregistered-retired-cylinder-case"
        case["manifest_path"] = case_path(case["id"])
        self.fixture.refresh_and_write_external()
        self._assert_rejected("case-set")

    def test_19d_output_nonclaim_cannot_be_omitted(self):
        claim = self.fixture.payload["claim"]
        claim["public_claim"] = claim["public_claim"].replace(
            "output counts, digests, numerical values, and pixels are not independently verified",
            "pixels are not independently verified",
        )
        self.fixture.refresh_and_write_external()
        self._assert_rejected("claim")

    def test_20_external_receipt_digest_is_lifecycle_authority(self):
        self.fixture.wrapper["admission"]["receipt"]["sha256"] = "f" * 64
        self.fixture.external_record.write_bytes(canonical_file(self.fixture.wrapper))
        self._assert_rejected("receipt-digest")

    def test_21_record_and_media_symlinks_are_rejected(self):
        real_record = self.fixture.external_record.with_name("real-record.json")
        self.fixture.external_record.rename(real_record)
        self.fixture.external_record.symlink_to(real_record)
        self._assert_rejected("path")

    def test_22_duplicate_json_key_and_raw_cap_are_rejected(self):
        self.fixture.external_record.write_bytes(b'{"schema":1,"schema":2}\n')
        self._assert_rejected("json-duplicate")
        self.fixture.external_record.write_bytes(b"x" * (checker.MAX_JSON_BYTES + 1))
        self._assert_rejected("raw-cap")


if __name__ == "__main__":
    unittest.main()
