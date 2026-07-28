# Reference provenance

The small deterministic Eqiora CG implementation is an independent oracle for
the SPD path. The nonsymmetric solution is manufactured exactly. faer 0.24.4
provides the production CG and BiCGSTAB algorithms behind the isolated adapter:

- <https://docs.rs/faer/0.24.4/faer/matrix_free/conjugate_gradient/>
- <https://docs.rs/faer/0.24.4/faer/matrix_free/bicgstab/>

Eqiora recomputes the accepted true residual independently of faer's recursive
residual estimate.

## Pre-committed, not yet claimed

[`sparse-lu-oracle.md`](sparse-lu-oracle.md) documents the frozen exact-rational
oracle for the `SparseLu` direct algorithm proposed in
[Issue #126](https://github.com/nkiyohara/eqiora/issues/126). Its provenance is
different in kind from the references above: it is not a description of an
algorithm this case executes, but evidence committed before the implementation
exists, by an author who read no implementation. That reference states the
authoring boundary, the fixture digest, and the nonclaims.
