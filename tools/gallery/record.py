"""Private build record and production admission for one gallery-media slice."""

from __future__ import annotations

import hashlib
import json
import math
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


RECORD_KIND = "eqiora.gallery.private-fixed-reference-fsi-media"
RECORD_VERSION = 1
DEVELOPMENT_ENTRY_ID = "fixed-reference-fsi-development-film"
DEVELOPMENT_EXPERIENCE_ID = "fixed-reference-fsi-development"
DEVELOPMENT_REASON = (
    "development fixture over a bounded accepted result; not a contracted public "
    "gallery experience"
)
CLAIM_STATEMENT = (
    "One installed-Python composition renders the accepted two-state fixed-reference "
    "FSI result into a deterministic 15-second development media bundle with complete "
    "private provenance and accessibility assets."
)
NON_CLAIMS = (
    "not a gallery entry, publication, or flagship experience",
    "not scientific evidence or validation",
    "no new scientific observable is derived; pressure is presented and solid "
    "displacement is labelled presentation geometry",
    "presentation-interpolated frames are not solved dynamics",
    "no general animation, export, video, or scene-graph capability",
    "no public Rust item, artifact, wire, schema, registry, Python API, or wheel extra",
    "encoded bytes reproduce only inside the recorded producer and encoder profile",
    "no cross-platform byte identity, pixel oracle, or site autoplay wiring claim",
)
QUANTITIES = {
    "fluid_pressure": "Pa",
    "solid_displacement": "m",
    "physical_time": "s",
}
DOSSIER_ROUTE = (
    "verify/interfaces/python-fixed-reference-fsi-gallery-build/README.md"
)
WATERMARK = "DEVELOPMENT PREVIEW — NOT PUBLISHABLE"
DELIVERY_REQUIREMENT = (
    "replace motion with the reduced-motion still when prefers-reduced-motion is reduce"
)
FLAGSHIP_EXPERIENCE_IDS = frozenset(
    {
        "cylinder-wake",
        "shell-collapse",
        "turek-hron-fsi3",
        "stokes-shape-optimization",
        "taylor-green-3d",
        "notched-plate-fracture",
        "dam-break-obstacle",
        "electric-motor",
    }
)
OUTPUT_CONTRACTS = {
    "poster": ("dev-poster.png", "image/png"),
    "film_modern": ("dev-film.webm", 'video/webm; codecs="vp9"'),
    "film_fallback": ("dev-film.mp4", 'video/mp4; codecs="avc1.640028"'),
    "reduced_motion_still": ("dev-reduced-motion.png", "image/png"),
    "text_alternative": (
        "dev-text-alternative.txt",
        "text/plain; charset=utf-8",
    ),
}
EXPECTED_SEGMENTS = (
    ("poster-open", 0, 44, "accepted-state"),
    ("neutral-establish", 45, 89, "labelled-neutral-fade"),
    ("state-1-hold", 90, 149, "accepted-state"),
    ("presentation-blend", 150, 299, "presentation-interpolation"),
    ("state-2-hold", 300, 374, "accepted-state"),
    ("neutral-reset", 375, 419, "labelled-neutral-fade"),
    ("poster-return", 420, 449, "labelled-poster-return"),
)
FORBIDDEN_OBSERVABLE_TOKENS = (
    "vorticity",
    "stress",
    "force",
    "energy",
    "torque",
    "streamline",
    "magnitude",
    "q-criterion",
    "drag",
    "lift",
)
_DIGEST = re.compile(r"[0-9a-f]{64}")
_CASE_ID = re.compile(r"[a-z0-9-]+\.[a-z0-9-]+")
_WALL_CLOCK = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}")


@dataclass(frozen=True)
class FileFact:
    """Recomputed identity of one output file."""

    sha256: str
    bytes: int


@dataclass(frozen=True)
class ClaimContract:
    """Protected-base claim boundary for one private production candidate."""

    statement: str
    non_claims: tuple[str, ...]
    quantities: tuple[tuple[str, str], ...]
    evidence_cases: frozenset[str]
    dossier_route: str


