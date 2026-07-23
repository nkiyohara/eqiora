# Python scalar-elliptic Realization verification

This case verifies one bounded Python control-plane path from an accepted
spatial Model to numerical execution without exposing a second Realization
language. A typed `ScalarElliptic` FEM/FVM request is not identity. The shared
Rust application service previews it against the exact Model and host-serial
environment before numerical allocation, returning an opaque immutable
model-bound `Realization` with canonical persisted bytes and a content digest.

The registered Rust/PyO3 target proves deterministic equal preview, distinct
FEM/FVM identity, exact Model binding, and synchronous or submitted execution
through one native worker/state/cache lifecycle that owns no Python objects.
The accepted result retains Field location/count/range, continuous balance, and
independently checked linear convergence summaries. A successful run retains
one `RunManifestV2` naming its exact Model, semantic revision, Realization,
actual host-serial adapter/backend topology, and reduction policy. Exact-profile
replay succeeds, while a foreign Model, a mismatched Realization, and tampered
manifest linkage fail closed before producing a result.

The accepted solution transfers its complete primary Field allocation once
into the existing immutable CPU `float64` `Array`: FEM publishes canonical
vertex order including essential endpoints; FVM publishes canonical primary
cell order rather than its Q1 reconstruction. The Field descriptor exposes the
accepted spatial dimension and logical per-axis shape while the rank-one
transport buffer retains canonical Cartesian row-major order.

The registered test keeps the original constant-source one-dimensional
acceptance and adds an asymmetric affine oracle in one through three
dimensions for both methods. It therefore falsifies reversed/permuted values,
a free-unknown-only FEM result, a reconstructed FVM result, count/shape drift,
and a hidden origin copy. Every value agrees with the accepted summary, and a
read-only zero-copy NumPy view remains alive after Result destruction.

The spatial application contract reports exactly `PlanReplayed`,
`SystemFinalized`, and `SolutionAccepted`. These are accepted facts rather than
an inferred percentage. The linear solve is one atomic interval between the
last two phases. The native lifecycle publishes them through one single slot,
observes an explicit cancellation request only at those phases, returns typed
cancellation evidence and `EQ0506`, and never materializes a partial
`ScalarEllipticResult`. Repeated requests are idempotent. The installed-package
companion additionally proves that blocking `run(...)`, `submit(...).result()`,
and `await submit(...)` share equal accepted arrays, fingerprints, and Run
manifests.

The result separately exposes the algebraic execution receipt's exact
accepted-output fingerprint. That value is neither a digest of the semantic
Field array nor a durable `ArtifactDigest`; the persisted Run manifest
therefore has an empty output set. This evidence does not claim coordinate or
mesh arrays, non-Cartesian or nonuniform logical layouts, solver-iteration or
low-latency cancellation/progress, Python callbacks, worker/backend/solver
selection, production/Rayon/MPI/CUDA execution, general graph/PDE construction,
or durable result Artifacts.

Run:

```bash
cargo test --locked -p eqiora-python --test python_scalar_elliptic_realization
cargo run --locked -p eqiora-verify -- run --case interfaces.python-scalar-elliptic-realization
```

The installed-package companion uses the public `preview_realization(...)`,
`run(model, realization=...)`, and `submit(model, realization=...)` surfaces.
