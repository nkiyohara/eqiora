# Independent references

Two non-implementing derivations independently assemble and solve the frozen
discrete witness:

- [`../routes/python/README.md`](../routes/python/README.md): closed-form affine
  MINI/P1 blocks, static condensation, and 40-decimal-digit dense LU.
- [`../routes/julia/README.md`](../routes/julia/README.md): independently
  reconstructed bases, positive numerical quadrature, uncondensed bubbles, and
  256-bit `BigFloat` dense LU.

[`../agreement/README.md`](../agreement/README.md) owns their agreement gate.
[`../amendment/README.md`](../amendment/README.md) owns the measured residual
policy amendment; it does not change any physical observation or tolerance.
