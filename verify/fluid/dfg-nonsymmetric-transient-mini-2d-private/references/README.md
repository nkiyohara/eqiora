# Accepted lineage and reused evidence

This NUM0 package composes the following registered predecessor evidence
without re-deriving it:

- `geometry.exact-circular-hole-geometry`
- `geometry.geometry-boundary-relation-scope`
- `geometry.circular-hole-chordal-reference-mesh`
- `geometry.circular-hole-chordal-realization-binding`
- `fluid.exact-circular-hole-stokes-2d`
- `fluid.fixed-domain-transient-navier-stokes-2d`
- `fluid.canonical-inlet-outlet-navier-stokes-2d`
- `fluid.non-box-transient-navier-stokes-2d-private`

The exact geometry owner remains authoritative for source, realized geometry,
mesh, and correspondence. The existing transient path remains authoritative
for MINI/P1 spaces, backward Euler, energy-skew convection, checked assembly,
Newton/line search, centered Jacobian audit, identity/revision admission,
initial-state admission, and the trajectory/state shape.

Mutants 9, 10, 13, and 14 reuse those accepted gates because the DFG delta
does not alter their formulas or owners. The new Rust positive proves their
composition is not bypassed. Mutants 1--8, 11, 12, 15, and 16 are the focused
NUM0 delta and are fixed by the exact oracle, DFG semantic binding, local-pair
discriminator, no-gauge structure, and ordinary nonzero step.

The governing sealed artifacts are:

- NUM0 contract SHA-256
  `96e63d87a7c7686cc87662ca7ede8eeb378611a252303f5414aed3f33e082f23`;
- contract review SHA-256
  `400bed8531ca6a9b0215d250859be35830ebfc0792161635ad3e4512c875ef34`;
- analytic derivation SHA-256
  `d0776d536d4fdc4d1824273b4796f66f604d563c3c9ab196bfe4d4dc39de373a`;
- symbolic affine-MINI derivation SHA-256
  `d44e76fe673118f0d955a2a6ac4c04bc54937fec28f44eed417cf846d4d6419f`;
  and
- accepted dual-derivation reconciliation SHA-256
  `b5427867b7039d15e7a776a80a6a3a6bf9a34b0993850a640b6cdc416c5e9a78`.

No external benchmark sample or source-extracted value is an oracle for NUM0.
Source acquisition, MESH0, OBS0, S1/S2 values, #149, H2, Python, Studio, and
publication are not prerequisites for this private mechanics cell.
