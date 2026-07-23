# Canonical Cartesian FEM/FVM Poisson verification

This case compiles one two-dimensional Eqiora Language revision and executes
it through two resolved Realization plans.

The canonical Relation lowers once to a dimension-two scalar source tape and
four typed boundary relations. Q1 FEM and cell-centered orthogonal TPFA FVM
share the generated `CartesianMesh`, quadrature/local-operator/assembly
contracts, `SolverPlan`, reference backend, and true-residual acceptance.
They retain different unknown locations and local operators.

FEM error is the continuous L2 norm of its primal Q1 field. FVM error is the
continuous L2 norm of an explicit dual-grid Q1 reconstruction through cell
centers and canonical boundary data; it is not a cell-center-only sample norm.

- Canonical model: [`models/poisson.eqi`](models/poisson.eqi)
- Analytic derivation: [`models/problem.md`](models/problem.md)
- Reference provenance: [`references/README.md`](references/README.md)
- Reproducible table: [`expected/convergence.csv`](expected/convergence.csv)

Run:

```bash
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-fem-fvm
```

The verified claim is a scalar, constant-coefficient, axis-aligned 2D box
with complete essential boundary data, generated Cartesian meshes, Q1 FEM,
and orthogonal TPFA. It does not claim vector/tensor fields, mixed/high-order
elements, unstructured meshes, adaptivity, nonorthogonal FVM, or 3D
end-to-end realization.
