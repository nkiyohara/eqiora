#!/usr/bin/env python3
"""Independent precommitted oracle for Eqiora Issue #129.

Freezes which continuous Relation scopes onto an admitted `GeometryBoundary`
are accepted, which mutations reject and at which detector, and the exact
constant parent-outward normals the geometry owner may project.

Written by a non-implementing agent at base a5c122f before any production code
exists. It reads no implementation of the new capability and no canonical
artifact bytes: the fixture in `../expected/boundary-scope-contract.json` is an
artificial symbolic table whose handles the implementing lane binds to real
artifacts. Stdlib only.

Two routes must agree: the table enumerated in the frozen contract, transcribed
from the public claim in Issue #129, and the rule model below, which re-derives
every outcome, every mutation classification and both normals from the fixture.
"""

from __future__ import annotations

import hashlib
import itertools
import json
import math
import pathlib
import sys
import tomllib

ORACLE = pathlib.Path(__file__).resolve()
CASE = ORACLE.parents[1]
ROOT = ORACLE.parents[4]
CONTRACT = CASE / "expected" / "boundary-scope-contract.json"

# Freezing the fixture digest outside the fixture is what stops a later lane
# from widening the table and calling the widened table "the oracle".
CONTRACT_SHA256 = "6dc106a8fe682b998c05bfd03eb75240f8904e57e988cdde08f4ff21499706ac"

CIRCULAR_HOLE = "circular-hole-planar-v1"
UNFROZEN = "unfrozen"
PROPOSED_CASE_ID = "geometry.geometry-boundary-relation-scope"

# Pre-existing diagnostic sentences, transcribed as compatibility obligations
# rather than authored here. Issue #129 adds no diagnostic text of its own.
ABSENT = "geometry boundary entity set `{name}` is absent from its parent artifact"
DIM = "geometry boundary entity set `{name}` has dimension {found}, expected {want}"

# Digest of the sorted nonclaim slugs, so the required set survives a fixture
# regeneration that also refreshed CONTRACT_SHA256.
NONCLAIMS_SHA256 = "4ee6db5ef8435c42ac942b06fb25586ae394435997de83e463ad003de7696fdc"
NONCLAIM_COUNT = 10

# Exact binary64 spellings the projected components may take. A dyadic value
# has one canonical hex form, so a drifted constant cannot hide in decimals.
DYADIC = {-1.0: "-0x1.0000000000000p+0", 0.0: "0x0.0p+0", 1.0: "0x1.0000000000000p+0"}

BAD_SPELLINGS = ("[-1, 0]", "[1, 0]", "[0, -1]", "[0, 1]", "[-0.0", ", -0.0]")


class Failure(AssertionError):
    """One frozen expectation did not hold."""


def require(condition: object, message: str) -> None:
    if not condition:
        raise Failure(message)


def located(contract: dict, program: str) -> dict[str, str]:
    """Region identity -> artifact handle, for one program."""
    return {region[0]: region[1] for region in contract["programs"][program][2]}


def sets_of(contract: dict, handle: str) -> dict[str, list]:
    return {entry[0]: entry for entry in contract["artifacts"][handle]["sets"]}


def handle_of(contract: dict, row: dict) -> str:
    return located(contract, row["program"])[row["parent"]]


# --- route 2: the rule model -------------------------------------------------


def outward_normal(axis: int, side: str) -> list[float]:
    """Parent-outward unit normal of one axis-aligned rectangle side.

    Derived, not transcribed: the lower side of an axis faces that axis's
    negative direction and the upper side faces the positive one. Every other
    component is positive zero.
    """
    require(side in ("lower", "upper"), f"unknown side {side}")
    sign = -1.0 if side == "lower" else 1.0
    return [sign if index == axis else 0.0 for index in range(2)]


def reject(detector: str, diag: str) -> dict:
    return {
        "outcome": "reject",
        "detector": detector,
        "diag": diag,
        "normal": None,
        "reason": None,
    }


