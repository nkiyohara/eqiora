# Python exact-source-bound chordal reference mesh

This case freezes one deliberately limited installed-Python adapter:
`eqiora.meshing.circular_hole_chordal`. It accepts the existing immutable
`eqiora.geometry.RectangleWithCircularHole` and the three explicit policies
`max_boundary_error`, `required_minimum_mean_ratio`, and `max_segments`. It
returns one immutable `eqiora.meshing.CircularHoleChordalMesh`.

The Python object is a same-process owner. It retains the exact source, the
opaque RFC 0082 chordal owner, the accepted inner
`SimplicialMeshEnvelopeV1`, and the Rust-derived authored-region
correspondence. Python does not select the chord count, sample the circle,
construct connectivity, compute quality, encode an artifact, derive a digest,
or infer selection membership from coordinates.

## Independently owned positive witness

The non-implementing evidence lane froze the API shape, claim boundary, and
falsifiers before the public Python implementation existed. The first oracle
revision's exact artifact values were wrong: it reconstructed the accepted
fluid fixture's local cell order instead of replaying the public owner. The
installed-package gate exposed that provenance mismatch.

Before acceptance, the independent evidence owner corrected the values below
by replaying the pre-existing public Rust producer, without reading or changing
the new Python implementation and without consuming its output. The producer's
inputs come from the already accepted
[`geometry.circular-hole-chordal-reference-mesh`](../../geometry/circular-hole-chordal-reference-mesh/README.md)
case. The expected artifact is derived through the exact public Rust chain the
adapter must expose:

```text
CanonicalCircularHoleGeometryV1
  -> CircularHoleChordalMeshV1::from_exact(..., 1e-4, 50, MeshQualityGate(1e-5))
  -> SimplicialMeshEnvelopeV1::from_mesh(owner.mesh())
```

The accepted fluid `mesh.json` remains an independent topology witness, not a
substitute serialization for the public owner. Its existing conformance check
matches coordinates within the RFC allowance and compares unordered cell
vertex sets. It deliberately does not freeze the owner's local cell rotations.

The resulting inner mesh artifact has:

- 4,835 canonical bytes;
- raw canonical-byte SHA-256
  `d977d9125488fffee72deaf9a0f146bc42dc05a135692919a374d746da0f1079`;
  and
- domain-separated mesh digest
  `148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a`.

These two hashes are intentionally different. `mesh_digest` is the latter:

```text
sha256(
  b"eqiora.simplicial-mesh-envelope/v1"
  || 0x00
  || canonical_json
)
```

The test independently recomputes both hashes from the public
`mesh_canonical_json`, pins its byte count, parses its closed topology,
geometry, acceptance, and evidence fields, and requires the public property
to equal the domain-separated digest. The explicit `mesh_` prefix prevents
these inner mesh bytes from being mistaken for a durable encoding of the live
source-bound wrapper. The inner mesh bytes contain no exact-source field; the
wrapper's separate `source_digest` proves only the live same-process ownership
described below.

The DFG witness requires 50 circular chords, 104 vertices, and 104 triangles.
Rust correspondence expands the exact names to
`inlet=14`, `outlet=2`, `walls=38`, `cylinder=50`, and `fluid=104`.
Every public selection name is resolved through
`region_entity_set_entities(...).len()`; the adapter does not independently
count mesh facets. The four boundary-selection counts sum to the RFC witness's
104 boundary facets.

## Numeric evidence boundary

RFC 0082 remains the independent scientific oracle for chord selection,
boundary error, area deficit, and perimeter deficit. This adapter case reuses
its published allowances; it does not tune them from Python output.

The minimum mean ratio `0.003213006369764433` and minimum signed measure scale
`0.0004210245914983321` are compared exactly because
`SimplicialMeshEnvelopeV1` serializes their binary64 values as canonical
acceptance evidence and rejects any bitwise-different replay. That exact
comparison verifies byte-for-byte adapter fidelity. It is not a new claim
that this one cell-quality value is an independently derived scientific
tolerance or a production mesh-quality target.

### Local-cell-order audit

The first oracle revision rebuilt a `SimplicialMesh` directly from the accepted
fluid fixture's recorded cell order. That was the wrong projection. After the
owner vertices are mapped to fixture indices, owner cell 0 is
`[50, 52, 1]`; the accepted fixture records the same oriented triangle as
`[1, 50, 52]`. Their unordered vertex set is identical, so the accepted RFC
topology check passes, but the local reference origin differs.

The current `AffineMapQuality` uses the Frobenius norm of the Jacobian whose
columns are based at local vertex 0. A cyclic cell rotation therefore preserves
orientation and signed measure but does not preserve this recorded quality
value. Rebuilding from fixture order produces the superseded
`minimum_mean_ratio=0.0064272786692910235`, 4,843 bytes, and mesh digest
`c0d57813a0ca56aade9b286d1f4fff7df217ff130ac176515be5ef174b07847b`.
Those values describe a different in-memory ordering and are now an explicit
falsifier for bypassing the public owner.

This was not consumption of `falsifier-wrong-diagonal.json`. That file declares
`role=wrong-contract-falsifier`, fails the mapped unordered-cell-set comparison,
and independently produces `minimum_mean_ratio=0.006427278669291052` and mesh
digest
`17f363d9cea003e89508473b9857b2b11206c9b6e02e9e9203be28567899ec56`.
The accepted fixture and wrong-diagonal fixture are therefore distinguished
both structurally and by artifact identity.

## Falsifiers

The test requires structured `ValidationError` when 49 segments cannot meet
the request, when a `0.5` mean-ratio gate rejects the frozen mesh, and when an
unknown realized selection is queried.

A stronger identity mutant uses the same exact coordinates with `inlet` and
`outlet` assigned to opposite x sides. Its inner mesh bytes and digest must
remain unchanged, while its exact source digest changes and its realized
counts become `inlet=2` and `outlet=14`. This kills both a source identity
reconstructed from mesh bytes and hard-coded standard selection counts.

Changing only the exact geometry classification tolerance from `1e-12` to
`1e-10` likewise changes exact source identity while retaining the explicit
`1e-4` mesh request, topology, approximation evidence, and inner mesh digest.
That falsifies an adapter that silently reuses geometry classification
tolerance as meshing policy.

The package test always launches an equivalent public program with
`python -I -c`, so an isolated sdist consumer still executes the complete
public path. When the repository example tree is present, it separately
executes `examples/python/exact_cylinder_mesh.py`. Only that repository-file
check skips when a packaged consumer intentionally has no examples directory.

## Boundary and future dependency

This is not a generic `Mesh`, `MeshRequest`, generated-mesh protocol, or
external import surface. It claims no Delaunay or production mesher, curved
element, Model, solver, Result, visualization, performance, or physical
validation.

In particular, `mesh_canonical_json` and the inner
`SimplicialMeshEnvelopeV1` digest are not a durable
source-to-mesh binding. The live wrapper is not a cross-process proof and
cannot publish or replay the generated realization for a later Result
lineage. Cross-process publication, replay, and future Result lineage require
the `CircularHoleChordalRealizationEnvelopeV1` tracked by the
[circular-hole realization artifact work](https://github.com/nkiyohara/eqiora/issues/128).

Run the registered evidence after implementation:

```bash
python3 tools/ci/python_package_gate.py
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-circular-hole-chordal-mesh
```
