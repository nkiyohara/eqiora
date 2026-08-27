# References

## Route record

[`derive_transition_identities.py`](derive_transition_identities.py) is the
independent route for every literal this case freezes. It imports, links, and
executes nothing from the Rust producer. Run it from the repository root:

```bash
python3 verify/artifacts/current-model-relational-identity-transition/references/derive_transition_identities.py
```

It exits non-zero if any committed literal disagrees with the derivation.

For each deterministic fixture it re-renders the committed Model bytes as
compact JSON, checks that nodes, values, edges, and boundary members are sorted
and unique under the `EntityKind` declaration order, rebuilds the RFC 0008
schema-domain preimage by hand — schema bytes, one NUL, then the content
projection that omits `source_revision` — and hashes it with `hashlib`. It then
recomputes every downstream artifact's digest in that artifact's own domain,
reads every reference edge out of the downstream bytes at its exact JSON path,
closes the ALE segment and root chain from complete intermediate bytes, and
proves each replacement is the original raw byte stream with only its unique
same-length identity literals substituted.

For each historical bundle it hashes the untouched bytes, derives the historical
artifact digest from those bytes without a product decoder, checks that the
recorded Realization and Run still observe the historical Model, and checks that
the current bridge Model carries the same Model ULID and revision, a different
schema-domain digest, and an equal generation-v2 fingerprint.

The same route hashes the retained Realization v4 payload as 8,333 opaque
canonical bytes in the `eqiora.realization-envelope/v4` domain. It never decodes
the historical Model reference embedded by that separately versioned family.

It finally re-walks the repository with its own implementation of the candidate
sweep — the frozen 338-path inventory is independent evidence only if a second
route reproduces it — and checks the two-state transition contract. The counts
are frozen individually and are not one partition: 52 retired, of which 42 are
inventory members and 10 carry no search signal; 296 preserved; and 13 required
after the reset, which are additions and one in-place replacement rather than
inventory members. What partitions the 338 candidates is 42 + 296, and the route
checks that sum rather than restating the three headline numbers. It then checks
that every retired path exists today, that the version-named source owners
`model_v8.rs` and
`model_transaction_v8.rs` retire while the unversioned `model_wire.rs` and
`model_transaction_wire.rs` are required afterwards and do not exist yet, that
each staged source carries its frozen length and digest, that those digests
agree with the nine the control-v2 lane independently froze in its own
`fixtureDigests` — with that intersection frozen by name and by count first, so
a foreign record that drops, adds, or renames a fixture fails instead of
silently agreeing over whatever still overlaps — that the retired control-v1
request and schema survive
byte-for-byte at their promoted paths, that the superseded v7 cylinder's 16,798
bytes are promoted to the historical specimen directory while its unversioned
sibling stays preserved, that promoted evidence is invariant evidence at neither
path and carries frozen bytes across, and that exactly three of the promoted
paths carry a Model search signal.

Two further partitions are checked here rather than assumed. First, the sweep's
self-exclusion: this case's executable oracle is two files — the integration-test
root and the private support module it includes — and this route checks that
exactly those two are excluded, that each really does carry the search signal
that makes the exclusion necessary, that neither is also a classified candidate,
and that ordinary test files such as `current_model_wire_oracle.rs` are still in
the inventory. The exclusion is by exact path; nothing about a directory or
about tests in general is inferred.

Second, the fates: the seven-name disposition vocabulary is respelled here, every
entry naming paths must declare one of them, no path may be named twice, and the
220-path remainder plus the 118 explicitly classified paths must come to exactly
338. It checks by name that the fifteen compatibility-only and eight displaced
application-shaped FSI lifecycle paths are
deleted; that `model_v2.rs` and `model_transaction_v2.rs` are decomposed by
claim, with the historical branch deleted and the current v8 implementation
migrating to `model_wire.rs` and `model_transaction_wire.rs` respectively; that
`model_v8.rs` and `model_transaction_v8.rs` are renamed into those same two
owners; and that no retired inventory path carries a fate only a surviving path
could have. Both rename directions are respelled here as ordered `from`/`to`
pairs and compared pairwise, so swapping the Model and Transaction targets fails
this route as well as the Rust one; the parallel `paths`/`renames_to` arrays are
accepted only because the classification declares their pairing positional, and
this route zips them strictly and compares against the same frozen pairs. The
twenty-seven formerly unnamed retired paths are re-counted here as 23 + 2 + 2.

## The post-reset forbidden-token contract, derived twice

The 102 forbidden tokens, the three scope path lists, and the four deliberately
permitted tokens are **respelled here** rather than read out of
`classification.json`. A route that quoted the declaration back at itself would
agree with any declaration, including a silently shortened one, so this route
generates `encode_v1`--`encode_v8`, the schema strings, and the rest from their
own ranges and compares.

It then re-implements the scope matcher and the substring scan, and runs both
over its own synthetic content maps: a clean post-reset product source that
spells every permitted token is accepted, and one Rust private generation
branch, one Python exact-codec selector, and one control-plane selector are each
refused by name. It checks that no scope reaches a verify fixture, conformance
kit, crate test directory, RFC, documentation page, changelog, schema, or
retained golden; that the one Rust test-only exclusion applies to the control
tokens and not to the Rust ones; and that each scope does reach the files the
reset must actually clean.

The last check is the honest one, and it is a measurement rather than a slogan.
A contract whose tokens were absent before the reset would be checking nothing,
so this route walks every file the three scopes cover and reports, per scope,
which forbidden tokens the pre-reset tree spells and which it does not: 86 of
the 90 Rust tokens, 6 of 6 Python, 6 of 6 control — 98 present, and exactly
`from_program_v2`, `from_json_v2`, `from_transaction_v2`, and `digest_v2`
absent. Those four are prospective post-reset guards, not observations: they
name the per-generation entry points a renamed historical v2 branch would most
plausibly reappear under, so the route also checks that each of them is refused
after the reset exactly like a token that exists today. This case never claims
that the current checkout spells all 102.

## How the candidate bytes were observed

The five accepted deterministic producers were replayed once, outside this
repository, through their already-live current encoder — the only change was the
selected codec — and their canonical bytes were captured. The bridge Models were
built by decoding each recorded accelerator Model through its historical decoder
while that decoder still exists and re-encoding the same semantic program
through the current owner.

Those observations are how the candidates were *found*. They are not the
authority: every committed value above is re-derived from the committed bytes
alone by this script and re-checked by the registered Rust test through the
current owner. The reset writer is a separate lineage and may wire these values
but may not regenerate or select them.

## What this route does not own

It does not derive the structural semantic fingerprint byte projection; RFC 0073
owns that, and this case owns only the equality relation between two freshly
observed values. It does not assert `serde_json`'s float notation: numbers are
normalized to one shared spelling before byte comparison, because this route and
Rust choose positional versus exponential form at different magnitudes. The
producer's exact float bytes are owned by
`artifacts.current-model-canonical-identity` and by the registered Rust test,
which round-trips these exact bytes through the current codec.
