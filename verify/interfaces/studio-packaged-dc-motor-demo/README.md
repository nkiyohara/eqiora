# Native Studio packaged DC-drive demonstration

This case registers one bounded application composition over the already
verified `hybrid.packaged-dc-motor-controller` model. Native Studio prepares
the exact checked-in electrical, drive, and root releases through the ordinary
compiler-derived package path, derives their exact lock record, compiles
`Main`, and executes the accepted host-serial `f64` backward-Euler
configuration for 100 one-millisecond steps.

The application projects only three production trajectory fields: motor
current, load angular speed, and the zero-order-held voltage command. It
retains integer step provenance, 10 complete hold intervals, and 11 controller
commit boundaries. The closed WebView protocol rejects missing, reordered,
nonfinite, wrong-unit, non-held, or foreign-package payloads before
ready-state publication. Browser preview returns an explicit native-only
failure rather than canned scientific values.

Run and package/Run-binding identities are constructed only after that
structural projection accepts. The workspace shows the exact three-package
closure and Model, compilation, Run, and binding digests. It binds its
scientific attribution to the registered verified
`hybrid.packaged-dc-motor-controller` manifest at compile time.

No motor equation, controller law, reference propagator, residual, physical
port reduction, power term, or energy term is reimplemented by Studio. The
existing hybrid case remains the sole scientific authority for those claims.

Run:

```bash
cargo test --locked -p eqiora --test packaged_dc_motor_controller
cargo run --locked -p eqiora-verify -- run --case interfaces.studio-packaged-dc-motor-demo
npm --prefix studio run check
npm --prefix studio test
cargo test --manifest-path studio/src-tauri/Cargo.toml --locked dc_motor_demo
```

The claim is exactly one checked-in package closure, solver configuration,
initial condition, controller period, and 0.1-second trajectory. It does not
claim a general trajectory artifact, other drives or controllers, physical
motor validation, accuracy order, derived results, production solvers,
real-time execution, distribution, devices, or performance.
