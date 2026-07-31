#!/usr/bin/env python3
"""Independent derivation of every relational identity frozen by this case.

This is the route record for
`artifacts.current-model-relational-identity-transition`. It never imports,
links, or executes the Rust producer. It reads the committed canonical bytes,
re-renders canonical JSON with the standard library, rebuilds the RFC 0008
schema-domain preimage by hand, hashes with `hashlib`, and reads every
artifact-reference edge out of the downstream bytes.

Run from the repository root:

    python3 verify/artifacts/current-model-relational-identity-transition/\
references/derive_transition_identities.py

It exits non-zero if any committed literal disagrees with the derivation.
"""

from __future__ import annotations

import fnmatch
import hashlib
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[4]
CASE = ROOT / "verify/artifacts/current-model-relational-identity-transition"

MODEL_SCHEMA = "eqiora.model-envelope/v8"

# `WireModelContentV2` omits `source_revision`: a graph revision is provenance,
# not meaning. Every other family in this case hashes its complete envelope.
MODEL_DIGEST_FIELDS = (
    "schema",
    "encoding",
    "model_ulid",
    "nodes",
    "values",
    "edges",
    "boundary",
)

# The package family separates its digest domain from its schema string.
PACKAGE_DIGEST_DOMAIN = {
    "eqiora.package-compilation.v1": "eqiora.package-compilation.sha256.v1",
    "eqiora.package-run-binding.v1": "eqiora.package-run-binding.sha256.v1",
    "eqiora.package-execution-binding.v1": "eqiora.package-execution-binding.sha256.v1",
}

# `EntityKind` declaration order fixes the `(kind, ULID)` sort the encoder
# applies to nodes, values, edges, and the Model boundary.
ENTITY_KINDS = (
    "domain",
    "representation",
    "field",
    "parameter",
    "port",
    "relation",
    "activation",
    "connection",
    "clock-domain",
)

failures: list[str] = []


def render(value: object) -> bytes:
    """`serde_json::to_vec`: compact, declaration order, shortest round-trip f64."""
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


NUMBER_START = b"-0123456789"
NUMBER_BODY = b"-+.eE0123456789"


def canonical(raw: bytes) -> bytes:
    """Rewrite every JSON number outside a string to one shared notation.

    Both this route and `serde_json` emit the shortest decimal that round
    trips, but they choose positional versus exponential form at different
    magnitudes and spell the exponent differently. That is a rendering
    convention, not a different value. Normalizing both sides keeps this
    check meaningful for what it actually owns -- compact separators,
    preserved key order, no insignificant whitespace, and identical string
    content -- without asserting a float spelling this route does not own.
    The producer's exact float bytes are owned by
    `artifacts.current-model-canonical-identity` and by the registered Rust
    test, which round-trips these exact bytes through the current codec.
    """
    output = bytearray()
    index = 0
    in_string = False
    while index < len(raw):
        byte = raw[index : index + 1]
        if in_string:
            output += byte
            if byte == b"\\":
                output += raw[index + 1 : index + 2]
                index += 2
                continue
            in_string = byte != b'"'
            index += 1
            continue
        if byte == b'"':
            in_string = True
            output += byte
            index += 1
            continue
        if byte in NUMBER_START:
            end = index
            while end < len(raw) and raw[end : end + 1] in NUMBER_BODY:
                end += 1
            token = raw[index:end]
            output += repr(float(token)).encode("ascii")
            index = end
            continue
        output += byte
        index += 1
    return bytes(output)


