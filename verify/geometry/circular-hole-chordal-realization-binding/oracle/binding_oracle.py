#!/usr/bin/env python3
r"""Minimal independent oracle for the versioned chordal-realization binding.

It owns the closed thirteen-field canonical encoding of
``eqiora.circular-hole-chordal-realization-envelope/v1`` and the relational
replay contract binding it to resources it does not build. It does not rederive
circle sampling, trigonometric metrics, geometry vertices, mesh topology,
correspondence assignments, or any actual resource digest: those are separate
component contracts, whose oracles are pinned in :data:`UPSTREAM` and never
copied or executed here. The only frozen identity below is an artificial
encoding witness whose every field is chosen; it must never be wired as a
positive oracle for a real realization. See ``../README.md``.

Run ``python3 <this file>``; ``--emit`` rewrites the frozen fixture.
"""

from __future__ import annotations

import hashlib
import json
import math
import pathlib
import sys

CASE = pathlib.Path(__file__).resolve().parent.parent
REPO = CASE.parents[2]
FIXTURE = CASE / "expected" / "binding-contract.json"
EXPECTED_DOC = CASE / "expected" / "README.md"

ENVELOPE_SCHEMA = "eqiora.circular-hole-chordal-realization-envelope/v1"
ENCODING = "eqiora.canonical-json/v1"

FIELD_ORDER = tuple(
    "schema encoding source_geometry_sha256 realized_geometry_sha256 mesh_sha256"
    " correspondence_sha256 requested_max_boundary_error_m boundary_evaluation_allowance_m"
    " boundary_error_bound_m circle_segments circle_area_deficit_m2"
    " circle_perimeter_deficit_m required_minimum_mean_ratio".split()
)
DIGEST_FIELDS = FIELD_ORDER[2:6]
FLOAT_FIELDS = tuple(f for f in FIELD_ORDER[6:] if f != "circle_segments")

# Upstream component oracles, named as authorities and never re-executed here.
UPSTREAM = {
    "verify/geometry/exact-circular-hole-geometry/oracle.py": "df423b7848833e2667d8a064542c9adbd88543f054a2d364b90124393cf20d19",
    "verify/geometry/circular-hole-chordal-reference-mesh/oracle.py": "0bdbbec6f9ff9c532ba5f30c856d1cd3b25e64949e4b11abf5fa3823e6a25742",
}

# Every value below is chosen by this lane: four syntactically valid but
# obviously artificial digest slots, and exact powers of two whose compact JSON
# spelling is a short plain decimal in any shortest-round-trip renderer.
WITNESS = {
    "schema": ENVELOPE_SCHEMA,
    "encoding": ENCODING,
    "source_geometry_sha256": "5a" * 32,
    "realized_geometry_sha256": "6b" * 32,
    "mesh_sha256": "7c" * 32,
    "correspondence_sha256": "8d" * 32,
    "requested_max_boundary_error_m": 0.0625,
    "boundary_evaluation_allowance_m": 0.00390625,
    "boundary_error_bound_m": 0.03125,
    "circle_segments": 12,
    "circle_area_deficit_m2": 0.015625,
    "circle_perimeter_deficit_m": 0.125,
    "required_minimum_mean_ratio": 0.5,
}
IS_REALIZATION_PREDICTION = False


class ContractError(ValueError):
    """The artificial witness violates the frozen wire contract."""


