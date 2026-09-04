# Eqiora.Fluid 0.3.0

`SteadyStokes2d` is the curated two-dimensional steady incompressible Newtonian
model. It composes the public `SteadyNewtonianBalance2d` and
`VelocityTractionInterface2d` parts, so a project can use the common model or
replace either part through ordinary package composition.

The same import provides zero, normal, and field-driven boundary conditions.
`PrescribedVelocity2d` and `PrescribedTraction2d` accept vector Fields defined
on the fluid body. Their traces prescribe the complete boundary velocity or
parent-outward traction.