def project_normal(contract: dict, handle: str, name: str, members: list) -> tuple:
    """Single parent-outward normal of one admitted boundary set, or none.

    Necessary conditions run in the order that yields the mutation
    classification: family, then cardinality, then curvature. Only the two
    frozen pairs carry a value; any other set meeting every necessary condition
    is deliberately left unfrozen rather than turned into a catalogue.
    """
    catalog = {member[0]: member for member in contract["members"]}
    if contract["artifacts"][handle]["family"] != CIRCULAR_HOLE:
        return None, "family"
    if len(members) != 1:
        return None, "member-count"
    member = catalog[members[0]]
    if member[1] != "straight-axis-side":
        return None, "curved-member"
    for pair_handle, pair_name, frozen in contract["normal_rule"]["frozen_pairs"]:
        if (pair_handle, pair_name) != (handle, name):
            continue
        derived = outward_normal(member[2], member[3])
        require(derived == frozen, f"derived {derived} != frozen {frozen}")
        return derived, None
    return UNFROZEN, None


def decide(contract: dict, row: dict, bundle: list[str]) -> dict:
    """Re-derive one scenario's outcome from the fixture and the public claim.

    Detector order follows RFC 0080: Domain topology precedes bundle closure,
    which precedes parent-relative entity-set admission. The artifact-free
    entry path never reaches any of them.
    """
    admitted, _, regions = contract["programs"][row["program"]]
    if not admitted:
        return reject("support-use", "free-relation")
    if row["parent"] not in located(contract, row["program"]):
        return reject("topology", "parent-kind")
    if any(region[1] not in bundle for region in regions):
        return reject("bundle", "missing-artifact")

    handle = handle_of(contract, row)
    sets, name = sets_of(contract, handle), row["set"]
    if name not in sets:
        return reject("set-admission", f"absent-{name}")
    _, dimension, members = sets[name]
    if dimension != contract["dimensions"]["topological"] - 1:
        return reject("set-admission", f"dim-{name}")
    if row["consumer"].startswith("port-"):
        return reject("support-use", "port-embedding")

    normal, reason = project_normal(contract, handle, name, members)
    return {
        "outcome": "accept",
        "detector": "set-admission",
        "diag": None,
        "normal": normal,
        "reason": reason,
    }


# --- machine checks ----------------------------------------------------------


def check_digest(raw: bytes) -> str:
    actual = hashlib.sha256(raw).hexdigest()
    require(actual == CONTRACT_SHA256, f"digest {actual} != {CONTRACT_SHA256}")
    require(len(raw) <= 8192, f"fixture is {len(raw)} bytes, over its 8192 ceiling")
    return actual


def rows(contract: dict) -> list[dict]:
    columns = contract["scenario_columns"]
    return [dict(zip(columns, row, strict=True)) for row in contract["scenarios"]]


def check_completeness(contract: dict, table: list[dict]) -> None:
    declared = {entry[0] for entry in contract["requirements"]}
    require(len(declared) == len(contract["requirements"]), "duplicate requirement id")
    for identifier, section in contract["covered_elsewhere"].items():
        require(identifier in declared, f"{identifier} is covered but not declared")
        require(contract.get(section), f"{identifier} names absent section {section}")

    cited: set[str] = set()
    seen: set[str] = set()
    for row in table:
        require(row["id"] not in seen, f"duplicate scenario {row['id']}")
        seen.add(row["id"])
        require(row["reqs"], f"{row['id']} is evidence for no requirement")
        unknown = set(row["reqs"]) - declared
        require(not unknown, f"{row['id']} cites unknown {sorted(unknown)}")
        cited.update(row["reqs"])
    missing = declared - cited - set(contract["covered_elsewhere"])
    require(not missing, f"requirements with no evidence: {sorted(missing)}")

    exercised = {
        "programs": {row["program"] for row in table},
        "detectors": {row["detector"] for row in table},
        "diagnostics": {row["diag"] for row in table if row["diag"]},
    }
    for key, used in exercised.items():
        unknown = used - set(contract[key])
        require(not unknown, f"{key} referenced but undeclared: {sorted(unknown)}")
        dead = set(contract[key]) - used
        require(not dead, f"{key} declared but never exercised: {sorted(dead)}")


