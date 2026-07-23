# Canonical 3D Cartesian FEM/FVM Poisson verification

This case compiles one three-dimensional Eqiora Language revision and executes
it through two resolved Realization plans. It is the dimension-three member of
the same reference path used by the independent 1D and 2D cases, not a
3D-specific solver path.

The canonical Relation lowers once to a dimension-three scalar source tape and
six typed boundary relations. Q1 FEM and cell-centered orthogonal TPFA FVM
share the generated `CartesianMesh`, quadrature/local-operator/assembly
contracts, `SolverPlan`, reference backend, and true-residual acceptance. They
retain different unknown locations and local operators.

FEM error is the continuous L2 norm of its primal Q1 field. FVM error is the
continuous L2 norm of an explicit dual-grid Q1 reconstruction through cell
centers and canonical boundary data; it is not a cell-center-only sample norm.

- Canonical model: [`models/poisson.eqi`](models/poisson.eqi)
- Analytic derivation: [`models/problem.md`](models/problem.md)
- Reference provenance: [`references/README.md`](references/README.md)
- Reproducible table: [`expected/convergence.csv`](expected/convergence.csv)

Run:

```bash
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-3d-fem-fvm
```

The verified claim is a scalar, constant-coefficient, axis-aligned 3D box with
complete essential boundary data, generated Cartesian meshes, Q1 FEM, and
orthogonal TPFA. It does not claim vector/tensor fields, mixed or high-order
elements, unstructured meshes, adaptivity, nonorthogonal FVM, or general
curved geometry.
