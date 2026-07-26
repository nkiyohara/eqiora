# Problem

This case adds no model of its own. It reuses the registered manufactured
Poisson cube of
[`numerics.cartesian-poisson-3d-fem-fvm`](../../cartesian-poisson-3d-fem-fvm/models/poisson.eqi)
verbatim, so the operator under study is one already accepted by an independent
analytic convergence and conservation case:

```text
-div(grad(u)) = 3 pi^2 sin(pi x) sin(pi y) sin(pi z)   on (0, 1)^3
u = 0 on the complete boundary
u_exact = sin(pi x) sin(pi y) sin(pi z)
```

Reusing that model is deliberate. The envelope claim is about how the *solver*
behaves as the mesh is refined, so the discretization must be one whose
correctness is already established elsewhere; otherwise a growing iteration
count could be blamed on the operator rather than on the preconditioner.

The evidence harness varies exactly three things:

| Axis | Values |
| --- | --- |
| `MeshPolicy::GeneratedUniform { cells_per_axis }` | 4, 8, 16, 32 |
| `PreconditionerPolicy` | `Identity`, `Jacobi` |
| `DiscretizationMethod` | `ContinuousGalerkin` (Q1), `CellCenteredFiniteVolume` (TPFA) |

Everything else — scalar type, layout, target, execution schedule, reduction
policy, backend, solver algorithm, and both tolerances — is held fixed, so the
recorded iteration counts differ only by refinement, preconditioner, and
method.