def domain_digest(domain: str, content: bytes) -> str:
    hasher = hashlib.sha256()
    hasher.update(domain.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(content)
    return hasher.hexdigest()


def artifact_digest(raw: bytes) -> str:
    wire = json.loads(raw)
    schema = wire["schema"]
    if schema == MODEL_SCHEMA:
        return domain_digest(
            schema, render({key: wire[key] for key in MODEL_DIGEST_FIELDS})
        )
    return domain_digest(PACKAGE_DIGEST_DOMAIN.get(schema, schema), raw)


def committed(path: pathlib.Path) -> bytes:
    return path.read_bytes().rstrip(b"\n")


def check(label: str, derived: object, frozen: object) -> None:
    equal = derived == frozen
    shown = f"{len(derived)} bytes" if isinstance(derived, bytes) else derived
    print(f"  [{'ok ' if equal else 'DIFF'}] {label}: {shown}")
    if not equal:
        failures.append(f"{label}: derived {derived!r} != committed {frozen!r}")


def entity_key(reference: dict) -> tuple[int, str]:
    return (ENTITY_KINDS.index(reference["kind"]), reference["ulid"])


def check_model(raw: bytes, entry: dict) -> None:
    wire = json.loads(raw)
    check("canonical compact re-render", canonical(render(wire)), canonical(raw))
    check("schema", wire["schema"], MODEL_SCHEMA)
    check("encoding", wire["encoding"], "eqiora.canonical-json/v1")
    check("canonical byte length", len(raw), entry["model_canonical_bytes"])
    check("raw sha256", hashlib.sha256(raw).hexdigest(), entry["model_raw_sha256"])

    nodes = [entity_key(node["id"]) for node in wire["nodes"]]
    check("nodes sorted and unique", nodes, sorted(set(nodes)))
    values = [entity_key(value["target"]) for value in wire["values"]]
    check("values sorted and unique", values, sorted(set(values)))
    edges = [
        (entity_key(edge["from"]), entity_key(edge["to"]), edge["kind"])
        for edge in wire["edges"]
    ]
    check("edges sorted and unique", edges, sorted(set(edges)))
    boundary = [entity_key(member) for member in wire["boundary"]]
    check("boundary sorted and unique", boundary, sorted(set(boundary)))

    check("RFC 0008 artifact digest", artifact_digest(raw), entry["model_digest"])
    check("Model ULID", wire["model_ulid"], entry["model_ulid"])
    check("source revision", wire["source_revision"], entry["source_revision"])


def resolve(document: object, pointer: str) -> object:
    """Walk one `/key[index]...` path produced by `flatten`."""
    node = document
    for token in pointer.strip("/").split("/"):
        node = node[token.split("[", 1)[0]]
        for index in re.findall(r"\[(\d+)\]", token):
            node = node[int(index)]
    return node


def flatten(document: object, prefix: str = ""):
    if isinstance(document, dict):
        for key, value in document.items():
            yield from flatten(value, f"{prefix}/{key}")
    elif isinstance(document, list):
        for index, value in enumerate(document):
            yield from flatten(value, f"{prefix}[{index}]")
    else:
        yield prefix, document


def check_deterministic(entry: dict) -> None:
    print(f"Deterministic {entry['name']}")
    model_raw = committed(CASE / entry["model_bytes"])
    check_model(model_raw, entry)

    identities = {entry["model_pointer"]: entry["model_digest"]}
    replacement_identities = {entry["model_pointer"]: entry["model_digest"]}
    artifacts = entry.get("supporting_artifacts", []) + entry["edges"]
    for edge in artifacts:
        raw = committed(
            CASE / "expected/deterministic" / entry["name"] / edge["artifact"]
        )
        wire = json.loads(raw)
        check(
            f"{edge['artifact']} canonical re-render",
            canonical(render(wire)),
            canonical(raw),
        )
        check(f"{edge['artifact']} schema", wire["schema"], edge["schema"])
        check(
            f"{edge['artifact']} domain digest",
            domain_digest(edge["digest_domain"], raw),
            edge["digest"],
        )
        check(
            f"{edge['artifact']} raw sha256",
            hashlib.sha256(raw).hexdigest(),
            edge["raw_sha256"],
        )
        identities[edge["pointer"]] = edge["digest"]
    for edge in entry["edges"]:
        replacement_identities[edge["pointer"]] = edge["digest"]

    # Each downstream artifact names the identities it was sealed against, so
    # every reference edge is readable without decoding either side.
    for edge in artifacts:
        wire = json.loads(
            committed(
                CASE / "expected/deterministic" / entry["name"] / edge["artifact"]
            )
        )
        for reference in edge["references"]:
            check(
                f"{edge['artifact']} edge {reference['path']} -> {reference['target']}",
                resolve(wire, reference["path"]),
                identities[reference["target"]],
            )

    replacement_raw = committed(CASE / entry["target_replacement"])
    target_raw = committed(ROOT / entry["target"])
    replacement = json.loads(replacement_raw)
    target = json.loads(target_raw)
    check(
        "replacement leaf keys unchanged",
        sorted(pointer for pointer, _ in flatten(replacement)),
        sorted(pointer for pointer, _ in flatten(target)),
    )
    changed = sorted(
        pointer
        for pointer, value in flatten(target)
        if resolve(replacement, pointer) != value
    )
    check(
        "delta is exactly the identity pointers",
        changed,
        sorted(replacement_identities),
    )
    exact_replacement = target_raw
    for pointer, digest in replacement_identities.items():
        superseded = entry["superseded"][pointer].encode("ascii")
        successor = digest.encode("ascii")
        check(
            f"{pointer} substitution preserves byte length",
            len(successor),
            len(superseded),
        )
        check(
            f"{pointer} superseded literal occurs exactly once",
            exact_replacement.count(superseded),
            1,
        )
        exact_replacement = exact_replacement.replace(superseded, successor, 1)
    check(
        "replacement is byte-exact identity substitution",
        exact_replacement,
        replacement_raw,
    )
    for pointer, digest in replacement_identities.items():
        check(f"replacement {pointer}", resolve(replacement, pointer), digest)
        check(
            f"superseded {pointer} retired",
            resolve(replacement, pointer) != entry["superseded"][pointer],
            True,
        )
    superseded = set(entry["superseded"].values())
    survivors = sorted(
        pointer for pointer, value in flatten(replacement) if value in superseded
    )
    check("no superseded identity survives", survivors, [])


def check_bridge(entry: dict) -> None:
    print(f"Bridge {entry['name']}")
    historical = committed(ROOT / entry["historical_artifact"])
    check(
        "historical raw sha256",
        hashlib.sha256(historical).hexdigest(),
        entry["historical_raw_sha256"],
    )
    check("historical byte length", len(historical), entry["historical_bytes"])
    historical_wire = json.loads(historical)
    check("historical schema", historical_wire["schema"], entry["historical_schema"])
    # Hashed from the untouched bytes, never admitted through a product decoder.
    check(
        "historical artifact digest",
        domain_digest(
            historical_wire["schema"],
            render({key: historical_wire[key] for key in MODEL_DIGEST_FIELDS}),
        ),
        entry["historical_artifact_digest"],
    )

    current = committed(CASE / entry["current_model_bytes_path"])
    check(
        "current canonical re-render",
        canonical(render(json.loads(current))),
        canonical(current),
    )
    check(
        "current raw sha256",
        hashlib.sha256(current).hexdigest(),
        entry["current_raw_sha256"],
    )
    check(
        "current artifact digest",
        artifact_digest(current),
        entry["current_artifact_digest"],
    )
    check(
        "schema domain separates the two identities",
        entry["current_artifact_digest"] != entry["historical_artifact_digest"],
        True,
    )
    check(
        "semantic fingerprints agree",
        entry["current_fingerprint"],
        entry["historical_fingerprint"],
    )
    check(
        "bridge fingerprint generation",
        entry["current_fingerprint"].split(":")[0],
        "eqiora.structural-semantic-fingerprint/v2",
    )

    current_wire = json.loads(current)
    check("Model ULID survives", current_wire["model_ulid"], entry["model_ulid"])
    check(
        "source revision survives",
        current_wire["source_revision"],
        entry["source_revision"],
    )

    for member in entry["historical_bundle"]:
        raw = committed(ROOT / member["path"])
        name = pathlib.PurePath(member["path"]).name
        check(
            f"{name} raw sha256", hashlib.sha256(raw).hexdigest(), member["raw_sha256"]
        )
        check(
            f"{name} artifact digest", artifact_digest(raw), member["artifact_digest"]
        )
        # The recorded bundle keeps observing the historical Model, never the
        # current one. A relabelled Run would break exactly here.
        check(
            f"{name} still observes the historical Model",
            json.loads(raw)["model_sha256"],
            entry["historical_artifact_digest"],
        )


def check_retained_realization_v4() -> None:
    print("Retained realization-v4")
    raw = committed(CASE / "expected/retained/realization-v4.json")
    check("canonical byte length", len(raw), 8_333)
    check(
        "raw sha256",
        hashlib.sha256(raw).hexdigest(),
        "ba9efbdbca265dea0fdf9b1476ea2cae876eb2c97b4ac6f332f3755d866b5d9e",
    )
    check(
        "RFC 0008 artifact digest",
        domain_digest("eqiora.realization-envelope/v4", raw),
        "b5bbe49235f75163bf764f37cb2a1168c4471271cd85c5b09f5d5e411ce52c7f",
    )
    wire = json.loads(raw)
    check("schema", wire["schema"], "eqiora.realization-envelope/v4")
    check("encoding", wire["encoding"], "eqiora.canonical-json/v1")
    check(
        "opaque historical Model reference",
        wire["model_sha256"],
        "16d7bfb39746ccfda33c07ac3f054b42827ee5dd65380c553b93f7c3751d26ba",
    )


def check_retained_family_golden(entry: dict) -> None:
    """The post-reset route for one retained separate-family golden.

    `realization_v4_wire.rs` reconstructs its golden today by decoding the
    historical fixed-reference CUDA Model and re-encoding a Realization over it.
    The reset removes that decoder, and the two obvious repairs are both wrong:
    admitting those bytes through the current Model owner, and rebuilding the
    golden over a current Model. This route freezes the third one -- the bytes
    are the evidence, and the Model reference inside them is an opaque string --
    without ever parsing the referenced Model.
    """
    print(f"Retained family golden {entry['name']}")
    raw = committed(CASE / entry["golden_bytes"])
    check("canonical byte length", len(raw), entry["canonical_bytes"])
    check("raw sha256", hashlib.sha256(raw).hexdigest(), entry["raw_sha256"])
    check(
        "RFC 0008 artifact digest",
        domain_digest(entry["family"], raw),
        entry["artifact_digest"],
    )
    wire = json.loads(raw)
    check("schema", wire["schema"], entry["family"])
    check("encoding", wire["encoding"], entry["encoding"])
    check("Model ULID", wire["model_ulid"], entry["model_ulid"])
    check("semantic revision", wire["semantic_revision"], entry["semantic_revision"])

    opaque = entry["opaque_model_reference"]
    check(
        "opaque Model reference",
        resolve(wire, opaque["pointer"]),
        opaque["value"],
    )
    # Read from the referenced bytes' own text, never decoded: this route has no
    # Model decoder either, which is the property the reset has to preserve.
    referenced = committed(ROOT / opaque["bytes"])
    check(
        "referenced raw sha256",
        hashlib.sha256(referenced).hexdigest(),
        opaque["raw_sha256"],
    )
    check("referenced schema", json.loads(referenced)["schema"], opaque["schema"])
    check(
        "the reference is opaque, so it is not the current Model schema",
        opaque["schema"] != MODEL_SCHEMA,
        True,
    )
    check(
        "the golden decodes without a Model decoder",
        entry["post_reset_acceptance"]["model_decoder_calls"],
        0,
    )

    # Relabelling: internally consistent, externally a different artifact. Only
    # the frozen bytes refuse it, which is why they are frozen.
    relabelled = raw.replace(
        opaque["value"].encode("ascii"),
        entry["forbidden"]["relabelled_model_reference"].encode("ascii"),
    )
    check("a relabelled golden keeps its byte length", len(relabelled), len(raw))
    check(
        "a relabelled golden carries its frozen bytes",
        hashlib.sha256(relabelled).hexdigest(),
        entry["forbidden"]["relabelled_raw_sha256"],
    )
    check(
        "a relabelled golden is a different artifact",
        domain_digest(entry["family"], relabelled) != entry["artifact_digest"],
        True,
    )


def check_model_input_consumer(entry: dict) -> None:
    """The identity-only delta of one consumer whose Model *input* moves.

    `moving_spatial_v2_wire.rs` builds its SpatialState, segment, and prefix root
    at run time and freezes three digests of what it built. Both states of all
    three artifacts are committed, so this is a comparison and not a claim: the
    replacement must be the pre-reset bytes with the frozen identity table
    applied, and every other byte must be untouched.
    """
    print(f"Model-input consumer {entry['name']}")
    table = entry["identity_substitutions"]
    check("substitution count", len(table), entry["identity_substitution_count"])
    check(
        "the Model edge is the precommitted current Model",
        table.get(entry["pre_reset_model_digest"]),
        entry["current_model_digest"],
    )
    identity = re.compile(r"\A[0-9a-f]{64}\Z")
    check(
        "every side is one 64-character lowercase hex identity",
        all(identity.match(old) and identity.match(new) for old, new in table.items()),
        True,
    )
    check(
        "no replacement is itself superseded, so the table is order-free",
        set(table) & set(table.values()),
        set(),
    )
    current = committed(ROOT / entry["current_model_input"])
    check(
        "current Model input raw sha256",
        hashlib.sha256(current).hexdigest(),
        committed_raw_sha256(entry),
    )
    check(
        "current Model input digest",
        artifact_digest(current),
        entry["current_model_digest"],
    )
    check(
        "current Model schema",
        json.loads(current)["schema"],
        entry["current_model_schema"],
    )
    check(
        "Model ULID survives the input change",
        json.loads(current)["model_ulid"],
        entry["model_ulid"],
    )

    replayed: dict[str, str] = {}
    for artifact in entry["artifacts"]:
        name, schema = artifact["name"], artifact["schema"]
        pre = committed(CASE / artifact["pre_reset_path"])
        new = committed(CASE / artifact["replacement_path"])
        check(f"{name} canonical byte length", len(pre), artifact["canonical_bytes"])
        check(f"{name} replacement is byte-length identical", len(new), len(pre))
        check(
            f"{name} pre-reset raw sha256",
            hashlib.sha256(pre).hexdigest(),
            artifact["pre_reset_raw_sha256"],
        )
        check(
            f"{name} replacement raw sha256",
            hashlib.sha256(new).hexdigest(),
            artifact["replacement_raw_sha256"],
        )
        check(
            f"{name} pre-reset digest",
            domain_digest(schema, pre),
            artifact["pre_reset_digest"],
        )
        check(
            f"{name} replacement digest",
            domain_digest(schema, new),
            artifact["replacement_digest"],
        )

        before, after = dict(flatten(json.loads(pre))), dict(flatten(json.loads(new)))
        check(f"{name} keeps its exact leaf set", sorted(before), sorted(after))
        changed = {path for path in before if before[path] != after[path]}
        check(
            f"{name} substituted pointers",
            sorted(changed),
            artifact["substituted_pointers"],
        )
        check(
            f"{name} every changed leaf is in the table",
            all(table.get(before[path]) == after[path] for path in changed),
            True,
        )
        check(
            f"{name} unchanged leaves",
            len(before) - len(changed),
            artifact["unchanged_leaves"],
        )
        check(f"{name} reconstructs from the table alone", substitute(pre, table), new)
        check(
            f"{name} carries no superseded identity",
            [old for old in table if old.encode("ascii") in new],
            [],
        )
        replayed[name] = artifact["replacement_digest"]

    for artifact in entry["artifacts"]:
        wire = json.loads(committed(CASE / artifact["replacement_path"]))
        for edge in artifact["references"]:
            target = edge["target"]
            if target.startswith("artifact:"):
                expected = replayed[target.removeprefix("artifact:")]
            elif target == "current_model_digest":
                expected = entry["current_model_digest"]
            else:
                expected = entry["downstream_current_identities"][target]
            check(
                f"{artifact['name']}{edge['path']} -> {target}",
                resolve(wire, edge["path"]),
                expected,
            )

    frozen_literals = entry["frozen_literals"]
    check(
        "one frozen literal per artifact", len(frozen_literals), len(entry["artifacts"])
    )
    for literal, artifact in zip(frozen_literals, entry["artifacts"]):
        check(
            f"{literal['artifact']} literal is its pre-reset identity",
            literal["pre_reset"],
            artifact["pre_reset_digest"],
        )
        check(
            f"{literal['artifact']} replacement is its post-reset identity",
            literal["replacement"],
            artifact["replacement_digest"],
        )


def committed_raw_sha256(entry: dict) -> str:
    """The current Model input's raw hash, read out of the bridge that owns it."""
    transition = json.loads(committed(CASE / "expected/transition.json"))
    bridge = next(
        item
        for item in transition["bridge"]
        if item["name"] == entry["current_model_bridge"]
    )
    return bridge["current_raw_sha256"]


def substitute(source: bytes, table: dict[str, str]) -> bytes:
    """Apply a length-preserving identity table, so no offset can move."""
    out = source
    for old, new in table.items():
        out = out.replace(old.encode("ascii"), new.encode("ascii"))
    return out


# The candidate sweep, reimplemented here rather than shared with the Rust
# oracle: the frozen inventory is only independent evidence if a second route
# reproduces it. Keep these tokens identical to the ones `classification.json`
# records under `search.method`.
SEARCH_TOKENS = (
    "eqiora.model-envelope/v",
    "eqiora.model-transaction-envelope/v",
    "model_sha256",
    "modelSha256",
    "model_digest",
    "modelDigest",
    "ModelEnvelopeV",
    "ModelTransactionEnvelopeV",
    "ExactModelCodec",
    "compile_exact",
    "exact_codec",
)
SKIPPED_DIRECTORIES = {".git", "target", "node_modules", "dist", "__pycache__"}
HEX_DIGITS = set("0123456789abcdef")
# This case's own executable oracle is two files: the integration-test root and
# the private support module it includes with `#[path]`, split because one file
# exceeded the 2,000-line test ceiling. Both spell the tokens above, so a sweep
# that read them would report the oracle as a candidate of itself. Both are
# excluded by exact path -- not by their directory, not by a suffix rule, and
# not by anything resembling "tests". Every other test file in the repository
# stays a classified candidate, and this route checks that below.
ORACLE_FILES = (
    "crates/eqiora-artifact/tests/current_model_relational_identity_transition.rs",
    "crates/eqiora-artifact/tests/current_model_relational_identity_transition/"
    "transition_contract.rs",
)
# This case's own outputs, and maturin's packaging staging directory: building
# the Python extension copies checked-in example resources into it, so a tree
# that has run the gate holds an untracked build copy of a classified resource.
EXCLUDED_TREES = {
    "verify/artifacts/current-model-relational-identity-transition",
    "bindings/python/python/eqiora/examples",
}
# Ordinary test files that must never inherit the exclusion above.
CLASSIFIED_TEST_FILES = [
    "crates/eqiora-artifact/tests/current_model_wire_oracle.rs",
    "crates/eqiora-artifact/tests/model_v8_wire.rs",
    "crates/eqiora-artifact/tests/realization_v4_wire.rs",
]


def carries_model_search_signal(raw: bytes) -> bool:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        return False
    if any(token in text for token in SEARCH_TOKENS):
        return True
    for line in text.split("\n"):
        lowered = line.lower()
        if "model" not in lowered and "transaction" not in lowered:
            continue
        if any(
            all(character in HEX_DIGITS for character in line[start : start + 64])
            for start in range(len(line) - 63)
        ):
            return True
    return False


def sweep() -> set[str]:
    """Every checked-in path carrying a Model/Transaction search signal."""
    found: set[str] = set()
    pending = [ROOT]
    while pending:
        for entry in sorted(pending.pop().iterdir()):
            relative = entry.relative_to(ROOT).as_posix()
            if entry.is_dir():
                if (
                    entry.name not in SKIPPED_DIRECTORIES
                    and relative not in EXCLUDED_TREES
                ):
                    pending.append(entry)
            elif relative not in ORACLE_FILES and carries_model_search_signal(
                entry.read_bytes()
            ):
                found.add(relative)
    return found


def frozen_classification() -> dict:
    return json.loads(committed(CASE / "expected/classification.json"))


def frozen_inventory() -> set[str]:
    return {
        line
        for line in (CASE / "expected/classification-inventory.txt")
        .read_text()
        .splitlines()
        if line
    }


def check_search_exclusions() -> None:
    """The sweep excludes exactly two executor files, by exact path."""
    print("Sweep self-exclusion")
    search = frozen_classification()["search"]
    inventory = frozen_inventory()
    check(
        "declared excluded executor files", search["excluded_paths"], list(ORACLE_FILES)
    )
    check(
        "declared excluded trees",
        sorted(search["excluded_trees"]),
        sorted(EXCLUDED_TREES),
    )
    check("excluded executor file count", len(ORACLE_FILES), 2)
    # Load-bearing, not decorative: both halves spell the tokens the sweep
    # searches for, so without this exact exclusion the oracle finds itself.
    check(
        "excluded executor files that do not carry the search signal",
        sorted(
            path
            for path in ORACLE_FILES
            if not carries_model_search_signal((ROOT / path).read_bytes())
        ),
        [],
    )
    check(
        "excluded executor files that are also classified candidates",
        sorted(path for path in ORACLE_FILES if path in inventory),
        [],
    )
    # Exact, and only exact: no directory, no suffix, no rule about tests.
    check(
        "ordinary test files wrongly excluded",
        sorted(path for path in CLASSIFIED_TEST_FILES if path in ORACLE_FILES),
        [],
    )
    check(
        "ordinary test files missing from the inventory",
        sorted(path for path in CLASSIFIED_TEST_FILES if path not in inventory),
        [],
    )
    check(
        "excluded trees standing in for a rule about tests",
        sorted(tree for tree in EXCLUDED_TREES if "tests" in tree),
        [],
    )


# The fates an entry may assign. Respelled here rather than read out of the
# declaration, for the same reason the forbidden tokens are: a route that quoted
# the vocabulary back at itself would accept any vocabulary.
EXPECTED_DISPOSITIONS = [
    "decompose-by-claim",
    "delegate",
    "delete",
    "migrate",
    "migrate-in-place",
    "preserve-bytes",
    "rename-source",
]
# A path the reset removes cannot survive in place, so these are the only fates
# a retired inventory member may carry.
RETIRED_DISPOSITIONS = {"delete", "rename-source", "delegate", "decompose-by-claim"}
# The nineteen retired inventory paths no fixture entry named, and how they
# divide: fifteen whose sole claim is a removed generation, two that host the
# current v8 implementation beside a removed generation and are therefore
# decomposed by claim, and two version-named current owners the reset renames
# rather than deletes. RFC 0083 is explicit that "the current implementation
# hosts v8 encoding in `model_v2.rs`", so a fate of `delete` for either v2 module
# would delete the current encoder.
EXPECTED_DELETED = [
    "bindings/python/python/eqiora/compatibility.py",
    "bindings/python/python/eqiora/compatibility.pyi",
    "crates/eqiora-api/src/codec.rs",
    "crates/eqiora-api/tests/control_compile_v1.rs",
    "crates/eqiora-api/tests/versioned_model_document.rs",
    *[f"crates/eqiora-artifact/src/model_v{n}.rs" for n in range(3, 8)],
    *[f"crates/eqiora-artifact/src/model_transaction_v{n}.rs" for n in range(3, 8)],
]
# Source file -> the unversioned owner its surviving current claim migrates to.
EXPECTED_DECOMPOSED = {
    "crates/eqiora-artifact/src/model_v2.rs": (
        "crates/eqiora-artifact/src/model_wire.rs"
    ),
    "crates/eqiora-artifact/src/model_transaction_v2.rs": (
        "crates/eqiora-artifact/src/model_transaction_wire.rs"
    ),
}
# Ordered `from` -> `to`, not a set: swapping the two targets must fail.
EXPECTED_RENAME_PAIRS = [
    (
        "crates/eqiora-artifact/src/model_v8.rs",
        "crates/eqiora-artifact/src/model_wire.rs",
    ),
    (
        "crates/eqiora-artifact/src/model_transaction_v8.rs",
        "crates/eqiora-artifact/src/model_transaction_wire.rs",
    ),
]
EXPECTED_RENAMED = {source for source, _ in EXPECTED_RENAME_PAIRS}


def check_inventory_dispositions() -> None:
    """Every one of the 338 candidates has exactly one fate, and no retired
    path is left to inherit the remainder's in-place migration."""
    print("Inventory dispositions")
    classification = frozen_classification()
    search = classification["search"]
    inventory = frozen_inventory()
    retired = set(search["transition"]["retired"])
    entries = classification["entries"]

    check(
        "declared disposition vocabulary",
        sorted(classification["dispositions"]),
        EXPECTED_DISPOSITIONS,
    )
    check(
        "entries naming paths without a declared disposition",
        sorted(
            entry["class"]
            for entry in entries
            if entry.get("paths")
            and entry.get("disposition") not in EXPECTED_DISPOSITIONS
        ),
        [],
    )
    check(
        "mixed surfaces not decomposed by claim",
        sorted(
            entry["class"]
            for entry in entries
            if entry["class"] == "mixed-claim-surface"
            and entry.get("disposition") != "decompose-by-claim"
        ),
        [],
    )

    fate: dict[str, str] = {}
    duplicated: list[str] = []
    for entry in entries:
        for path in entry.get("paths", []):
            if path in fate:
                duplicated.append(path)
            fate[path] = entry["disposition"]
    check("paths classified by more than one entry", sorted(duplicated), [])

    remainder_entries = [entry for entry in entries if entry.get("inventory_remainder")]
    check("declared inventory remainder entries", len(remainder_entries), 1)
    remainder_entry = remainder_entries[0]
    check("remainder disposition", remainder_entry["disposition"], "migrate-in-place")
    check("remainder excludes retired paths", remainder_entry["excludes_retired"], True)
    check(
        "the remainder is a rule, not a second path list",
        "paths" in remainder_entry,
        False,
    )

    remainder = {path for path in inventory if path not in fate}
    classified = len(inventory) - len(remainder)
    check("remainder path count", len(remainder), remainder_entry["path_count"])
    check(
        "explicitly classified inventory paths",
        classified,
        search["classified_inventory_path_count"],
    )
    check(
        "classified paths and the remainder cover the inventory",
        classified + len(remainder),
        search["candidate_path_count"],
    )

    check("retired paths left to the remainder", sorted(remainder & retired), [])
    check(
        "retired inventory paths with no disposition",
        sorted(path for path in retired & inventory if path not in fate),
        [],
    )
    check(
        "retired inventory paths carrying a survivor's fate",
        sorted(
            f"{path}:{fate[path]}"
            for path in retired & inventory
            if fate.get(path) not in RETIRED_DISPOSITIONS
        ),
        [],
    )
    check("the fifteen deleted compatibility surfaces", len(EXPECTED_DELETED), 15)
    check(
        "compatibility surfaces not marked deleted",
        sorted(path for path in EXPECTED_DELETED if fate.get(path) != "delete"),
        [],
    )
    check(
        "the nineteen formerly unnamed retired paths",
        len(EXPECTED_DELETED) + len(EXPECTED_DECOMPOSED) + len(EXPECTED_RENAME_PAIRS),
        19,
    )
    check(
        "the two version-named current owners are renamed, not deleted",
        sorted(path for path in EXPECTED_RENAMED if fate.get(path) != "rename-source"),
        [],
    )

    # The two v2-named source modules host the current v8 implementation as well
    # as the historical branch, so neither may be deleted whole and neither may
    # be left with a single verb for a fate.
    check(
        "v2-named source modules not decomposed by claim",
        sorted(
            f"{path}:{fate.get(path)}"
            for path in EXPECTED_DECOMPOSED
            if fate.get(path) != "decompose-by-claim"
        ),
        [],
    )
    for path, owner in sorted(EXPECTED_DECOMPOSED.items()):
        entry = next(e for e in entries if path in e.get("paths", []))
        check(
            f"{path} claim dispositions",
            sorted(c["disposition"] for c in entry["claims"]),
            ["delete", "migrate"],
        )
        migrating = [c for c in entry["claims"] if c["disposition"] == "migrate"]
        check(
            f"{path} surviving claim class",
            [c["class"] for c in migrating],
            ["current-owner-assertion"],
        )
        check(
            f"{path} migrates its current v8 implementation to",
            [c["owner"] for c in migrating],
            [owner],
        )

    renamed = [
        entry for entry in entries if entry.get("disposition") == "rename-source"
    ]
    check("entries renaming a source owner", len(renamed), 1)
    check(
        "renamed source owners", sorted(renamed[0]["paths"]), sorted(EXPECTED_RENAMED)
    )
    # Per file, not per set: two sets with the same members would let Model and
    # Transaction swap targets silently.
    check(
        "the frozen source -> target rename pairs",
        [(r["from"], r["to"]) for r in renamed[0]["renames"]],
        EXPECTED_RENAME_PAIRS,
    )
    check(
        "the parallel arrays declare their positional pairing",
        "positional" in renamed[0].get("rename_pairing", ""),
        True,
    )
    check(
        "the declared positional zip of paths and renames_to",
        list(zip(renamed[0]["paths"], renamed[0]["renames_to"], strict=True)),
        EXPECTED_RENAME_PAIRS,
    )
    check(
        "the rename targets are exactly the existence-only post-reset owners",
        sorted(renamed[0]["renames_to"]),
        sorted(search["transition"]["required_post_reset_without_frozen_bytes"]),
    )


# The exact nine fixtures the control-v2 lane froze in its own `fixtureDigests`,
# by basename, and the two promotion sources it does not record: its own
# expected contract, which cannot hash itself, and the historical cylinder,
# which belongs to a different lane entirely.
STAGED_DIGEST_NAMES = [
    "accepted-v2.json",
    "compile-v1.schema.json",
    "compile-v2.schema.json",
    "forbidden-model-wire-v2.json",
    "forbidden-required-features-v2.json",
    "rejected-source-v2.json",
    "retired-v1.json",
    "unknown-command-v2.json",
    "unknown-protocol-v2.json",
]
UNFROZEN_PROMOTION_NAMES = [
    "contract.json",
    "steady-flow-past-cylinder.model-v7.json",
]


def check_transition_contract() -> None:
    """The exact two-state contract: what retires, what stays, what is added."""
    print("Two-state transition contract")
    classification = json.loads(committed(CASE / "expected/classification.json"))
    search = classification["search"]
    transition = search["transition"]
    inventory = {
        line
        for line in (CASE / "expected/classification-inventory.txt")
        .read_text()
        .splitlines()
        if line
    }
    retired = set(transition["retired"])
    evidence = set(transition["preserved_evidence"])
    required = set(transition["required_post_reset"])
    existence_only = set(transition["required_post_reset_without_frozen_bytes"])

    found = sweep()
    check("frozen candidate path count", len(inventory), search["candidate_path_count"])
    check("candidates outside the frozen inventory", sorted(found - inventory), [])
    check("frozen candidates the sweep no longer finds", sorted(inventory - found), [])

    check("retired path count", len(retired), transition["retired_path_count"])
    check(
        "preserved path count",
        len(inventory - retired),
        transition["preserved_path_count"],
    )
    check(
        "required post-reset path count",
        len(required),
        transition["required_post_reset_path_count"],
    )

    # The version-named source owners retire with everything else they name, and
    # the unversioned owners that replace them are required by existence alone:
    # this case does not own the current encoding and freezes no hash for them.
    check(
        "version-named Rust source owners retired",
        sorted(
            path
            for path in (
                "crates/eqiora-artifact/src/model_v8.rs",
                "crates/eqiora-artifact/src/model_transaction_v8.rs",
            )
            if path not in retired
        ),
        [],
    )
    check(
        "unversioned Rust owners required after the reset",
        sorted(existence_only),
        [
            "crates/eqiora-artifact/src/model_transaction_wire.rs",
            "crates/eqiora-artifact/src/model_wire.rs",
        ],
    )
    check(
        "unversioned Rust owners do not exist yet",
        sorted(path for path in existence_only if (ROOT / path).exists()),
        [],
    )
    check(
        "unversioned Rust owners are neither classified nor retired",
        sorted(existence_only & (inventory | retired)),
        [],
    )
    check(
        "retired paths outside the inventory",
        sorted((retired - inventory) ^ set(transition["retired_outside_inventory"])),
        [],
    )
    check(
        "retired paths missing from the working tree",
        sorted(path for path in retired if not (ROOT / path).exists()),
        [],
    )
    check("preserved evidence that is also retired", sorted(evidence & retired), [])
    check("preserved evidence outside the inventory", sorted(evidence - inventory), [])
    check(
        "preserved evidence missing from the working tree",
        sorted(path for path in evidence if not (ROOT / path).exists()),
        [],
    )

    promotion = transition["promotion"]
    targets = {entry["target"] for entry in promotion}
    check(
        "promotion targets and existence-only targets overlap",
        sorted(targets & existence_only),
        [],
    )
    check(
        "byte-frozen and existence-only targets cover the required set",
        sorted((targets | existence_only) ^ required),
        [],
    )
    for entry in promotion:
        raw = (ROOT / entry["source"]).read_bytes()
        check(
            pathlib.Path(entry["source"]).name,
            [len(raw), hashlib.sha256(raw).hexdigest()],
            [entry["bytes"], entry["sha256"]],
        )
        check(
            f"{pathlib.Path(entry['target']).name} does not exist yet",
            (ROOT / entry["target"]).exists(),
            entry["target_exists_pre_reset"],
        )

    # Evidence whose location changes is separated from the invariant kind, so
    # the real pre state and the synthetic post state are each exact rather than
    # the union of both phases.
    promoted_evidence = transition["promoted_evidence"]
    check(
        "promoted evidence retires at its old path",
        sorted(
            e["pre_reset"] for e in promoted_evidence if e["pre_reset"] not in retired
        ),
        [],
    )
    check(
        "promoted evidence is required at its new path",
        sorted(
            e["post_reset"]
            for e in promoted_evidence
            if e["post_reset"] not in required
        ),
        [],
    )
    check(
        "promoted evidence is not also invariant evidence",
        sorted(
            path
            for e in promoted_evidence
            for path in (e["pre_reset"], e["post_reset"])
            if path in evidence
        ),
        [],
    )
    check(
        "promoted evidence carries frozen bytes across",
        sorted(
            e["pre_reset"]
            for e in promoted_evidence
            if not any(
                p["source"] == e["pre_reset"] and p["target"] == e["post_reset"]
                for p in promotion
            )
        ),
        [],
    )

    # The superseded v7 cylinder is historical oracle input after the reset, not
    # a product example. Its exact bytes move; its unversioned sibling does not.
    cylinder = ROOT / "examples/steady-flow-past-cylinder.model-v7.json"
    check(
        "the historical cylinder promotion is byte-exact",
        [len(cylinder.read_bytes()), hashlib.sha256(cylinder.read_bytes()).hexdigest()],
        next(
            [p["bytes"], p["sha256"]]
            for p in promotion
            if p["source"] == "examples/steady-flow-past-cylinder.model-v7.json"
        ),
    )
    check(
        "the historical cylinder target sits beside the other historical specimens",
        next(
            p["target"]
            for p in promotion
            if p["source"] == "examples/steady-flow-past-cylinder.model-v7.json"
        ),
        "verify/artifacts/current-model-canonical-identity/expected/historical/"
        "steady-flow-past-cylinder.model-v7.json",
    )
    check(
        "the unversioned cylinder example is preserved unchanged",
        "examples/steady-flow-past-cylinder.model.json" in (inventory - retired),
        True,
    )

    # The staged control-v2 lane froze its own digests for nine of these files.
    # Agreeing with a second, foreign record is what makes this table consumed
    # rather than authored here -- but only if the record is the one that was
    # agreed. The intersection is therefore frozen by name and by count before
    # any digest is compared: a foreign record that drops a fixture, adds one,
    # or renames one no longer intersects over exactly these nine names and
    # fails here, instead of silently agreeing over whatever still overlaps.
    staged = json.loads(
        committed(
            ROOT
            / "verify/interfaces/control-plane-compile-check/oracle/v2/expected/contract.json"
        )
    )["fixtureDigests"]
    names = [pathlib.Path(entry["source"]).name for entry in promotion]
    check("promotion source basenames are unique", len(set(names)), len(names))
    check(
        "the foreign record holds exactly its frozen names",
        sorted(staged),
        STAGED_DIGEST_NAMES,
    )
    check("the foreign record holds exactly nine fixtures", len(staged), 9)
    check(
        "promotion sources the foreign record freezes",
        sorted(name for name in names if name in staged),
        STAGED_DIGEST_NAMES,
    )
    check(
        "promotion sources with no foreign record",
        sorted(name for name in names if name not in staged),
        UNFROZEN_PROMOTION_NAMES,
    )
    compared = [
        pathlib.Path(entry["source"]).name
        for entry in promotion
        if (name := pathlib.Path(entry["source"]).name) in staged
        and [entry["bytes"], entry["sha256"]]
        == [staged[name]["bytes"], staged[name]["sha256"]]
    ]
    check(
        "digests the control-v2 contract independently freezes",
        sorted(compared),
        STAGED_DIGEST_NAMES,
    )
    check("foreign digest comparisons performed", len(compared), 9)

    live = ROOT / "verify/interfaces/control-plane-compile-check"
    check(
        "the retired control-v1 request survives byte-for-byte as retired-v1.json",
        (live / "models/accepted-v1.json").read_bytes(),
        (live / "oracle/v2/models/retired-v1.json").read_bytes(),
    )
    check(
        "the retired control-v1 schema survives byte-for-byte as the historical copy",
        (ROOT / "schemas/control/compile-v1.schema.json").read_bytes(),
        (live / "oracle/v2/expected/historical/compile-v1.schema.json").read_bytes(),
    )
    check(
        "new signal-bearing paths the reset promotes",
        sorted(
            entry["target"]
            for entry in promotion
            if entry["target"] not in inventory
            and carries_model_search_signal((ROOT / entry["source"]).read_bytes())
        ),
        [
            "schemas/control/compile-v2.schema.json",
            "verify/artifacts/current-model-canonical-identity/expected/historical/"
            "steady-flow-past-cylinder.model-v7.json",
            "verify/interfaces/control-plane-compile-check/expected/historical/"
            "compile-v1.schema.json",
        ],
    )


# The post-reset forbidden-token contract, respelled here rather than read out
# of the declaration. A route that quoted the declaration back at itself would
# agree with any declaration, including a silently shortened one.
EXPECTED_RUST_TOKENS = (
    [f"ModelEnvelopeV{n}" for n in range(1, 9)]
    + [f"ModelTransactionEnvelopeV{n}" for n in range(1, 9)]
    + [
        "WireModelEnvelopeV1",
        "WireModelEnvelopeV2",
        "WireModelContentV1",
        "WireModelContentV2",
        "WireModelTransactionEnvelopeV1",
        "WireModelTransactionEnvelopeV2",
        "ExactModelCodec",
        "ModelArtifactGeneration",
        "AcceptedModelEnvelope",
        "VersionedModelTransactionEnvelope",
        "ModelSchemaVersion",
        "TransactionSchemaVersion",
        "ModelOperationWireVersion",
    ]
    + [f"encode_v{n}" for n in range(1, 9)]
    + [f"ensure_v{n}" for n in range(1, 9)]
    + [f"from_program_v{n}" for n in range(2, 9)]
    + [f"from_json_v{n}" for n in range(2, 9)]
    + [f"from_transaction_v{n}" for n in range(2, 9)]
    + [f"digest_v{n}" for n in range(2, 9)]
    + ["reject_coordinate_dependency_before_v8", "exact_codec", "eqiora::compatibility"]
    + [f"eqiora.model-envelope/v{n}" for n in range(1, 8)]
    + [f"eqiora.model-transaction-envelope/v{n}" for n in range(1, 8)]
)
EXPECTED_PYTHON_TOKENS = [
    "eqiora.compatibility",
    "ExactModelCodec",
    "compile_exact",
    "define_exact",
    "replay_exact",
    "exact_codec",
]
EXPECTED_CONTROL_TOKENS = [
    "modelWire",
    "requiredFeatures",
    "model-wire/",
    "CompileFeatureV1",
    "COMPILE_FEATURE_V1",
    "MAX_COMPILE_REQUIRED_FEATURES_V1",
]
EXPECTED_SCOPE_PATHS = {
    "rust-product-source": [
        "crates/eqiora-artifact/src/**/*.rs",
        "crates/eqiora-api/src/**/*.rs",
        "crates/eqiora/src/**/*.rs",
        "crates/eqiora-python/src/**/*.rs",
        "studio/src-tauri/src/**/*.rs",
    ],
    "python-product-source": [
        "bindings/python/python/eqiora/**/*.py",
        "bindings/python/python/eqiora/**/*.pyi",
        "examples/python/**/*.py",
    ],
    "control-product-source": [
        "crates/eqiora-api/src/control/**/*.rs",
        "crates/eqiora-python/src/**/*.rs",
        "studio/src-tauri/src/compile.rs",
        "studio/src/**/*.ts",
        "studio/src/**/*.tsx",
    ],
}
# Named for the same reason the tokens are: the v2 decoder's rejection
# diagnostic may name the protocol it refuses, and the persisted current schema
# keeps its released `v8` spelling.
EXPECTED_PERMITTED = [
    "eqiora.control/v1",
    "eqiora.model-envelope/v8",
    "eqiora.model-transaction-envelope/v8",
    "model.compile-check/v1",
]
# Nothing under these roots may fall inside a declared scope: they hold the
# negative corpus and the historical record the reset must keep.
UNSCANNED = [
    "verify/interfaces/control-plane-compile-check/oracle/v2/schema/compile-v2.schema.json",
    "verify/interfaces/control-plane-compile-check/models/forbidden-model-wire-v2.json",
    "verify/interfaces/control-plane-compile-check/expected/contract.json",
    "verify/artifacts/current-model-canonical-identity/expected/historical/model-v7.json",
    "verify/artifacts/realization-run-wire/expected/realization-v1.json",
    "crates/eqiora-artifact/tests/current_model_wire_oracle.rs",
    "crates/eqiora-api/tests/control_compile_v1.rs",
    "bindings/python/tests/test_control_plane.py",
    "rfcs/0083-current-model-artifact-epoch.md",
    "docs/capability-matrix.md",
    "CHANGELOG.md",
    "schemas/control/compile-v2.schema.json",
    "studio/src/control-protocol.test.ts",
    "studio/src/state.spec.ts",
]


def scope_covers(scope: dict, path: str) -> bool:
    """`**/` is any depth; `*` is any run inside one segment. Nothing else."""
    name = path.rsplit("/", 1)[-1]
    if path in scope.get("exclude_paths", []):
        return False
    if any(
        fnmatch.fnmatchcase(name, pattern) for pattern in scope.get("exclude_names", [])
    ):
        return False
    for pattern in scope["paths"]:
        prefix, marker, tail = pattern.partition("**/")
        if marker:
            if path.startswith(prefix) and fnmatch.fnmatchcase(name, tail):
                return True
        elif fnmatch.fnmatchcase(path, pattern):
            return True
    return False


def scan_forbidden(scopes: list[dict], content: dict[str, str]) -> list[str]:
    """Every (path, token) the declared scopes refuse, as `path:token`."""
    return sorted(
        f"{path}:{token}"
        for scope in scopes
        for path, source in content.items()
        if scope_covers(scope, path)
        for token in scope["forbidden"]
        if token in source
    )


def scoped_content(scopes: list[dict]) -> dict[str, str]:
    """Every checked-in file any declared scope covers, with its text.

    The same trees the candidate sweep skips are skipped here, so the two
    routes measure the same checked-in content rather than a build copy.
    """
    content: dict[str, str] = {}
    pending = [ROOT]
    while pending:
        for entry in sorted(pending.pop().iterdir()):
            relative = entry.relative_to(ROOT).as_posix()
            if entry.is_dir():
                if (
                    entry.name not in SKIPPED_DIRECTORIES
                    and relative not in EXCLUDED_TREES
                ):
                    pending.append(entry)
            elif any(scope_covers(scope, relative) for scope in scopes):
                try:
                    content[relative] = entry.read_text(encoding="utf-8")
                except UnicodeDecodeError:
                    continue
    return content


# The four Rust tokens no product source spells today. They stay forbidden
# after the reset: they name the per-generation entry points a renamed
# historical v2 branch would most plausibly reappear under.
EXPECTED_PROSPECTIVE_RUST_TOKENS = [
    "from_program_v2",
    "from_json_v2",
    "from_transaction_v2",
    "digest_v2",
]

# A synthetic post-reset product source, written independently of the Rust
# oracle's. It spells every deliberately permitted token, and its last three
# entries sit outside every scope.
CLEAN_POST_RESET = {
    "crates/eqiora-artifact/src/model_wire.rs": (
        'pub const SCHEMA: &str = "eqiora.model-envelope/v8";\n'
        "impl ModelEnvelope { fn encode(&self) {} fn ensure(&self) {} }\n"
    ),
    "crates/eqiora-artifact/src/model_transaction_wire.rs": (
        'pub const SCHEMA: &str = "eqiora.model-transaction-envelope/v8";\n'
    ),
    "crates/eqiora-api/src/control/compile.rs": (
        'const PROTOCOL: &str = "eqiora.control/v2";\n'
        'const RETIRED: &str = "eqiora.control/v1";\n'
        'const COMMAND: &str = "model.compile-check/v1";\n'
    ),
    "bindings/python/python/eqiora/__init__.pyi": "def compile(source: str) -> Model: ...\n",
    "studio/src/control-protocol.ts": "export const PROTOCOL = 'eqiora.control/v2';\n",
    "crates/eqiora-api/src/control/tests.rs": 'json!({"modelWire": "x"})\n',
    "studio/src/control-protocol.test.ts": "it('rejects modelWire', () => {});\n",
    "verify/artifacts/current-model-canonical-identity/expected/historical/model-v7.json": (
        '{"schema":"eqiora.model-envelope/v7"}\n'
    ),
}

# One violation per scope, each of the kind path existence cannot see.
SYNTHETIC_VIOLATIONS = [
    (
        "crates/eqiora-artifact/src/model/node.rs",
        "pub(crate) fn encode_v3(node: &KernelNode) { Self::encode(node, WireVersion::V3) }\n",
        "encode_v3",
    ),
    (
        "bindings/python/python/eqiora/__init__.pyi",
        "def compile_exact(source: str, generation: int) -> Model: ...\n",
        "compile_exact",
    ),
    (
        "studio/src/control-protocol.ts",
        "export interface CompileRequest { modelWire?: string }\n",
        "modelWire",
    ),
]


def check_forbidden_product_tokens() -> None:
    """The post-reset-only token contract: declared, narrow, and executable."""
    print("Post-reset forbidden product tokens")
    classification = json.loads(committed(CASE / "expected/classification.json"))
    declared = classification["search"]["forbidden_product_tokens"]
    scopes = declared["scopes"]

    check("contract applies post-reset only", declared["applies"], "post_reset_only")
    check("declared scope count", len(scopes), declared["scope_count"])
    check(
        "declared token count",
        sum(len(scope["forbidden"]) for scope in scopes),
        declared["forbidden_token_count"],
    )
    by_name = {scope["name"]: scope for scope in scopes}
    check("declared scope names", sorted(by_name), sorted(EXPECTED_SCOPE_PATHS))
    for name, paths in EXPECTED_SCOPE_PATHS.items():
        check(f"{name} paths", by_name[name]["paths"], paths)
    check(
        "rust tokens", by_name["rust-product-source"]["forbidden"], EXPECTED_RUST_TOKENS
    )
    check(
        "python tokens",
        by_name["python-product-source"]["forbidden"],
        EXPECTED_PYTHON_TOKENS,
    )
    check(
        "control tokens",
        by_name["control-product-source"]["forbidden"],
        EXPECTED_CONTROL_TOKENS,
    )
    for scope in scopes:
        check(
            f"{scope['name']} repeats no token",
            len(set(scope["forbidden"])),
            len(scope["forbidden"]),
        )

    every = {token for scope in scopes for token in scope["forbidden"]}
    check(
        "deliberately permitted tokens",
        [entry["token"] for entry in declared["deliberately_permitted"]],
        EXPECTED_PERMITTED,
    )
    check(
        "a permitted token is never forbidden, nor contains a forbidden one",
        sorted(
            token
            for token in EXPECTED_PERMITTED
            if token in every or any(forbidden in token for forbidden in every)
        ),
        [],
    )

    check(
        "scopes that reach unscanned evidence",
        sorted(
            f"{scope['name']}:{path}"
            for scope in scopes
            for path in UNSCANNED
            if scope_covers(scope, path)
        ),
        [],
    )
    # The one test-only exclusion is scoped to the control tokens alone.
    check(
        "the control scope excludes its test-only module",
        scope_covers(
            by_name["control-product-source"], "crates/eqiora-api/src/control/tests.rs"
        ),
        False,
    )
    check(
        "a historical Model spelling in product Rust is a branch wherever it sits",
        scope_covers(
            by_name["rust-product-source"], "crates/eqiora-api/src/control/tests.rs"
        ),
        True,
    )
    check(
        "scopes reach the files the reset must clean",
        sorted(
            f"{name}:{path}"
            for name, path in (
                ("rust-product-source", "crates/eqiora-artifact/src/model/node.rs"),
                ("rust-product-source", "crates/eqiora/src/lib.rs"),
                ("rust-product-source", "studio/src-tauri/src/compile.rs"),
                ("python-product-source", "examples/python/fixed_reference_fsi.py"),
                ("control-product-source", "crates/eqiora-api/src/control/schema.rs"),
                ("control-product-source", "studio/src/control-protocol.ts"),
            )
            if not scope_covers(by_name[name], path)
        ),
        [],
    )

    check(
        "a clean post-reset product source is accepted",
        scan_forbidden(scopes, CLEAN_POST_RESET),
        [],
    )
    for path, source, token in SYNTHETIC_VIOLATIONS:
        check(
            f"a surviving `{token}` is refused",
            scan_forbidden(scopes, CLEAN_POST_RESET | {path: source}),
            [f"{path}:{token}"],
        )

    # The pre-reset checkout is never subjected to this contract, and what it
    # does carry is measured rather than asserted. A post-reset-only contract is
    # worth nothing if the tokens were never there to remove -- but "the tree
    # spells all 102" is false, so the exact presence and absence is frozen and
    # checked here instead.
    occurrence = declared["pre_reset_occurrence"]
    check(
        "the occurrence record is measured against the pre-reset state",
        occurrence["measured_state"],
        "pre_reset",
    )
    content = scoped_content(scopes)
    check("scoped product-source files found", len(content) >= 200, True)
    present_total = 0
    absent_total = 0
    for scope in scopes:
        record = next(
            entry for entry in occurrence["scopes"] if entry["name"] == scope["name"]
        )
        covered = [
            source for path, source in content.items() if scope_covers(scope, path)
        ]
        present = [
            token
            for token in scope["forbidden"]
            if any(token in source for source in covered)
        ]
        absent = [token for token in scope["forbidden"] if token not in present]
        check(
            f"{scope['name']} declared token count",
            len(scope["forbidden"]),
            record["declared"],
        )
        check(
            f"{scope['name']} tokens present in the pre-reset tree",
            len(present),
            record["present"],
        )
        check(
            f"{scope['name']} tokens absent from the pre-reset tree",
            absent,
            record["absent"],
        )
        present_total += len(present)
        absent_total += len(absent)
    check(
        "tokens the pre-reset tree carries",
        present_total,
        occurrence["present_token_count"],
    )
    check(
        "prospective post-reset-only tokens",
        absent_total,
        occurrence["prospective_token_count"],
    )
    check(
        "present and prospective cover every declared token",
        present_total + absent_total,
        declared["forbidden_token_count"],
    )
    check(
        "the absent Rust tokens are exactly the four prospective guards",
        next(
            entry["absent"]
            for entry in occurrence["scopes"]
            if entry["name"] == "rust-product-source"
        ),
        EXPECTED_PROSPECTIVE_RUST_TOKENS,
    )
    # Post-reset the contract is unchanged: a prospective guard is enforced
    # exactly like a token that exists today.
    for token in EXPECTED_PROSPECTIVE_RUST_TOKENS:
        check(
            f"a post-reset `{token}` is refused",
            scan_forbidden(
                scopes,
                CLEAN_POST_RESET
                | {
                    "crates/eqiora-artifact/src/model_wire.rs": (
                        f"impl ModelEnvelope {{ fn {token}() {{}} }}\n"
                    )
                },
            ),
            [f"crates/eqiora-artifact/src/model_wire.rs:{token}"],
        )


def main() -> int:
    transition = json.loads(committed(CASE / "expected/transition.json"))
    for entry in transition["deterministic"]:
        check_deterministic(entry)
        print()
    for entry in transition["bridge"]:
        check_bridge(entry)
        print()
    check_retained_realization_v4()
    print()
    for entry in transition["retained_family_goldens"]:
        check_retained_family_golden(entry)
        print()
    for entry in transition["model_input_consumers"]:
        check_model_input_consumer(entry)
        print()
    check_search_exclusions()
    print()
    check_transition_contract()
    print()
    check_inventory_dispositions()
    print()
    check_forbidden_product_tokens()
    print()

    if failures:
        print(f"FAILURES ({len(failures)}):")
        for line in failures:
            print(" ", line)
        return 1
    print("every committed relational identity agrees with the independent derivation")
    return 0


if __name__ == "__main__":
    sys.exit(main())
