# Eqiora.Fluid.Incompressible

`SteadyStokesWithPotential2d` and `ConservativeNavierStokesWithPotential3d`
provide steady 2D and conservative transient 3D incompressible Newtonian laws.
They bind root-owned velocity, pressure, and conservative force-potential Fields.

`NewtonianMechanicalInterface2d` and `NewtonianMechanicalInterface3d` bind
velocity trace and parent-outward Cauchy traction on the complete exterior to
`Eqiora.Mechanics.Interfaces::VelocityTractionBoundary` Ports.

These Components expand into ordinary typed Relations. The enclosing Model
supplies Domains, Fields, and boundary data; the Realization selects mesh,
discretization, solver, and execution policies.
