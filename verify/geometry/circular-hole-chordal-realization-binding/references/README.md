# References

Every frozen value is either read from a published contract or chosen by this
lane and labelled synthetic. No production output was read or executed to
obtain an expected value.

## Contracts consulted

- [RFC 0008](../../../../rfcs/0008-canonical-artifact-wire-v1.md) — canonical
  JSON identifier, domain-separated digest framing, canonical round trip, and
  decoder resource bounds.
- [RFC 0079](../../../../rfcs/0079-authored-planar-geometry-artifact.md) — the
  canonical straight-edged region consumed as the realized geometry.
- [RFC 0081](../../../../rfcs/0081-exact-circular-hole-geometry.md) — the exact
  circular-hole source family and its distinct schema domain.
- [RFC 0082](../../../../rfcs/0082-source-bound-chordal-circular-hole-mesh.md) —
  requested boundary-error policy, segment work limit, required mesh-quality
  threshold, measured approximation observations, and minimum eight segments.
- [RFC 0049](../../../../rfcs/0049-geometry-identity-and-mesh-correspondence.md)
  — general geometry/mesh correspondence completeness, identity, and
  parent-outward orientation semantics.
- [`geometry_mesh_correspondence_sources.rs`](../../../../crates/eqiora-artifact/src/geometry_mesh_correspondence_sources.rs)
  — the accepted public `from_region` / `validate_against_region` contract for
  the Model-free `authored-planar-region-v1` correspondence variant. Its wire
  binds only canonical authored geometry, simplicial mesh, dimension, and
  derived assignments; the binding oracle does not use the Model-bound
  Cartesian variant from the same outer artifact family.
- [`authored_region_correspondence.rs`](../../../../crates/eqiora-artifact/tests/authored_region_correspondence.rs)
  — accepted executable examples of totality, missing or relabelled facets,
  wrong orientation, exterior/hole assignment swaps, and alternative conforming
  topology.
- [RFC 0013](../../../../rfcs/0013-realization-and-run-provenance-wire.md) — the
  simplicial mesh artifact content and its recomputed quality evidence.
- [`eqiora.meshing` public stub](../../../../bindings/python/python/eqiora/meshing.pyi)
  — the accepted distinction between the input
  `required_minimum_mean_ratio` threshold and the measured
  `minimum_mean_ratio`.

The accepted source and tests above were consulted only to select and name the
already-public Model-free correspondence contract. The stdlib oracle neither
imports nor executes Rust, and no frozen bytes, digest, metric, topology, or
assignment were copied from production.

## Upstream component authorities

- [`../../exact-circular-hole-geometry`](../../exact-circular-hole-geometry/README.md)
  owns exact centre, radius, boundary identity, canonical bytes, and source
  digest.
- [`../../circular-hole-chordal-reference-mesh`](../../circular-hole-chordal-reference-mesh/README.md)
  owns segment selection, generated coordinates, approximation observations,
  named chordal region, and source-owned reference mesh.

The binding oracle pins both sibling oracle files by SHA-256 on every run. It
does not reimplement their sampling, trigonometric metrics, geometry vertices,
mesh topology, or resource digests.

The authored-region correspondence digest is deliberately not predicted. It
binds generated realized-geometry and mesh digests plus assignments derived
from those resources, so it inherits their runtime binary64 coordinate and
topology identities.

## Values chosen by this lane

- four synthetic repeated-pair 64-hex sentinel slots, not copied from runtime
  resources; no assertion is made that SHA-256 cannot output those patterns;
- six exact positive dyadic floating values and a segment count of twelve for
  the encoding witness; and
- dyadic values for one encoding-only policy variant whose replay outcome is
  explicitly `not_evaluated`.

The Python JSON encoder is used only for those selected dyadic values and
mutants. Production `serde_json` remains authoritative for arbitrary runtime
binary64 canonical spelling.

Published flow, drag, lift, or Strouhal results are not evidence for this
wire-only case. No binding digest over a real realization is frozen here.
