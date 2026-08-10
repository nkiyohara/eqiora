# Pironneau source map

This map separates the inspected source lineage from Eqiora's bounded
finite-dimensional benchmark specializations. It is governed by
[`scientific-contract.md`](scientific-contract.md).

## Inspected publisher artifact

- O. Pironneau, *On optimum profiles in Stokes flow*, *Journal of Fluid
  Mechanics* 59(1), 117--128 (1973).
- DOI: `10.1017/S002211207300145X`.
- Publisher artifact SHA-256:
  `a3845478ce7bb336480a4d2cdd630afde19c02812037bca3fb766ce5f139ef2e`.

## Source-derived lineage

- Section 2, especially equation (2.1): a fixed outer boundary, a moving
  no-slip body, prescribed outer velocity, a steady Stokes state, and the
  symmetric-gradient dissipation formulation.
- Equation (2.2): only Pironneau's qualified dissipation/drag discussion. It
  is not used as a finite-cell force equality.
- Section 4: first-variation and fixed-volume lineage.
- Section 5: descent lineage and the stated unbounded-domain caveat.

## Eqiora benchmark specializations

The following are Eqiora choices, not statements or numerical results from
Pironneau: two spatial dimensions; the square half-width `10 r_A`; the
normalized two-even-mode polar family; the coefficient diamond; MINI/P1
discretization; the exact reference and refined topologies; fixed-reference
harmonic interior motion; the complete discrete reduced derivative; the
12-point quadrature and direct-solve conventions; finite-difference probes;
optimizer constants and budget; immutable history representation; resource
bounds; and every expected value, comparison band, and tolerance.

## Explicit exclusion

S. Richardson, *Optimum Profiles in Two-Dimensional Stokes Flow* (1995), DOI
`10.1098/rspa.1995.0103`, is exclusion-only here. Exterior two-dimensional
flow, equivalent/effective-radius drag, constant-surface-vorticity, source
profile, force, and optimum claims remain deferred. No formula, coefficient,
value, or tolerance from an unavailable full text is used by this evidence.

## Reused repository mechanisms

The following accepted cases are reusable product mechanisms, not new
scientific oracles:

- `fluid.exact-circular-hole-stokes-2d`;
- `geometry.exact-circular-hole-geometry`;
- `geometry.circular-hole-chordal-reference-mesh`;
- `fsi.fixed-topology-ale-monolithic-2d`;
- `differentiation.spatial-shape-optimization`;
- `interfaces.python-exact-cylinder-stokes-result`;
- `interfaces.python-exact-cylinder-pressure-still`; and
- the accepted #239 common gallery admission/media path.
