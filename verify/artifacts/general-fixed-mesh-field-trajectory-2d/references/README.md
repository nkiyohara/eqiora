# Independent structural oracle

The pre-implementation oracle derives the accepted graph from the public
claim rather than implementation details:

```text
Model + Realization + Geometry + Correspondence + fixed Tri3 Mesh
  -> DiscreteField blocks -> Field snapshots -> ordered SpatialStates
  -> immutable Segments -> final Trajectory root <- sole Run output
```

It requires explicit 2D admission, complete unique catalogs, exact physical
snapshot replay, immutable-prefix reconstruction, fixed-step coordinates, and
Run outputs equal to the singleton final-root set. The existing registered FSI
cases independently own all numerical values, tolerances, and physical
acceptance.
