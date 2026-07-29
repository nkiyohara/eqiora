# Reference minimum-residual W64

This case verifies one deliberately ill-conditioned symmetric-indefinite
binary64 system against an exact dyadic oracle. It exists to distinguish full
two-pass Krylov reorthogonalization with the retained full Hessenberg projection
from the short Lanczos recurrence used by the earlier reference implementation.

The witness was frozen before implementation by two isolated Opus 5 Max
derivations. One used exact integer/dyadic Sylvester-Hadamard analysis and
rounding sandwiches; the other used independently generated high-precision and
exact-rational matrices, inertia checks and eigensolves. The implementer did not
choose or tune the constants.

## Construction

For `n = 64`, let

```text
H[i,j] = (-1)^popcount(i & j)
Q = H / 8
m_k = max(round(2^(32 k / 63)), m_(k-1) + 1)
lambda_k = (-1)^(k+1) m_k / 65536
A = Q diag(lambda) Q^T
b = Q lambda
x* = Q 1 = 8 e_0
```

The comma-separated `m_k` sequence has SHA-256
`8e505379c493dd40e2a82d901e3f79900d8b8e8afb4a05bbfc6ce151dae1114c`.
The matrix is exactly representable, symmetric and indefinite, with inertia
`(32+, 32-, 0)`, `||A||_2 = 65536`, `min |lambda| = 2^-16`, and condition
number `2^32`. Its exact Krylov grade is 64.

The selected plan has target `9.217895952054019e-8`. Exact arithmetic gives
`A x* - b = 0`; the residual-only forward bound is `0.0060411`, so the solution
assertions use the independently precommitted absolute tolerance `0.01`.

## Decisive checks

- The accepted solve closes at exactly grade 64, satisfies the independently
  reapplied target, preserves the complete plan/provider identity, and matches
  `8 e_0`.
- An absolute target of `1e-20` fails at Krylov-space closure even though the
  projected residual is zero.
- The grade-two diagonal witness accepts at exactly two iterations.
- The W64 witness with a 32-iteration plan fails at that plan limit.
- A recording execution observes at least
  `iterations * (iterations + 1)` inner products, which rejects deleting the
  second complete Gram-Schmidt pass.

The case does not claim performance, scale, preconditioning, restart,
refinement, direct factorization, arbitrary conditioning, singular-system
MINRES-QLP, MPI, or accelerator behavior. The implementation retains
`O(dimension * min(maximum_iterations, dimension))` data.

Run it with:

```bash
cargo run -p eqiora-verify -- run --case numerics.reference-minres-w64
```
