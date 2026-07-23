# RFC 0063: Host-staged MPI plus CUDA fixed-mesh fluid--structure interaction

- Status: Implemented and verified for the bounded one-host 2D slice
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0058](0058-portable-realization-and-execution-graphs.md),
  [RFC 0060](0060-distributed-spatial-ownership-and-assembly.md), [RFC
  0061](0061-mpi-fixed-mesh-fsi.md), and [RFC
  0062](0062-cuda-fixed-mesh-fsi.md)

## Summary

Eqiora composes the accepted distributed fixed-reference FSI partition with
one CUDA device per MPI rank. The composition adds neither physics nor a
second numerical lowering. Accepted owner-row assembly shards remain the sole
source of rank-local CSR/RHS storage; the CUDA adapter materializes each exact
rectangular owned-row action on its rank's selected device.

The first transport is deliberately and visibly host-staged:

```text
accepted distributed FSI assembly
  -> exact owner-row shard plus deterministic halo
  -> one selected CUDA device and queue per MPI rank
  -> host MPI halo exchange
  -> pack [owned | ghost] in the admitted local layout
  -> resident rank-local CUDA CSR action
  -> host-staged owned output
  -> host Krylov update and reproducible MPI reduction
  -> explicit-index complete candidate gather
  -> independent complete-host residual acceptance
  -> unchanged fixed-reference FSI finish
```

The complete CSR retained by the FSI finalizer is an identity and verifier. It
is never repartitioned into a second solver layout and is never solved by each
device. The composition therefore preserves both parent authorities: RFC
0061 owns partition, halo, collective protocol, and complete reconstruction;
RFC 0062 owns CUDA device, queue, sparse action, transfer, and completion.

## Motivation

The portable graph already permits a distributed algebraic system to request
`CudaDevices { devices_per_partition: 1 }`. The execution binding and the two
runtime adapters intentionally stopped short of implementing that shape.
Their current boundaries are exclusive:

- distributed execution binds a process group and applies owner-row shards on
  the host; and
- CUDA execution binds one device but accepts only a replicated complete
  square system.

Calling the replicated CUDA solver once per rank would be wrong. A distributed
CSR shard has owned rows and owned-plus-ghost columns, and its Krylov scalars
are global collectives. Conversely, adding CUDA calls inside the MPI adapter
would make the transport provider depend on one accelerator vendor.

This RFC adds the missing typed composition and one bounded execution path. It
does not invent a universal distributed-vector or plugin abstraction.

## Decision

### One distributed CUDA Realization

The accepted tuple is exactly:

```text
MinimumResidual + SymmetricIndefinite + Identity + Reproducible + f64
CudaDevices { devices_per_partition: 1 }
Distributed + Offline
```

`CudaGpu { device }` remains the compatibility-plan spelling for the selected
rank-local device. The portable graph erases that deployment-local ordinal and
retains one device per partition. The distributed owner map is selected by the
Realization requirements and remains independent of device discovery.

The tuple shares the complete `SolverPlan`, exact mesh partition, local
layouts, halo, and assembly receipt with the MPI parent. It shares the
finalized CSR/RHS fingerprint, algorithm, operator-property assertion,
preconditioner, tolerances, and iteration limit with the single-device parent.
Reduction policy cannot be identical to both parents: RFC 0062 uses `Fast`
because cuBLAS owns its complete-device Krylov reductions. This composition
keeps Krylov vectors and reductions on the host and therefore retains RFC
0061's explicit reproducible MPI reduction tree. The rank-local cuSPARSE row
action separately records `SparseActionPolicy::Deterministic`, whose guarantee
is scoped to repeated actions on the same admitted runtime. That device-local
guarantee is not Eqiora's placement-independent solver-reduction contract. No
cross-rank-count or cross-placement solution-bit identity is claimed.

No independent `MpiCudaMinresConfig` exists. The sole `SolverPlan` remains the
complete numerical control.

### Composite deployment binding

The L2 execution layer adds
`DeploymentBinding::bind_distributed_cuda`. Its inputs remain orthogonal:

