# Eqiora.Solid.LinearElasticity

`IsotropicBalanceWithPotential2d` provides intrinsic 2D small-strain isotropic
balance with Lamé parameters and a conservative load potential.
`IsotropicMechanicalInterface2d` binds displacement trace and parent-outward
traction to `QuasistaticMechanicalBoundary` Ports. `FixedDisplacement2d` and
`ZeroTraction2d` provide homogeneous boundary conditions.

`IsotropicElastodynamicsWithPotential2d` and
`IsotropicElastodynamicsWithPotential3d` provide first-order dynamics:

```text
derivative(displacement) - velocity = 0
density * derivative(velocity) - div(stress(displacement))
  - grad(load_potential) = 0
```

Their matching `ElastodynamicMechanicalInterface2d` and
`ElastodynamicMechanicalInterface3d` Components bind velocity trace and elastic
traction to `Eqiora.Mechanics.Interfaces::VelocityTractionBoundary`. This
power-conjugate dynamic boundary differs from quasistatic virtual work.

Each Field has an explicit spatial vector type on its declared body. The
enclosing Model and Realization supply initial fields, discretization,
solver, and execution target.