def check_normal_spellings(contract: dict, table: list[dict], text: str) -> None:
    pairs = contract["normal_rule"]["frozen_pairs"]
    require(len(pairs) == 2, "the frozen surface is two pairs, not a catalogue")
    frozen = [pair[2] for pair in pairs]
    frozen += [row["normal"] for row in table if isinstance(row["normal"], list)]
    for normal in frozen:
        require(len(normal) == 2, f"{normal} is not planar")
        for component in normal:
            require(isinstance(component, float), f"{component} is not binary64")
            require(component in DYADIC, f"{component} is not a frozen dyadic value")
            spelling = component.hex()
            require(spelling == DYADIC[component], f"{component!r} spells {spelling}")
            if component == 0.0:
                # `0.0 == -0.0` in binary64, so equality alone cannot see this.
                require(math.copysign(1.0, component) == 1.0, "zero must be positive")
        square = normal[0] * normal[0] + normal[1] * normal[1]
        require(square == 1.0, f"{normal} is not exactly a unit vector")
        require(json.dumps(normal) in text, f"{normal} is not spelled exactly in JSON")
    for bad in BAD_SPELLINGS:
        require(bad not in text, f"fixture carries the wrong number spelling {bad}")


def check_mutation_classifications(contract: dict, table: list[dict]) -> None:
    require(
        contract["diagnostics_authored_by"] == "pre-existing"
        and contract["new_diagnostic_texts"] == [],
        "this slice may not author a new diagnostic text",
    )
    for key, entry in contract["diagnostics"].items():
        require(entry[0] in ("EQ0302", "EQ0901"), f"{key}: unknown code {entry[0]}")
        require(len(entry) == 2 or entry[2] == "prefix", f"{key}: unknown match mode")

    reasons = set(contract["normal_rule"]["reasons"])
    used: set[str] = set()
    for row in table:
        name = row["id"]
        require(row["outcome"] in ("accept", "reject"), f"{name}: bad outcome")
        if row["outcome"] == "reject":
            require(row["diag"], f"{name}: a rejection must name its diagnostic")
            require(row["normal"] is None, f"{name}: a rejection carries no normal")
            require(row["reason"] is None, f"{name}: a rejection carries no reason")
            continue
        require(row["diag"] is None, f"{name}: an acceptance carries no diagnostic")
        if row["normal"] is None:
            require(row["reason"] in reasons, f"{name}: unclassified missing normal")
            used.add(row["reason"])
        else:
            require(row["reason"] is None, f"{name}: a projected normal has no reason")
            require(
                isinstance(row["normal"], list) or row["normal"] == UNFROZEN,
                f"{name}: unknown normal form {row['normal']!r}",
            )
    dead = reasons - used
    require(not dead, f"reasons declared but never exercised: {sorted(dead)}")


def check_diagnostic_texts(contract: dict, table: list[dict]) -> None:
    """Each parameterized pre-existing sentence must match the fixture numbers."""
    want = contract["dimensions"]["topological"] - 1
    for row in table:
        diag = row["diag"] or ""
        if not diag.startswith(("absent-", "dim-")):
            continue
        sets, name = sets_of(contract, handle_of(contract, row)), row["set"]
        if diag.startswith("absent-"):
            require(name not in sets, f"{row['id']}: {name} is present after all")
            expected = ABSENT.format(name=name)
        else:
            expected = DIM.format(name=name, found=sets[name][1], want=want)
        actual = contract["diagnostics"][diag][1]
        require(actual == expected, f"{row['id']}: {actual!r} != {expected!r}")


def check_nonclaims(contract: dict, table: list[dict]) -> None:
    declared = sorted(contract["nonclaims"])
    require(len(declared) == NONCLAIM_COUNT, f"{len(declared)} nonclaims declared")
    actual = hashlib.sha256("\n".join(declared).encode("utf-8")).hexdigest()
    require(actual == NONCLAIMS_SHA256, f"nonclaim set {actual} != frozen")
    pairs = {(pair[0], pair[1]) for pair in contract["normal_rule"]["frozen_pairs"]}
    catalog = {member[0]: member for member in contract["members"]}
    for row in table:
        if not isinstance(row["normal"], list):
            continue
        handle = handle_of(contract, row)
        require(
            (handle, row["set"]) in pairs,
            f"{row['id']} projects outside the two frozen pairs",
        )
        members = sets_of(contract, handle)[row["set"]][2]
        require(len(members) == 1, f"{row['id']} projects from a multi-edge set")
        require(
            catalog[members[0]][1] == "straight-axis-side",
            f"{row['id']} projects from a curved member",
        )


