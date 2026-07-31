# Native Studio exact-cylinder steady Stokes demonstration

This case registers one immutable application composition. Native Studio
decodes the checked exact rectangle-minus-circle geometry and canonical Model
example from the sole current artifact epoch, replays them through a fresh
graph store, realizes the accepted source-owned 50-chord affine mesh, resolves
the existing coherent-SI MINI/P1
Stokes plan, and executes it with the accepted Faer SparseLU tuple. It then
creates a Model- and Realization-bound Run, authored pressure snapshot, and
unstructured P1 application projection before publishing anything to Studio's
bounded session cache.

The command response contains summary evidence, not renderer-owned scientific
meaning: exact and realized geometry identities and error bounds; the named
cylinder constraint force on the fluid; physical-parent-outward inlet and
outlet fluxes; complete reaction/body/traction momentum closure; and the frozen
solver tuple with independently reapplied true and continuity residuals. The
pressure workspace opens through the existing three-stream data plane only
when its Model, semantic revision, Realization, Run, snapshot, mesh, Field, and
Domain identities agree.

The Rust integration target remains the scientific authority and retains the
existing independently frozen physical observations and falsifiers. Native
adapter tests replay the complete embedded example and its serialized result.
TypeScript protocol and session tests reject geometry, unit, flux, momentum,
residual, descriptor, and stream drift before ready-state publication. Browser
preview returns an explicit native-only failure and never substitutes canned
scientific values.

Run:

```bash
cargo test --locked -p eqiora --test exact_circular_hole_stokes_2d
cargo run --locked -p eqiora-verify -- run --case interfaces.studio-exact-cylinder-stokes-demo
npm --prefix studio run check
npm --prefix studio test
cargo test --manifest-path studio/src-tauri/Cargo.toml --locked cylinder_demo
```

This is a steady Stokes demonstration on one coarse affine mesh. The displayed
cylinder vector is the algebraic constrained-vertex force on the fluid. It is
not a curved-element, Navier--Stokes, Reynolds-number, drag/lift coefficient,
vortex-shedding, Strouhal-number, PDE-convergence, benchmark, validation,
general meshing, or production-scale claim.
