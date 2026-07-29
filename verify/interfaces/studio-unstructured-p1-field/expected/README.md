# Expected outcome

The exact-Run-output-bound fixed-reference FSI pressure snapshot produces one
complete affine-triangle P1 scalar projection. A Run that does not register the
snapshot output, foreign Run lineage, a vector P1 snapshot, or a coefficient
block belonging to another Field fails before projection materialization.
Studio-side tests additionally require fail-closed descriptor, chunk,
connectivity, bounds, range, and stale-state behavior.
