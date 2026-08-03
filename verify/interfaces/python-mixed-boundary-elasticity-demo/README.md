# Installed Python mixed-boundary elasticity demo

This case verifies that the installed Python package compiles the accepted
byte-exact source through the current Model owner, executes the same Rust-owned
bounded application result as Studio, exposes immutable co-indexed Q1
displacement data and relational Model → Realization → displacement
Snapshot → Run lineage, and
produces a headless, caller-owned displacement still with an explicit visible
scale.

Scientific meaning and tolerances remain owned by
[`solid.mixed-boundary-elasticity-2d`](../../solid/mixed-boundary-elasticity-2d/README.md).
The Studio consumer is
[`interfaces.studio-mixed-boundary-elasticity-demo`](../studio-mixed-boundary-elasticity-demo/README.md).
The complete executable contract is in [`case.toml`](case.toml).
