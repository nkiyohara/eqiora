# Reference evidence

Before the first impact, the exact continuous solution is

```text
h(t) = h0 + v0 t - g t^2 / 2
v(t) = v0 - g t
t_impact = sqrt(2 h0 / g)
```

The current executable oracle uses backward Euler, so exact mechanics are a
refinement reference rather than an equality claim at finite `max_step`.
Executable evidence checks the directed crossing, bracketed event time,
atomic restitution reset, insertion-order independence, and bounded chatter.
