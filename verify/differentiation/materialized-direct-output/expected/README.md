# Expected evidence

This package stores no new reduced expected scalar and no new tolerance. The ordinary normal and transposed solutions, residual bounds, componentwise ceiling, and all three falsifiers are derived at runtime from the digest-bound sparse-LU fixture and the symbolic projections in [`../references/README.md`](../references/README.md).

A result is accepted only by the registered Rust evidence after both ordinary calls pass and the wrong-source-RHS, wrong-transpose-result, and foreign-source probes reject at their precommitted boundaries. This file is structural package metadata only; it does not establish Stokes E2, another backend, performance, persistence, or a wider materialized-solve claim.
