# References

Evidence compares Eqiora's projected step actions and implicit solves with:

- the exact one-step map `x_next = x_previous / (1 + h p)` and
  `z_next = x_next^2`;
- the exact JVP/VJP duality pairing at the accepted step; and
- centered finite differences of the independent step map and scalar
  objective.

Finite differences are a verification oracle only. They are not used by the
declared JVP/VJP implementation.
