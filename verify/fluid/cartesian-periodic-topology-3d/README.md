# Exact Cartesian periodic topology witness in three dimensions

This case registers one structural reference slice for the exact current
Model authored in `models/periodic-box.eqi` and the exact Cartesian mesh axes
sealed in the private library oracle. The parent side lengths are 5, 7, and
11 coherent-SI units. The physical-axis cell counts are exactly `2 x 3 x 4`,
and every sealed axis is nonuniform.

The test begins with the ordinary path: `.eqi` compilation, whole-Model
validation, current Transaction and Model artifact round-trips, exact Model
replay, current Cartesian mesh v1 round-trip, and the existing semantic pair
composition for all three `SpatialPeriodic` Connections. Only then does the
crate-private group/projection run and the independent replayer compare every
quotient entity, box orbit, ordered closure, face/cell incidence, lifted seam,
and positive-axis packet. The permuted source reverses Connection endpoints
and reorders declarations while retaining the same canonical meaning.

The oracle is a pre-implementation package. On base `b364bec5` the production
module and crate-root registration do not exist, so the registered library
selector is intentionally unavailable until the implementation and
integration-owned registration are composed. After composition, run:

```bash
mise run affected -- --case fluid.cartesian-periodic-topology-3d
```

The claim is only the two exact artifact references exercised by this case.
Private implementation formulas may accept other inputs, but this evidence
does not verify another count tuple, coordinate sequence, side-length tuple,
Model revision, or mesh revision. It adds no numerical flow operator, scalar,
vector, or tensor execution, incompressible CFD, pressure, time integration,
Taylor--Green result, public quotient API, persisted quotient, MPI/GPU,
performance, scale, rendering, or publication claim. It owns no numerical
tolerance and no producer-generated golden.

All 22 RFC 0071 mutant IDs are constructed only after the positive replay.
The driver also proves its order-independence by reversing producer container
traversal without changing acceptance; traversal and allocation layout are
not identities. Rejected selected groups are checked for absence of projection
allocation and publication events, so an earlier unrelated parser, artifact,
or resource denial cannot make those falsifiers pass vacuously.
