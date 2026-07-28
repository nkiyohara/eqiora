# Independent oracle

The acceptance properties and falsifiers were fixed before implementation.
The flux compatibility oracle is the P1 facet integral of prescribed velocity
dotted with the parent-outward Cartesian normal. The pressure oracle has two
exclusive regimes: any traction facet removes the gauge, while a complete
essential boundary introduces exactly one zero-integral constraint.

The existing homogeneous evidence oracle is exact structural and numerical
equality with the direct MINI trajectory, including all reported nonlinear,
linear, assembly, conservation, and Jacobian-audit quantities.

## Open-boundary convection identity oracle

`oracle.py` freezes `S - C = B/2 - D/2` for constant density with P1 velocity
and P1 test rows on affine triangles, where `B` is the parent-outward boundary
flux and `D` the divergence defect. Two routes share only the mesh and the
rational type: an analytic route integrating polynomial coefficients exactly,
and a degree-3 quadrature route built from point evaluations alone. Arithmetic
is `fractions.Fraction`, so every comparison is exact and no tolerance exists.
Four witnesses cover the open, interior-facet, zero-flux and divergence-free
configurations; five falsifiers (omitted boundary, reversed normal, omitted
divergence, two degree-1 facet rules) are each caught. Run `python3 oracle.py`;
`--emit-frozen` regenerates the frozen table. It audits no Rust behaviour.
