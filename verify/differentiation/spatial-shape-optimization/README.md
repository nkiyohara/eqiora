# Fixed-topology spatial shape differentiation

This case selects the upper bounds of one canonical Cartesian Domain as
explicit design coordinates. The generated mesh keeps its normalized axis
coordinates, so each selected direction produces one affine
reference-to-physical map JVP without changing topology.

The same geometry action is consumed by Q1 FEM and orthogonal TPFA FVM.
Every method-native state sensitivity and the mean-state adjoint gradient are
compared with centered differences from independently compiled and solved
Domain revisions.

As an executable consumer, a scalar log-aspect coordinate uses
`Lx = exp(s)` and `Ly = exp(-s)`. Existing implicit-adjoint gradients are
pulled back through that parameterization and consumed by Armijo
backtracking. Both discretizations improve the declared algebraic objective
and recover the square stationary shape from a nonsquare initial box.

This is a discrete, fixed-topology shape derivative. It is not evidence for a
mesh-independent continuous shape derivative, arbitrary vertex motion,
remeshing, adaptive refinement, topology change, nonorthogonal fluxes, or a
production optimization package.
