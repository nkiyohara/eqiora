# Cartesian Poisson FEM/FVM convergence evidence

This case closes one two-dimensional path from canonical meaning through
resolved realization and method-native operators to quantitative evidence.

## Problem and semantic path

```text
-Delta u = 2 pi^2 sin(pi x) sin(pi y),  (x, y) in (0, 1)^2
u = 0 on the complete boundary
u_exact = sin(pi x) sin(pi y)
```

One Eqiora Language revision owns the Cartesian Domain, scalar continuum
Field, strong Relation, coordinate-dependent source, and four axis-side trace
Relations. Lowering produces one `ScalarEllipticCartesianModel`: dimension-two
bounds, a positive constant coefficient, one immutable source tape, and one
typed boundary tape per side. Mesh density, method, quadrature, solver, and
backend occur only in the two `ResolvedRealization` plans.

Both plans use the same runtime-dimensional `CartesianMesh`, entity geometry,
`LocalContribution`, constraint-aware `AssemblyMap`, CSR artifact,
`SolverPlan`, deterministic reference backend, and independently recomputed
true residual.

- FEM uses a continuous Q1 vertex field and cell-local diffusion/load forms.
- FVM uses one P0 unknown per cell and orthogonal two-point fluxes on interior
  and boundary facets.
- FEM error is the continuous L2 norm of the primal Q1 field.
- FVM error is the continuous L2 norm of an explicit Q1 dual-grid
  reconstruction through cell centers and canonical boundary samples. It is
  not a sampled cell-center norm.
- Conservation compares integrated source with recovered FEM boundary
  reactions or FVM outward boundary fluxes.

## Results

| Cells/axis | max h | FEM L2 | FEM order | FVM L2 | FVM order | FEM balance | FVM balance |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 4 | 2.50000000e-1 | 3.01801604e-2 | — | 3.01552500e-2 | — | 5.55e-17 | 0 |
| 8 | 1.25000000e-1 | 7.58721366e-3 | 1.991958 | 7.58681706e-3 | 1.990843 | 3.89e-16 | 5.48e-16 |
| 16 | 6.25000000e-2 | 1.89970457e-3 | 1.997795 | 1.89969834e-3 | 1.997724 | 2.78e-16 | 0 |
| 32 | 3.12500000e-2 | 4.75111668e-4 | 1.999437 | 4.75111571e-4 | 1.999432 | 2.55e-15 | 1.11e-16 |

CI compares the deterministic table, requires strictly decreasing errors,
every observed order above `1.9`, and both relative balance defects below
`2e-11`. Reproduce the evidence with:

```bash
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-fem-fvm
```

## Claim boundary

This case verifies scalar, constant-coefficient, axis-aligned 2D Cartesian
boxes with complete essential boundary data, generated conforming meshes, Q1
FEM, and orthogonal TPFA. The independent
[`numerics.cartesian-poisson-3d-fem-fvm`](cartesian-poisson-3d-fem-fvm.md)
case exercises the same path in 3D; neither case substitutes for the other.

This evidence does not cover vector/tensor fields, mixed or higher-order
elements, unstructured meshes, adaptive refinement, curved geometry,
nonorthogonal FVM, natural/Robin 2D execution, variable/tensor coefficients,
or accelerator residency. Ordered one/four-worker assembly equivalence is
covered independently by `numerics.threaded-cpu`; this convergence table
remains the method-accuracy oracle.
