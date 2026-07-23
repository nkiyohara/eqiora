# Acceptance contract

Acceptance requires:

- one exact `Eqiora.Solid.LinearElasticity@0.2.0` release;
- pinned semantic and source digests over verification-owned immutable package
  bytes, with the live package required to match exactly;
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
