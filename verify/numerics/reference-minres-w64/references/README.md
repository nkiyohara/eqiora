# Oracle provenance

Two isolated non-implementing Opus 5 Max sessions derived this witness before
the solver implementation began. Route A used exact integer/dyadic
Sylvester-Hadamard analysis and integer rounding sandwiches. Route B used
independent 140/300-digit generation, exact rational matrices, Bareiss/Jacobi
inertia, LAPACK and high-precision eigensolves.

Both routes agreed on the complete conditioning-integer sequence and its hash,
the exact matrix/vector samples, inertia, spectral extrema, condition number,
right-hand-side norm, residual target, exact solution, Krylov grade and the
residual-only forward bound. The implementer only wired those frozen values
into the integration test.

No production implementation, candidate residual, or existing fixture was an
input to either derivation.