- the existing `DistributedExecutorDescriptor`, which owns one logical
  process-group slot, exact partition count, one host control worker per
  partition, and the solver capability;
- a narrow `CudaPartitionPlacement`, which owns the rank-local selected
  `DeviceDescriptor`, `QueueSlot`, and sparse-action capability; and
- `DistributedDeviceTransport::HostStaged`.

There is no Cartesian-product `DistributedCudaExecutor` or new target kind.
The distributed solver provider, partition-local compute placement, and
transport are separate axes assembled once in the immutable binding.

MPI communicator handles, CUDA contexts, streams, pointers, vendor events,
and library versions remain L3 Run evidence. The binding validates the
portable graph, solver tuple, device capabilities, queue ownership, and
transport choice before either adapter allocates numerical workspace.

`HostStaged` is a type, not an inferred fallback. A future GPU-aware transport
is a new admitted variant with independent capability discovery and evidence;
an adapter may never inspect a pointer and silently choose between the two.

Every rank resolves the same Realization-local `CudaGpu { device: 0 }`. The
launcher exposes exactly one physical device to each process, so CUDA's
documented visible-device remapping makes that device ordinal zero without
changing Realization identity between ranks. The CUDA adapter also queries the
live physical device UUID. Before numerical traffic, MPI all-gathers
fixed-size rank/device records and requires:

- one record for every partition in rank order;
- the binding and runtime device identities to agree locally;
- local ordinal zero and distinct physical UUIDs for distinct ranks; and
- the same process count, CUDA runtime, transport mode, and solver tuple.

This proves one physical device per rank for the registered one-host
environment while preserving one canonical Realization. UUIDs are adapter and
Run evidence, not Semantic Model or portable Realization identity. No
cross-host physical topology meaning is inferred.

### Rank-local CUDA action

The distributed algebra layer provides a checked capture of one
`LocalCsrShard` into a rectangular local CSR matrix. Rows are the exact
ascending owned indices. Columns are remapped deterministically to:

```text
[owned indices in LocalLayout order | ghost indices in LocalLayout order]
```

This capture contains no MPI or CUDA type and changes no distributed identity.
The CUDA adapter consumes it through a run-owned resident sparse-action
session:

- row offsets, remapped column indices, and coefficients transfer once;
- one input and one output buffer remain allocated for the run;
- every action copies the already exchanged `[owned | ghost]` host vector to
  the device, invokes cuSPARSE, copies owned rows back, and waits before the
  MPI local-action status boundary; and
- allocation, transfer, submission, generation, completion, workspace, and
  library observations are retained as bounded adapter evidence.

The MPI Krylov recurrence, owned vectors, vector updates, and scalar
reductions remain on the host in this first slice. This is an honest staged
sparse-action composition, not a device-resident distributed Krylov claim.
Matrix residency still matters: the accepted owner-row operator is transferred
exactly once rather than being reconstructed or retransferred at every
iteration.

### Adapter ownership

The composition lives in the L3 `eqiora-backend-mpi-cuda` crate. It is the
only production crate allowed to depend on both isolated backend adapters.
Neither `eqiora-backend-mpi` depends on CUDA nor `eqiora-backend-cuda` depends
on MPI.

The MPI adapter exposes a narrow local-action injection seam over Eqiora-owned
distributed types. Its existing host shard action remains the default and its
existing MPI evidence must remain bit-for-bit valid. The CUDA adapter exposes
the resident rectangular CSR-action session. The composition crate performs
device selection, rank/device agreement, action injection, and composite
evidence construction.

This same-layer dependency is justified only by this executable consumer and
is enforced explicitly by `xtask check-layers`; it is not permission for
arbitrary adapter-to-adapter dependencies.

### Synchronized failure and execution evidence

The existing MPI phase-status protocol remains the failure authority. A CUDA
action returns only after its input transfer, sparse action, and output
transfer have completed or failed. The subsequent `LocalAction` agreement
turns any rank-local diagnostic into the same collective failure before any
rank enters the next reduction or gather.

