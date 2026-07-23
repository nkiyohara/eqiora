# Fixed-reference FSI distributed assembly over MPI in 2D

This case carries the transport-neutral owner-routing protocol verified by
[`fsi.fixed-reference-distributed-assembly-loopback-2d`](../fixed-reference-distributed-assembly-loopback-2d/README.md)
through physical MPI processes. It reuses the exact canonical Model, Geometry
Identity, geometry--mesh correspondence, eight-cell affine-triangle mesh,
fluid/solid partition, mixed Realization, previous state, and two-target FSI
assembly work from the serial fixed-reference case.

Every rank authenticates the mesh artifact digest against the resolved
Realization before deriving its distributed mesh layout. The one-rank case
owns every cell on rank zero. The two-rank case places all solid cells on rank
zero and all fluid cells on rank one, while the four-rank case assigns cells
by index modulo four. Thus the physical interface crosses process ownership,
and the pressure-row owner cannot be inferred from generic vertex ownership.

Only the owning rank evaluates each canonical cell packet. The MPI adapter
admits a common fixed-size layout/plan identity, transports checked row-route
payloads with bounded allocations, folds each owner inbox in target and
global-packet order, gathers checked owner shards, reconstructs both complete
targets, and agrees one payload-bound receipt before exposing the result.

At one, two, and four physical ranks on one host, both reconstructed target
systems must match a separately executed complete CPU reference assembly in
every CSR index and every matrix/RHS `f64` bit. The reduced canonical
fingerprint must match as well. The reconstructed reduced system is then
solved by the existing serial-host reference MINRES and must pass the unchanged
FSI residual, incompressibility, kinematic, interface-action, and energy
acceptance path. That final solve checks composition after reconstruction; it
is not a distributed solver claim.

For two and four ranks, one rank also substitutes a foreign mesh-revision
identity while retaining a locally coherent layout. Every rank must return the
same diagnostic from the fixed-size collective admission boundary. A following
collective proves that no rank advanced into a variable-size transport phase;
the parent timeout additionally bounds liveness.

Run on a host with Open MPI or MPICH and the mpi-rs build dependencies:

```bash
cargo test --locked -p eqiora \
  --features mpi \
  --test fixed_reference_fsi_distributed_assembly_mpi_2d \
  -- --test-threads=1
cargo run --locked -p eqiora-verify -- run \
  --case fsi.fixed-reference-distributed-assembly-mpi-2d
```

This evidence is physical distributed assembly on one host. It does not claim
distributed MINRES or end-to-end distributed FSI solve, a distributed result
Field, multiple physical nodes, scaling, parallel I/O, hybrid rank/thread
execution, MPI plus CUDA, process-failure recovery, ALE, remeshing, or a
durable distributed-assembly artifact.
