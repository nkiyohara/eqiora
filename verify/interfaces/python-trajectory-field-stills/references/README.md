# Reference provenance

This evidence contract was derived before the adapters existed, by an agent
that does not implement them, from the public bounded claim registered as
`interfaces.python-trajectory-field-stills` and the accepted contracts below
at revision `0bf2d2059499c98c70d811aca59e524a8c2a3b0c`.

- `interfaces.python-fixed-mesh-trajectory` owns the accepted `Trajectory`,
  `State`, and `FieldSnapshot` projection, its exact Model-bound
  `FieldRef` selection, its whole-mesh zero-extended coefficient blocks, and
  its exact support membership.
- `interfaces.python-fixed-reference-fsi-demo` and
  `fsi.fixed-reference-monolithic-step-2d` own the admitted trajectory's
  physics, solver values, partition, and lineage.
- `interfaces.python-mixed-boundary-elasticity-demo` and ordinary installed
  Matplotlib tests own the presentation conventions this case reuses: explicit connectivity, captured
  public renderer inputs, canonical unique undirected edges, an explicit
  visible scale, and a headless caller-owned Figure.
- Matplotlib's public `tripcolor`, `LineCollection`, and `Figure` contracts own
  unstructured triangular input, segment collections, and figure ownership.
- The Agg environment owns headless raster rendering for this test.

No new physical derivation, expected value, tolerance, plot image, or pixel
baseline is introduced, so the dual independent oracle rule for
derivation-bearing slices does not apply to this case.