The unchanged L2 distributed receipt retains the common solver, layout,
collective, output, and complete-host acceptance facts. The L3 composition
result pairs it, without flattening, with a distributed-CUDA trace containing:

- the existing distributed admission/layout/collective trace;
- the exact host-staged transport choice;
- the rank/device topology agreement identity;
- the resident local-matrix transfer identity;
- action count and monotone input/output generations;
- waited action/output completion boundaries; and
- the exact selected device and queue slot from the deployment binding.

Vendor versions, timings, workspace bytes, and concrete completion handles
remain paired adapter evidence. Every rank agrees a domain-separated common
summary over the distributed receipt, topology identity, transport, and
per-rank action count before exposing a result. Rank-local device identities
and transfer handles are intentionally distinct observations within that
agreed topology. A valid MPI receipt plus unrelated CUDA evidence, or vice
versa, cannot manufacture the composite result. No durable Run wire is added
by this slice.

The final complete candidate is independently accepted with the retained
complete host CSR and then consumed by the unchanged FSI `finish`. No local
device result is published as a distributed physical Field.

## Failure rules and falsifiers

The slice fails closed for at least:

- a distributed algebra paired with host placement, or replicated algebra
  paired with distributed CUDA placement;
- zero, multiple, duplicated-UUID, nonzero-ordinal, or binding-substituted
  rank-local devices;
- a queue that belongs to another device or a runtime descriptor that differs
  from live CUDA discovery;
- an implicit, unknown, or GPU-aware transport when `HostStaged` was admitted;
- a `Fast`, CG, BiCGSTAB, Jacobi, non-`f64`, real-time, or different
  operator-property substitution;
- any owner map, layout, halo, assembly receipt, reduced/full target, complete
  fingerprint, or rank-local shard drift;
- a local CSR column that is neither admitted owned storage nor admitted ghost
  storage, or any change to canonical owned-then-ghost ordering;
- aliased staging regions, a matrix retransferred after admission, missing
  input/output transfer, skipped generation, foreign queue, or unwaited
  completion;
- a CUDA allocation, transfer, library, action, non-finite, or completion
  failure on any rank;
- one rank proceeding to a reduction, gather, host acceptance, or receipt
  agreement after another rank's device failure;
- a composite receipt assembled from different distributed and device runs;
- disagreement with the CPU/MPI parent partition and assembly identities, the
  single-device parent complete operator, or the independent complete-host
  residual oracle; or
- final Field identity, support, order, length, normalized-value, residual,
  incompressibility, kinematic, interface-action, or energy disagreement.

No failure path may silently fall back to a host local action or publish a
partial accepted execution.

## Alternatives considered

### Run the replicated CUDA solver on every rank

Rejected. It duplicates complete storage and solves the wrong algebraic
problem. Owner-row shards are rectangular actions coupled by halo and global
reductions, not independent square systems.

### Add CUDA calls directly to the MPI backend

Rejected. Transport and device providers must remain independently optional.
The composition adapter is the only owner of their joint lifecycle.

### Require CUDA-aware MPI immediately

Rejected. CUDA-aware behavior is an implementation capability, not an MPI
semantic guarantee, and support differs by MPI transport and collective.
Open MPI documents CUDA awareness as pointer detection provided by selected
components, while even some reduction paths use staging internally. The first
slice therefore names and verifies host staging explicitly.

### Keep all Krylov vectors resident on the devices

Deferred. Device-resident distributed Krylov requires a distinct vector
ownership, device-aware collective, scalar synchronization, and failure
protocol. It is not necessary to prove that the accepted distributed shard can
execute on exactly one device per rank.

### Add a universal distributed device-vector trait

Rejected. One staged FSI consumer does not justify a trait spanning host,
CUDA, ROCm, GPU-aware MPI, NCCL, and future transports. The accepted local
action seam is the smallest abstraction with two real implementations.

## Compatibility