def canonical_json(values: dict) -> bytes:
    """Encode the chosen dyadic witness family, not arbitrary runtime floats."""
    keys = tuple(values)
    if set(keys) != set(FIELD_ORDER):
        raise ContractError(f"vocabulary {sorted(set(keys) ^ set(FIELD_ORDER))}")
    if keys != FIELD_ORDER:
        raise ContractError("fields are not in canonical order")
    if values["schema"] != ENVELOPE_SCHEMA or values["encoding"] != ENCODING:
        raise ContractError("unsupported schema or canonical-encoding identifier")
    for field in DIGEST_FIELDS:
        digest = values[field]
        if (
            not isinstance(digest, str)
            or len(digest) != 64
            or any(character not in "0123456789abcdef" for character in digest)
        ):
            raise ContractError(f"{field} is not lowercase SHA-256")
    for field in FLOAT_FIELDS:
        value = values[field]
        if not isinstance(value, float) or not math.isfinite(value) or value <= 0.0:
            raise ContractError(f"{field} is not finite and positive")
    segments = values["circle_segments"]
    if isinstance(segments, bool) or not isinstance(segments, int) or segments < 8:
        raise ContractError("circle_segments is not an integer of at least eight")
    return json.dumps(values, separators=(",", ":"), allow_nan=False).encode()


def identity(payload: bytes) -> str:
    """RFC 0008 framing: sha256(schema-domain || 0x00 || canonical bytes)."""
    return hashlib.sha256(ENVELOPE_SCHEMA.encode() + b"\x00" + payload).hexdigest()


def variant(**changes: object) -> dict:
    """A copy of the witness with fields replaced in place, preserving order."""
    return {**WITNESS, **changes}


def simple_dyadic(value: object) -> bool:
    """True for a positive exact power of two with an unambiguous plain spelling."""
    if not isinstance(value, float) or not math.isfinite(value) or value <= 0.0:
        return False
    text = json.dumps(value)
    plain = "e" not in text and "E" not in text and float(text) == value
    return plain and math.frexp(value)[0] == 0.5


def artificial_slot(digest: str) -> bool:
    """True for a synthetic repeated-pair sentinel not copied from a resource."""
    hexish = len(digest) == 64 and all(c in "0123456789abcdef" for c in digest)
    return hexish and digest == digest[:2] * 32


