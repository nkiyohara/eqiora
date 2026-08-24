# Python rich Trajectory display

This case verifies the N3 Trajectory slice of the Notebook presentation
contract: the already accepted two-state,
fixed-mesh 2D `Trajectory` is a bare read-only output in exact JupyterLab and
marimo hosts. It reuses the existing private wheel-local presentation bundle;
it adds no public viewer type, dependency, distribution asset, scientific
value, tolerance, interpolation, or generated frame.

The adapter admits only one field meaning that is unambiguous in every stored
state: the unique invariant scalar Vertex `FieldSnapshot` with the same exact
Model-bound field identity, dimension, frame, Mesh digest, and accepted support
membership. Whole-mesh zero-extended coefficients are gathered through those
support indices; nonzero values, names, array lengths, and renderer visibility
never establish support.

The frontend receives owned little-endian copies of exact coordinates,
triangles, support, steps, times, and supported values. SHA-256 checks reject
transport drift before rendering. Previous, next, slider, play/pause, and speed
controls select only stored sequence members. The visible metadata is the
stored state index, step, physical time, field ID, dimension tuple, and frame.
All controls are client-only presentation state and send nothing back to
Python.

The ordinary positive path runs in the same exact candidate-host fixtures as
the accepted Mesh view. Focused Python tests own hook filtering, identity,
support, immutable delegate state, and bundled-asset behavior; TypeScript tests
own byte admission and reordered-state rejection; Playwright owns the two real
hosts, exact discrete control transitions, and loopback-only runtime boundary.

This is not arbitrary trajectory or field visualization, field selection,
interpolation, a topology-changing or 3D path, vector/tensor/cell rendering,
stored widget replay, publication media, or scientific evidence from pixels.

```bash
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-rich-trajectory-display
```
