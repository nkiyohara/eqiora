# Input trajectory

There is no new Model, mesh, or result fixture. The gate performs the live
installed workflow from the packaged `fixed-reference-fsi.eqi` Model resource
and consumes the common `Trajectory` reached through
`eqiora.fsi.solve_fixed_reference_fsi(model).trajectory`.

The foreign-identity falsifier compiles the same packaged source once more with
its model name changed, which yields a structurally equivalent Model with a
different exact digest and identically named fields. No second physical model
is introduced.

The sole scientific, lineage, and support authorities remain
`interfaces.python-fixed-mesh-trajectory`,
`interfaces.python-fixed-reference-fsi-demo`, and
`fsi.fixed-reference-monolithic-step-2d`.
