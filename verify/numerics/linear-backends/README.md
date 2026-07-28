# Linear backend verification

This case checks that one Eqiora-owned `SolverPlan` and `LinearOperator`
contract can drive the independent reference and faer adapters without moving
backend types into canonical model or spatial APIs.

The executable evidence includes:

- reference CG and faer CG agreement on a symmetric positive-definite system;
- faer BiCGSTAB on a nonsymmetric manufactured system;
- identity and Jacobi adapter actions;
- one exact faer 0.24.4 provider release bound before execution and retained
  consistently by the producer report and accepted execution receipt;
- independent `||b - A x||_2` acceptance through the Eqiora operator;
- explicit rejection of a reproducible-reduction request that faer does not
  currently guarantee; and
- the same canonical Poisson model revision solved by reference and faer.

The claim is host-local, `f64`, replicated, and single-worker. It is not a
threaded, distributed, accelerator, or performance claim.

Run:

```bash
cargo test -p eqiora-backend-faer
cargo test -p eqiora --test faer_spatial
cargo run -p eqiora-verify -- run --case numerics.linear-backends
```

See [RFC 0010](../../../rfcs/0010-execution-backend-contracts.md) and the
[backend strategy](../../../docs/development/library-and-accelerator-strategy.md)
for ownership and graduation rules.

## Pre-committed evidence for an unimplemented capability

`expected/sparse-lu-contract.json` and `oracle/sparse_lu_oracle.py` freeze the
exact-rational oracle for the proposed `SparseLu` direct algorithm of
[Issue #126](https://github.com/nkiyohara/eqiora/issues/126). They were written
by an agent that does not implement the slice, before any implementation
existed, and later amended by that same agent to bind the acceptance threshold
the implementer is not permitted to author. Neither the original nor the
amendment read, ran, or saw any implementation.

They are **not** part of the claim above. No `SparseLu` capability is claimed,
executed, or verified by this case today: the oracle stands alone, the case
manifest is unchanged, and nothing in the list above depends on it. The
implementing agent wires the fixture and updates the manifest and the capability
matrix in the pull request that adds the implementation.

```bash
python3 verify/numerics/linear-backends/oracle/sparse_lu_oracle.py --summary
```

See [the oracle reference](references/sparse-lu-oracle.md) for the witnesses,
the falsifiers, the acceptance threshold and the plan it binds, the fixture
digest, and the nonclaims.
