# Reference strategy

This interface case introduces no new scientific oracle. The existing
`mixed_boundary_elasticity_2d` Rust target owns the independent analytic
displacement/gradient route, convergence, recovered-traction checks, balance,
and implementation falsifiers.

The interface oracle is structural and independent of those equations:

- exact 17-by-17 vertex and 16-by-16 cell cardinalities;
- ordered, finite, coherent-SI displacement payloads;
- the frozen public Realization and reference-solver tuple;
- true residual no greater than the solver-owned target;
- exact Model, Realization, and Run lineage;
- compile-time attribution to the registered verified case; and
- fail-closed asynchronous and browser publication.

Studio applies only a reversible display transform `x + scale * u`; the
retained solver values and evidence are not changed or recomputed.
