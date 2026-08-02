"""Mutants for the private fixed-reference FSI gallery build record."""

from __future__ import annotations

import inspect

import pytest

import record


REVISION = "a" * 40
EVIDENCE = (
    "fsi.fixed-reference-monolithic-step-2d",
    "artifacts.fixed-reference-fsi-spatial-trajectory",
)
TEXT = """DEVELOPMENT PREVIEW — NOT PUBLISHABLE

This silent presentation shows the result-owned two-dimensional fluid region,
solid region, and conforming interface. Fluid pressure [Pa] is the only primary
field and uses one fixed display scale throughout. The solid displacement [m]
is presentation geometry drawn at an explicit ×12 exaggeration, with reference
and deformed outlines distinguished by line style as well as color.

The source owns accepted step 1 at t1 = 0.05 s and accepted step 2 at t2 = 0.1
s. The moving middle frames are presentation interpolation between those two
accepted states and are not solved dynamics or a continuous physical-time
claim. A labelled neutral scene reset changes opacity only; the physics is
never played backwards. The reduced-motion image places accepted step 1 and
accepted step 2 side by side with the same scale, geometry, units, and lineage.

Evidence: fsi.fixed-reference-monolithic-step-2d and
artifacts.fixed-reference-fsi-spatial-trajectory. Run digest
1111111111111111111111111111111111111111111111111111111111111111.
"""


def digest(label: str) -> str:
    return record.sha256_bytes(label.encode("utf-8"))


def sealed(value: dict[str, object], key: str) -> dict[str, object]:
    value[key] = record.content_digest(value, key)
    return value


