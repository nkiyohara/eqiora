# Problem

The registered falsifier runs on the **constant-source** cube in
[`constant-source-poisson.eqi`](constant-source-poisson.eqi):

```text
-div(grad(u)) = 1   on (0, 1)^3
u = 0 on the complete boundary
```

That model is what the evidence harness compiles and what every recorded
iteration count belongs to.

## Why not the registered manufactured cube

The first attempt reused the manufactured Poisson cube of
[`numerics.cartesian-poisson-3d-fem-fvm`](../../cartesian-poisson-3d-fem-fvm/models/poisson.eqi)
verbatim:

```text
-div(grad(u)) = 3 pi^2 sin(pi x) sin(pi y) sin(pi z)   on (0, 1)^3
u = 0 on the complete boundary
u_exact = sin(pi x) sin(pi y) sin(pi z)
```

Reusing an already-accepted discretization was deliberate — an envelope claim is
about the *solver*, so a growing iteration count must not be blamable on an
unvalidated operator.

That probe turned out to measure nothing. On a uniform mesh the load vector of a
single separable sine mode is proportional to a discrete eigenvector of the
assembled operator, so conjugate gradients terminates in one iteration at every
refinement. The run is retained and voided in
[the case README](../README.md); it supports no conclusion in either direction.

The constant source replaces it because its load vector is `h^3` on every free
unknown and expands over the sine basis across all odd triples, so it occupies
no low-dimensional invariant subspace. Declared thresholds were carried over
unchanged.

**This substitution is not a free choice made after seeing results.** It is
recorded as a probe replacement under a pre-declared validity condition, and the
limits of how auditable that ordering is are stated in the case manifest's
`declared_envelope` block rather than glossed over.

## What the harness varies

| Axis | Values |
| --- | --- |
| `MeshPolicy::GeneratedUniform { cells_per_axis }` | 4, 8, 16, 32 |
| `PreconditionerPolicy` | `Identity`, `Jacobi` |
| `DiscretizationMethod` | `ContinuousGalerkin` (Q1), `CellCenteredFiniteVolume` (TPFA) |

Everything else — scalar type, layout, target, execution schedule, reduction
policy, backend, solver algorithm, and both tolerances — is held fixed, so the
recorded iteration counts differ only by refinement, preconditioner, and method.
