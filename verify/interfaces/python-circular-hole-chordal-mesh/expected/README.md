# Frozen adapter observations

The installed-wheel test embeds independently owned acceptance observations.
The API shape and falsifiers were frozen before implementation. The first
oracle revision's exact artifact values were wrong; the installed-package gate
found that provenance mismatch. Before acceptance, the independent evidence
owner corrected the values below by replaying the pre-existing public Rust
owner-to-envelope producer, without reading or changing the Python
implementation and without taking values from its output:

- exact source digest
  `b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`;
- inner mesh canonical byte count `4,835`;
- raw canonical-byte SHA-256
  `d977d9125488fffee72deaf9a0f146bc42dc05a135692919a374d746da0f1079`;
- domain-separated public `mesh_digest`
  `148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a`;
- 50 circular chords and 104 vertices and triangles;
- realized entity counts `inlet=14`, `outlet=2`, `walls=38`,
  `cylinder=50`, and `fluid=104`;
- quality evidence `minimum_mean_ratio=0.003213006369764433` and
  `minimum_signed_measure_scale=0.0004210245914983321`.

The exact quality comparisons pin the existing artifact's canonical binary64
evidence, not a new scientific accuracy target. Boundary, area, and perimeter
comparisons retain RFC 0082's previously frozen allowances.

The public byte property is named `mesh_canonical_json` because it encodes only
the inner `SimplicialMeshEnvelopeV1`; its raw SHA-256 must not be substituted
for the domain-separated `mesh_digest`.

These values are the direct public
`CircularHoleChordalMeshV1::from_exact` to
`SimplicialMeshEnvelopeV1::from_mesh(owner.mesh())` projection. Reconstructing
from the accepted fluid fixture's cyclically rotated local cell order is an
explicit negative route: it yields a different quality observation and
artifact identity even though its mapped unordered cell sets agree.

The same-coordinate swapped-x-role witness preserves the inner mesh bytes and
digest, changes exact source identity, and requires `inlet=2`,
`outlet=14`. This is the precommitted falsifier for source identity derived
from mesh bytes and standard-name counts hard-coded in the adapter.