def fixture() -> tuple[
    dict[str, object], record.ProductionContext, dict[str, record.FileFact]
]:
    lineage = {
        "model_digest": digest("model"),
        "semantic_revision": 3,
        "geometry_digest": digest("geometry"),
        "correspondence_digest": digest("correspondence"),
        "mesh_digest": digest("mesh"),
        "realization_digest": digest("realization"),
        "realization_revision": 4,
        "run_digest": "1" * 64,
        "run_manifest_sha256": digest("run-manifest"),
        "state_digests": [digest("state-1"), digest("state-2")],
        "trajectory_digest": digest("trajectory"),
        "scientific_case_ids": list(EVIDENCE),
    }
    source = {
        "build_revision": REVISION,
        "build_script_path": "tools/gallery/build_fixed_reference_fsi.py",
        "build_script_sha256": digest("build-script"),
        "record_module_path": "tools/gallery/record.py",
        "record_module_sha256": digest("record-module"),
        "scene_module_path": "tools/gallery/scene.py",
        "scene_module_sha256": digest("scene-module"),
        "model_source_sha256": digest("model-source"),
        "result_digest": lineage["run_digest"],
        "result_frame_input_sha256": digest("frame-input"),
        "eqiora_version": "0.1.0-alpha.1",
        "eqiora_module_is_installed": True,
    }
    segments = [
        {
            "name": name,
            "first_frame": first,
            "last_frame": last,
            "frame_count": last - first + 1,
            "seconds": (last - first + 1) / 30,
            "kind": kind,
            "label": record.EXPECTED_SEGMENT_LABELS[name],
        }
        for name, first, last, kind in record.EXPECTED_SEGMENTS
    ]
    scene = sealed(
        {
            "profile_id": "fixed-reference-fsi-development-film/1",
            "profile_sha256": "",
            "width": 1280,
            "height": 720,
            "fps": 30,
            "frame_count": 450,
            "duration_s": 15.0,
            "primary_quantity": "fluid_pressure_pa",
            "geometry_state": "solid_displacement_m_times_12",
            "pressure_display_bound_pa": 0.12,
            "displacement_scale": 12.0,
            "axis_limits": {"x": [-0.1, 1.1], "y": [-0.2, 0.8]},
            "physical_time": {
                "state_1_time_s": 0.05,
                "state_2_time_s": 0.1,
                "interval_s": 0.05,
                "presentation_time_is_not_physical_time": True,
            },
            "segments": segments,
            "interpolation": {
                "kind": "presentation-only-linear",
                "fields": ["fluid_pressure_pa", "solid_displacement_m"],
                "first_frame": 150,
                "last_frame": 299,
                "tau_rule": "(frame-150)/150",
                "label": ("PRESENTATION INTERPOLATION t1 → t2 — not solved dynamics"),
            },
            "frame_sequence_sha256": digest("frames"),
            "fields_presented": [
                "fluid_pressure_pa",
                "solid_displacement_geometry_m",
            ],
            "per_frame_autoranging": False,
            "watermark": record.WATERMARK,
        },
        "profile_sha256",
    )
    renderer = {
        "identity": "eqiora.gallery.private-fsi-renderer/1",
        "matplotlib_version": "3.11.1",
        "backend": "Agg",
        "module_sha256": digest("scene-module"),
        "png_metadata": {"Software": "Eqiora private gallery renderer/1"},
    }
    encoder = sealed(
        {
            "ffmpeg_path": "/usr/bin/ffmpeg",
            "ffmpeg_sha256": digest("ffmpeg"),
            "ffmpeg_version": "ffmpeg version 8.1.2",
            "ffmpeg_configuration": "--enable-libvpx --enable-libx264",
            "ffprobe_path": "/usr/bin/ffprobe",
            "ffprobe_sha256": digest("ffprobe"),
            "webm_argv": [
                "/usr/bin/ffmpeg",
                "-an",
                "-threads",
                "1",
                "-filter_threads",
                "1",
                "-filter_complex_threads",
                "1",
                "-fps_mode",
                "cfr",
                "-bitexact",
                "-fflags",
                "+bitexact",
                "-flags:v",
                "+bitexact",
                "-map_metadata",
                "-1",
                "-map_chapters",
                "-1",
                "-c:v",
                "libvpx-vp9",
                "dev-film.webm",
            ],
            "mp4_argv": [
                "/usr/bin/ffmpeg",
                "-an",
                "-threads",
                "1",
                "-filter_threads",
                "1",
                "-filter_complex_threads",
                "1",
                "-fps_mode",
                "cfr",
                "-bitexact",
                "-fflags",
                "+bitexact",
                "-flags:v",
                "+bitexact",
                "-map_metadata",
                "-1",
                "-map_chapters",
                "-1",
                "-c:v",
                "libx264",
                "dev-film.mp4",
            ],
            "profile_sha256": "",
        },
        "profile_sha256",
    )
    environment = sealed(
        {
            "platform": "Linux-6.0",
            "machine": "x86_64",
            "python_version": "3.13.0",
            "eqiora_version": "0.1.0-alpha.1",
            "numpy_version": "2.4.0",
            "matplotlib_version": "3.11.1",
            "source_date_epoch": 0,
            "tz": "UTC",
            "locale": "C",
            "pythonhashseed": "0",
            "profile_sha256": "",
        },
        "profile_sha256",
    )
    output_digests = {
        role: record.sha256_bytes(TEXT.encode("utf-8"))
        if role == "text_alternative"
        else digest(f"output-{role}")
        for role in record.OUTPUT_CONTRACTS
    }
    output_facts = {
        role: record.FileFact(value, 1000 + index)
        for index, (role, value) in enumerate(output_digests.items())
    }
    outputs: dict[str, object] = {}
    for role, (filename, media_type) in record.OUTPUT_CONTRACTS.items():
        if role in {"poster", "reduced_motion_still"}:
            width, height = 1280, 720
            duration_s = fps = frame_count = stream_count = codec = None
        elif role.startswith("film_"):
            width, height = 1280, 720
            duration_s, fps, frame_count, stream_count = 15.0, 30, 450, 1
            codec = "vp9" if role == "film_modern" else "h264"
        else:
            width = height = duration_s = fps = frame_count = stream_count = codec = (
                None
            )
        fact = output_facts[role]
        outputs[role] = {
            "filename": filename,
            "media_type": media_type,
            "sha256": fact.sha256,
            "bytes": fact.bytes,
            "width": width,
            "height": height,
            "duration_s": duration_s,
            "fps": fps,
            "frame_count": frame_count,
            "has_audio": False,
            "stream_count": stream_count,
            "codec": codec,
        }
    accessibility = {
        "reduced_motion_still": "dev-reduced-motion.png",
        "text_alternative": "dev-text-alternative.txt",
        "delivery_requirement": record.DELIVERY_REQUIREMENT,
        "dossier_route": record.DOSSIER_ROUTE,
        "description_sha256": record.sha256_bytes(TEXT.encode("utf-8")),
    }
    candidate = record.development_record(
        lineage=lineage,
        source=source,
        scene=scene,
        renderer=renderer,
        encoder=encoder,
        environment=environment,
        outputs=outputs,
        accessibility=accessibility,
    )
    candidate["publication_status"] = "production"
    candidate["non_publishable_reason"] = None
    candidate["experience_id"] = "turek-hron-fsi3"
    claim = candidate["claim"]
    assert isinstance(claim, dict)
    contract = record.ClaimContract(
        statement=claim["statement"],
        non_claims=tuple(claim["non_claims"]),
        quantities=tuple(sorted(claim["quantities"].items())),
        evidence_cases=frozenset(EVIDENCE),
        dossier_route=claim["dossier_route"],
    )
    context = record.ProductionContext(
        protected_base_revision=REVISION,
        registered_case_ids=frozenset(EVIDENCE),
        lineage_by_run_digest={lineage["run_digest"]: record.lineage_digest(lineage)},
        contracts={"turek-hron-fsi3": contract},
    )
    return candidate, context, output_facts


