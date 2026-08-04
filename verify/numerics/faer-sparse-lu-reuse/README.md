# Accepted faer sparse-LU factorization reuse

This case freezes the public half of Issue #256. It executes the ordinary
`FinalizedScalarEllipticParameterPoint -> DeploymentBinding ->
AdmittedExecution -> FaerSparseLuReuseOwner::execute ->
AcceptedLinearExecution` path for the two-element one-dimensional Q1 fixture.
The two independently derived scientific authorities are
[`analytic.json`](expected/analytic.json) and
[`symbolic.json`](expected/symbolic.json); [`run_case.py`](run_case.py) checks
their exact agreement and regeneration before the Rust acceptance tests run.

The public Rust oracle compares independent cold owners with one warm owner for
`p0 -> p1 -> p2`, including exact `LinearSolution`, true-residual bits,
`ExecutionReceipt`, and portable Model/Realization lineage. It freezes the
counter inventory `(attempted, accepted, symbolic, numeric) = (3, 3, 1, 2)`,
the symbolic/numeric identity relations, the existing full-CSR fingerprint's
right-hand-side sensitivity, and the `2..=64` attempt bound.

Failure evidence covers same-graph structural mismatch, a byte-identical CSR
under a foreign Model graph, a foreign provider descriptor, capacity
exhaustion, and `p0 -> singular candidate -> p1`. Every preflight failure must
leave public counters and committed identities unchanged. The singular
candidate consumes an attempt but cannot replace the committed p0 state; p1
then reuses p0's numeric factors and accepts.

All six permutations of the three compatible points must retain each point's
cold solution, residual, receipt, and lineage. The test intentionally does not
compare factorization counts across those permutations: no phase-count
order-independence claim is made.

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

At the preimplementation base the production owner, module wiring, export, and
dependencies do not exist, so the Rust evidence is intentionally red. The
Python scientific-agreement and state-manifest checker is independently
executable and must already pass.

This case makes no timing or scale claim and admits no global, directory, or
persistent cache; multi-RHS solve; cross-process/provider/release reuse;
parallel study; alternate policy; preconditioner; CUDA; MPI; distributed or
out-of-core factorization; or durable factor artifact.
