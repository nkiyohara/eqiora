# Analytic reference

For initial height one, zero velocity, gravity `g`, and restitution `e`, the
first impact is `t = sqrt(2/g)` with `v^- = -g t` and `v^+ = -e v^-`. For a
post-impact interval `dt`, flight is
`h = v^+ dt - g dt^2 / 2`, `v = v^+ - g dt`. The saltation entries use the
same first-impact formulas recorded by `differentiation.hybrid-event`.
