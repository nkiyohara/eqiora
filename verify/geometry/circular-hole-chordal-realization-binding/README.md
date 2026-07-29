# Chordal realization binding verification

This case covers one closed versioned canonical artifact,
`CircularHoleChordalRealizationEnvelopeV1`, that binds an exact circular-hole
source geometry to the chordal realization derived from it, and to the realized
region, mesh, and correspondence resources that realization accepted. Its
schema and digest domain are:

```text
eqiora.circular-hole-chordal-realization-envelope/v1
eqiora.canonical-json/v1
```

Canonical field order is exactly:

```text
schema, encoding,
source_geometry_sha256, realized_geometry_sha256, mesh_sha256,
correspondence_sha256,
requested_max_boundary_error_m, boundary_evaluation_allowance_m,
boundary_error_bound_m, circle_segments,
circle_area_deficit_m2, circle_perimeter_deficit_m,
reference_minimum_mean_ratio
```

The identity is `sha256(schema-domain || 0x00 || canonical JSON)`, the framing
RFC 0008 fixes for every canonical artifact. This is a reusable realization
artifact and therefore deliberately carries **no Model digest**: the same
realization may be bound by more than one Model.

The in-memory chordal owner is the sibling case
[`../circular-hole-chordal-reference-mesh`](../circular-hole-chordal-reference-mesh/README.md)
(RFC 0082), whose nonclaims include a durable source-to-mesh wire. This case is
that wire. The exact source is
[`../exact-circular-hole-geometry`](../exact-circular-hole-geometry/README.md)
(RFC 0081).

## Status

This directory holds a **pre-implementation** oracle only. No implementation of
the envelope exists yet, this case carries no `case.toml`, and no capability row
claims it. The oracle, its frozen expected fixture, and its falsifier table were
frozen before any implementation began, by a lane that wrote no production Rust
and read none to obtain a value.

An unregistered directory here is **not** inert to the local gate. The planner
derives a case ID from the path shape `verify/<area>/<case>/` alone
(`local_verify.py:changed_case_ids`) without checking that a manifest exists, so
`fast` and `affected` both select
`geometry.circular-hole-chordal-realization-binding` and run

```text
cargo run --locked -p eqiora-verify -- run --case geometry.circular-hole-chordal-realization-binding
```

which exits 1 with `unknown verification case ID`. Until this case is
registered, the gate therefore fails on this directory's mere presence. That is
a known, recorded consequence of freezing the oracle ahead of the manifest, not
a defect in the oracle; the implementing slice closes it by adding the
`case.toml` that registers the exact claim.

Recorded on this tree, `python3 tools/ci/local_verify.py fast --base
origin/main`: formatting passed, 1515 Rust tests passed with none failing,
default-feature Clippy passed, `geometry.authored-planar-geometry-artifact`
passed, and the gate then stopped fail-fast on the unregistered case above with
exit status 2. Every stage that does not depend on this manifest passes.

## Replay contract

External replay regenerates rather than trusts. From the exact source resolved
by `source_geometry_sha256`, it regenerates the chordal owner using the stored
`requested_max_boundary_error_m`, the stored `circle_segments` as the segment
*maximum*, and the stored `reference_minimum_mean_ratio` as the quality
threshold. It then requires:

- exact equality for the deterministic metrics that regeneration reproduces;
- the realized geometry to equal the regenerated region;
- correspondence conformance to replay; and
- all four bound resource digests to match exactly.

Because the segment count is replayed as a maximum rather than as an answer, a
stored count that is too small cannot satisfy the stored request, and a stored
count that is too large regenerates a smaller one. Both directions fail.

The envelope supports an arbitrary conforming affine mesh, including a fixed
external mesh. What the regenerated chordal owner proves is the exact-source to
realized-region step; the mesh is admitted through its own bound digest and the
correspondence conformance replay, not by being the mesh this path would have
generated.

## Frozen oracle

The non-implementing oracle is
[`oracle/binding_oracle.py`](oracle/binding_oracle.py), SHA-256
`1d920610df68b7256bda2f9186978aaec4df11dc4546bd11d6e4b3192ccb83db`. It is
standard library only, runs at 80 decimal digits, and reports 73 checks with 0
failures. Its expected values are frozen in
[`expected/binding-contract.json`](expected/binding-contract.json), which the
oracle re-derives and compares byte-for-byte on every run.

It derives, from published contracts alone:

- the canonical binary64 rendering rule of `eqiora.canonical-json/v1`,
  reconstructed from its branch structure and validated against two frozen
  repository literals this lane did not author — the 482-byte RFC 0079
  square-with-hole region and the 511-byte RFC 0081 DFG exact source. This
  matters: a naive `repr` spells `1e-05`, while the canonical wire spells the
  same value `0.00001`;
- the exact source identity
  `b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`;
