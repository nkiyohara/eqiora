# Canonical FEM/FVM Poisson verification

This case fixes one analytic Poisson problem in Eqiora Language and compares
two numerical realizations of the same validated canonical model.

The canonical Relation contains the coordinate-dependent manufactured source.
Lowering produces one immutable scalar source tape; continuous P1 FEM and
cell-centered two-point-flux FVM evaluate that same tape while retaining their
native local operators, DOF locations, and assembly maps. The analytic exact
solution remains reference evidence rather than model meaning.

- Canonical model: [`models/poisson.eqi`](models/poisson.eqi)
- Human derivation: [`models/problem.md`](models/problem.md)
- Reference provenance: [`references/README.md`](references/README.md)
- Reproducible expected table: [`expected/convergence.csv`](expected/convergence.csv)
- Detailed evidence and claim boundary:
  [`docs/verification/poisson-fem-fvm.md`](../../../docs/verification/poisson-fem-fvm.md)

Run:

```bash
cargo test -p eqiora-numerics --test poisson_fem_fvm
cargo run -p eqiora-numerics --example poisson_convergence
```

CI requires decreasing continuous-L2 errors, observed orders above `1.9` for
both realizations, and relative global balance defects below `2e-12`. The
comparison is one-dimensional; runtime-dimensional canonical coordinates and
realization contracts do not constitute multidimensional solver evidence. The
separate [`numerics.cartesian-poisson-fem-fvm`](../cartesian-poisson-fem-fvm/)
case supplies independent 2D evidence without widening this case.
