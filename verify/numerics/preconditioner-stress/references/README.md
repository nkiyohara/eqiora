# Reference provenance

For positive diagonal `S`, `S T S` is SPD because
`v^T S T S v = (S v)^T T (S v) > 0` for every nonzero `v`.
Its Jacobi diagonal is `2 S^2`; symmetric Jacobi scaling therefore reduces
the operator to `T / 2`, independent of the prescribed contrast.

Eqiora's fixed-order reference CG is the deterministic oracle. faer 0.24.4 is
the independent production implementation behind the isolated adapter:

- <https://docs.rs/faer/0.24.4/faer/matrix_free/conjugate_gradient/>

Eqiora accepts both results only after a separate operator application and
true-residual norm calculation.
