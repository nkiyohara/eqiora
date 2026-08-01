# Independent analytic route

For rectangle width `w = 0.08 m`, height `h = 0.05 m`, extrusion depth
`d = 0.02 m`, and cut radius `r = 0.008 m`, inclusion/exclusion gives

```text
V        = whd - pi r^2 d       = 8e-5 - 1.28e-6 pi m^3
A_cap    = wh - pi r^2          = 4e-3 - 6.4e-5 pi m^2
A_x      = hd                   = 1e-3 m^2
A_y      = wd                   = 1.6e-3 m^2
A_wall   = 2 pi r d             = 3.2e-4 pi m^2
A_total  = 2 A_cap + 2 A_x + 2 A_y + A_wall
         = 0.0132 + 1.92e-4 pi m^2
```

The exact cross-section is connected with one hole, so extrusion yields one
body, one shell, genus one, and seven faces. Four predecessor laterals remain
unchanged, the two caps retain provenance and gain one circular boundary, and
the cylinder wall is created. No predecessor face is deleted, split, or
merged.

The signed admission predicate is

```text
min(cx-x0, x1-cx, cy-y0, y1-cy) - r > requested_boolean_tolerance.
```

For the witness it equals `0.012 m`; an absolute side-line distance would
incorrectly admit an outside centre. The binary64 relative bound `4e-15` covers
the direct formulas and a canonical seven-positive-term sum while remaining
far below the smallest precommitted mutant separation.
