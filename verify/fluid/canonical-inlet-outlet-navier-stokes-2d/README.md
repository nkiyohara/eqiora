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

The method-native audit now reproduces the complete open-boundary identity:
one half of the parent-outward momentum flux minus one half of the discrete
divergence defect. It derives every normal from the unique incident parent
cell, integrates the cubic P1 trace term with declared degree-three facet
quadrature, and rejects degree-one quadrature before assembly. The authored
low-inertia run remains, while a separate density-`1 kg/m^3` witness proves the
same nonzero inlet and traction-pressure regime with non-negligible convection.

This does not change the selected energy-skew weak operator or claim the
classical advective/DFG do-nothing outlet law. It makes acceptance exact for the
operator Eqiora already selects.

Run:

```sh
cargo run --locked -p eqiora-verify -- run --case fluid.canonical-inlet-outlet-navier-stokes-2d
python3 verify/fluid/canonical-inlet-outlet-navier-stokes-2d/oracle.py
```
