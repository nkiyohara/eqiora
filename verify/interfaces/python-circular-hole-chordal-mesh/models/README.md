# Input model

This adapter case has no physics Model. Its input is the exact DFG-shaped
`eqiora.geometry.RectangleWithCircularHole` already covered by
`interfaces.python-exact-circular-hole-geometry`, plus explicit chordal
Realization policy:

```text
max_boundary_error = 1e-4 m
required_minimum_mean_ratio = 1e-5
max_segments = 50
```

The exact geometry classification tolerance remains `1e-12 m` and does not
become mesh approximation policy.
