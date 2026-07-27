# Canonical transient inlet/outlet boundary

This case proves that the canonical transient Navier--Stokes MINI path carries
two boundary conditions already supported by the numerical object: a
non-homogeneous essential velocity on selected facets and a constant traction
on the remainder. The domain remains the exact Cartesian unit box and the
mesh remains affine simplicial.

The inlet profile is a scalar velocity coefficient Field defined on the volume
by an ordinary coordinate expression. Its boundary law converts the scalar to
the parent-outward normal vector with `normal(isotropic_lift(...))`; the sign in
the residual makes the prescribed velocity parent-inward. The outlet applies
the exact Newtonian parent-outward traction with constant value zero.

The mixed boundary selects boundary-determined pressure and therefore has no
zero-integral constraint or gauge row. The companion homogeneous model selects
the other regime and both owns and uses its zero-integral gauge. An all-traction
model is rejected because the velocity would otherwise retain a constant
translation mode.

For an all-essential boundary, the prescribed P1 trace is integrated over
every exact boundary facet before any assembly adapter is called. Nonzero net
parent-outward flux is rejected explicitly. Separate lowering falsifiers reject
an uncovered side and two conditions on one side.

The executable uses positive low inertia because the existing method-native
skew/conservative audit assumes boundary-negligible convective flux. That audit
is outside this slice; the registered run still advances one genuine transient
step with a nonzero 0.1 m/s inlet and the traction pressure regime.

Run:

```sh
cargo run --locked -p eqiora-verify -- run --case fluid.canonical-inlet-outlet-navier-stokes-2d
```