def check(name: str, ok: object, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def rejected(values: dict) -> bool:
    try:
        canonical_json(values)
    except (ContractError, TypeError, ValueError):
        return True
    return False


def raw_json(values: dict) -> bytes:
    """Serialize a deliberately invalid admission input without validation."""
    return json.dumps(values, separators=(",", ":"), allow_nan=True).encode()


def object_depth(value: object) -> int:
    """Small independent structural depth measure for the artificial witness."""
    if isinstance(value, dict):
        return 1 + max((object_depth(item) for item in value.values()), default=0)
    if isinstance(value, list):
        return 1 + max((object_depth(item) for item in value), default=0)
    return 0


def decode_pairs(pairs: list[tuple[str, object]]) -> dict:
    """Preserve field order and reject duplicate JSON names."""
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ContractError(f"duplicate field {key}")
        value[key] = item
    return value


def reject_constant(value: str) -> object:
    """Reject the non-standard NaN/Infinity tokens accepted by Python JSON."""
    raise ContractError(f"non-finite JSON token {value}")


def admit_artificial(
    payload: bytes,
    *,
    max_bytes: int = 4096,
    max_depth: int = 64,
) -> dict:
    """Admit only canonical bytes in the artificial dyadic witness domain."""
    if len(payload) > max_bytes:
        raise ContractError("encoded-byte budget exceeded")
    try:
        value = json.loads(
            payload,
            object_pairs_hook=decode_pairs,
            parse_constant=reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ContractError("invalid JSON") from error
    if not isinstance(value, dict):
        raise ContractError("top-level value is not an object")
    if object_depth(value) > max_depth:
        raise ContractError("nesting-depth budget exceeded")
    if canonical_json(value) != payload:
        raise ContractError("input is not the canonical encoding")
    return value


OPS = {
    "nibble": lambda v: f"{(int(v[0], 16) + 1) % 16:x}{v[1:]}",
    "halve": lambda v: v / 2,
    "increment": lambda v: v + 1,
    "decrement": lambda v: v - 1,
}

# The replay relation, frozen as a finite table: "<step> <phase> <requirement>".
# No runtime value is manufactured; each step names what must be regenerated, or
# compared against a resource the implementation was supplied.
REPLAY = (
    "k1 construction capture every field from already-validated resources",
    "k2 construction never accept a caller-supplied raw field tuple",
    "p admission decode the binding envelope under its closed canonical vocabulary and budgets, and require the four resources to have passed their separately owned bounded decoders before relational replay",
    "a validation resolve the exact source; its digest must equal source_geometry_sha256",
    "b validation regenerate the chordal owner from that source, the stored request, stored circle_segments as a maximum, and the stored required_minimum_mean_ratio",
    "c validation exact-compare every regenerated metric against the stored scalar",
    "d validation require the supplied realized geometry to equal the regenerated region",
    "e validation replay the Model-free authored-planar-region-v1 correspondence against the supplied realized geometry and mesh",
    "f validation exact-compare all four bound resource digests",
)
REQUIRED_STEPS = frozenset({"k1", "k2", "p", "a", "b", "c", "d", "e", "f"})

# The six detection axes, each mapped to the replay steps that carry it.
AXES = {
    "envelope_digest": ("envelope canonical digest", ()),
    "decoder_admission": ("binding-envelope decoder admission", ("p",)),
    "source_semantics": ("semantic source type/digest", ("a",)),
    "owner_replay": ("deterministic owner replay", ("b", "c")),
    "region_equality": ("realized-region equality", ("d",)),
    "authored_correspondence": ("authored-region correspondence replay", ("e",)),
    "resource_digest": ("bound resource digest", ("f",)),
}

# Single-field envelope mutations, "<id> <field> <op> <axes>". Each changes the
# canonical digest and breaks a replay relation. schema and encoding need no
# row: the vocabulary model rejects any other value outright.
ENVELOPE_MUTATIONS = (
    "source_digest_nibble source_geometry_sha256 nibble envelope_digest|source_semantics|resource_digest",
    "realized_digest_nibble realized_geometry_sha256 nibble envelope_digest|region_equality|resource_digest",
    "mesh_digest_nibble mesh_sha256 nibble envelope_digest|resource_digest",
    "correspondence_digest_nibble correspondence_sha256 nibble envelope_digest|authored_correspondence|resource_digest",
    "allowance_halved boundary_evaluation_allowance_m halve envelope_digest|owner_replay",
    "bound_halved boundary_error_bound_m halve envelope_digest|owner_replay",
    "segments_above circle_segments increment envelope_digest|owner_replay",
    "segments_below circle_segments decrement envelope_digest|owner_replay",
    "area_deficit_halved circle_area_deficit_m2 halve envelope_digest|owner_replay",
    "perimeter_deficit_halved circle_perimeter_deficit_m halve envelope_digest|owner_replay",
)

# Replayed policy inputs with no regenerated counterpart. Changing one alone
# changes the canonical digest; replay rejection is not claimed, because whether
# the regenerated selection changes is owned by the upstream chordal contract.
POLICY_CHANGES = (
    "request_halved requested_max_boundary_error_m halve envelope_digest",
    "mean_ratio_halved required_minimum_mean_ratio halve envelope_digest",
)

# Pre-replay malformed inputs, "<id> <axes>". These have no admitted envelope
# identity and must be rejected before any resource relation is evaluated.
ADMISSION_FALSIFIERS = (
    "unknown_vocabulary decoder_admission",
    "missing_vocabulary decoder_admission",
    "reordered_vocabulary decoder_admission",
    "extra_vocabulary decoder_admission",
    "noncanonical_json decoder_admission",
    "malformed_digest decoder_admission",
    "nonfinite_scalar decoder_admission",
    "zero_scalar decoder_admission",
    "negative_scalar decoder_admission",
    "byte_budget_overflow decoder_admission",
    "depth_budget_overflow decoder_admission",
)
REQUIRED_ADMISSION_CLASSES = frozenset(
    {
        "unknown_vocabulary",
        "missing_vocabulary",
        "reordered_vocabulary",
        "extra_vocabulary",
        "noncanonical_json",
        "malformed_digest",
        "nonfinite_scalar",
        "zero_scalar",
        "negative_scalar",
        "byte_budget_overflow",
        "depth_budget_overflow",
    }
)

# Substitutions inside a bound resource, "<id> <axes>". The envelope bytes are
# unchanged, so the canonical digest never catches these; only replay does.
SUBSTITUTIONS = (
    "source_center_perturbed source_semantics|owner_replay",
    "source_radius_perturbed source_semantics|owner_replay",
    "source_boundary_identity source_semantics",
    "polygonal_source_same_name source_semantics",
    "realized_vertex_perturbed region_equality|resource_digest",
    "realized_order_rotated region_equality|resource_digest",
    "mesh_refined authored_correspondence|resource_digest",
    "mesh_renumbered authored_correspondence|resource_digest",
    "mesh_topology_changed authored_correspondence|resource_digest",
    "correspondence_relabelled authored_correspondence|resource_digest",
    "correspondence_incomplete authored_correspondence|resource_digest",
    "correspondence_reoriented authored_correspondence|resource_digest",
    "correspondence_stale authored_correspondence|resource_digest",
    "correspondence_inlet_outlet_swapped authored_correspondence|resource_digest",
    "correspondence_exterior_hole_swapped authored_correspondence|resource_digest",
    "conforming_mesh_substituted resource_digest",
)
REQUIRED_SUBSTITUTION_CLASSES = frozenset(
    {
        "source_center_perturbed",
        "source_radius_perturbed",
        "source_boundary_identity",
        "polygonal_source_same_name",
        "realized_vertex_perturbed",
        "realized_order_rotated",
        "mesh_refined",
        "mesh_renumbered",
        "mesh_topology_changed",
        "correspondence_relabelled",
        "correspondence_incomplete",
        "correspondence_reoriented",
        "correspondence_stale",
        "correspondence_inlet_outlet_swapped",
        "correspondence_exterior_hole_swapped",
        "conforming_mesh_substituted",
    }
)

# An artificial encoding variant. No resource relation is evaluated here.
COHERENT_ID = "coherent_policy_variant"
COHERENT = variant(
    requested_max_boundary_error_m=0.03125,
    boundary_error_bound_m=0.015625,
    circle_segments=16,
    circle_area_deficit_m2=0.0078125,
    circle_perimeter_deficit_m=0.0625,
)

MUTATIONS = tuple(r.split() for r in ENVELOPE_MUTATIONS + POLICY_CHANGES)
ADMISSIONS = tuple(r.split() for r in ADMISSION_FALSIFIERS)
SUBS = tuple(r.split() for r in SUBSTITUTIONS)
ROWS = (
    tuple((r[0], r[3]) for r in MUTATIONS)
    + tuple((r[0], r[1]) for r in ADMISSIONS)
    + tuple((r[0], r[1]) for r in SUBS)
)
AXES_OF = {name: axes.split("|") for name, axes in ROWS}
CHECKS: list[tuple[str, bool, str]] = []

BYTES = canonical_json(WITNESS)
TEXT = BYTES.decode()
SHA256 = identity(BYTES)


def admission_rejects(case_id: str) -> bool:
    """Execute one malformed artificial-wire class without production code."""
    max_bytes = 4096
    max_depth = 64
    if case_id == "unknown_vocabulary":
        payloads = (
            raw_json(variant(schema="eqiora.other/v1")),
            raw_json(variant(encoding="raw-json/v1")),
        )
    elif case_id == "missing_vocabulary":
        payloads = (raw_json({k: v for k, v in WITNESS.items() if k != "mesh_sha256"}),)
    elif case_id == "reordered_vocabulary":
        payloads = (raw_json({k: WITNESS[k] for k in FIELD_ORDER[::-1]}),)
    elif case_id == "extra_vocabulary":
        payloads = (raw_json({**WITNESS, "extra": 1}),)
    elif case_id == "noncanonical_json":
        payloads = (b" " + BYTES,)
    elif case_id == "malformed_digest":
        payloads = tuple(
            raw_json(variant(**{field: "not-a-lowercase-sha256"}))
            for field in DIGEST_FIELDS
        )
    elif case_id == "nonfinite_scalar":
        payloads = tuple(
            raw_json(variant(**{field: float("nan")})) for field in FLOAT_FIELDS
        )
    elif case_id == "zero_scalar":
        payloads = tuple(
            raw_json(variant(**{field: 0.0})) for field in FLOAT_FIELDS
        ) + (raw_json(variant(circle_segments=0)),)
    elif case_id == "negative_scalar":
        payloads = tuple(
            raw_json(variant(**{field: -abs(WITNESS[field])})) for field in FLOAT_FIELDS
        ) + (raw_json(variant(circle_segments=-1)),)
    elif case_id == "byte_budget_overflow":
        payloads = (BYTES,)
        max_bytes = len(BYTES) - 1
    elif case_id == "depth_budget_overflow":
        payloads = (BYTES,)
        max_depth = 0
    else:
        raise AssertionError(f"unknown admission class {case_id}")

    for payload in payloads:
        try:
            admit_artificial(payload, max_bytes=max_bytes, max_depth=max_depth)
        except ContractError:
            continue
        return False
    return True


# 1. field order, checked directly and again against the emitted bytes.
_pos = [TEXT.find(f'"{name}":') for name in FIELD_ORDER]
_order = tuple(WITNESS) == FIELD_ORDER and len(set(FIELD_ORDER)) == 13
check("order.thirteen_unique_fields_in_frozen_order", _order)
check("order.bytes_ordered", all(p >= 0 for p in _pos) and _pos == sorted(_pos))
# 2. the schema vocabulary model rejects what it must.
_missing = {k: v for k, v in WITNESS.items() if k != "mesh_sha256"}
check("model.rejects_unknown_field", rejected({**WITNESS, "extra": 1}))
check("model.rejects_missing_field", rejected(_missing))
check("model.rejects_reordered", rejected({k: WITNESS[k] for k in FIELD_ORDER[::-1]}))
check("model.rejects_unknown_schema", rejected(variant(schema="eqiora.other/v1")))
check("model.rejects_unknown_encoding", rejected(variant(encoding="raw-json/v1")))
# 3. the witness values are artificial, in range, and unambiguously spelled.
_slots = [WITNESS[f] for f in DIGEST_FIELDS]
_artificial = all(artificial_slot(d) for d in _slots)
_dyadic = all(simple_dyadic(WITNESS[f]) for f in FLOAT_FIELDS)
_segments = WITNESS["circle_segments"]
_request = WITNESS["requested_max_boundary_error_m"]
_bound = WITNESS["boundary_error_bound_m"]
_allowance = WITNESS["boundary_evaluation_allowance_m"]
_ordered = _bound <= _request and _allowance < _request
_parsed = json.loads(TEXT)
check("witness.four_distinct_digest_slots", len(set(_slots)) == 4)
check("witness.slots_valid_synthetic_sentinels", _artificial)
check("witness.scalars_simple_dyadic", _dyadic)
check("witness.segments_min_eight", isinstance(_segments, int) and _segments >= 8)
check("witness.bound_and_allowance_within_request", _ordered)
check("witness.round_trips", _parsed == WITNESS and tuple(_parsed) == FIELD_ORDER)
check("witness.domain_separated", SHA256 != hashlib.sha256(BYTES).hexdigest())
check(
    "witness.not_a_realization_prediction_declared", IS_REALIZATION_PREDICTION is False
)
# 4. the replay contract is complete and reachable from the detection axes.
_steps = [r.split(" ", 2) for r in REPLAY]
_carried = {step for _, steps in AXES.values() for step in steps}
_checked = {s[0] for s in _steps if s[1] in {"admission", "validation"}}
_construction = {s[0] for s in _steps if s[1] == "construction"}
check("replay.steps_complete", frozenset(s[0] for s in _steps) == REQUIRED_STEPS)
check("replay.construction_captures_and_refuses_tuple", _construction == {"k1", "k2"})
check("replay.every_admission_and_validation_step_has_an_axis", _carried == _checked)
# 5. coverage: every mutable field and Issue class, with every detector axis used.
_mutable = set(FIELD_ORDER) - set(FIELD_ORDER[:2])
_admission_ids = [r[0] for r in ADMISSIONS]
_sub_ids = [r[0] for r in SUBS]
_mut_axes = [AXES_OF[r[0]] for r in MUTATIONS]
_admission_axes = [AXES_OF[i] for i in _admission_ids]
_sub_axes = [AXES_OF[i] for i in _sub_ids]
_known = all(
    axes and set(axes) <= set(AXES) for axes in _sub_axes + _admission_axes + _mut_axes
)
_used = {a for axes in AXES_OF.values() for a in axes}
_unknown_axes = _used - set(AXES)
_no_digest = all("envelope_digest" not in a for a in _sub_axes)
_two_axes = all("envelope_digest" in a and len(a) > 1 for a in _mut_axes[:10])
_only_digest = all(a == ["envelope_digest"] for a in _mut_axes[10:])
check("coverage.every_mutable_field_mutated", {r[1] for r in MUTATIONS} == _mutable)
check(
    "coverage.required_admission_classes_exact",
    frozenset(_admission_ids) == REQUIRED_ADMISSION_CLASSES,
)
check(
    "coverage.required_substitution_classes_exact",
    frozenset(_sub_ids) == REQUIRED_SUBSTITUTION_CLASSES,
)
check("coverage.row_names_unique", len(AXES_OF) == len(ROWS) == 39)
check("coverage.rows_declare_known_axes", _known, ",".join(sorted(_unknown_axes)))
check(
    "coverage.every_axis_exercised",
    _used == set(AXES),
    f"missing={','.join(sorted(set(AXES) - _used))}",
)
check("coverage.subs_never_claim_digest", _no_digest)
check("coverage.env_mutations_digest_and_replay", _two_axes)
check("coverage.policy_identity_only_in_oracle", _only_digest)
check(
    "coverage.admission_is_pre_replay_only",
    all(axes == ["decoder_admission"] for axes in _admission_axes),
)
# 6. every identity mutation and artificial admission falsifier is executed.
DIGESTS: dict[str, str] = {}
for _id, _field, _op, _axes in MUTATIONS:
    _values = variant(**{_field: OPS[_op](WITNESS[_field])})
    DIGESTS[_id] = identity(canonical_json(_values))
    check(f"mutation.{_id}", DIGESTS[_id] != SHA256, DIGESTS[_id])
check("mutation.twelve_distinct_identities", len(set(DIGESTS.values())) == 12)
ROLL = hashlib.sha256("\n".join(f"{k}={DIGESTS[k]}" for k in sorted(DIGESTS)).encode())
REPLAY_ROLL = hashlib.sha256("\n".join(REPLAY).encode())
for _id in _admission_ids:
    check(f"admission.{_id}", admission_rejects(_id))
# 7. one policy variant has a different encoding identity; replay is not evaluated.
COHERENT_BYTES = canonical_json(COHERENT)
COHERENT_SHA256 = identity(COHERENT_BYTES)
check("coherent.identity_differs", COHERENT_SHA256 != SHA256)
check("coherent.not_listed_as_a_falsifier", COHERENT_ID not in AXES_OF)
check("coherent.scalars_dyadic", all(simple_dyadic(COHERENT[f]) for f in FLOAT_FIELDS))
# 8. upstream authorities are pinned by digest, not re-executed.
for _path, _sha in UPSTREAM.items():
    _f = REPO / _path
    _seen = hashlib.sha256(_f.read_bytes()).hexdigest() if _f.is_file() else "absent"
    check(f"upstream.{pathlib.PurePath(_path).parent.name}", _seen == _sha, _seen)

CONTRACT = {
    "schema": ENVELOPE_SCHEMA,
    "encoding": ENCODING,
    "digest_framing": "sha256(schema-domain||0x00||canonical-json)",
    "canonical_field_order": list(FIELD_ORDER),
    "upstream_authorities": UPSTREAM,
    "artificial_encoding_witness": {
        "is_realization_prediction": IS_REALIZATION_PREDICTION,
        "exact_bytes_in": "expected/README.md",
        "canonical_bytes": len(BYTES),
        "sha256": SHA256,
    },
    "replay_contract": {
        "steps": [step.split(" ", 1)[0] for step in REPLAY],
        "sha256": REPLAY_ROLL.hexdigest(),
    },
    "detection_axes": {k: ",".join(v[1]) for k, v in AXES.items()},
    "envelope_mutations": {r[0]: r[3] for r in MUTATIONS[:10]},
    "policy_changes": {
        r[0]: [r[3], "outcome_owned_by_deterministic_regeneration"]
        for r in MUTATIONS[10:]
    },
    "admission_falsifiers": [r[0] for r in ADMISSIONS],
    "substitutions": {r[0]: r[1] for r in SUBS},
    "mutation_digest_roll": ROLL.hexdigest(),
    "not_a_falsifier": {
        "id": COHERENT_ID,
        "classification": "canonical_digest_change",
        "replay_outcome": "not_evaluated",
        "canonical_bytes": len(COHERENT_BYTES),
        "sha256": COHERENT_SHA256,
    },
}
SERIALIZED = json.dumps(CONTRACT, separators=(",", ":"), ensure_ascii=False) + "\n"

if "--emit" in sys.argv:
    FIXTURE.write_text(SERIALIZED, encoding="utf-8")
    sys.stdout.write(f"emitted {FIXTURE}\n")

_frozen = FIXTURE.read_text(encoding="utf-8") if FIXTURE.is_file() else None
_doc = EXPECTED_DOC.read_text(encoding="utf-8") if EXPECTED_DOC.is_file() else ""
_size = len(SERIALIZED.encode())
check("fixture.matches_derivation", _frozen == SERIALIZED)
check("fixture.within_size_limit", _size <= 4096, str(_size))
check("fixture.exact_bytes_frozen_in_expected_readme", TEXT in _doc)

_failed = [name for name, ok, _ in CHECKS if not ok]
for _name, _ok, _detail in CHECKS:
    if not _ok:
        sys.stdout.write(f"FAIL {_name} {_detail}\n")
for _key, _value in (
    ("oracle.field_order", ",".join(FIELD_ORDER)),
    ("witness.canonical_json", TEXT),
    ("witness.canonical_bytes", len(BYTES)),
    ("witness.sha256", SHA256),
    ("witness.is_realization_prediction", "false"),
    ("replay.steps", ",".join(s[0] for s in _steps)),
    ("replay.sha256", REPLAY_ROLL.hexdigest()),
    ("coverage.admission_rows", len(ADMISSIONS)),
    ("coverage.substitution_rows", len(SUBS)),
    ("coverage.total_rows", len(ROWS)),
    ("coverage.mutation_digest_roll", ROLL.hexdigest()),
    ("not_a_falsifier.classification", "canonical_digest_change"),
    ("not_a_falsifier.replay_outcome", "not_evaluated"),
    ("not_a_falsifier.sha256", COHERENT_SHA256),
    ("not_claimed.resource_digests", ",".join(DIGEST_FIELDS[1:])),
    ("fixture.bytes", _size),
    ("checks.total", len(CHECKS)),
    ("checks.failed", len(_failed)),
    ("oracle.result", "pass" if not _failed else "fail"),
):
    sys.stdout.write(f"{_key}={_value}\n")
sys.exit(0 if not _failed else 1)
