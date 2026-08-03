# Reference provenance

Two provider-separated routes agreed before this oracle was written: an
analytic derivation and an independent exact-rational construction. They agree
on axis samples, last-axis-fastest vertex and cell indexing, tensor-product/Z
local connectivity, Cartesian facet ordinals, body and boundary membership,
and the nodal sequence `u_x/L = xi-xi^2/2`, `u_y/L = 0`.

That analytic sequence checks mathematical meaning only through the unchanged
`solid.mixed-boundary-elasticity-2d` tolerance. This artifact case instead
requires the full application snapshot coefficients to be bit-identical to
the existing accepted solver projection. It derives no solver bytes, expected
digest, or tighter tolerance.
