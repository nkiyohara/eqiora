# Studio spatial Realization capability verification

This case verifies Studio's first spatial Realization surface. It proves one
complete application path from a canonical scalar elliptic model to a
capability decision before numerical allocation, exact-plan execution, and
independently checked evidence. It does not make React or the Tauri adapter a
numerical authority.

## Contract under test

```text
canonical scalar elliptic revision
        ↓ shared spatial lowering
typed Realization intent
        ↓ preview before numerical allocation
content-addressed SpatialRunPlan
        ↓ exact key replay
mesh → quadrature → local operator → assembly → solve
        ↓ summary-only control projection
field summary + algebraic evidence + continuous evidence
        ↓ explicit View field action
exact field-view identity → bounded raw chunks → complete table/raster
```

`eqiora-api` owns the application sequence. It derives dimension and scalar
requirements from canonical lowering, bounds the generated Cartesian mesh and
field shape before allocation, constructs one coherent FEM or FVM
discretization, resolves host placement through the shared Realization
capability contract, and creates a `RealizationEnvelopeV1`-identified plan.
Execution reconstructs that exact plan before allocating numerical storage.

Studio bridge v5 projects only the information required to understand and
replay the decision: model digest, independent Realization revision, method,
mesh and field counts, quadrature/space, solver policy, placement, worker
budget, acceptance policy, and plan identity. React stores editable strings
and a Realization revision; it does not translate those values into another
solver configuration. An obsolete preview cannot become current.

The host worker limit is captured once when the native Studio session starts
from Rust's available-parallelism estimate, bounded to 64, and labelled
`studio-session-budget`. It is an admission budget for this client session,
not a claim about physical cores or exclusive machine capacity. One worker
selects the serial host adapter; two or more select a run-owned Rayon pool.
The protocol rejects an adapter/worker contradiction.

The run result intentionally crosses the control IPC without a mesh or field
array. It contains a bounded field location/count/range summary, assembly and solver
producer topology, solver verifier topology, reported and independently
recomputed true residual, residual target, iteration count, and the continuous
boundary/source balance. Spatial assembly and solve are atomic in this slice, so the UI
uses indeterminate status and exposes neither a percentage nor cancellation.

For the admitted two-dimensional result, an explicit action opens the separate
`eqiora.studio.field-view/v1` data plane. The application-owned plan has
already fixed Field/Domain identity, dimension, coherent-SI Cartesian bounds,
association, logical shape, and value order. A two-entry native session cache
then emits fixed 4,096-value raw little-endian `f64` chunks for that exact
Model/run/Realization identity. The client validates every chunk and the final
count/range before publishing one complete value array to its synchronized
semantic table, inspector, and Cartesian raster.

## Falsifying cases

- a model outside the admitted scalar elliptic lowering has no spatial
  workflow projection;
- invalid cells or workers remain editable UI state but cannot create a bridge
  request;
- dimensional cell or field counts above 250,000 fail before mesh allocation;
- worker count above the session budget fails capability resolution;
- FEM paired with cell-constant/centroid policy, FVM paired with Q1/Gauss
  policy, or an adapter/worker contradiction fails protocol validation;
- editing method, cells, or workers increments the Realization revision and
  invalidates the accepted plan while retaining completed evidence as stale;
- an obsolete asynchronous preview cannot replace the current Realization;
- a forged plan key, changed model digest, revision, method, cells, or workers
  fails exact replay before execution;
- another Studio run cannot overlap spatial assembly/solve;
- result field shape or location differing from the pre-allocation preview is
  rejected;
- a descriptor for another Model, run, or plan, a missing/reordered/short/long
  or non-finite chunk, or final count/range drift cannot publish a renderable
  Field;
- partial chunk transfer never reaches the renderer, and only the explicit
  user action starts transfer;
- a true residual above its admitted target cannot be shown as verified; and
- the browser interaction path has no serious or critical automated WCAG 2.2
  finding and remains within the 1440×900 viewport.

## Commands

```bash
cargo test -p eqiora-api --all-features --locked
cargo clippy -p eqiora-api --all-targets --all-features --locked -- -D warnings

cd studio
npm ci
npm run check
npm test
npm run build
npm run test:e2e
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run tauri build -- --no-bundle
```

The repository-wide layer, test, and clippy gates remain authoritative for the
shared contracts.

## Nonclaims

This case is not evidence for imported or adaptive meshes, high-order or mixed
spaces, vector/tensor fields, nonlinear spatial physics, transient spatial
execution, arbitrary solver/preconditioner selection, distributed assembly,
MPI, CUDA, NUMA placement, multiple GPUs, unstructured/nonuniform mesh
rendering, vector/tensor/complex Fields, 3D rendering, GPU residency/zero-copy,
durable Field artifacts, LOD/production visualization, checkpoint/restart,
spatial progress, or spatial cancellation. It verifies
generated Cartesian scalar elliptic FEM/FVM with `f64`, replicated layout,
identity-preconditioned reproducible CG, local serial/Rayon host execution,
and one bounded explicit 2D scalar Field view. The registered Field projection
case is
[`interfaces.studio-scalar-field-view`](../../verify/interfaces/studio-scalar-field-view/README.md).
Each broader capability must enter through the same requirements → Realization
→ adapter → evidence path and receive its own falsifying verification case.
