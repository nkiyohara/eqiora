# Independent oracle

The accepted target is compiled from `../models/target.eqi`; it is not
constructed with the edit API. Its Cartesian measure is evaluated directly as

```text
(0.6 - -0.6) * (0.5 - -0.5) * (0.75 - -0.75) = 1.8 m^3
```

The edited child must be structurally equivalent to this independent target,
while exact occurrence identities are checked only against the immutable base
lineage. A child that applies only the x-axis member has volume `1.2 m^3` and
is therefore rejected independently of edit-plan internals.