No Semantic Kernel, canonical Model, package, transaction, geometry, mesh,
assembly, Field, Realization wire, or Run wire changes. Existing host, MPI,
single-device CUDA, RFC 0062, and RFC 0061 paths remain exact and unchanged.

The portable Realization graph already represents distributed algebra with
one CUDA device per partition. This RFC implements its previously rejected
execution branch. New execution types remain workspace-public until a curated
Run API has an independent consumer. The `mpi-cuda` facade feature is optional;
default, MPI-only, CUDA-only, and MSRV builds do not link the composition.

## Verification

1. Unit-test the exact distributed-CUDA graph and binding tuple; reject every
   adjacent layout, placement, count, transport, device, queue, solver, and
   capability substitution before allocation.
2. Unit-test deterministic local shard capture and CPU equivalence for one,
   two, and four partitions, including nonempty ghost columns.
3. Unit-test the resident CUDA action lifecycle and prove the matrix transfers
   once while input/output generations and completions advance per action.
4. Inject a device failure and prove every rank returns the same collective
   diagnostic before the next collective phase.
5. Finalize the fixed-reference FSI operator through accepted owner-row
   assembly, require the same complete fingerprint as both parent cases, and
   run one selected device per rank at one, two, and four ranks on one host.
6. Require one visible ordinal-zero device and a distinct live physical UUID
   per rank for the multi-rank cases, then record the exact rank-to-device map,
   host-staged transport, MPI/CUDA/library versions,
   partition/layout/admission identities, transfers, completions, and receipt
   summary.
7. Gather by explicit global indices, independently accept the complete host
   residual on every rank, invoke the unchanged FSI finish, and compare exact
   Field identities/supports/order/length plus normalized values with both
   parent oracles under the existing documented tolerance.
8. Register a bounded portable replay plus the physical four-GPU observation,
   then update the capability matrix without inheriting multi-node,
   performance, GPU-aware MPI, or device-resident Krylov claims.

## Prior art and external boundary

- [MPI 5.0](https://www.mpi-forum.org/docs/) defines the communication
  contract; accelerator-pointer acceptance remains an implementation
  capability.
- [Open MPI CUDA-aware
  support](https://docs.open-mpi.org/en/v5.0.7/tuning-apps/networking/cuda.html)
  documents component-specific device-pointer detection and transport.
- [Open MPI collective release
  notes](https://docs.open-mpi.org/en/v5.0.8/release-notes/mpi-collectives.html)
  explicitly describe a CUDA reduction component that stages device buffers.
- [CUDA device enumeration](https://docs.nvidia.com/cuda/cuda-programming-guide/05-appendices/environment-variables.html)
  defines how `CUDA_VISIBLE_DEVICES` changes both visibility and ordinals. The
  registered one-host evidence therefore gives each rank exactly one physical
  selector, requires local ordinal zero, and separately records live UUIDs.

These sources inform deployment evidence only. Their vendor/runtime behavior
does not become Semantic Model meaning.

## Nonclaims

This RFC does not claim GPU-aware MPI, GPUDirect, NCCL, unified or pinned host
memory, asynchronous halo/action overlap, device-resident Krylov vectors or
reductions, placement-independent reproducible device sparse actions,
multiple devices per rank,
device sharing, unrestricted visible-device namespaces, multiple physical
nodes, topology-aware placement, load balancing, performance, scale, GPU
assembly, matrix-free FSI, distributed physical Fields, checkpoint/restart,
failure recovery, transient multi-step FSI, ALE, remeshing, nonlinear
structure, Navier--Stokes FSI, contact, sensitivity, shape optimization, or
adjoints.

In particular, “MPI plus CUDA FSI” means one fixed-reference monolithic 2D
step whose accepted owner-row sparse actions run on four explicitly distinct
UUID-identified devices at up to four MPI ranks on one host. It is not a
general heterogeneous distributed solver claim.

## Unresolved questions

None for the bounded host-staged slice. GPU-aware transport, durable composite
receipts, cross-host physical topology identity, device-resident distributed Krylov, and
multi-node topology become decisions only when their own evidence consumers
exist.
