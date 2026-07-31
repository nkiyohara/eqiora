# Reference provenance

This evidence contract was derived independently before the renderer
implementation was inspected.

- `interfaces.python-exact-cylinder-stokes-result` owns the accepted Result,
  its exact lineage, pressure, coordinate, and connectivity associations.
- Matplotlib's public `tripcolor` contract owns explicit unstructured
  triangular-grid input and vertex-associated Gouraud presentation.
- The Agg environment owns headless raster rendering for this test.

No new fluid derivation, pressure constants, plot image, or pixel baseline is
introduced.
