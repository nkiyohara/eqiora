# Canonical problem

For height `h` and vertical velocity `v`, flight is the implicit relation

```text
h' - v = 0
v' + g = 0
```

with `h(0) = 1 m`, `v(0) = 0 m/s`, and `g = 9.81 m/s^2`. A falling crossing
of guard `h = 0` activates two simultaneous residual Relations:

```text
next(h) = 0
next(v) + e pre(v) = 0,  e = 0.8
```

Splitting the reset deliberately proves that coincidence and atomic commit are
activation semantics rather than an accident of one multi-root Relation.