- the evaluation allowance `6.252776074688882e-14 m`, shown to be an *exact*
  dyadic product `2476979795053773 / 2^95` in binary64, so no rounding enters
  it and its association order is irrelevant;
- the accepted segment count 50, by two mutually independent routes — a
  monotone search over `sagitta(n)` that uses no inverse function at all, and
  RFC 0082's stable half-angle inverse — required to agree; and
- the ideal sagitta, area-deficit, and perimeter-deficit at 80 digits from an
  independently implemented `pi`/`sin`/`cos`/`asin` kernel.

The only inequalities it asserts are the ones RFC 0082 already mandates: the
accepted bound never exceeds the request, the request exceeds the allowance,
and the count is at least eight. It introduces no tunable numeric tolerance.

## What the oracle deliberately does not freeze

Three of the four bound resource digests are **not derivable** from published
contracts, and are reported as blocked rather than invented:

| Field | Why it is blocked |
| --- | --- |
| `realized_geometry_sha256` | The wire is published (RFC 0079), but the binary64 vertex coordinates are not determined. RFC 0082 pins only the mathematical phase `theta_i = 2 pi i / n`; it pins neither a binary64 association order nor a correctly-rounded transcendental, and RFC 0082 and the capability matrix both explicitly non-claim cross-platform mesh-byte identity. Every regular inscribed polygon with `n >= 8` has irrational vertices, so no choice of witness avoids this. |
| `mesh_sha256` | `eqiora.simplicial-mesh-envelope/v1` has no published canonical field order. RFC 0013 lists its content only; no RFC, schema, or document gives its key spelling, field order, or vertex/cell numbering rule. It also inherits the coordinate block above. |
| `correspondence_sha256` | `eqiora.geometry-mesh-correspondence-envelope/v1` has no published wire in any RFC, schema, or document. RFC 0049 additionally closes it over one exact Model artifact, whose Domain ULIDs are author-chosen and are not determined by the frozen claim. |

The same block reaches `boundary_error_bound_m`,
`circle_area_deficit_m2`, and `circle_perimeter_deficit_m`, which RFC 0082
stores as *measured* rather than closed-form values. The oracle freezes their
ideal high-precision values and the binary64 spelling of those ideals, and
states that a measured value may differ in the last places.

The consequence is stated plainly: **the DFG envelope's literal canonical bytes
and digest cannot be frozen ahead of implementation.** What is frozen instead
is the total function `canonical_envelope(values)`, which takes the thirteen
real field values and derives the canonical bytes and identity independently.
That is the oracle an implementation must agree with byte-for-byte.

The fixture additionally carries one 765-byte *encoding witness* whose three
blocked fields are explicitly declared slots, so that the byte production and
every single-field mutation are exactly checkable today. It is marked
`is_dfg_realization_prediction: false` and must never be wired as a positive
oracle for the real chain.

## Falsifiers

Twenty-one falsifiers are frozen; eighteen carry exact expected bytes or
digests. Each is classified by the failure it must produce.

| Falsifier | Failure mode |
| --- | --- |
| each of the four bound digests, one nibble | canonical-byte/digest, plus semantic-source for the source and resource-digest for the other three |
| requested error, evaluation allowance, realized error bound, both deficits, minimum mean ratio, each one ulp | canonical-byte/digest and deterministic replay mismatch |
| segment count 51 and 49 | canonical-byte/digest and deterministic replay mismatch, in both directions |
| exact-circle centre one ulp, radius one ulp, circular boundary set membership | semantic source mismatch, each with an exact mutated source identity |
| a same-named polygonal source carrying all five entity-set names | semantic source mismatch; a different family under a different schema domain is not the exact circle |
| realized boundary vertex one ulp | realized-region and resource-digest mismatch |
| externally supplied rotated realized outer loop | realized-region and canonical-byte mismatch; rejected at admission rather than renormalized |
| mesh boundary topology, correspondence entity mapping | conformance and resource-digest mismatch |
| a valid conforming mesh substituted without updating the bound digest | resource-digest mismatch, before any conformance work |

The last three are contract-level rather than byte-frozen, for exactly the
reason the blocked table gives. Signed-zero normalization is checked as
identity-*preserving*, not identity-changing.

## Run

```bash
python3 verify/geometry/circular-hole-chordal-realization-binding/oracle/binding_oracle.py
```

It prints machine-readable `key=value` lines and exits non-zero on any failure.
`--emit` regenerates the frozen fixture; an ordinary run only compares against
it.

## Not claimed

No mathematically exact curved mesh edge, NURBS or isogeometric basis, generic
CAD kernel, cross-platform mesh byte identity, solver accuracy, DFG benchmark
coefficient, automatic remeshing policy, or Model binding is claimed. Nothing
here claims an implementation exists, and nothing here claims the DFG
realization chain's literal digests.
