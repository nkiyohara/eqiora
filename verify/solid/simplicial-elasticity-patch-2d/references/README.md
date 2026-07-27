# Independent reference

Two independent pre-implementation derivations agreed on this case.

The analytic route used barycentric P1 shape gradients transformed by
`J^{-T}` and the intrinsic two-dimensional law
`sigma = 2 mu sym(grad(u)) + lambda trace(sym(grad(u))) I`. It established
exact affine interpolation, zero rigid strain, constant stress, and global
reaction equilibrium from partition of unity.

The separate route assembled the four triangles with exact-rational algebra,
then repeated the calculation independently in binary64. Its largest correct
error was `2.03e-15`. A `J^{-1}`/`J^{-T}` swap and omitted determinant scaling
produced errors of at least `1.15e-2`, over `2e11` times the accepted bound.
For the nonzero body load it derived the interior displacement
`(554001/55700000, -554001/64280000)`, which also makes the solved response
sensitive to the complete isotropic constitutive form.

The frozen numerical values are in `../expected/observables.toml`.
