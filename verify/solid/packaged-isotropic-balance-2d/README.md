# Exact-packaged isotropic balance in two dimensions

This case verifies the first reusable solid-mechanics application of
[RFC 0040](../../../rfcs/0040-occurrence-bound-field-slots.md) through the
unchanged execution path from
[RFC 0039](../../../rfcs/0039-canonical-isotropic-elasticity-2d.md).

The verification-owned, immutable `Eqiora.Solid.LinearElasticity@0.1.0`
release under `package-v0.1.0` exports only
`IsotropicBalanceWithPotential2d`: an exact two-dimensional volume support,
displacement and load-potential Field slots, the two Lamé Parameters, and the
canonical isotropic balance Relation. It owns no Domain, Field, boundary,
load definition, mesh, discretization, solver, target, or schedule. The exact
root package owns the Cartesian body, its four sides, the two continuum
Fields, four tunable Parameters, the manufactured load definition, and all
four homogeneous displacement-trace Relations. Its Lamé Parameters are
forwarded through the Component slots without acquiring occurrence-local
identities.

After occurrence elaboration, the package hierarchy and the existing
explicit-flat fixture must have identical typed Kernel structure under a
complete deterministic identity bijection. Residual expression tapes are
compared by their lowered coefficients, evaluated actions, solutions,
equilibrium, and convergence; cross-frontend expression-DAG canonicalization
belongs to [RFC 0073](../../../rfcs/0073-structural-semantic-fingerprint.md).
Dependency aliases, declaration order, binding order, file
order, and even a renamed provider package cannot become numerical dispatch
keys.

The independent affine-potential root supplies a nonzero unit body force in
the first component. Its algebraic solution, integrated force, and boundary
reaction must also agree exactly with the existing explicit-flat fixture, so
zero resultant cannot conceal a package-boundary wiring error.

The accepted execution retains exact package-compilation, Model v4,
Realization v1, Run v2, and package-execution-binding lineage.

Run:

```bash
cargo test --locked -p eqiora --test packaged_isotropic_balance_2d
cargo run --locked -p eqiora-verify -- run --case solid.packaged-isotropic-balance-2d
```

This case does not claim a traction Port, boundary partition, material model
beyond constant intrinsic-2D isotropic small strain, plane stress/strain, 3D,
unstructured or mixed elements, nonlinear or dynamic elasticity, FSI, or a
general package-based executor.

The release is copied into this conformance root because exact package
identity includes every bundled byte. Later live package versions therefore
cannot silently rewrite evidence already accepted for `0.1.0`.
