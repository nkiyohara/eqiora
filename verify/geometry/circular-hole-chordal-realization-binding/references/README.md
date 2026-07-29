# References

Every value in this case is derived from contracts published in this
repository. No production Rust was run to obtain a value, and none was copied
from production test output.

The consulted contracts are:

- [RFC 0008](../../../../rfcs/0008-canonical-artifact-wire-v1.md) — canonical
  JSON encoding identifier and the domain-separated digest framing
  `sha256(schema-domain || 0x00 || canonical bytes)` that this envelope reuses
  unchanged.
- [RFC 0079](../../../../rfcs/0079-authored-planar-geometry-artifact.md) —
  the straight-edged planar region wire, its canonical normalization order, and
  the binary64 rendering rule identity is pinned to. Its frozen 482-byte
  square-with-hole literal is one of the two validation witnesses for this
  oracle's renderer.
- [RFC 0081](../../../../rfcs/0081-exact-circular-hole-geometry.md) — the exact
  circular-hole wire, field order, and the DFG witness values. Its frozen
  511-byte literal is the second renderer validation witness, and the source
  identity this envelope binds.
- [RFC 0082](../../../../rfcs/0082-source-bound-chordal-circular-hole-mesh.md)
  — the approximation contract: fixed phase, minimum eight segments, the
  sagitta/area-deficit/perimeter-deficit closed forms, the stable half-angle
  selection inverse, the scale-derived evaluation allowance, and the two
  inequalities this oracle asserts.
- [RFC 0049](../../../../rfcs/0049-geometry-identity-and-mesh-correspondence.md)
  — the correspondence semantics, and the reason its digest is not derivable
  here: it is closed over one exact Model artifact whose Domain ULIDs are
  author-chosen.
- [RFC 0013](../../../../rfcs/0013-realization-and-run-provenance-wire.md) —
  the simplicial mesh envelope's content list. It names the schema and what the
  artifact records, but no published document gives its canonical field order.

The sibling cases are
[`../../exact-circular-hole-geometry`](../../exact-circular-hole-geometry/README.md),
which owns the exact centre/radius identity, and
[`../../circular-hole-chordal-reference-mesh`](../../circular-hole-chordal-reference-mesh/README.md),
which owns the in-memory chordal realization and its approximation evidence.
This case owns only the durable binding wire between them, which the latter
lists among its nonclaims.

The high-precision ideal values are reproduced by this oracle's own kernel
rather than quoted from an external table or from the sibling oracle, so no
third-party numerical source and no other lane's output is evidence here.
Published DFG benchmark results are not evidence for this wire-only case: no
flow, drag, lift, or Strouhal value is claimed.
