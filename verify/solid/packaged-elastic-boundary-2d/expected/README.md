# Acceptance contract

Acceptance requires:

- an exact release closure prepared from the current `Eqiora.Solid.LinearElasticity`
  and `Eqiora.Mechanics.Interfaces` sources through the ordinary package owner;
- the unchanged closed `IsotropicBalanceWithPotential2d` contract plus one
  nominal displacement/traction Connector and one separate boundary
  Component;
- four exact generated boundary Ports, Relations, and Activations;
- exact Connector and Boundary payload agreement between each package Port
  and its connected singular terminal Port;
- two componentwise residual roots per boundary, containing the displacement
  trace and full isotropic parent-outward traction expression;
- invariant root semantic identity and canonical Model bytes under exterior
  member order and dependency-alias spelling; and
- no mesh, facet, Realization, solver, or execution claim in this semantic
  evidence root.
