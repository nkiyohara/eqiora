# Manufactured problem

Let `T` be the 64 by 64 tridiagonal matrix with diagonal `2` and adjacent
off-diagonals `-1`. For contrast `c`, define

```text
S[i,i] = c^(i / (2 (n - 1)))
A(c)   = S T S.
```

The fixed transformed solution `y` combines a constant and two sine/cosine
modes. The exact physical solution is `x = S^-1 y`, and the right-hand side is
formed by the independently callable operator action `b = A x`. This choice
keeps the Jacobi-transformed problem invariant across the contrast sequence
without removing nearest-neighbour coupling.
