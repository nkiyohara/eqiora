# Installed Python fixed-reference FSI demo

This case verifies one installed-Python projection of the already accepted
fixed-reference FSI meaning through the root common lifecycle. Python authors
the exact adjacent-partition Geometry, generates its authenticated common Mesh,
compiles the equations-only Component, resolves exact Domain-scoped spatial,
temporal, solve, and scaling policies, and supplies four exact-Field initial
assignments. The common `Result` retains the selected two-state `Trajectory`
and exact Model → Geometry → correspondence → Mesh → Realization → State →
Trajectory → Run lineage. `eqiora.fsi.evidence(result)` retains only the
partition and state-keyed numerical observations.

Common scalar and deformed stills over this common Result's accepted
`Trajectory` are
owned separately by
[`interfaces.python-trajectory-field-stills`](../python-trajectory-field-stills/README.md).
This case retains no demo-specific presentation entry point.

Scientific meaning, tolerances, and expected values remain owned by
[`fsi.fixed-reference-monolithic-step-2d`](../../fsi/fixed-reference-monolithic-step-2d/README.md)
and
[`artifacts.fixed-reference-fsi-spatial-trajectory`](../../artifacts/fixed-reference-fsi-spatial-trajectory/README.md).
The complete bounded executable contract is in [`case.toml`](case.toml).
