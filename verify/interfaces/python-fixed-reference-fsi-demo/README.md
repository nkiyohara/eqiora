# Installed Python fixed-reference FSI demo

This case verifies one installed-Python projection of the already accepted
two-step fixed-reference FSI application. Python compiles the packaged
byte-exact source through the current Model owner and passes that immutable
Model to the same Rust-owned composition used by Studio. The result retains
the complete two-state
trajectory, relational Model → geometry → correspondence → mesh → Realization
→ state → trajectory → Run lineage, and solver-owned coupled fields.

Common scalar and deformed stills over this result's accepted `Trajectory` are
owned separately by
[`interfaces.python-trajectory-field-stills`](../python-trajectory-field-stills/README.md).
This case retains no demo-specific presentation entry point.

Scientific meaning, tolerances, and expected values remain owned by
[`fsi.fixed-reference-monolithic-step-2d`](../../fsi/fixed-reference-monolithic-step-2d/README.md)
and
[`artifacts.fixed-reference-fsi-spatial-trajectory`](../../artifacts/fixed-reference-fsi-spatial-trajectory/README.md).
The complete bounded executable contract is in [`case.toml`](case.toml).
