# Reference contract

Three independent references participate in acceptance:

1. The checked-in direct-flat fixture and current component source
   `verify/fluid/packaged-steady-stokes-2d/models/component.eqi` define
   package-neutral canonical meaning. The test prepares an exact release from
   that source; the adjacent historical `package-v0.1.0` remains unchanged.
2. The analytic physical solution is `u = 0` and
   `p = q = 0.75 Pa/m * (x - 2 m)`. It fixes the zero pressure integral,
   `(6 N/m, 0)` integrated body force, and `(-6 N/m, 0)` complete-boundary
   reaction per unit out-of-plane thickness.
3. A verification-only coherent-SI algebra oracle reuses the verified MINI
   local assembly on the physical mesh, independently of the SI scaling
   adapter. It compares every stored coefficient of `D A D / Theta` and
   every row of `D b / Theta` with the production dimensionless operator and
   load. This covers the complete reduced velocity, pressure, and gauge blocks.

The mixed-unit oracle is deliberately bounded to verification. It is not a
production assembly path, a second Realization, a solver acceptance norm, or
permission for public APIs to expose dimensionally heterogeneous matrices.
RFC 0043 remains the independent MINI stability and convergence reference;
this exact affine bridge does not duplicate or widen that claim.
