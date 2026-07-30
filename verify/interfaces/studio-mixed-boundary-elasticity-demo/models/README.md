# Model input

The native composition embeds the exact repository-owned source at
`verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi` and compiles it
explicitly with `ExactModelCodec::V4`. It does not copy the equations into the
application or depend on the packaged authoring variant.

The registered `solid.mixed-boundary-elasticity-2d` case remains authoritative
for the Model's isotropic elasticity meaning, mixed boundary closure, analytic
convergence, recovered traction, and global balance.
