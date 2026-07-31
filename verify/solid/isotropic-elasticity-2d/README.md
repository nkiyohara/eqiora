# Canonical two-dimensional isotropic elasticity

This case verifies one complete semantic-to-numerical slice for a
two-dimensional isotropic small-strain solid. One current Model contains a
spatial-vector displacement Field, a scalar conservative-load-potential Field,
its pointwise definition, the canonical tensor balance, and homogeneous trace
Relations on all four sides of a Cartesian box.

The Model lowers once to a method-neutral elasticity contract. That contract
retains exact Domain and Field identities, Cartesian bounds, finite coercive
Lamé coefficients, the immutable scalar tape defining the load potential, and
complete homogeneous boundary meaning. It contains no mesh, element,
quadrature, matrix, solver, or target choice.

The registered reference Realization selects generated Cartesian Q1, explicit
two-point tensor-product Gauss quadrature, existing local-contribution and
ordered-assembly contracts, replicated `f64` CSR, and host-serial conjugate
gradient. The scalar load-potential Field is exactly prescribed by its
canonical Relation and is eliminated before displacement assembly; this is
not a mixed finite-element claim.

The evidence is deliberately redundant across different failure modes:

- one-cell stiffness symmetry, rigid translations, infinitesimal rotation,
  pure shear, uniform dilatation, and a nonzero cross-component block;
- one two-by-two affine patch with exact Q1 constant strain and assembled
  interior equilibrium;
- a smooth zero-boundary manufactured solution with approximately
  second-order continuous displacement L2 convergence;
- recovered boundary reaction against integrated conservative body force in
  each component, including a nonzero affine-potential falsifier; and
- current Model, Realization v1, and Run v2 identity separation and replay; and
- rejection before assembly when legacy compatibility resolution admits only
  a symmetric-indefinite MINRES tuple but the elasticity finalizer requires
  its known symmetric-positive-definite operator property.

The authoritative contract and package deferral rationale are in
[`RFC 0039`](../../../rfcs/0039-canonical-isotropic-elasticity-2d.md).
Acceptance thresholds are summarized in
[`expected/README.md`](expected/README.md), and the independent mathematical
oracle is recorded in [`references/README.md`](references/README.md).

Run:

```bash
cargo test --locked -p eqiora --test isotropic_elasticity_2d
cargo run -p eqiora-verify -- run --case solid.isotropic-elasticity-2d
```

The claim is exact to a closed 2D Cartesian, homogeneous-Dirichlet,
constant-coefficient, conservative-load, Q1/CSR/CG reference slice. It does
not claim a Model Package, public physical Port, FSI, plane stress or plane
strain, 3D, traction conditions, nonzero public vector boundary data,
unstructured or mixed methods, high order, nonlinear material behavior, or
dynamics.
