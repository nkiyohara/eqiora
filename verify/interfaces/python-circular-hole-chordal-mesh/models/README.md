# Input geometry and request

This case has no physics Model. Its input is the exact DFG-shaped common
`eqiora.geometry.Geometry` already covered by
`interfaces.python-exact-circular-hole-geometry`, plus:

```text
maximum_boundary_error = 1e-4 m
minimum_mean_ratio = 1e-5
maximum_boundary_facets = 50
```

The exact source is first realized as its accepted 50-chord straight-edged
planar region. The classification tolerance remains `1e-12 m`; it is not an
interior mesh-size control. No new public type or request field is introduced.
