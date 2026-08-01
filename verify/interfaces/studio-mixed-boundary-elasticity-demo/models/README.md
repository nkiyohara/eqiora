# Model input

The native composition embeds the exact repository-owned source at
`verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi` and compiles it
through the ordinary current `ModelDocument::compile` path. It does not copy
the equations into the application, depend on the packaged authoring variant,
or expose a historical Model codec or generation selector.

The registered `solid.mixed-boundary-elasticity-2d` case remains authoritative
for the Model's isotropic elasticity meaning, mixed boundary closure, analytic
convergence, recovered traction, and global balance.
