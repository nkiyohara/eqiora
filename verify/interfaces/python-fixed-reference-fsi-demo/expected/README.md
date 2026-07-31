# Frozen observations

The installed result must expose the accepted 9 coordinates, 8 affine
triangles, exhaustive fluid/solid/interface partition, and two consecutive
accepted steps. Each step retains complete MINI vertex and bubble velocity,
fluid P1 pressure support, solid displacement, interface action, energy,
acceptance, solver, and assembly evidence. All arrays are co-indexed,
memoized, read-only, owner-independent, and finite.

The renderer is checked by capturing its region, connectivity, pressure,
displacement, interface, and velocity inputs. It may derive only cell pressure
for presentation and `coordinates + scale * displacement` for a finite
nonnegative visible scale. Image bytes and scientific values are not frozen by
this adapter case.
