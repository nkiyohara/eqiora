# Frozen adapter observations

The installed-wheel test embeds the pre-implementation expected observations:

- exact source digest
  `b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`;
- inner mesh canonical byte count `4,843`;
- raw canonical-byte SHA-256
  `fc4ffb57a6d7402d219eeccd921c2bd6bf1e3292946c6bc445d94805d76ef94b`;
- domain-separated public `mesh_digest`
  `c0d57813a0ca56aade9b286d1f4fff7df217ff130ac176515be5ef174b07847b`;
- 50 circular chords and 104 vertices and triangles;
- realized entity counts `inlet=14`, `outlet=2`, `walls=38`,
  `cylinder=50`, and `fluid=104`;
- quality evidence `minimum_mean_ratio=0.0064272786692910235` and
  `minimum_signed_measure_scale=0.0004210245914983321`.

The exact quality comparisons pin the existing artifact's canonical binary64
evidence, not a new scientific accuracy target. Boundary, area, and perimeter
comparisons retain RFC 0082's previously frozen allowances.

The public byte property is named `mesh_canonical_json` because it encodes only
the inner `SimplicialMeshEnvelopeV1`; its raw SHA-256 must not be substituted
for the domain-separated `mesh_digest`.

The same-coordinate swapped-x-role witness preserves the inner mesh bytes and
digest, changes exact source identity, and requires `inlet=2`,
`outlet=14`. This is the precommitted falsifier for source identity derived
from mesh bytes and standard-name counts hard-coded in the adapter.
