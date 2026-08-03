# Generated Cartesian Q1 spatial output

This specified case freezes the exact low-level spatial artifacts for the
already accepted `solid.mixed-boundary-elasticity-2d` solve. The ordinary path
is `MixedBoundaryElasticityResult2d`: its exact Model, Realization, Geometry,
Cartesian mesh, correspondence, displacement snapshot, and output-bearing Run
must form one closed content-addressed lineage.

The mesh has two equal uniform axes, but index association is not symmetric:
vertex and cell indices are last-axis-fastest, and each Q1 row uses
tensor-product/Z local order. Correspondence retains the one body and the exact
four boundary facet inventories. The snapshot retains the solver-owned
binary64 displacement projection without recomputing an analytic field.

This case adds no numerical tolerance or new scientific claim. The existing
solid case remains authoritative for displacement accuracy, convergence,
traction recovery, and force balance. Exact canonical bytes and digests are
not frozen before their production types exist; the independent integration
test instead freezes schema names, top-level key order, semantic values,
round-trip byte stability, digest stability, and cross-artifact relations.