@dataclass(frozen=True)
class ProductionContext:
    """Independent protected-base facts supplied to production admission."""

    protected_base_revision: str
    registered_case_ids: frozenset[str]
    lineage_by_run_digest: Mapping[str, str]
    contracts: Mapping[str, ClaimContract]


@dataclass(frozen=True)
class Admission:
    """Complete fail-closed admission outcome."""

    reasons: tuple[str, ...]

    @property
    def accepted(self) -> bool:
        """Return whether every production check passed."""

        return not self.reasons


def canonical_bytes(value: object) -> bytes:
    """Encode one private record value as deterministic UTF-8 JSON."""

    return (
        json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
        + "\n"
    ).encode("utf-8")


def sha256_bytes(payload: bytes) -> str:
    """Return the lowercase SHA-256 identity of bytes."""

    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    """Return the lowercase SHA-256 identity of one regular file."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def content_digest(value: Mapping[str, object], digest_key: str) -> str:
    """Digest a mapping after excluding its self-identity field."""

    return sha256_bytes(
        canonical_bytes({key: item for key, item in value.items() if key != digest_key})
    )


def lineage_digest(lineage: Mapping[str, object]) -> str:
    """Digest the complete private lineage projection."""

    return sha256_bytes(canonical_bytes(lineage))


def development_record(
    *,
    lineage: Mapping[str, object],
    source: Mapping[str, object],
    scene: Mapping[str, object],
    renderer: Mapping[str, object],
    encoder: Mapping[str, object],
    environment: Mapping[str, object],
    outputs: Mapping[str, object],
    accessibility: Mapping[str, object],
) -> dict[str, object]:
    """Construct the only real build-record variant this slice can emit."""

    evidence = list(_sequence(lineage.get("scientific_case_ids")))
    return {
        "record_kind": RECORD_KIND,
        "record_version": RECORD_VERSION,
        "entry_id": DEVELOPMENT_ENTRY_ID,
        "experience_id": DEVELOPMENT_EXPERIENCE_ID,
        "publication_status": "development-preview",
        "non_publishable_reason": DEVELOPMENT_REASON,
        "claim": {
            "statement": CLAIM_STATEMENT,
            "non_claims": list(NON_CLAIMS),
            "quantities": dict(QUANTITIES),
            "dossier_route": DOSSIER_ROUTE,
        },
        "evidence_cases": evidence,
        "lineage": dict(lineage),
        "source": dict(source),
        "scene": dict(scene),
        "renderer": dict(renderer),
        "encoder": dict(encoder),
        "environment": dict(environment),
        "outputs": dict(outputs),
        "accessibility": dict(accessibility),
    }


def admit(
    candidate: Mapping[str, object],
    *,
    context: ProductionContext,
    output_facts: Mapping[str, FileFact],
    text_alternative: str,
) -> Admission:
    """Evaluate every private production check without short-circuiting."""

    reasons: list[str] = []

    def reject(code: str) -> None:
        if code not in reasons:
            reasons.append(code)

    top_keys = {
        "record_kind",
        "record_version",
        "entry_id",
        "experience_id",
        "publication_status",
        "non_publishable_reason",
        "claim",
        "evidence_cases",
        "lineage",
        "source",
        "scene",
        "renderer",
        "encoder",
        "environment",
        "outputs",
        "accessibility",
    }
    if set(candidate) != top_keys:
        reject("record-shape")
    if (
        candidate.get("record_kind") != RECORD_KIND
        or candidate.get("record_version") != RECORD_VERSION
        or not _nonempty(candidate.get("entry_id"))
    ):
        reject("record-identity")

    if (
        candidate.get("publication_status") != "production"
        or candidate.get("non_publishable_reason") is not None
    ):
        reject("publication-status")
    experience_id = candidate.get("experience_id")
    contract = (
        context.contracts.get(experience_id)
        if isinstance(experience_id, str)
        else None
    )
    if experience_id not in FLAGSHIP_EXPERIENCE_IDS or contract is None:
        reject("experience-id")

    claim = _mapping(candidate.get("claim"))
    if set(claim) != {"statement", "non_claims", "quantities", "dossier_route"}:
        reject("claim-shape")
    if (
        not _nonempty(claim.get("statement"))
        or not _sequence(claim.get("non_claims"))
        or not _mapping(claim.get("quantities"))
        or not _nonempty(claim.get("dossier_route"))
    ):
        reject("claim-boundary")
    if contract is not None and (
        claim.get("statement") != contract.statement
        or tuple(_sequence(claim.get("non_claims"))) != contract.non_claims
        or tuple(sorted(_mapping(claim.get("quantities")).items()))
        != contract.quantities
        or claim.get("dossier_route") != contract.dossier_route
    ):
        reject("claim-boundary")

    evidence_cases = tuple(_sequence(candidate.get("evidence_cases")))
    if (
        not evidence_cases
        or any(
            not isinstance(case, str)
            or _CASE_ID.fullmatch(case) is None
            or case not in context.registered_case_ids
            for case in evidence_cases
        )
    ):
        reject("evidence-cases")
    if contract is not None and frozenset(evidence_cases) != contract.evidence_cases:
        reject("evidence-cases")

    lineage = _mapping(candidate.get("lineage"))
    _check_lineage(lineage, reject)
    if tuple(_sequence(lineage.get("scientific_case_ids"))) != evidence_cases:
        reject("evidence-cases")
    run_digest = lineage.get("run_digest")
    if (
        not isinstance(run_digest, str)
        or context.lineage_by_run_digest.get(run_digest) != lineage_digest(lineage)
    ):
        reject("protected-lineage")

    source = _mapping(candidate.get("source"))
    _check_source(source, lineage, context, reject)
    scene = _mapping(candidate.get("scene"))
    _check_scene(scene, reject)
    renderer = _mapping(candidate.get("renderer"))
    _check_renderer(renderer, reject)
    encoder = _mapping(candidate.get("encoder"))
    _check_encoder(encoder, reject)
    environment = _mapping(candidate.get("environment"))
    _check_environment(environment, reject)
    outputs = _mapping(candidate.get("outputs"))
    _check_outputs(outputs, output_facts, reject)
    accessibility = _mapping(candidate.get("accessibility"))
    _check_accessibility(accessibility, outputs, text_alternative, reject)

    scanned = canonical_bytes(
        {
            "scene": scene,
            "accessibility": accessibility,
            "text_alternative": text_alternative,
        }
    ).decode("utf-8").lower()
    if any(token in scanned for token in FORBIDDEN_OBSERVABLE_TOKENS):
        reject("derived-observable")
    whole_record = canonical_bytes(candidate).decode("utf-8")
    if _WALL_CLOCK.search(whole_record) is not None or any(
        key in whole_record
        for key in ('"created_at"', '"timestamp"', '"built_at"', '"generated_at"')
    ):
        reject("wall-clock-provenance")
    return Admission(tuple(reasons))


def _check_lineage(lineage: Mapping[str, object], reject: Any) -> None:
    keys = {
        "model_digest",
        "semantic_revision",
        "geometry_digest",
        "correspondence_digest",
        "mesh_digest",
        "realization_digest",
        "realization_revision",
        "run_digest",
        "run_manifest_sha256",
        "state_digests",
        "trajectory_digest",
        "scientific_case_ids",
    }
    if set(lineage) != keys:
        reject("lineage-shape")
    digest_keys = keys - {
        "semantic_revision",
        "realization_revision",
        "state_digests",
        "scientific_case_ids",
    }
    state_digests = tuple(_sequence(lineage.get("state_digests")))
    if (
        any(not _is_digest(lineage.get(key)) for key in digest_keys)
        or len(state_digests) != 2
        or len(set(state_digests)) != 2
        or any(not _is_digest(value) for value in state_digests)
        or not _nonnegative_integer(lineage.get("semantic_revision"))
        or not _nonnegative_integer(lineage.get("realization_revision"))
    ):
        reject("lineage-complete")


def _check_source(
    source: Mapping[str, object],
    lineage: Mapping[str, object],
    context: ProductionContext,
    reject: Any,
) -> None:
    keys = {
        "build_revision",
        "build_script_path",
        "build_script_sha256",
        "record_module_path",
        "record_module_sha256",
        "scene_module_path",
        "scene_module_sha256",
        "model_source_sha256",
        "result_digest",
        "result_frame_input_sha256",
        "eqiora_version",
        "eqiora_module_is_installed",
    }
    if set(source) != keys:
        reject("source-shape")
    for key in (
        "build_script_sha256",
        "record_module_sha256",
        "scene_module_sha256",
        "model_source_sha256",
        "result_frame_input_sha256",
    ):
        if not _is_digest(source.get(key)):
            reject("source-digests")
    for key in ("build_script_path", "record_module_path", "scene_module_path"):
        value = source.get(key)
        if not isinstance(value, str) or Path(value).is_absolute() or ".." in Path(value).parts:
            reject("source-paths")
    if (
        source.get("build_revision") != context.protected_base_revision
        or source.get("result_digest") != lineage.get("run_digest")
        or source.get("eqiora_module_is_installed") is not True
        or not _nonempty(source.get("eqiora_version"))
    ):
        reject("source-lineage")


def _check_scene(scene: Mapping[str, object], reject: Any) -> None:
    keys = {
        "profile_id",
        "profile_sha256",
        "width",
        "height",
        "fps",
        "frame_count",
        "duration_s",
        "primary_quantity",
        "geometry_state",
        "pressure_display_bound_pa",
        "displacement_scale",
        "axis_limits",
        "physical_time",
        "segments",
        "interpolation",
        "frame_sequence_sha256",
        "fields_presented",
        "per_frame_autoranging",
        "watermark",
    }
    if set(scene) != keys:
        reject("scene-shape")
    if (
        scene.get("profile_id") != "fixed-reference-fsi-development-film/1"
        or scene.get("width") != 1280
        or scene.get("height") != 720
        or scene.get("fps") != 30
        or scene.get("frame_count") != 450
        or scene.get("duration_s") != 15.0
        or scene.get("primary_quantity") != "fluid_pressure_pa"
        or scene.get("geometry_state") != "solid_displacement_m_times_12"
        or scene.get("fields_presented")
        != ["fluid_pressure_pa", "solid_displacement_geometry_m"]
        or scene.get("per_frame_autoranging") is not False
        or scene.get("watermark") != WATERMARK
    ):
        reject("scene-contract")
    if not _is_digest(scene.get("profile_sha256")) or scene.get(
        "profile_sha256"
    ) != content_digest(scene, "profile_sha256"):
        reject("scene-digest")
    if (
        not _positive_finite(scene.get("pressure_display_bound_pa"))
        or scene.get("displacement_scale") != 12.0
    ):
        reject("scene-scales")
    limits = _mapping(scene.get("axis_limits"))
    if set(limits) != {"x", "y"} or any(
        not _ordered_finite_pair(limits.get(axis)) for axis in ("x", "y")
    ):
        reject("scene-scales")
    physical_time = _mapping(scene.get("physical_time"))
    t1 = physical_time.get("state_1_time_s")
    t2 = physical_time.get("state_2_time_s")
    if (
        set(physical_time)
        != {
            "state_1_time_s",
            "state_2_time_s",
            "interval_s",
            "presentation_time_is_not_physical_time",
        }
        or not _finite(t1)
        or not _finite(t2)
        or not float(t1) < float(t2)
        or physical_time.get("interval_s") != float(t2) - float(t1)
        or physical_time.get("presentation_time_is_not_physical_time") is not True
    ):
        reject("physical-time")
    segments = tuple(_sequence(scene.get("segments")))
    observed_segments = tuple(
        (
            _mapping(segment).get("name"),
            _mapping(segment).get("first_frame"),
            _mapping(segment).get("last_frame"),
            _mapping(segment).get("kind"),
        )
        for segment in segments
    )
    if observed_segments != EXPECTED_SEGMENTS or any(
        _mapping(segment).get("frame_count")
        != _mapping(segment).get("last_frame")
        - _mapping(segment).get("first_frame")
        + 1
        for segment in segments
        if isinstance(_mapping(segment).get("last_frame"), int)
        and isinstance(_mapping(segment).get("first_frame"), int)
    ):
        reject("frame-mapping")
    interpolation = _mapping(scene.get("interpolation"))
    if interpolation != {
        "kind": "presentation-only-linear",
        "fields": ["fluid_pressure_pa", "solid_displacement_m"],
        "first_frame": 150,
        "last_frame": 299,
        "tau_rule": "(frame-150)/150",
        "label": "PRESENTATION INTERPOLATION t1 → t2 — not solved dynamics",
    }:
        reject("frame-mapping")
    if not _is_digest(scene.get("frame_sequence_sha256")):
        reject("frame-sequence")


def _check_renderer(renderer: Mapping[str, object], reject: Any) -> None:
    if set(renderer) != {
        "identity",
        "matplotlib_version",
        "backend",
        "module_sha256",
        "png_metadata",
    }:
        reject("renderer-shape")
    if (
        renderer.get("identity") != "eqiora.gallery.private-fsi-renderer/1"
        or renderer.get("backend") != "Agg"
        or renderer.get("png_metadata")
        != {"Software": "Eqiora private gallery renderer/1"}
        or not _nonempty(renderer.get("matplotlib_version"))
        or not _is_digest(renderer.get("module_sha256"))
    ):
        reject("renderer-identity")


def _check_encoder(encoder: Mapping[str, object], reject: Any) -> None:
    keys = {
        "ffmpeg_path",
        "ffmpeg_sha256",
        "ffmpeg_version",
        "ffmpeg_configuration",
        "ffprobe_path",
        "ffprobe_sha256",
        "webm_argv",
        "mp4_argv",
        "profile_sha256",
    }
    if set(encoder) != keys:
        reject("encoder-shape")
    if (
        not _absolute_path(encoder.get("ffmpeg_path"))
        or not _absolute_path(encoder.get("ffprobe_path"))
        or not _is_digest(encoder.get("ffmpeg_sha256"))
        or not _is_digest(encoder.get("ffprobe_sha256"))
        or not _nonempty(encoder.get("ffmpeg_version"))
        or not _nonempty(encoder.get("ffmpeg_configuration"))
        or not _is_digest(encoder.get("profile_sha256"))
        or encoder.get("profile_sha256") != content_digest(encoder, "profile_sha256")
    ):
        reject("encoder-identity")
    for key, codec in (("webm_argv", "libvpx-vp9"), ("mp4_argv", "libx264")):
        argv = tuple(_sequence(encoder.get(key)))
        required = (
            "-an",
            "-threads",
            "1",
            "-fps_mode",
            "cfr",
            "-bitexact",
            "-map_metadata",
            "-1",
            codec,
        )
        if not argv or any(value not in argv for value in required) or "-metadata" in argv:
            reject("encoder-profile")


def _check_environment(environment: Mapping[str, object], reject: Any) -> None:
    keys = {
        "platform",
        "machine",
        "python_version",
        "eqiora_version",
        "numpy_version",
        "matplotlib_version",
        "source_date_epoch",
        "tz",
        "locale",
        "pythonhashseed",
        "profile_sha256",
    }
    if set(environment) != keys:
        reject("environment-shape")
    if (
        any(
            not _nonempty(environment.get(key))
            for key in (
                "platform",
                "machine",
                "python_version",
                "eqiora_version",
                "numpy_version",
                "matplotlib_version",
            )
        )
        or environment.get("source_date_epoch") != 0
        or environment.get("tz") != "UTC"
        or environment.get("locale") != "C"
        or environment.get("pythonhashseed") != "0"
        or not _is_digest(environment.get("profile_sha256"))
        or environment.get("profile_sha256")
        != content_digest(environment, "profile_sha256")
    ):
        reject("environment-identity")


def _check_outputs(
    outputs: Mapping[str, object],
    facts: Mapping[str, FileFact],
    reject: Any,
) -> None:
    if set(outputs) != set(OUTPUT_CONTRACTS) or set(facts) != set(OUTPUT_CONTRACTS):
        reject("outputs-complete")
    observed_digests: list[str] = []
    for role, (filename, media_type) in OUTPUT_CONTRACTS.items():
        output = _mapping(outputs.get(role))
        fact = facts.get(role)
        if set(output) != {
            "filename",
            "media_type",
            "sha256",
            "bytes",
            "width",
            "height",
            "duration_s",
            "fps",
            "frame_count",
            "has_audio",
            "stream_count",
            "codec",
        }:
            reject("outputs-complete")
        if output.get("filename") != filename or output.get("media_type") != media_type:
            reject("media-types")
        digest = output.get("sha256")
        if not _is_digest(digest) or fact is None or digest != fact.sha256:
            reject("output-digests")
        elif isinstance(digest, str):
            observed_digests.append(digest)
        if (
            fact is None
            or output.get("bytes") != fact.bytes
            or not isinstance(output.get("bytes"), int)
            or output.get("bytes", 0) <= 0
        ):
            reject("output-digests")
        if role in {"poster", "reduced_motion_still"}:
            if (
                output.get("width") != 1280
                or output.get("height") != 720
                or any(
                    output.get(key) is not None
                    for key in (
                        "duration_s",
                        "fps",
                        "frame_count",
                        "stream_count",
                        "codec",
                    )
                )
                or output.get("has_audio") is not False
            ):
                reject("output-dimensions")
        elif role.startswith("film_"):
            expected_codec = "vp9" if role == "film_modern" else "h264"
            if (
                output.get("width") != 1280
                or output.get("height") != 720
                or output.get("duration_s") != 15.0
                or output.get("fps") != 30
                or output.get("frame_count") != 450
                or output.get("has_audio") is not False
                or output.get("stream_count") != 1
                or output.get("codec") != expected_codec
            ):
                reject("output-dimensions")
        elif any(
            output.get(key) is not None
            for key in (
                "width",
                "height",
                "duration_s",
                "fps",
                "frame_count",
                "stream_count",
                "codec",
            )
        ) or output.get("has_audio") is not False:
            reject("output-dimensions")
    if len(observed_digests) != len(set(observed_digests)):
        reject("output-digests")


def _check_accessibility(
    accessibility: Mapping[str, object],
    outputs: Mapping[str, object],
    text: str,
    reject: Any,
) -> None:
    if set(accessibility) != {
        "reduced_motion_still",
        "text_alternative",
        "delivery_requirement",
        "dossier_route",
        "description_sha256",
    }:
        reject("accessibility-shape")
    poster = _mapping(outputs.get("poster"))
    reduced = _mapping(outputs.get("reduced_motion_still"))
    text_output = _mapping(outputs.get("text_alternative"))
    if (
        accessibility.get("reduced_motion_still") != reduced.get("filename")
        or accessibility.get("text_alternative") != text_output.get("filename")
        or accessibility.get("delivery_requirement") != DELIVERY_REQUIREMENT
        or accessibility.get("dossier_route") != DOSSIER_ROUTE
        or accessibility.get("description_sha256") != sha256_bytes(text.encode("utf-8"))
        or accessibility.get("description_sha256") != text_output.get("sha256")
        or reduced.get("sha256") == poster.get("sha256")
    ):
        reject("accessibility-assets")
    required_text = (
        WATERMARK,
        "Fluid pressure [Pa]",
        "solid displacement [m]",
        "×12",
        "accepted step 1",
        "accepted step 2",
        "not solved dynamics",
        "physics is never played backwards",
        "fsi.fixed-reference-monolithic-step-2d",
        "artifacts.fixed-reference-fsi-spatial-trajectory",
        "Run digest",
    )
    normalized_text = " ".join(text.split())
    if len(text) < 400 or any(
        fragment not in normalized_text for fragment in required_text
    ):
        reject("text-alternative")


def _mapping(value: object) -> Mapping[str, Any]:
    return value if isinstance(value, Mapping) else {}


def _sequence(value: object) -> Sequence[Any]:
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return value
    return ()


def _is_digest(value: object) -> bool:
    return isinstance(value, str) and _DIGEST.fullmatch(value) is not None


def _nonempty(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _nonnegative_integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _finite(value: object) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def _positive_finite(value: object) -> bool:
    return _finite(value) and float(value) > 0.0


def _ordered_finite_pair(value: object) -> bool:
    pair = _sequence(value)
    return len(pair) == 2 and _finite(pair[0]) and _finite(pair[1]) and pair[0] < pair[1]


def _absolute_path(value: object) -> bool:
    return isinstance(value, str) and Path(value).is_absolute()
