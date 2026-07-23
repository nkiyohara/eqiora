# Eqiora.Fluid.InertialStokes

`Eqiora.Fluid.InertialStokes@0.1.0` owns one reusable, method-neutral
two-dimensional incompressible Newtonian volume law with fluid inertia and a
conservative load potential.

`InertialStokesWithPotential2d` declares

```text
density * derivative(velocity)
  - div(2 * dynamic_viscosity * symmetric_part(grad(velocity))
        - isotropic_lift(pressure))
  - grad(force_potential) = 0

div(velocity) = 0.
```

The package intentionally owns no boundary condition, mesh, finite element,
time method, initial state, pressure reference, solver, target, or FSI policy.
Boundary meaning composes independently through
`Eqiora.Fluid.Incompressible` and `Eqiora.Mechanics.Interfaces`. Navier--Stokes
advection is not part of this release.
