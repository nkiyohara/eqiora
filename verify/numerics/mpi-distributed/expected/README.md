# Acceptance

For one, two, and four ranks, every local owned-row result must exactly equal
the corresponding global CSR oracle value. Rank-ordered reproducible gather
and native fast all-reduce must both equal the analytic dot product.
Jacobi-preconditioned distributed CG under both policies must recover the
manufactured owned solution, and a fresh global residual must satisfy the sole
`SolverPlan` target. Rank count, thread support, local shapes, halo ownership,
and finite values fail closed before producing accepted output.
