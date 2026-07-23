# Reference contract

- [RFC 0039](../../../../rfcs/0039-canonical-isotropic-elasticity-2d.md)
  defines the unchanged name-independent elasticity lowerer, Q1 realization,
  convergence oracle, and Model/Realization/Run separation.
- [RFC 0040](../../../../rfcs/0040-occurrence-bound-field-slots.md) defines
  exact support-first Field binding, slot elimination, identity, and
  provenance.
- [RFC 0022](../../../../rfcs/0022-exact-package-identity-and-resolution.md)
  defines exact offline package resolution.
- [RFC 0032](../../../../rfcs/0032-typed-package-execution-lineage.md) defines
  the package-compilation-to-Realization-v1-to-Run-v2 binding.

The explicit-flat fixture and analytical convergence function are reused from
the already verified `solid.isotropic-elasticity-2d` case. No second compiler,
package-specific lowerer, or frozen floating-point solution supplies the
oracle.

The ordinary author input is the exact `package-v0.1.0` release owned by this
verification directory. Its immutable package bytes preserve the evidence
after the live public package advances. The additional component sources
exist only to falsify provider-name and input-order coupling.
