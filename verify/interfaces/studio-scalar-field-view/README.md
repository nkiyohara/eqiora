# Studio bounded scalar Field view

This case registers the application-owned meaning beneath Studio's first
two-dimensional scalar Field view. The accepted scalar-elliptic plan fixes the
canonical Field and Domain identities, optional presentation alias, value
dimension, coherent-SI Cartesian bounds, vertex or cell-centre association,
logical shape, and last-axis-fastest order before numerical allocation.

The ordinary FEM and FVM execution paths must return summaries and complete
values that match that previewed projection exactly. Replaying the same Model
without compiler aliases proves that a non-semantic name is optional. Executing
the accepted plan against a changed Model fails before execution.

Studio adds a separate `eqiora.studio.field-view/v1` data plane. A successful
run response stays summary-only. Only the explicit **View field** action opens
the exact Model/run/Realization identity and reads fixed-size raw little-endian
`f64` chunks from a two-entry session cache. TypeScript tests reject foreign
descriptors, missing/reordered/short/long/non-finite chunks, and final
count/range drift before any complete value array reaches the renderer.
Playwright covers the synchronized semantic table, keyboard cursor, pointer
selection, exact selected value, responsive containment, and accessibility.
Those client checks are local Studio validation; the registered Cargo target
owns the shared application projection.

Run:

```bash
cargo test --locked -p eqiora-api --test scalar_field_projection
cargo run --locked -p eqiora-verify -- run --case interfaces.studio-scalar-field-view
```

This case does not claim unstructured or nonuniform grids, vector/tensor/complex
or mixed Fields, a 3D renderer, GPU residency or zero-copy transfer, a durable
Field artifact, level-of-detail streaming, or production-scale visualization.
