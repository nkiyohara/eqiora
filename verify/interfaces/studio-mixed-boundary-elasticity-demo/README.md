# Native Studio mixed-boundary elasticity demonstration

This case registers one bounded application composition over the already
verified `solid.mixed-boundary-elasticity-2d` direct Model. Native Studio
compiles the exact checked-in Model v4 source, resolves a generated 16-by-16
Cartesian continuous-Q1 plan through the public Eqiora facade, and executes
the accepted host-serial `f64` conjugate-gradient path.

The closed WebView response contains the 289 ordered mesh vertices, 256
quadrilateral cells, solver-owned two-component displacement values,
constrained reaction, integrated body force, solve/assembly evidence, and
content-addressed Model, Realization, and output-less Run identities. Studio
draws original and displaced coordinates with an explicit presentation-only
scale and keeps a keyboard-accessible selected-vertex table synchronized with
the retained numeric values.

The application does not reconstruct a constitutive law, stress, strain,
traction, analytic solution, error norm, or convergence order. Those scientific
claims and their independent oracles remain solely owned by
`solid.mixed-boundary-elasticity-2d`. Browser preview returns an explicit
native-only failure rather than canned structural values.

Run:

```bash
cargo test --locked -p eqiora --test mixed_boundary_elasticity_2d
cargo run --locked -p eqiora-verify -- run --case interfaces.studio-mixed-boundary-elasticity-demo
npm --prefix studio run check
npm --prefix studio test
npm --prefix studio run test:e2e
cargo test --manifest-path studio/src-tauri/Cargo.toml --locked structural_demo
```

The claim is exactly one checked-in direct Model, one frozen Q1 realization,
and one presentation result. It does not claim a reusable deformation viewer,
other geometry or loads, stress postprocessing, nonlinear structure, contact,
3D, validation, production solvers, durable Field output, distribution,
devices, performance, or scale.
