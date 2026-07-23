# Fixed-reference FSI distributed assembly loopback in 2D

This case closes the transport-neutral part of distributed spatial assembly
before MPI is allowed to implement it. It reuses the exact Model, Geometry
Identity, geometry--mesh correspondence, eight-cell affine-triangle mesh,
fluid/solid partition, mixed Realization, and two-target block system from
[`fsi.fixed-reference-monolithic-step-2d`](../fixed-reference-monolithic-step-2d/README.md).

The mesh artifact digest is checked against the resolved Realization before
its fixed 32-byte value enters the L2 layout. Cell ownership is the only
spatial partition input. Vertex and facet residency, ghost entities, process
boundaries, and owner-to-receiver entity exchanges are derived from mesh
incidence. Process boundaries remain distinct from the physical FSI
interface, although the two physical interface facets are deliberately split
in the two- and four-partition fixtures.

The same authenticated digest also binds the ordered packet set carried by
the checked `AssemblyWork`. A foreign layout with the same topology and cell
count but a different mesh revision is rejected before packet evaluation.

The same checked `AssemblyWork` used by reference FSI assembly emits one packet
per canonical cell and maps every packet to both the reduced solve target and
the full reaction/interface-evidence target. The cell owner evaluates that
packet exactly once. `eqiora-assembly` performs constraint elimination and
the packet-local floating-point fold once, producing canonical row deltas.
The spatial protocol derives each row owner from the actual equation support,
routes those deltas, validates a complete unordered inbox, and folds them by
target and ascending global packet index. Every producer retains an opaque
admission proving that its full projected route inventory is sealed and that
the collective row-owner result is exactly the minimum of actual equation
support; all producer admissions are required before reconstruction can issue
a receipt.

At one, two, and four logical partitions, complete systems reconstructed only
from accepted owner-row shards must match independent serial reference
assembly in every CSR index and every matrix/RHS `f64` bit. The reduced
canonical fingerprint must also match. The reconstructed operator is then
solved by the existing serial-host reference MINRES and passes the unchanged
FSI residual, incompressibility, kinematic, interface-action, and energy
acceptance path. That final solve checks composability; it is not a distributed
solver claim.

Run:

```bash
cargo test --locked -p eqiora \
  --test fixed_reference_fsi_distributed_assembly_loopback_2d \
  -- --test-threads=1
cargo run --locked -p eqiora-verify -- run \
  --case fsi.fixed-reference-distributed-assembly-loopback-2d
```

This case does not claim MPI transport, distributed MINRES or FSI execution,
multiple physical nodes, scaling, a distributed result Field, parallel I/O,
GPU execution, ALE, remeshing, or a durable distributed-assembly artifact.
