# Reference provenance

The small deterministic Eqiora CG implementation is an independent oracle for
the SPD path. The nonsymmetric solution is manufactured exactly. faer 0.24.4
provides the production CG and BiCGSTAB algorithms behind the isolated adapter:

- <https://docs.rs/faer/0.24.4/faer/matrix_free/conjugate_gradient/>
- <https://docs.rs/faer/0.24.4/faer/matrix_free/bicgstab/>

Eqiora recomputes the accepted true residual independently of faer's recursive
residual estimate.

## Pre-committed sparse-LU oracle

[`sparse-lu-oracle.md`](sparse-lu-oracle.md) documents the frozen exact-rational
oracle consumed by this case for the `SparseLu` direct algorithm added under the
`SolverPlan` contract of
[RFC 0010](../../../../rfcs/0010-execution-backend-contracts.md). Its provenance is
different in kind from the references above: it was committed before the
implementation existed, by an author who read no implementation, and now fixes
the values and falsifiers consumed by the registered Rust target. That reference
states the authoring boundary, the fixture digest, and the nonclaims.
