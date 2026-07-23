# Imported simplicial Realization verification

This case serializes one accepted affine-simplex mesh as
`eqiora.simplicial-mesh-envelope/v1`, decodes it through explicit resource
limits, and binds its domain-separated content digest into a typed Realization
plan. The decoded mesh then drives the same canonical two-dimensional Poisson
model through P1 local operators, assembly, and the reference linear solver.
The same accepted artifact is then resolved for a four-worker, run-owned Rayon
pool. Each simplex cell produces one local packet feeding both reduced and full
systems; concurrent evaluation returns to the exact reference scatter order.

The four-triangle mesh has one free degree of freedom. Its independently
derived discrete value is exactly `1 / 12`; the assembled source and boundary
reaction must balance to roundoff. One/four-worker nodal values, source,
reaction, solver evidence, and the four-packet/two-target assembly shape must
be bit-identical while placement reports remain distinct. Excluded mesh
capability, content-identity drift, dimension drift, unknown wire fields,
resource excess, and forged quality evidence all fail before accepted
execution evidence is returned.

This is not a Gmsh/XDMF importer or a performance claim. Paths, importer
configuration, curved or mixed cells, global overlap proofs, partitioning,
adaptive refinement, 3D PDE execution, fast accumulation, NUMA placement, and
distributed/device assembly remain outside this verification boundary.

Run:

```bash
cargo test -p eqiora --test imported_simplicial_realization
cargo run -p eqiora-verify -- run --case artifacts.imported-simplicial-realization
```
