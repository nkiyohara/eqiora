# Chordal realization binding verification

This case covers one closed versioned canonical artifact binding an exact
circular-hole source geometry to its chordal realization observations, realized
region, mesh, and Model-free authored-region correspondence. It deliberately
carries no Model digest.

```text
schema    eqiora.circular-hole-chordal-realization-envelope/v1
encoding  eqiora.canonical-json/v1
identity  sha256(schema-domain || 0x00 || canonical JSON)      [RFC 0008]

field order, exactly:
schema, encoding, source_geometry_sha256, realized_geometry_sha256,
mesh_sha256, correspondence_sha256, requested_max_boundary_error_m,
boundary_evaluation_allowance_m, boundary_error_bound_m, circle_segments,
circle_area_deficit_m2, circle_perimeter_deficit_m, required_minimum_mean_ratio
```

`required_minimum_mean_ratio` is the input threshold used to construct the
source-owned chordal reference mesh. It is neither that reference mesh's
measured minimum mean ratio nor the acceptance threshold of a separately
supplied conforming mesh artifact.

## What this case owns, and what it does not

It owns the encoding and binding relation, not geometry, approximation metrics,
or resource content. Circle sampling, trigonometric metrics, geometry vertices,
mesh topology, correspondence assignments, and every actual resource digest
belong to existing component contracts:

| Upstream authority | SHA-256 |
| --- | --- |
| [`../exact-circular-hole-geometry/oracle.py`](../exact-circular-hole-geometry/oracle.py) | `df423b7848833e2667d8a064542c9adbd88543f054a2d364b90124393cf20d19` |
| [`../circular-hole-chordal-reference-mesh/oracle.py`](../circular-hole-chordal-reference-mesh/oracle.py) | `0bdbbec6f9ff9c532ba5f30c856d1cd3b25e64949e4b11abf5fa3823e6a25742` |

The oracle verifies both digests on every run and never copies or executes their
algorithms. Re-deriving them would duplicate authoritative derivations.

## Claim and nonclaims

1. The envelope is a closed canonical content-addressed binding over thirteen
   field values, with frozen field order and digest domain.
2. Construction captures fields from validated resources and never accepts a
   caller-supplied raw field tuple.
3. Binding-envelope admission is bounded and precedes relational replay. Each
   supplied resource remains admitted by its existing family-specific decoder.
4. Relational validation regenerates and compares rather than trusting stored
   values. The replay contract below is complete for this bounded family.
5. Every required mutation, admission fault, or substitution has a named
   detector axis.
6. Nothing else: the artificial bytes are **not a realization prediction**;
   there is no cross-platform identity claim for generated binary64 coordinates
   or mesh bytes, no general mesh or correspondence wire claim, no exact curved
   mesh, CAD kernel, solver value, remeshing policy, Model binding, or claim that
   an implementation exists.

## Why no actual binding identity is frozen

Published contracts cannot supply the three generated downstream resource
digests or three measured binary64 observations ahead of runtime:

- `realized_geometry_sha256` inherits generated binary64 polygon coordinates.
- `mesh_sha256` inherits those coordinates and the realized topology.
- `correspondence_sha256` is the digest of the accepted Model-free
  `authored-planar-region-v1` correspondence, whose assignments and bound
  geometry/mesh digests inherit the same generated resources.
- `boundary_error_bound_m`, `circle_area_deficit_m2`, and
  `circle_perimeter_deficit_m` are measured from the generated loop.

The claim is therefore relational. Runtime resources are captured and replayed;
the only frozen identity here is a deliberately synthetic encoding witness.

## Replay contract

| Step | Phase | Requirement |
| --- | --- | --- |
| k1 | construction | capture every field from already-validated resources |
| k2 | construction | never accept a caller-supplied raw field tuple |
| p | admission | decode the binding envelope under its closed canonical vocabulary and budgets; require the four resources to have passed their separately owned bounded decoders before replay |
| a | validation | resolve the exact source; its digest must equal `source_geometry_sha256` |
| b | validation | regenerate the chordal owner from that source, the stored request, stored `circle_segments` as a maximum, and stored `required_minimum_mean_ratio` |
| c | validation | exact-compare every regenerated observation against its stored scalar |
| d | validation | require the supplied realized geometry to equal the regenerated region |
| e | validation | replay the Model-free `authored-planar-region-v1` correspondence against the supplied realized geometry and mesh |
| f | validation | exact-compare all four bound resource digests |

The selected segment count is replayed as a maximum, not trusted as an answer.
A separately constructed conforming affine mesh is admissible only through its
own mesh and authored-region correspondence digests after the same admission and
replay checks.

## Detector axes

| Axis | Meaning | Steps |
| --- | --- | --- |
| `envelope_digest` | envelope canonical digest | — |
| `decoder_admission` | binding-envelope decoder admission | p |
| `source_semantics` | semantic source type/digest | a |
| `owner_replay` | deterministic owner replay | b, c |
| `region_equality` | realized-region equality | d |
| `authored_correspondence` | Model-free authored-region correspondence replay | e |
| `resource_digest` | bound resource digest | f |

## Identity mutations

The first ten rows change one digest or regenerated observation. They change
identity and break a named resource relation against unchanged resources.

