# 3D Cartesian Poisson FEM/FVM convergence evidence

This case closes one three-dimensional path from canonical meaning through
resolved realization and method-native operators to quantitative evidence.
It uses the shared runtime-dimensional implementation exercised independently
in 1D and 2D.

## Problem and semantic path

```text
-Delta u = 3 pi^2 sin(pi x) sin(pi y) sin(pi z),  (x, y, z) in (0, 1)^3
u = 0 on the complete boundary
u_exact = sin(pi x) sin(pi y) sin(pi z)
```

One Eqiora Language revision owns the Cartesian Domain, scalar continuum
Field, strong Relation, coordinate-dependent source, and six axis-side trace
Relations. Lowering produces one `ScalarEllipticCartesianModel`: dimension-three
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
  reconstruction through cell centers and canonical boundary samples.
- Conservation compares integrated source with recovered FEM boundary
  reactions or FVM outward boundary fluxes.

## Results

| Cells/axis | max h | FEM L2 | FEM order | FVM L2 | FVM order | FEM balance | FVM balance |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 4 | 2.50000000e-1 | 2.29830238e-2 | — | 3.72992973e-2 | — | 4.65e-16 | 2.15e-16 |
| 8 | 1.25000000e-1 | 5.74560170e-3 | 2.000039 | 9.63803723e-3 | 1.952337 | 1.74e-16 | 4.56e-15 |
| 16 | 6.25000000e-2 | 1.43667368e-3 | 1.999726 | 2.42949771e-3 | 1.988081 | 4.42e-15 | 8.79e-15 |

CI compares the deterministic table, requires strictly decreasing errors,
every observed order above `1.9`, and both relative balance defects below
`2e-11`. Reproduce the evidence with:

```bash
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-3d-fem-fvm
```

## Claim boundary

This case verifies scalar, constant-coefficient, axis-aligned 3D Cartesian
boxes with complete essential boundary data, generated conforming meshes, Q1
FEM, and orthogonal TPFA. Together with the independent 1D and 2D cases, it
admits the reference realization envelope `1D..=3D`.

This evidence does not cover vector/tensor fields, mixed or higher-order
elements, unstructured meshes, adaptive refinement, curved geometry,
nonorthogonal FVM, natural/Robin execution, variable/tensor coefficients,
or accelerator residency. Ordered one/four-worker assembly equivalence is
covered independently by `numerics.threaded-cpu`; this convergence table
remains the method-accuracy oracle.
