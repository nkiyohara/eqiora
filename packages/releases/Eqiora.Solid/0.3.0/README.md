# Eqiora.Solid 0.3.0

`PlaneStrainLinearElasticity2d` and `PlaneStressLinearElasticity2d` are the
curated small-strain isotropic models. Both take Young's modulus and Poisson's
ratio, with the two-dimensional constitutive assumption explicit in the
component name.

`YoungModulus` and `PoissonRatio` are reusable property contracts.
`PlaneStrainMaterial2d` and `PlaneStressMaterial2d` accept a typed material
composition built from those contracts.

The same package exposes `IsotropicBalance2d` and
`DisplacementTractionInterface2d` for projects that work directly with Lamé
parameters or replace one part. Its boundary components accept fixed, free, or
field-driven displacement and parent-outward traction data.
