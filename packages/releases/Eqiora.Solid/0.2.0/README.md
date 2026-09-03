# Eqiora.Solid 0.2.0

`PlaneStrainLinearElasticity2d` and `PlaneStressLinearElasticity2d` are the
curated small-strain isotropic models. Both take Young's modulus and Poisson's
ratio, with the two-dimensional constitutive assumption explicit in the
component name.

`YoungModulus` and `PoissonRatio` are reusable property contracts.
`PlaneStrainMaterial2d` and `PlaneStressMaterial2d` accept a typed material
composition built from exact releases of those contracts.

The same package exposes `IsotropicBalance2d` and
`DisplacementTractionInterface2d` for projects that work directly with Lamé
parameters or replace one part. `FixedDisplacement2d` and `TractionFree2d`
provide the common homogeneous boundary conditions.

These declarations define continuous physical meaning. Meshes, weak forms,
elements, quadrature, solvers, and execution are selected separately.
