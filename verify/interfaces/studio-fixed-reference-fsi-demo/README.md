# Native Studio fixed-reference FSI demonstration

This case registers one bounded, immutable application composition over the
already verified `fsi.fixed-reference-monolithic-step-2d` direct Model and the
already verified `artifacts.fixed-reference-fsi-spatial-trajectory` lineage.
Native Studio compiles the exact checked-in
`verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi` source as
`ExactModelCodec::V4`, reconstructs the exact 9-vertex, 8-affine-triangle
conforming mesh whose fluid body is `[0 m, 1 m] x [0 m, 1 m]`, whose solid body
is `[1 m, 2 m] x [0 m, 1 m]`, and whose complete interface lies at `x = 1 m`,
and resolves the existing fieldwise coupled plan at `dt = 0.05 s` with
`L = 2 m`, `U = 0.5 m/s`, and `P = 4 Pa` on the existing host-serial `f64`
reference MINRES/identity/reproducible tuple.

Execution starts from the existing prestrained previous state, whose only
nonzero coefficient is the `[0.02 m, 0 m]` displacement of the free interface
midpoint at `(1 m, 0.5 m)`. Studio then performs two genuine consecutive
accepted steps at `0.05 s` and `0.10 s` — the second consumes the first
accepted velocity and displacement state, never a duplicated observation — and
constructs the existing immutable two-state fixed-spatial trajectory lineage in
memory.

The closed WebView response retains only solver-owned quantities: fluid, solid
and interface region identities; shared mesh-vertex velocity and fluid MINI
cell-bubble velocity; P1 pressure support and coefficients on the fluid
closure; solid displacement; free-interface action; the backward-Euler energy
terms; the numerical residual, continuity, kinematic, exact shared-trace jump
and interface action imbalance; assembly and solve evidence; and the exact
Model, geometry, correspondence, mesh, Realization, final Run, state and
trajectory digests. Intrinsic two-dimensional action is presented in `N/m` and
intrinsic energy in `J/m`; the two units are never conflated. Solver stopping
evidence stays a separate presented group from physics acceptance, because a
converged backend is not an accepted step.

Studio recomputes no physics. It reimplements no weak form, constitutive law,
trace quotient, backward-Euler elimination, residual, interface action, or
energy identity, and it derives no stress, drag, or lift. Display scaling is
reversible presentation applied over retained values only; the retained values
themselves are never rewritten. The two registered scientific cases remain the
sole authority for every numerical claim, and this case reuses their accepted
thresholds by reference rather than restating any accepted value. Browser
preview returns an explicit native-only failure and publishes no canned
scientific substitute.

Run:

```bash
cargo test --locked -p eqiora --test fixed_reference_monolithic_fsi_step_2d
cargo test --locked -p eqiora --test fixed_reference_fsi_spatial_trajectory
cargo run --locked -p eqiora-verify -- check --case interfaces.studio-fixed-reference-fsi-demo
npm --prefix studio run check
npm --prefix studio test
cargo test --manifest-path studio/src-tauri/Cargo.toml --locked fsi_demo
```

This manifest is precommitted: it is authored before the composition exists,
by an agent that does not implement it. Its status is therefore `specified`
and it declares no executable evidence yet. The integrator promotes it to
`verified` and registers `[evidence]` only once the native composition and its
fail-closed protocol path pass against an unmodified oracle; an implementer
that believes the oracle is wrong returns the proof instead of editing this
directory.

The claim is exactly one checked-in direct Model, one reconstructed mesh, one
frozen fieldwise plan and solver tuple, one prestrained initial state, two
accepted steps, and one in-memory trajectory presentation. It does not claim a
moving mesh or ALE, advection, remeshing, partitioned coupling, any other
Model, geometry, mesh or time plan, 3D, derived stress, drag or lift, analytic
comparison, validation or benchmarking, a general viewer, durable disk
publication, production solvers, MPI or GPU execution, performance, or scale.