def outcome(
    candidate: dict[str, object],
    context: record.ProductionContext,
    facts: dict[str, record.FileFact],
    text: str = TEXT,
) -> record.Admission:
    return record.admit(
        candidate,
        context=context,
        output_facts=facts,
        text_alternative=text,
    )


def test_synthetic_structurally_complete_candidate_is_admitted() -> None:
    candidate, context, facts = fixture()
    assert outcome(candidate, context, facts).reasons == ()


def test_development_record_is_truthful_and_permanently_rejected() -> None:
    candidate, context, facts = fixture()
    candidate["publication_status"] = "development-preview"
    candidate["non_publishable_reason"] = record.DEVELOPMENT_REASON
    candidate["experience_id"] = record.DEVELOPMENT_EXPERIENCE_ID
    assert candidate["evidence_cases"] == list(EVIDENCE)
    assert outcome(candidate, context, facts).reasons == (
        "publication-status",
        "experience-id",
    )
    assert set(inspect.signature(record.development_record).parameters) == {
        "lineage",
        "source",
        "scene",
        "renderer",
        "encoder",
        "environment",
        "outputs",
        "accessibility",
    }


@pytest.mark.parametrize(
    ("mutate", "reason"),
    [
        (lambda value: value.update({"unknown": True}), "record-shape"),
        (
            lambda value: value.update(
                {
                    "publication_status": "development-preview",
                    "non_publishable_reason": record.DEVELOPMENT_REASON,
                }
            ),
            "publication-status",
        ),
        (lambda value: value.update({"experience_id": "unknown"}), "experience-id"),
        (lambda value: value.update({"evidence_cases": []}), "evidence-cases"),
        (
            lambda value: value.update({"evidence_cases": ["fsi.unregistered"]}),
            "evidence-cases",
        ),
        (
            lambda value: value["lineage"].update({"run_digest": "f" * 63}),
            "lineage-complete",
        ),
        (
            lambda value: value["lineage"].update(
                {"model_digest": digest("foreign-model")}
            ),
            "protected-lineage",
        ),
        (
            lambda value: value["source"].update({"result_digest": digest("wrong")}),
            "source-lineage",
        ),
        (
            lambda value: value["claim"].update({"statement": "a wider claim"}),
            "claim-boundary",
        ),
        (lambda value: widen_non_claims(value), "claim-boundary"),
        (
            lambda value: reseal_scene(value, pressure_display_bound_pa=0.0),
            "scene-scales",
        ),
        (lambda value: add_segment_key(value), "frame-mapping"),
        (
            lambda value: value["outputs"]["film_fallback"].update(
                {"media_type": 'video/webm; codecs="vp9"'}
            ),
            "media-types",
        ),
        (
            lambda value: value["outputs"]["film_modern"].update(
                {"sha256": digest("substituted")}
            ),
            "output-digests",
        ),
        (
            lambda value: value["outputs"]["film_modern"].update({"codec": "h265"}),
            "output-dimensions",
        ),
        (
            lambda value: reseal_encoder_without_threads(value),
            "encoder-profile",
        ),
        (
            lambda value: reseal_encoder_with_conflicting_threads(value),
            "encoder-profile",
        ),
        (lambda value: reseal_environment(value, tz="local"), "environment-identity"),
        (
            lambda value: copy_poster_digest_to_reduced(value),
            "accessibility-assets",
        ),
        (
            lambda value: reseal_scene(value, watermark="vorticity preview"),
            "derived-observable",
        ),
    ],
)
def test_one_field_mutants_fail_closed(mutate, reason: str) -> None:
    candidate, context, facts = fixture()
    mutate(candidate)
    assert reason in outcome(candidate, context, facts).reasons