def check_agreement(contract: dict, table: list[dict]) -> None:
    for row in table:
        derived = decide(contract, row, contract["programs"][row["program"]][1])
        for field in ("outcome", "detector", "diag", "normal", "reason"):
            require(
                derived[field] == row[field],
                f"{row['id']}: model {field}={derived[field]!r} "
                f"!= fixture {field}={row[field]!r}",
            )


def check_order_invariance(contract: dict, table: list[dict]) -> int:
    permutations = 0
    for row in table:
        bundle = contract["programs"][row["program"]][1]
        baseline = decide(contract, row, bundle)
        for candidate in itertools.permutations(bundle):
            permutations += 1
            require(
                decide(contract, row, list(candidate)) == baseline,
                f"{row['id']}: bundle order {candidate} changed the outcome",
            )
    return permutations


def check_obligations(contract: dict) -> None:
    for obligation, evidence in contract["compatibility_obligations"]:
        manifest = ROOT / evidence / "case.toml"
        require(manifest.is_file(), f"{obligation}: {evidence}/case.toml is absent")


def check_sequencing(contract: dict) -> str:
    """Frozen before registration, or registered under exactly the proposed ID."""
    registration = contract["registration"]
    require(
        registration["case_toml_at_freeze"] is False
        and registration["discovered_by_eqiora_verify_at_freeze"] is False,
        "the fixture must record that this package was unregistered when frozen",
    )
    require(registration["proposed_case_id"] == PROPOSED_CASE_ID, "unexpected case id")
    own = CASE / "case.toml"
    claim = f'id = "{PROPOSED_CASE_ID}"'
    for manifest in sorted((ROOT / "verify").glob("*/*/case.toml")):
        clear = manifest == own or claim not in manifest.read_text(encoding="utf-8")
        require(clear, f"{PROPOSED_CASE_ID} is already registered by {manifest}")
    if not own.is_file():
        return "frozen-before-registration"
    try:
        declared = tomllib.loads(own.read_text(encoding="utf-8")).get("id")
    except ValueError as error:  # tomllib.TOMLDecodeError derives from ValueError
        raise Failure(f"{own} is not a valid manifest: {error}") from error
    require(declared == PROPOSED_CASE_ID, f"{own} registers id {declared!r}")
    return "registered"


def main() -> int:
    raw = CONTRACT.read_bytes()
    text = raw.decode("utf-8")
    contract = json.loads(text)
    table = rows(contract)

    digest = check_digest(raw)
    check_completeness(contract, table)
    check_normal_spellings(contract, table, text)
    check_mutation_classifications(contract, table)
    check_diagnostic_texts(contract, table)
    check_nonclaims(contract, table)
    check_agreement(contract, table)
    permutations = check_order_invariance(contract, table)
    check_obligations(contract)
    sequencing = check_sequencing(contract)

    out = sys.stdout.write
    accepted = sum(1 for row in table if row["outcome"] == "accept")
    out(f"fixture={CONTRACT.relative_to(ROOT)}\n")
    out(f"bytes={len(raw)} sha256={digest}\n")
    out(f"requirements={len(contract['requirements'])} scenarios={len(table)}\n")
    out(f"accept={accepted} reject={len(table) - accepted}\n")
    for handle, name, normal in contract["normal_rule"]["frozen_pairs"]:
        hexes = " ".join(component.hex() for component in normal)
        out(f"normal {handle}.{name}={json.dumps(normal)} hex=[{hexes}]\n")
    out(f"bundle_permutations={permutations}\n")
    out(f"registration={sequencing}\n")
    out("OK\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