| Row | Field | Detected by |
| --- | --- | --- |
| `source_digest_nibble` | source digest | envelope digest, source semantics, resource digest |
| `realized_digest_nibble` | realized digest | envelope digest, region equality, resource digest |
| `mesh_digest_nibble` | mesh digest | envelope digest, resource digest |
| `correspondence_digest_nibble` | correspondence digest | envelope digest, authored correspondence, resource digest |
| `allowance_halved` | evaluation allowance | envelope digest, owner replay |
| `bound_halved` | measured bound | envelope digest, owner replay |
| `segments_above` | segment count | envelope digest, owner replay |
| `segments_below` | segment count | envelope digest, owner replay |
| `area_deficit_halved` | area deficit | envelope digest, owner replay |
| `perimeter_deficit_halved` | perimeter deficit | envelope digest, owner replay |

The two policy rows always change identity. Their relational outcome is owned by
deterministic regeneration, not this artificial oracle: a policy change may
reject, or may replay to the same four resources on a policy plateau.

| Row | Field | Frozen detector | Relational outcome |
| --- | --- | --- | --- |
| `request_halved` | requested error | envelope digest | owned by deterministic regeneration |
| `mean_ratio_halved` | required quality | envelope digest | owned by deterministic regeneration |

## Pre-replay admission falsifiers

Each row has no admitted envelope identity and is rejected by
`decoder_admission` before any resource relation is evaluated:

| Row | Fault class |
| --- | --- |
| `unknown_vocabulary` | unsupported schema or encoding vocabulary |
| `missing_vocabulary` | a required field is absent |
| `reordered_vocabulary` | fields are not in canonical order |
| `extra_vocabulary` | an extra field is present |
| `noncanonical_json` | valid but noncanonical JSON bytes |
| `malformed_digest` | a digest is not 64 lowercase hex characters |
| `nonfinite_scalar` | a scalar is NaN or infinite |
| `zero_scalar` | a required-positive scalar is zero |
| `negative_scalar` | a required-positive scalar is negative |
| `byte_budget_overflow` | encoded bytes exceed the decoder budget |
| `depth_budget_overflow` | JSON nesting exceeds the decoder budget |

The stdlib oracle executes these classes only for the binding envelope in its
synthetic dyadic wire domain. It does not execute or re-prove upstream resource
decoders. Production `serde_json` owns arbitrary runtime binary64 spelling and
the production binding limits; each resource family owns its own admission.
Malformed-digest mutation is exercised for all four digest fields, and each
non-finite/non-positive class is exercised for every applicable scalar field.

## Resource substitutions

Substitutions leave envelope bytes unchanged, so only relational replay detects
them.

| Row | Detected by |
| --- | --- |
| `source_center_perturbed` | source semantics, owner replay |
| `source_radius_perturbed` | source semantics, owner replay |
| `source_boundary_identity` | source semantics |
| `polygonal_source_same_name` | source semantics |
| `realized_vertex_perturbed` | region equality, resource digest |
| `realized_order_rotated` | region equality, resource digest |
| `mesh_refined` | authored correspondence, resource digest |
| `mesh_renumbered` | authored correspondence, resource digest |
| `mesh_topology_changed` | authored correspondence, resource digest |
| `correspondence_relabelled` | authored correspondence, resource digest |
| `correspondence_incomplete` | authored correspondence, resource digest |
| `correspondence_reoriented` | authored correspondence, resource digest |
| `correspondence_stale` | authored correspondence, resource digest |
| `correspondence_inlet_outlet_swapped` | authored correspondence, resource digest |
| `correspondence_exterior_hole_swapped` | authored correspondence, resource digest |
| `conforming_mesh_substituted` | resource digest |

The oracle machine-checks exact membership of the 11 admission and 16
substitution classes, all 12 identity-mutation rows, unique row names, known
axes, complete mutable-field coverage, and use of every axis: 39 rows total.

## Encoding-only policy variant

The oracle freezes one artificial policy variant only to prove a different
canonical identity. It does not own resources and does not evaluate whether
relational replay accepts or rejects it:

```text
classification  canonical_digest_change
replay_outcome  not_evaluated
```

Production evidence later supplies both a rejecting policy change and a
plateau-preserving valid change through the actual deterministic owner.

## Artificial encoding witness

The synthetic witness has four repeated-pair 64-hex sentinel slots and exact
positive dyadic scalars. Those sentinels were chosen by this lane and were not
copied from runtime resources. No claim is made that SHA-256 cannot output the
same bit pattern. The fixture explicitly records
`is_realization_prediction: false`.

Its exact bytes, identity, mutation roll, and encoding-only variant identity are
listed in [`expected/README.md`](expected/README.md). The Python encoder is
authoritative only for this selected dyadic witness and its selected dyadic
mutants; it is not a general replacement for production canonical JSON.

## Status and run

Pre-implementation oracle only: no implementation exists, this directory has no
`case.toml`, and no capability row claims it. The implementing slice must add
the manifest before invoking repository gates because an unregistered
`verify/<area>/<case>/` path is selected but cannot be resolved.

```bash
python3 verify/geometry/circular-hole-chordal-realization-binding/oracle/binding_oracle.py
```

Machine-readable `key=value` lines are emitted with non-zero exit on failure.
`--emit` regenerates the frozen fixture; an ordinary run only compares it.
