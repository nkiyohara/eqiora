# Eqiora.Mechanics.BoundaryLoads

This exact package supplies one method-neutral normal-pressure terminal for
the nominal velocity/traction Connector in `Eqiora.Mechanics.Interfaces`.
`NormalPressureTraction2d` binds a root-owned pressure-valued continuum Field
and contributes only

```text
terminal flux - parent-outward normal(isotropic pressure) = 0.
```

The conserving Connection reverses terminal flux at the connected physical
Port, so positive pressure produces inward traction on the connected body.
The separate Field preserves exact support and lets the enclosing Model own
its definition, parameters, and provenance.

Version `0.1.0` selects no boundary quadrature, trace space, pressure gauge,
mesh, solver, transfer, time law, or coupling algorithm. Its first execution
claim is restricted to a spatially constant Field in the mixed-boundary MINI
Stokes evidence; coordinate-varying load execution remains separate.
