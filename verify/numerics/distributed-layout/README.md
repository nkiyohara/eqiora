# Distributed layout verification

This case validates the backend-neutral contracts required before an MPI
adapter is admitted. A square global CSR operator is lowered into unique
owners, sorted local owned/ghost indices, owned-row shards, and an ordered halo
plan derived from off-owner column dependencies.

The loopback protocol oracle splits the input into owner-local storage,
executes only the declared owner-to-receiver halo exchanges, applies each
owned row from local plus ghost values, and gathers the uniquely owned output.
One-, two-, and four-partition results must exactly match the global CSR
action. A collective dot product separately reduces unique-owner local
contributions in partition order. Because loopback has no native collective,
the fast reduction policy fails closed.

This verifies layout, halo, and reproducible collective semantics in one
process. It is not an MPI, multi-process, multi-node, distributed-CG,
performance, or process-failure claim.

Run:

```bash
cargo test -p eqiora-distributed --test distributed_loopback
cargo run -p eqiora-verify -- run --case numerics.distributed-layout
```
