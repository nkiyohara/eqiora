# Chordal realization binding verification

This case covers one closed versioned canonical artifact binding an exact
circular-hole source geometry to the chordal realization derived from it, and to
the realized region, mesh, and correspondence resources that realization
accepted. It is reusable and therefore deliberately carries **no Model digest**.

```text
schema    eqiora.circular-hole-chordal-realization-envelope/v1
encoding  eqiora.canonical-json/v1
identity  sha256(schema-domain || 0x00 || canonical JSON)      [RFC 0008]

field order, exactly:
schema, encoding, source_geometry_sha256, realized_geometry_sha256,
mesh_sha256, correspondence_sha256, requested_max_boundary_error_m,
boundary_evaluation_allowance_m, boundary_error_bound_m, circle_segments,
circle_area_deficit_m2, circle_perimeter_deficit_m, reference_minimum_mean_ratio
```

## What this case owns, and what it does not

It owns the encoding and the binding relation, and no geometry, approximation
metric, or resource content. Circle sampling, trigonometric metrics, geometry
vertices, mesh topology, correspondence assignments, and every actual resource
digest belong to existing component contracts, which stay authoritative:

| Upstream authority | SHA-256 |
| --- | --- |
| [`../exact-circular-hole-geometry/oracle.py`](../exact-circular-hole-geometry/oracle.py) | `df423b7848833e2667d8a064542c9adbd88543f054a2d364b90124393cf20d19` |
| [`../circular-hole-chordal-reference-mesh/oracle.py`](../circular-hole-chordal-reference-mesh/oracle.py) | `0bdbbec6f9ff9c532ba5f30c856d1cd3b25e64949e4b11abf5fa3823e6a25742` |

The oracle names both, verifies those digests on every run, and never copies or
executes their algorithms. Re-deriving them would duplicate an authoritative
derivation rather than add evidence; an earlier revision that did so was rejected.

## What is claimed, and what is not

1. The envelope is a **closed canonical content-addressed binding** over
   thirteen field values, with frozen field order and digest domain.
2. **Construction captures, it never accepts a caller-supplied field tuple.**
3. **Validation is relational**: it regenerates and compares rather than trusting
   stored values, and the replay contract below is the whole of it.
4. Every listed mutation or substitution is detected on at least one named axis,
   and the table says which. Where nothing rejects, the row says so.
5. Nothing else: **no realization envelope byte sequence or digest** (the frozen
   envelope bytes here are artificial), **no cross-platform identity of generated
   binary64 coordinates or mesh bytes** (replay over generated values is
   same-environment), **no mesh or correspondence wire layout**, no exact curved
   mesh, CAD kernel, solver value, remeshing policy, or Model binding, and no
   claim that an implementation exists.

## The returned proof, kept

This lane first tried to freeze the literal canonical bytes and digest of the
envelope over the RFC 0081 DFG source, and returned a proof that published
contracts cannot supply them ahead of runtime. The contract owner accepted the
proof and narrowed the lane rather than weakening the gate:

- `realized_geometry_sha256` — the wire is published, but binary64 vertex
  coordinates are not. RFC 0082 pins the mathematical phase only, cross-platform
  mesh-byte identity is an explicit non-claim, and every inscribed polygon with
  at least eight segments has irrational vertices.
- `mesh_sha256` — `eqiora.simplicial-mesh-envelope/v1` has no published canonical
  field order, and inherits the coordinate problem above.
- `correspondence_sha256` — no published wire in any RFC, schema, or document.
- `boundary_error_bound_m`, `circle_area_deficit_m2`, `circle_perimeter_deficit_m`
  — RFC 0082 stores these as *measured* values from the generated loop, so they
  may differ from the closed form in the last places.

Three actual resource digests and three measured binary64 metrics are therefore
unavailable to any known-answer fixture: the claim is relational, and the only
frozen identity here is artificial.

## Replay contract

Frozen as a finite table in oracle and fixture; nothing is manufactured to fill it.

| Step | Phase | Requirement |
| --- | --- | --- |
| k1 | construction | capture every field from already-validated resources |
| k2 | construction | never accept a caller-supplied raw field tuple |
| a | validation | resolve the exact source; its digest must equal `source_geometry_sha256` |
| b | validation | regenerate the chordal owner from that source, the stored request, stored `circle_segments` as a **maximum**, and the stored `reference_minimum_mean_ratio` |
| c | validation | exact-compare every regenerated metric against the stored scalar |
| d | validation | require the supplied realized geometry to equal the regenerated region |
| e | validation | replay mesh and correspondence conformance |
| f | validation | exact-compare all four bound resource digests |

