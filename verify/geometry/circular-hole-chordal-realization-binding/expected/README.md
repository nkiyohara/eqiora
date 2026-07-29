# Expected values

The non-implementing oracle is
[`../oracle/binding_oracle.py`](../oracle/binding_oracle.py), SHA-256
`10dffe52de2078f6b936c7ece32db35beb367bc63111853e5a09e04e6ea695ec`. It reports
47 checks with 0 failures and freezes every expected value in
[`binding-contract.json`](binding-contract.json), 3568 bytes, which an ordinary
run re-derives and compares byte-for-byte, so the file cannot drift from the
derivation that produced it.

These values are frozen ahead of implementation. None is derived from production
output, and none may be tuned or relaxed by the implementing lane: an
implementer who believes a value is wrong returns the proof rather than
adjusting the value.

## The one frozen envelope identity

Exact compact canonical bytes of the artificial encoding witness, 704 bytes:

```json
{"schema":"eqiora.circular-hole-chordal-realization-envelope/v1","encoding":"eqiora.canonical-json/v1","source_geometry_sha256":"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a","realized_geometry_sha256":"6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b","mesh_sha256":"7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c","correspondence_sha256":"8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d","requested_max_boundary_error_m":0.0625,"boundary_evaluation_allowance_m":0.00390625,"boundary_error_bound_m":0.03125,"circle_segments":12,"circle_area_deficit_m2":0.015625,"circle_perimeter_deficit_m":0.125,"reference_minimum_mean_ratio":0.5}
```

| Quantity | Value |
| --- | --- |
| canonical bytes | 704 |
| identity `sha256(schema-domain \|\| 0x00 \|\| bytes)` | `fb1724d42f209a6ccf5e00af35aa5d567ec01e261c12af1aa676083844c980e4` |
| `is_realization_prediction` | `false` |

Every field is chosen by this lane: four artificial 64-hex slots, six exact
powers of two, and a segment count of twelve. Guards run on every pass — each
slot is a valid hex string of one repeated pair, which no content digest can be;
each scalar is an exact positive power of two whose compact JSON spelling is a
short plain decimal that round-trips; the segment count is an integer of at
least eight; and the stored bound and allowance stay within the stored request.

What it is for: it freezes canonical byte production, the frozen field order as
it appears in those bytes, and the exact effect of every single-field mutation,
all checkable today without production output. What it is not: a realization
envelope digest. Wiring it as a positive oracle for a real chain would be a
false positive and is forbidden.

The reusable expected value is `canonical_json(values)` in the oracle: given the
thirteen field values an implementation captured, it independently derives the
canonical bytes those values must have, and rejects any map whose vocabulary is
unknown, missing a field, reordered, or carrying an unsupported schema or
canonical-encoding identifier.

## Falsifier expected values

Twelve single-field mutations of the witness are executed on every run; each must
produce a valid encoding with an identity different from the witness, and all
twelve must be mutually distinct. Rather than freeze twelve digests of variants
no implementation ever produces, the fixture pins them collectively:

| Quantity | Value |
| --- | --- |
| `mutation_digest_roll` | `5844e35fa417e0c2e55f74ad42a90f34e6d02ee8007ff5452870d65add301743` |

The roll is `sha256` over `"<id>=<digest>"` lines for the twelve mutations,
sorted by id, so any change to any one of them breaks it.

Nine substitutions carry no envelope digest at all: they leave the envelope bytes
unchanged by construction, and the fixture records the replay axis that catches
each instead.

## Not a falsifier

| Quantity | Value |
| --- | --- |
| coherent policy variant identity | `2f64339cda832f74f0ef046426a357fa0a8b3f338ec0a0cc988ee7a0470ecc12` |
| classification | `canonical_digest_change` |
| `replay_rejects` | `false` |

A self-consistent policy change with the realization regenerated to match is a
distinct valid artifact. It is expected to differ in identity and to be rejected
by nothing.

## Deliberately not expected values

`realized_geometry_sha256`, `mesh_sha256`, and `correspondence_sha256` are not
frozen as constants, and neither are the three measured binary64 metrics.
Published contracts do not determine them ahead of runtime; the case README
carries the returned proof per field. No row here requires them as known
answers, and no upstream geometry, metric, or resource digest is re-derived in
this directory.
