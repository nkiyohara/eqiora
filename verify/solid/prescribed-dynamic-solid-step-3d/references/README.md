# Reference authorities

The checked-in [case manifest](../case.toml),
[independent oracle](../expected/accepted-step.json), and
[Rust evidence](../../../../crates/eqiora/tests/prescribed_dynamic_solid_step_3d.rs)
freeze the exact unit-cube topology, material and time data, dual independent
continuum/finite-element values, binary64 tolerances, public two-type API, and
required falsifiers for this case.

The canonical first-order isotropic-elastodynamics and velocity/traction
boundary meaning is the already registered
`solid.dynamic-linear-solid-semantics-2d` authority. The tetrahedral 3D
authoring and exact geometry/mesh/correspondence construction conventions are
exercised by `fsi.fixed-topology-ale-monolithic-3d`; this case does not inherit
that case's FSI, ALE, trajectory, or durable-artifact claims.

`expected/accepted-step.json` contains the independent case values. Production
output was not used to derive or tune any value or tolerance.
