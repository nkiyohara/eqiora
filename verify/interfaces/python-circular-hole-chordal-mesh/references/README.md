# Reference provenance

The adapter evidence was authored before its Python implementation. It
consumes, without changing:

- RFC 0082 and
  `geometry.circular-hole-chordal-reference-mesh` for the exact-source-bound
  chordal owner, topology, approximation observations, tolerances, and
  falsifiers;
- `interfaces.python-exact-circular-hole-geometry` for the installed-Python
  exact source; and
- the accepted `SimplicialMeshEnvelopeV1` contract for the inner mesh's
  canonical bytes and domain-separated identity.

The standalone test independently hashes the exposed `mesh_canonical_json`
bytes and compares the Python observations to these precommitted values. It
does not derive expected values from the new Python adapter. The property name
and the distinct raw/domain-separated hashes keep this inner mesh encoding
separate from the live source-bound wrapper.

The inner mesh artifact contains coordinates, cells, acceptance policy, and
quality evidence only. It contains no durable exact-source binding. A
cross-process generated realization and later Result lineage remain dependent
on the `CircularHoleChordalRealizationEnvelopeV1` tracked by the
[circular-hole realization artifact work](https://github.com/nkiyohara/eqiora/issues/128).
