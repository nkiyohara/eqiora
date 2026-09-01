# Accepted faer sparse-LU factorization reuse

This case freezes the accepted host sparse-LU reuse behavior. It executes the ordinary
`FinalizedScalarEllipticParameterPoint -> DeploymentBinding ->
AdmittedExecution -> PreparedLinearExecution -> FaerLinearSolver::with_prepared_linear ->
AcceptedLinearExecution` path for the two-element one-dimensional Q1 fixture.
The two independently derived scientific authorities are
[`analytic.json`](expected/analytic.json) and
[`symbolic.json`](expected/symbolic.json); [`run_case.py`](run_case.py) checks
their exact agreement and regeneration before the Rust acceptance tests run.

The public Rust oracle compares independent cold occurrences with one warm occurrence for
`p0 -> p1 -> p2`, including exact `LinearSolution`, true-residual bits,
`ExecutionReceipt`, and portable Model/Realization lineage. Provider-private
evidence freezes the factor inventory `(attempted, accepted, symbolic, numeric)
= (3, 3, 1, 2)`, symbolic/numeric identity relations, and full-CSR
fingerprint's right-hand-side sensitivity.

Failure evidence covers same-graph structural mismatch, a byte-identical CSR
under a foreign Model graph, a foreign provider descriptor, and `p0 ->
singular candidate -> p1`. Every preflight failure leaves the private accepted
commit and factors unchanged. The singular
candidate consumes an attempt but cannot replace the committed p0 state; p1
then reuses p0's numeric factors and accepts.

The public API exposes no provider session, factor identity, attempt counter,
device handle, or persistence surface. Reuse authority is minted only after
Eqiora true-residual and lineage acceptance.

The required companion case
[`numerics.faer-sparse-lu-reuse-private`](../faer-sparse-lu-reuse-private/README.md)
owns the private phase ledger, acceptance-boundary failure injection, component
omission mutants, and invalid factor-state rejection. The capability is
verified only when both cases pass.

Run both authorities with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case numerics.faer-sparse-lu-reuse \
  --case numerics.faer-sparse-lu-reuse-private
```

This case makes no timing or scale claim and admits no global, directory, or
persistent cache; multi-RHS solve; cross-process/provider/release reuse;
parallel study; alternate policy; preconditioner; CUDA; MPI; distributed or
out-of-core factorization; or durable factor artifact.
