# Accepted lineage

This structural oracle composes, without widening, the accepted predecessor
contracts indexed by these verification cases:

- `geometry.exact-circular-hole-geometry`
- `geometry.geometry-boundary-relation-scope`
- `geometry.circular-hole-chordal-reference-mesh`
- `geometry.circular-hole-chordal-realization-binding`
- `fluid.exact-circular-hole-stokes-2d`
- `fluid.fixed-domain-transient-navier-stokes-2d`
- `fluid.canonical-inlet-outlet-navier-stokes-2d`

The exact geometry owner remains authoritative for source, realized geometry,
mesh, and correspondence. The semantic Model supplies the source digest and
boundary names. The realization layer owns MINI/P1, imported-mesh identity,
scaling, backward Euler, energy-skew convection, nonlinear/linear plans, and
host/offline placement. The numerical path must retain checked assembly.

No predecessor is re-derived here, and no scientific expected value or
tolerance is introduced. The case owns only the private composition boundary
and its rejection/fixed-point falsifiers.
