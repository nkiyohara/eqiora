# General fixed-mesh 2D Field trajectory replay

This case admits the existing verified two-step fixed-reference FSI trajectory
through one physics-neutral public replay boundary. The boundary resolves the
exact Model, Realization, geometry, correspondence, affine-triangle mesh,
segments, ordered states, coherent-SI Field snapshots, normalized numerical
blocks, final immutable root, and Run. It creates no second result identity or
durable wire.

Catalog declaration order is deliberately irrelevant, but the artifact DAG is
closed: every referenced identity must resolve exactly once, every declared
object must be used, state and Field ordering remains canonical, and the Run
must contain exactly the final trajectory root as its sole output. The case
constructs that accepted observation trajectory directly from the unchanged
scientific support composition before replaying the complete dependency set.

Run:

```bash
cargo test --locked -p eqiora --test general_fixed_mesh_field_trajectory_2d
cargo run --locked -p eqiora-verify -- run --case artifacts.general-fixed-mesh-field-trajectory-2d
```

The existing FSI cases remain the authority for numerical and physical
acceptance. This case does not claim a new scientific result, variable-step
time, ALE, remeshing, 3D, restart, rendering, media encoding, archival storage,
or a universal trajectory interface.
