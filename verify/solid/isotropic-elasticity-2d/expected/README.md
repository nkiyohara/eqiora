# Acceptance contract

The evidence target owns quantitative assertions rather than a hand-edited
golden solution vector. Mesh-dependent arrays are recomputed from the exact
canonical fixture and independently checked on every run.

## Local operator

- the affine unit-cell eight-by-eight stiffness is symmetric to a scaled
  tolerance of `2e-14`;
- two rigid translations and `u = (-y, x)` have absolute energy at most
  `2e-14` on the unit cell;
- `u = (y, x)` has energy `2 mu` and `u = (x, y)` has energy
  `2 (mu + lambda)`, each within `5e-14` absolute error for the registered
  coefficients; and
- at least one off-component stiffness entry has magnitude greater than
  `1e-12`; the exact affine-patch reactions below are the independent
  component-coupling oracle.

## Affine patch

On a two-by-two affine Cartesian patch, Q1 reconstruction of an affine vector
field must reproduce its value and gradient to `5e-14`. The assembled
interior residual must be at most `5e-13` in infinity norm after scaling by the
largest boundary resultant. Boundary resultants must reproduce the constant
analytical stress to the same scaled tolerance.

This patch injects an algebraic nodal vector below the language boundary. It
does not constitute evidence for nonzero public vector boundary conditions.

## Manufactured convergence

For the registered `4, 8, 16, 32` cells-per-axis sequence:

- continuous displacement L2 error decreases monotonically;
- every reported value and error is finite;
- the last two observed L2 orders are at least `1.9` and the corresponding H1
  seminorm orders are at least `0.9`; and
- every CG result passes an independently recomputed true-residual target.

The exact solution is the gradient field derived in
[`references/README.md`](../references/README.md); it is evaluated independently
from the implementation's local operator and load assembly.

## Componentwise equilibrium

For both Cartesian components,

```text
boundary_reaction[i] + integrated_body_force[i]
```

must be at most `5e-11` relative to the sum of the two magnitudes, with an
absolute floor suitable for a zero resultant. A separate affine-potential
probe requires a nonzero integrated force in at least one component before the
balance assertion is admitted.

## Identity and replay

- numerical mesh, quadrature, or solver-plan changes preserve exact current Model
  canonical bytes and digest while changing Realization identity;
- current Model, Realization v1, and Run v2 encode/decode/re-encode without byte or
  digest drift; and
- substituted Model coefficients, Model digest, semantic revision, or
  Realization digest fail exact lineage replay.

No expected stress array, vector-field artifact, reaction artifact, package
binding, or performance number is part of this case.
