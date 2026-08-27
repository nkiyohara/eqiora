# Input trajectory

There is no new Model, mesh, or result fixture. The gate performs the live
installed workflow from the packaged component-only `fixed-reference-fsi.eqi`
resource, Python-authored adjacent Geometry, caller common Mesh, and root
Model-first scoped resolution, then consumes the common `Trajectory` reached
through common Run execution and
`result.trajectory`.

The identity falsifier compiles a parameter mutant from that same packaged
source. It deliberately keeps every semantic Field ID inside a different exact
Model artifact. The mutant is resolved only to obtain authenticated foreign
`FieldRef` values; it is never run, plotted, or compared against an expected
scientific value, so the edited magnitude carries no scientific meaning here.

The sole scientific, lineage, and support authorities remain
`interfaces.python-fixed-mesh-trajectory`,
`interfaces.python-fixed-reference-fsi-demo`, and
`fsi.fixed-reference-monolithic-step-2d`.
