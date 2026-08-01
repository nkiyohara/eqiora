# Reference provenance

The adapter API shape, claim boundary, and falsifiers were authored before its
Python implementation. The first oracle revision's exact artifact values were
wrong; the installed-package gate exposed that it had reconstructed the
accepted fluid fixture's local cell order rather than the deterministic producer.

Before acceptance, the independent evidence owner corrected those values by
replaying the pre-existing public Rust producer, without reading or changing
the new Python implementation and without taking values from its output. That
replay consumes, without changing:

- RFC 0082 and
  `geometry.circular-hole-chordal-reference-mesh` for the exact-source-bound
  private chordal reference, accepted artifact owner, topology, approximation observations, tolerances, and
  falsifiers;
- `interfaces.python-exact-circular-hole-geometry` for the installed-Python
  exact source; and
- the accepted `SimplicialMeshEnvelopeV1` contract for the inner mesh's
  canonical bytes and domain-separated identity.

The corrected artifact values were independently derived by applying
the accepted artifact constructor directly to the pre-existing deterministic
reference Mesh. The standalone test independently hashes the exposed
`mesh_canonical_json` bytes and compares the Python observations to those
acceptance values. It does not derive expected values from the new Python
adapter. The property name and the distinct raw/domain-separated hashes keep
this inner mesh encoding separate from the live source-bound wrapper.

The accepted fluid fixture is used only as an independent topology witness:
after the existing allowance-based vertex mapping, its unordered cell sets
equal the reference Mesh's. Its cyclic local cell rotations are not the reference's
artifact order. Direct fixture reconstruction was the superseded oracle
interpretation; the declared wrong-diagonal fixture separately fails the
mapped unordered-cell-set comparison.

The inner mesh artifact contains coordinates, cells, acceptance policy, and
quality evidence only. It contains no durable exact-source binding. A
cross-process generated realization and later Result lineage remain dependent
on future acceptance of the separate
`CircularHoleChordalRealizationEnvelopeV1` durable realization artifact.
