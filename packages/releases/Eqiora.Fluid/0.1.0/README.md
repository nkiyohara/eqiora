# Eqiora.Fluid 0.1.0

`SteadyStokes2d` is the curated two-dimensional steady incompressible Newtonian
model. It composes the public `SteadyNewtonianBalance2d` and
`VelocityTractionInterface2d` parts, so a project can use the common model or
replace either part through ordinary package composition.

The same import provides `NoSlip2d`, `TractionFree2d`, and
`NormalPressureOutlet2d`. The pressure outlet takes an independently defined
exterior-pressure Field; positive exterior pressure produces compressive
parent-outward traction through the conserving connection sign convention.

These declarations define continuous physical meaning. Meshing, weak forms,
stabilization, pressure constraints, solvers, and execution remain Plan and
Realization choices.