def test_interpolation_cannot_be_relabelled_as_sampled() -> None:
    candidate, context, facts = fixture()
    scene = candidate["scene"]
    scene["interpolation"]["kind"] = "sampled"
    scene["profile_sha256"] = record.content_digest(scene, "profile_sha256")
    assert "frame-mapping" in outcome(candidate, context, facts).reasons


def test_text_alternative_cannot_drop_units() -> None:
    candidate, context, facts = fixture()
    text = TEXT.replace("Fluid pressure [Pa]", "Fluid pressure")
    text_digest = record.sha256_bytes(text.encode("utf-8"))
    candidate["outputs"]["text_alternative"]["sha256"] = text_digest
    candidate["outputs"]["text_alternative"]["bytes"] = len(text.encode("utf-8"))
    candidate["accessibility"]["description_sha256"] = text_digest
    facts["text_alternative"] = record.FileFact(text_digest, len(text.encode("utf-8")))
    assert "text-alternative" in outcome(candidate, context, facts, text).reasons


def test_canonical_json_is_stable_under_key_reordering() -> None:
    assert record.canonical_bytes({"b": 2, "a": 1}) == record.canonical_bytes(
        {"a": 1, "b": 2}
    )


def reseal_scene(candidate: dict[str, object], **changes: object) -> None:
    scene = candidate["scene"]
    scene.update(changes)
    scene["profile_sha256"] = record.content_digest(scene, "profile_sha256")


def add_segment_key(candidate: dict[str, object]) -> None:
    scene = candidate["scene"]
    scene["segments"][0]["unknown"] = True
    scene["profile_sha256"] = record.content_digest(scene, "profile_sha256")


def widen_non_claims(candidate: dict[str, object]) -> None:
    candidate["claim"]["non_claims"].append("an uncontracted escape hatch")


def reseal_encoder_without_threads(candidate: dict[str, object]) -> None:
    encoder = candidate["encoder"]
    argv = encoder["webm_argv"]
    position = argv.index("-threads")
    del argv[position : position + 2]
    encoder["profile_sha256"] = record.content_digest(encoder, "profile_sha256")


def reseal_encoder_with_conflicting_threads(candidate: dict[str, object]) -> None:
    encoder = candidate["encoder"]
    encoder["webm_argv"].extend(["-threads", "4"])
    encoder["profile_sha256"] = record.content_digest(encoder, "profile_sha256")


def reseal_environment(candidate: dict[str, object], **changes: object) -> None:
    environment = candidate["environment"]
    environment.update(changes)
    environment["profile_sha256"] = record.content_digest(environment, "profile_sha256")


def copy_poster_digest_to_reduced(candidate: dict[str, object]) -> None:
    outputs = candidate["outputs"]
    outputs["reduced_motion_still"]["sha256"] = outputs["poster"]["sha256"]
