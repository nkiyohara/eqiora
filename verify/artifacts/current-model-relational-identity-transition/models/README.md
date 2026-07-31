# Models

This case authors no model source. Every classified fixture keeps its own
model input where it already lives: the packaged sources under `packages/`,
the ALE 3D sources under
`verify/fsi/fixed-topology-ale-monolithic-3d/models/`, and the recorded
accelerator programs inside their own `artifacts/model.json` bundles.

Duplicating a source here would create a second authority for a Model that
another case already owns.
