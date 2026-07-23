# Fixed-reference FSI spatial trajectory

This case publishes two complete accepted spatial states from consecutive
executions of the fixed-reference monolithic FSI operator. It keeps physical
Field meaning, logical numerical content, storage layout, state identity,
trajectory publication, and Dataset selection as separate typed contracts.

Each accepted state contains fluid velocity, fluid pressure, solid velocity,
and solid displacement. The MINI fluid velocity retains both its vertex and
cell-bubble coefficient blocks. Values outside each Field's semantic support
closure are canonical positive zero, while the fluid and solid velocity
snapshots retain bit-identical values on the exact conforming interface.

Two one-state segments prove immutable prefix extension. The final trajectory
is an exact output of its typed, Model/Realization-bound Run. An
identity-only Dataset view references the selected states and Fields without
copying values. A narrow raw-chunk storage witness proves that rechunking does
not change logical discrete-Field identity.

Run:

```bash
cargo test --locked -p eqiora --test fixed_reference_fsi_spatial_trajectory
cargo run --locked -p eqiora-verify -- run --case artifacts.fixed-reference-fsi-spatial-trajectory
```

This case does not claim general transient CFD, restart from spatial state,
variable-step integration, ALE, remeshing, production storage formats,
visualization conventions, ML feature semantics, or training pipelines.
