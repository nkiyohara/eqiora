# Model input

This interface case adds no model fixture whose bytes could drift. The native
composition embeds the exact repository-owned source at
`verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi` and compiles
it explicitly with `ExactModelCodec::V4`. It does not copy the fluid momentum,
incompressibility, solid kinematic, solid momentum, or conserving
velocity/traction interface Relations into the application, and it does not
substitute the packaged authoring variant beside that direct source.

The mesh is not a second authored input. Studio reconstructs the exact
9-vertex, 8-affine-triangle two-body mesh and takes its fluid cells, solid
cells, and interface facets from the exact geometry/mesh correspondence, so a
coordinate-matched, partial, or wrongly oriented interface cannot reach
presentation.

The registered `fsi.fixed-reference-monolithic-step-2d` case remains
authoritative for this Model's inertial incompressible Newtonian fluid
semantics, first-order linear small-strain solid semantics, exact conforming
MINI/P1 velocity-trace quotient, monolithic backward-Euler step, and complete
pressure closure. The registered
`artifacts.fixed-reference-fsi-spatial-trajectory` case remains authoritative
for the two accepted spatial states, their complete Field inventory, and the
immutable segmented trajectory those states publish.
