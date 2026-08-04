# Input trajectory

There is no new Model, mesh, or result fixture. The gate performs the live
installed workflow from the packaged `fixed-reference-fsi.eqi` Model resource
and consumes the common `Trajectory` reached through explicit
`FixedMeshMonolithic` resolution, common Run execution, and
`result.trajectory`.

The identity falsifiers derive two Models from that same packaged source, and
neither introduces a second physical model. Compiling it once more with its
model name changed yields a structurally equivalent Model with a different
exact digest, identically named fields, and — because independent compilation
allocates fresh semantic field ids — a disjoint field-id inventory. Committing
one value edit on the accepted Model yields the complementary fixture: every
semantic field id preserved inside a different exact Model artifact. That
revised Model is never solved, plotted, or compared against any expected value,
so the edited magnitude carries no scientific meaning here.

The sole scientific, lineage, and support authorities remain
`interfaces.python-fixed-mesh-trajectory`,
`interfaces.python-fixed-reference-fsi-demo`, and
`fsi.fixed-reference-monolithic-step-2d`.