Because the segment count is replayed as a maximum rather than as an answer, a
stored count too small cannot satisfy the stored request, and one too large
regenerates a smaller one. An arbitrary conforming affine mesh is admissible,
including a fixed external one: it enters through its own bound digest and
conformance replay, not by being the mesh this path would have built.

## Detection axes

| Axis | Meaning | Replay steps |
| --- | --- | --- |
| `envelope_digest` | envelope canonical digest | — |
| `source_semantics` | semantic source type/digest | a |
| `owner_replay` | deterministic owner replay | b, c |
| `region_equality` | realized-region equality | d |
| `correspondence` | correspondence conformance | e |
| `resource_digest` | bound resource digest | f |

## Mutations and substitutions

Twenty-one rows in three kinds. **Envelope** mutations change the canonical digest
— executed on the artificial witness — and each also breaks a replay relation.
**Policy** rows are replayed inputs with no regenerated counterpart, so they are
digest changes only: whether the regenerated selection also changes is owned by
the upstream chordal contract, which this oracle does not execute. **Substitution**
rows leave the envelope bytes unchanged, so the canonical digest never catches
them. The oracle machine-checks that every mutable field has a row, that every
required substitution subject is named, that rows declare axes from the vocabulary
above, and that every axis is exercised; `schema` and `encoding` need no row,
since any other value is rejected before encoding.

| Row | Kind | Detected by |
| --- | --- | --- |
| `source_digest_nibble` | envelope | envelope digest, source semantics, resource digest |
| `realized_digest_nibble` | envelope | envelope digest, region equality, resource digest |
| `mesh_digest_nibble` | envelope | envelope digest, resource digest |
| `correspondence_digest_nibble` | envelope | envelope digest, correspondence, resource digest |
| `allowance_halved` | envelope | envelope digest, owner replay |
| `bound_halved` | envelope | envelope digest, owner replay |
| `segments_above` | envelope | envelope digest, owner replay |
| `segments_below` | envelope | envelope digest, owner replay |
| `area_deficit_halved` | envelope | envelope digest, owner replay |
| `perimeter_deficit_halved` | envelope | envelope digest, owner replay |
| `request_halved` | policy | envelope digest only |
| `mean_ratio_halved` | policy | envelope digest only |
| `source_center_perturbed` | substitution | source semantics, owner replay |
| `source_radius_perturbed` | substitution | source semantics, owner replay |
| `source_boundary_identity` | substitution | source semantics |
| `polygonal_source_same_name` | substitution | source semantics — wrong family, refused by type before any digest comparison |
| `realized_vertex_perturbed` | substitution | region equality, resource digest |
| `realized_order_rotated` | substitution | region equality, resource digest |
| `mesh_topology_changed` | substitution | correspondence, resource digest |
| `correspondence_mapping_changed` | substitution | correspondence, resource digest |
| `conforming_mesh_substituted` | substitution | resource digest |

## What is not a falsifier

A changed but internally self-consistent policy, with the realization regenerated
to match, **defines a different valid artifact**: classified as a canonical-digest
change, not an automatic replay rejection, and never to be claimed as rejected.
The oracle freezes one such coherent variant — its identity differs and every
relation holds. Only an externally expected envelope digest separates the two.

## The artificial encoding witness

One 704-byte envelope, identity
`fb1724d42f209a6ccf5e00af35aa5d567ec01e261c12af1aa676083844c980e4`, built from
four artificial 64-hex slots and exact powers of two, with twelve segments.
Guards run on every pass: each slot is a valid but repeated-pair hex string no
content digest can be, each scalar is an exact positive power of two whose compact
JSON spelling is a short plain decimal, and the record is marked
`is_realization_prediction: false`. It freezes the encoding and nothing else;
wiring it as a positive oracle for a real chain is forbidden.

## Status, and how to run

Pre-implementation oracle only: no implementation exists, this case carries no
`case.toml`, and no capability row claims it. One sequencing fact for the
implementing lane — an unregistered directory here is not inert to the local gate.
`local_verify.py:changed_case_ids` derives a case ID from the path shape
`verify/<area>/<case>/` alone, while the registered set comes from a
`verify/*/*/case.toml` glob, so both tiers select
`geometry.circular-hole-chordal-realization-binding` and cannot resolve it, which
the implementing slice closes by adding `case.toml`. This revision changes only
evidence files here and was **not** run against the gate, so no stage result is
asserted for this tree.

```bash
python3 verify/geometry/circular-hole-chordal-realization-binding/oracle/binding_oracle.py
```

Machine-readable `key=value` lines, non-zero exit on any failure; `--emit`
regenerates the frozen fixture, and an ordinary run only compares against it.
