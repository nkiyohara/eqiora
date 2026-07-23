# Acceptance contract

The executable inequalities and structural assertions are the expected data;
there is no stored solution vector or fitted reference table.

## Exact numerical setup

For each `n` in `2, 4, 8`, split every square of the uniform `n x n`
unit-square grid along the lower-left to upper-right diagonal. Every triangle
is affine and passes the `0.5` simplex quality gate. The mesh must have one
cell-connected component because this version owns one pressure gauge.

Use the continuous MINI pair

```text
velocity: (P1 + span{27 lambda_0 lambda_1 lambda_2})^2,
pressure: P1,
```

and the positive triangle Duffy rule formed from three Gauss--Legendre points
per source-square axis. This assembly rule must declare total-degree exactness
four. Manufactured error norms use a separate positive Duffy rule with four
Gauss--Legendre points per source-square axis and declared total-degree
exactness six.

## Manufactured solution and convergence

With `mu = 1`, complete essential velocity, and

```text
u                 = (x^2, -2 x y),
grad(u)           = [[2x, 0], [-2y, -2x]],
p                 = x - 1/2,
f                 = (-1, 0),
div(u)            = 0,
integral_Omega p  = 0,
```

the discrete errors are integrated with the separate degree-six rule.
For each consecutive refinement pair, the base-two error-ratio orders must be
strictly greater than:

```text
velocity L2                 1.75
velocity H1 seminorm        0.85
pressure L2                 0.85
discrete divergence L2      0.85
```

These are bounded regression thresholds for this mesh family and solver
tolerance, not a claim about arbitrary meshes or all asymptotic MINI theory.

## Assembly and solve evidence

One local contribution has twelve local unknowns: eight vector-velocity
unknowns, three scalar-pressure unknowns, and one occurrence of the shared
gauge. The same ordered packet must feed:

1. the reduced system used by the solve; and
2. the uneliminated full system used for constrained-boundary reaction.

The reduced system must be labeled `SymmetricIndefinite`, and every dense
probe of its finalized CSR entries must satisfy bit-exact
`A(i,j) == A(j,i)`. The selected solve tuple is reference
`MinimumResidual`, identity preconditioning, reproducible reductions, `f64`,
relative tolerance `1e-11`, absolute tolerance `1e-13`, and at most `10000`
iterations. The independently recomputed true residual must not exceed the
reported residual target.

The pressure-row quantity `||B u||_2`, after removing the gauge-column
contribution, must independently satisfy its residual-scaled acceptance bound.
This weak continuity evidence is not replaced by the mixed residual or by the
strong `||div(u)||_L2` convergence diagnostic.

For `n = 4`, one-worker reference assembly and four-worker ordered Rayon
assembly must produce bit-identical reduced/full CSR and right-hand sides,
algebraic values, and reconstructed velocity/pressure fields. Assembly reports
must retain different execution identities.

At every refinement:

```text
abs(integral pressure)                         < 2e-10
abs(global gauge multiplier)                   < 2e-10
abs(boundary reaction_c + body force_c)        < 2e-9, c = 0, 1
```

The gauge multiplier bound is evidence that the multiplier normalized
pressure but did not mask incompatible incompressibility data.

## Fail-closed boundary

The registered test must reject all of the following without producing an
accepted solution:

- a prescribed P1 velocity trace with nonzero net parent-outward flux;
- two disconnected triangle components under one global pressure gauge;
- the degree-zero simplex centroid rule requested for assembly;
- zero viscosity;
- a body-force callback returning `NaN`;
- an essential-velocity callback returning a non-finite value;
- conjugate gradient requested for the symmetric-indefinite problem;
- identity-only reference MINRES requested with Jacobi; and
- one- or three-dimensional simplex meshes passed to the 2D realization.

Lower-level constructors continue to reject a non-triangular/intrinsically
non-2D geometry, a non-finite prescribed velocity, and incompatible
quadrature geometry. Those checks do not expand this registered case into a
general simplex, callback, or boundary-condition claim.

## Explicit nonclaims

No assertion in this case is evidence for canonical or packaged Stokes,
field-wise Realization artifacts, dimensional mixed algebra, natural/open
boundaries, faer MINRES, block preconditioning, parallel MINRES,
Navier--Stokes, or fluid--structure interaction.
