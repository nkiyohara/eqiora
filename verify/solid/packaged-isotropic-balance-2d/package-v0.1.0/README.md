# Eqiora.Solid.LinearElasticity

A deliberately small, ordinary Model Package for reusable linear-elasticity
Relations. Its first Component, `IsotropicBalanceWithPotential2d`, declares
one intrinsic two-dimensional volume support, two occurrence-bound continuum
Fields, two typed Lamé Parameter slots, and the isotropic small-strain balance
Relation.

The Component owns no Domain, Field, load definition, boundary condition,
mesh, element, quadrature rule, assembly policy, solver, execution target, or
schedule. An enclosing Model supplies the exact body, displacement, and load
potential and may realize the resulting flat Relation network independently.
The package has no privileged compiler or execution path.

This first release intentionally does not claim general body forces, traction
boundaries, plane-stress or plane-strain reduction, anisotropy, three
dimensions, nonlinear kinematics, dynamics, contact, or a broad solid
mechanics library. Those features must extend the same Model/Realization
boundary rather than enter through package-specific numerical behavior.
