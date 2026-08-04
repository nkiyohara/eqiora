# Reference provenance

The authoritative mathematical contract is the dual-oracle provenance retained
by this [registered evidence case](../README.md). Route A derived the values
from the analytic small-strain weak form with exact bilinear integration. Route
B independently used the potential-energy Hessian and an 80-decimal-digit
scatter. The routes were compared only after both reports were final and agreed
exactly.

The continuous and Q1 realization meanings are fixed by RFC 0039 and the
accepted `solid.isotropic-elasticity-2d` case. In particular, the affine
constant-strain patch and the loaded homogeneous-boundary reaction patch are
separate checks; the frozen affine state is not claimed to solve the loaded
problem.

No implementation output, generated golden table, external dataset, or
finite-difference result supplies an expected scientific value.
