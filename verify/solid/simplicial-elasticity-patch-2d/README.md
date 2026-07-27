# Simplicial elasticity patch test

This case verifies intrinsic two-dimensional isotropic small-strain elasticity
with continuous P1 displacement on four affine triangles filling the unit
square. The mesh is authored in `fixtures/distorted_patch.rs`; it is not
generated. Its only interior node is `(0.31, 0.63)`, and the four triangle
areas are `0.315`, `0.345`, `0.185`, and `0.155`.

Two independent pre-implementation routes supplied the oracle: an analytic P1
derivation and a separate exact-rational plus binary64 assembly. The shared
absolute bound is `256 * f64::EPSILON =
5.684341886080802e-14` for these order-one fixtures. The largest independently
observed correct error was `2.03e-15`; Jacobian-transpose and omitted-area
mutants missed by at least `1.15e-2`.

The evidence reproduces independent x-extension, y-extension, and shear fields
at every vertex; checks rigid translation and infinitesimal rotation energy;
checks the same constant stress in every triangle; balances a nonzero constant
body force with the reaction from one named complete-boundary surface while
checking the independently frozen constitutive displacement; and compares two
complete ordered displacement streams using `f64::to_bits`.

The domain remains a Cartesian box. Authored non-box geometry, Cook's membrane,
generated or adaptive meshes, plane-mode specialization, three dimensions,
nonlinear material or kinematics, contact, coupling, FSI, and transient
execution are not claimed.
