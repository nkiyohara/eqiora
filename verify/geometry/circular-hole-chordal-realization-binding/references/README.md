# References

Every value in this case is either read from a contract published in this
repository, or chosen by this lane and labelled as such at every appearance. No
production Rust was read or run to obtain a value, and no value was copied from
production test output.

## Contracts consulted

- [RFC 0008](../../../../rfcs/0008-canonical-artifact-wire-v1.md) — the
  canonical JSON encoding identifier and the domain-separated digest framing
  `sha256(schema-domain || 0x00 || canonical bytes)` this envelope reuses
  unchanged, plus the rule that every admitted finite IEEE-754 value must
  survive serialize/decode/serialize as the identical value and bytes.
- [RFC 0079](../../../../rfcs/0079-authored-planar-geometry-artifact.md) — the
  straight-edged planar region wire the realized region is encoded in, and its
  canonical normalization order, which is why a rotated boundary loop is not a
  canonical encoding of anything.
- [RFC 0081](../../../../rfcs/0081-exact-circular-hole-geometry.md) — the exact
  circular-hole wire and its distinct schema domain, which is what refuses a
  same-named polygonal source by type before any digest comparison.
- [RFC 0082](../../../../rfcs/0082-source-bound-chordal-circular-hole-mesh.md) —
  the approximation contract: the caller segment limit that the stored count is
  replayed as a maximum against, the quality threshold, the minimum of eight
  segments, and the statement that the stored metrics are *measured* from the
  generated loop rather than accepted from a closed form.
- [RFC 0049](../../../../rfcs/0049-geometry-identity-and-mesh-correspondence.md)
  — correspondence semantics, and the reason its digest is not derivable here:
  it is closed over one exact Model artifact whose Domain ULIDs are
  revision-local and author-chosen.
- [RFC 0013](../../../../rfcs/0013-realization-and-run-provenance-wire.md) — the
  simplicial mesh envelope's content list. It names the schema and what the
  artifact records; no published document gives its canonical field order.

## Upstream component authorities

The sibling cases own the science this binding consumes and remain authoritative
for how each bound resource is built:

- [`../../exact-circular-hole-geometry`](../../exact-circular-hole-geometry/README.md)
  — the exact centre/radius/boundary identity;
- [`../../circular-hole-chordal-reference-mesh`](../../circular-hole-chordal-reference-mesh/README.md)
  — the in-memory chordal realization, its segment selection, and its
  approximation metrics; its own nonclaims list the durable source-to-mesh wire
  that this case supplies.

This case owns the wire between them and nothing inside them. The oracle pins
both sibling oracles by path and SHA-256 and verifies those digests on every run,
so a change upstream surfaces here as a failure rather than as silent drift. It
does not re-implement circle sampling, trigonometric metrics, geometry vertices,
mesh topology, correspondence assignments, or any resource digest; an earlier
revision that duplicated those derivations was rejected as over-specification.
No high-precision numerical kernel is reproduced here for the same reason.

## Values chosen by this lane

Labelled artificial wherever they appear, and predicting nothing about any
realization:

- the four 64-hex digest slots of the encoding witness, each a single repeated
  hex pair so that no content digest can collide with the pattern;
- its six scalars, each an exact positive power of two with a short plain
  decimal spelling, and its segment count of twelve;
- the scalars of the coherent policy variant, chosen on the same rule.

Published DFG benchmark results are not evidence for this wire-only case: no
flow, drag, lift, or Strouhal value is claimed, and no envelope digest over any
real realization is frozen anywhere in this directory.
