# Reference provenance

The independent reference here is **analytic asymptotics**, not a second
implementation. The declared envelope is anchored to a classical result rather
than to a measured baseline, which is what lets it be written down before the
run.

For a second-order elliptic operator discretized on a quasi-uniform mesh of
size `h`, the assembled system has spectral condition number

```text
kappa(A) = O(h^-2).
```

Conjugate gradients applied to an SPD system converges with the standard bound

```text
||e_k||_A / ||e_0||_A <= 2 ((sqrt(kappa) - 1) / (sqrt(kappa) + 1))^k,
```

so reaching a fixed relative reduction costs `O(sqrt(kappa)) = O(h^-1)`
iterations. Halving `h` therefore roughly doubles the iteration count: slope
`1` in `log2(it)` against `log2(n)`, ratio `2` per halving.

Diagonal (Jacobi) preconditioning rescales rows and columns. On a uniform
Cartesian mesh with a constant coefficient it is a single scalar multiple of the
identity on the free-vertex block, which leaves the Krylov space and hence the
iteration count unchanged; on a graded or variable-coefficient problem it
improves the constant but leaves `kappa = O(h^-2)` intact. It is not a scalable
preconditioner for this operator class.

A scalable method, by contrast, is defined by mesh-independent iteration
counts: slope `0`, ratio `1`. The declared adequacy thresholds `s <= 0.5` and
`rho <= 1.4` sit strictly between those two regimes, and the declared breach
thresholds `s >= 0.85` and `rho >= 1.8` sit close to — but deliberately below —
the theoretical `1.0` and `2.0`, so that ordinary pre-asymptotic scatter cannot
manufacture a breach and cannot mask one either.

The measurement's own acceptance is not taken from the backend. Every recorded
iteration count belongs to a solve whose independently recomputed true residual
was accepted against the requested target by the existing
`SolveReport` contract, so a count is never the count of a solve that merely
*reported* convergence.

Standard references for both statements:

- Y. Saad, *Iterative Methods for Sparse Linear Systems*, 2nd ed., SIAM 2003,
  ch. 6 (Krylov convergence) and ch. 10 (why diagonal scaling is not scalable).
- U. Trottenberg, C. Oosterlee, A. Schuller, *Multigrid*, Academic Press 2001,
  ch. 1-2 (mesh-independent convergence as the defining property).
