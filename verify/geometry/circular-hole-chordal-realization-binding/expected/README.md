# Expected values

The non-implementing stdlib oracle is
[`../oracle/binding_oracle.py`](../oracle/binding_oracle.py), SHA-256
`0351c223c8100a96f4d11babcf46200737554f996653e9e58ab3378fa6240a41`.
It reports 60 checks with 0 failures and freezes every expected value in
[`binding-contract.json`](binding-contract.json), 3690 bytes.
An ordinary run re-derives that fixture and compares it byte-for-byte.

These values are frozen before binding implementation. None is derived from
production output, and none may be tuned or relaxed by the implementing lane.

## Artificial envelope identity

Exact compact canonical bytes of the synthetic encoding witness, 703 bytes:

```json
{"schema":"eqiora.circular-hole-chordal-realization-envelope/v1","encoding":"eqiora.canonical-json/v1","source_geometry_sha256":"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a","realized_geometry_sha256":"6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b","mesh_sha256":"7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c","correspondence_sha256":"8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d8d","requested_max_boundary_error_m":0.0625,"boundary_evaluation_allowance_m":0.00390625,"boundary_error_bound_m":0.03125,"circle_segments":12,"circle_area_deficit_m2":0.015625,"circle_perimeter_deficit_m":0.125,"required_minimum_mean_ratio":0.5}
```

| Quantity | Value |
| --- | --- |
| canonical bytes | 703 |
| identity `sha256(schema-domain \|\| 0x00 \|\| bytes)` | `b35012b4177cfd150e0cdeaaf4901b8d0976c50b2b929dde42885aa5bf6234ee` |
| `is_realization_prediction` | `false` |

The four digest slots are synthetic repeated-pair sentinels selected by this
lane and not copied from runtime resources. They are valid lowercase 64-hex
values; no claim is made that such a bit pattern cannot be a SHA-256 output.
The six floating scalars are exact positive powers of two with short plain JSON
spellings, and the segment count is twelve.

This witness freezes only the bytes shown, their field order, and digest
framing. The oracle's Python JSON producer is authoritative for this selected
dyadic witness and selected dyadic mutants only. Production `serde_json` owns
canonical spelling for arbitrary runtime binary64 observations.

## Identity mutation roll

Twelve single-field mutations are executed; each is a valid encoding with an
identity different from the witness, and all twelve identities are distinct.

| Quantity | Value |
| --- | --- |
| `mutation_digest_roll` | `ea8cef92d9a17a7938ef952d3a82945f2b3e0a447c596f58de343dae341e154e` |

The roll is SHA-256 over sorted `"<id>=<digest>"` lines. The first ten rows bind
digest or regenerated-observation mismatches to relational detectors. The two
policy rows freeze identity change only; deterministic production regeneration
owns whether replay rejects or lands on a valid policy plateau.

## Admission and substitution tables

| Table | Rows | Frozen meaning |
| --- | ---: | --- |
| pre-replay admission falsifiers | 11 | malformed or over-budget input is rejected by `decoder_admission` before relational replay |
| resource substitutions | 16 | unchanged envelope bytes are rejected by the named resource relation |
| all detector rows | 39 | 12 identity mutations + 11 admission faults + 16 substitutions |

Exact row identifiers and axes are frozen in `binding-contract.json`. The oracle
checks exact equality with the required class sets rather than prefix matching.

## Encoding-only policy variant

| Quantity | Value |
| --- | --- |
| artificial policy variant identity | `8f6a92ea083569280a2d7f8a959983ab8c7a3834c285d0fa1f1a586c7594f3d0` |
| classification | `canonical_digest_change` |
| `replay_outcome` | `not_evaluated` |

This proves only that changed artificial values produce a distinct encoding
identity. It owns no resources and supplies no evidence that relational replay
accepts or rejects.

## Deliberately absent expected values

`realized_geometry_sha256`, `mesh_sha256`, and `correspondence_sha256` are not
frozen as runtime constants, nor are the three measured binary64 observations.
No upstream geometry, mesh, correspondence, or numerical derivation is copied
into this directory.
