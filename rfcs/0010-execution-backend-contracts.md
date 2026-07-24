# RFC 0010: Execution backend contracts

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Eqiora separates mathematical operator action, numerical policy, data layout,
and execution placement so that serial CPU, threaded CPU, distributed CPU, and
device backends can realize the same validated model without acquiring one
another's semantics.

## Motivation

The current reference sparse path owns a CSR matrix and an unpreconditioned
conjugate-gradient routine. It is small and auditable, but its API couples the
algorithm directly to `LinearSystem` and allocates every matrix-vector result.
That shape cannot admit a matrix-free operator or a production library without
method-specific entry points.

`Target::HostCpu { threads }` currently records a requested worker bound, but
the numerical path does not consume it. A target value is not evidence that an
executor exists. Accepting a multi-thread or CUDA plan because its enum variant
exists would turn capability negotiation into a support claim that cannot be
reproduced.

Distributed execution adds a different concern. A rank owns only part of a
global vector, may cache ghost values, exchanges a halo before local operator
action, and performs collective scalar reductions during a Krylov solve. A
host-local `&[f64]` is not a distributed vector, and an MPI communicator is not
mathematical model meaning. Hiding both behind an ambiguous `size()` method
would make the simple case superficially uniform and the global invariants
implicit.

## Proposed design

### Orthogonal contracts

The execution boundary has four independent concepts:

```text
Operator       maps one mathematical vector space to another
Numerics       method, tolerances, preconditioner, convergence evidence
Layout         global indices, owned indices, ghost indices, residency
Execution      workers, ranks, queues, communication, reductions
```

The Semantic Model owns none of the latter three. The Realization Graph
selects them and run provenance records what was resolved. Backend adapters
consume validated Operator/Execution IR and must not reinterpret equations,
units, boundary meaning, or model-time activation.

### Contract and crate ownership

The backend-neutral seam is a dedicated L2 crate rather than another module in
the spatial implementation crate:

```text
eqiora-solver          SolverPlan, operator/backend traits, capability,
                       report, and the deliberately small reference oracle
eqiora-realization     semantic revision + discretization + solver + target
eqiora-distributed     global/local layout, halo, collective, loopback oracle
eqiora-meshing         mesh topology, geometry maps, quadrature, and quality
eqiora-assembly        local contributions, maps, packets, and sparse algebra
eqiora-numerics        local spatial operators, realization, and lowering
eqiora-backend-faer    production host linear-algebra adapter
eqiora-fabric          run-owned threaded host placement
eqiora-backend-mpi     MPI transport adapter
eqiora-backend-cuda    device execution adapter
```

`SolverPlan` has exactly one definition in `eqiora-solver` and is re-exported
where migration ergonomics require it. There is no method-specific
`ConjugateGradientConfig`. Numerical realization functions receive the same
plan that capability negotiation validated.

`eqiora-realization`, `eqiora-distributed`, and `eqiora-solver` are L2 because
they express policy and lowered algebra rather than a concrete transport. Two
directed same-layer composition edges are authorized: Realization consumes the
sole solver plan, while distributed algebra consumes only the shared scalar
and reduction-policy vocabulary. Neither edge permits solver to depend back on
those crates. Production adapters remain L3 and depend downward on the
contracts; they never enter `eqiora-numerics`.

### Host-local linear algebra v0

The first executable contract is deliberately exact about its admitted value
space: finite `f64` vectors wholly available to one process.

```text
LinearOperator
    rows()
    columns()
    apply(input, output)
    optional row_action().apply_rows(range, input, output_slice)

Preconditioner
    dimension()
    apply(residual, correction)

LinearSolverBackend
    capabilities()
    solve(problem, plan) -> LinearSolution

LinearSolution
    values
    SolveReport

SolveReport
    backend identity
    execution adapter identity + worker count
    convergence reason
    completed iterations
    initial residual norm
    reported residual norm
    independently recomputed true residual norm
```

The caller owns input, output, and solution buffers. `apply` does not allocate.
The current CSR matrix implements `LinearOperator`; a matrix-free realization
may implement it without pretending to be CSR. The reference CG routine is
rewritten against the trait and remains the independent conformance oracle.

This host-local trait is not relabelled as a distributed-vector abstraction.
Its Rust documentation states the admitted value space. A later distributed
adapter may reuse a local operator kernel, but owns a distinct layout and
collective contract.

### Mathematical properties and admission

An iterative method is admitted only when the problem contract declares the
properties it requires:

- CG requires a square symmetric positive-definite operator;
- BiCGSTAB requires a square operator and makes no symmetry claim;
- a preconditioner must have the solution-space dimension;
- all input values, controls, applications, and residuals must remain finite.

Properties are assertions of the selected realization, not facts inferred by
sampling a few matrix entries. Verification may falsify an assertion, but a
backend must not silently change algorithms when it is absent or unsupported.

The initial production policies are identity and Jacobi preconditioning.
Names for ILU, incomplete Cholesky, AMG, or domain decomposition are added only
with an executable adapter and falsifying evidence.

### Convergence evidence

A backend library's recursive residual estimate is useful operational data but
is not Eqiora's verification authority. After a backend reports convergence,
Eqiora applies the accepted operator to the returned solution and computes
`||b - A x||_2` independently. The solution is accepted only when that true
residual meets the declared absolute/relative threshold.

Breakdown, iteration exhaustion, non-finite arithmetic, unsupported policy,
and invalid problem properties are distinct typed convergence or diagnostic
outcomes. There is no automatic fallback to another method or backend.

### Threaded CPU execution

Worker count is placement capacity, while reduction ordering is numerical
policy. They remain distinct:

```text
Host placement     maximum workers
Reduction policy   reproducible | fast
```

The reproducible path partitions rows into stable logical chunks and combines
partial reductions in a fixed order independent of the available worker
count. Parallel row actions are valid when each output row has a unique writer.
The fast path may use a backend-native reduction tree and must record that
choice in run provenance.

Rayon is the first host task adapter. Eqiora creates an owned thread pool for a
resolved execution rather than mutating the process-global pool. A one-worker
plan and an N-worker plan must be independently testable in the same process.
Backend code may pass the resolved parallelism into faer, but faer or Rayon
types do not appear in model, realization-wire, or stable solver-report data.

The implemented first slice exposes row partitionability as an optional
operator capability. CSR admits it; an operator without it fails closed under
a multi-worker request. A solver decorator preserves the underlying solver
backend identity and reduction policy while recording Rayon placement and its
exact worker count separately. The canonical Poisson/P1 FEM reference-CG case
is bit-identical for one and four workers. This evidence covers replicated CSR
row actions only: parallel assembly, parallel vector reductions, NUMA, and
distributed memory are not inferred from it.

### Distributed execution

Distributed execution composes rather than widens the host slice contract:

```text
GlobalVectorSpace   scalar type + global dimension
Partition           stable owner for each global index
LocalLayout         owned indices + ordered ghost indices
HaloPlan            neighbor exchange derived from operator sparsity
Collective          scalar/vector reductions over one execution group
DistributedOperator halo update + local action on owned output
```

Each global degree of freedom has exactly one owner. Ghost entries are cached
read-only values identified by global index. Assembly routes contributions to
the owner and has a deterministic accumulation mode. A halo plan is derived
from a partitioned operator artifact and cannot change equation meaning.

MPI is the first multi-node transport adapter. MPI communicator, request,
datatype, rank, and status types remain private to that crate. A communicator
is never initialized implicitly by a library call; the application owns its
lifetime and passes an execution group to Eqiora. Process failure behavior and
thread-support level are validated before execution.

The first implemented evidence is transport-free: a global CSR artifact is
lowered into owned-row shards, ordered local layouts, and a halo plan derived
from off-owner column dependencies. A loopback oracle performs those exact
exchanges, matches direct global action for one, two, and four partitions, and
reduces a dot product from unique-owner contributions in partition order. The
loopback adapter admits only the reproducible policy; fast reduction fails
closed until a native collective exists. This validates the logical contract
without implying MPI, multi-process, or multi-node support.

The first transport slice is also implemented behind the optional
`eqiora-backend-mpi` adapter. The application initializes and finalizes MPI;
the adapter validates the provided thread-support level, duplicates the
communicator for the execution group, and maps its rank to the immutable
partition artifact. One-, two-, and four-rank tests on one CI host execute the
same halo plan and owned CSR action as the loopback oracle. Reproducible dot
products gather one partial per rank and fold in rank order; fast mode uses
native `MPI_Allreduce`. Jacobi-preconditioned distributed CG consumes the sole
`SolverPlan` and admits a solution only after recomputing the global true
residual. Rank-local values share one globally identical `SolveReport`, whose
typed topology distinguishes distributed ranks from host worker threads. MPI
handles remain private to L3. The same canonical case passes with one rank on
each of two distinct physical nodes, while the topology gate rejects two ranks
placed on one physical processor before numerical work. This graduates
functional two-node distributed-CG execution only: scalability, distributed
assembly, checkpoint/restart, scheduler integration, and process-failure
recovery remain unclaimed.

Reproducible distributed reductions use a specified rank order or a documented
reproducible accumulator. Fast reductions may use native collectives. The two
modes are different numerical policies because Krylov convergence and hybrid
event decisions can change with summation order.

A loopback or multi-process test on one host validates protocol logic, but a
multi-node support claim additionally requires CI or recorded verification on
at least two physical nodes. Compile-only coverage is insufficient. The
hardware-gated test enforces that distinction by gathering bounded exact
`MPI_Get_processor_name` values and rejecting fewer physical processors than
the explicitly requested minimum. Processor names remain verification inputs,
not portable model or run-artifact identity. The implemented two-node case
satisfies this gate with an identical executable and MPI runtime on both
nodes.

### Device execution

Device execution uses the same ownership separation:

```text
DeviceRuntime + DeviceBuffer + CommandQueue + Completion/Fence + TransferPlan
```

Completion identities are not Semantic Model events. Device allocation,
streams, kernels, and vendor handles live in adapters. A `CudaGpu` target is
unsupported until capability negotiation reaches an executable and verified
backend.

### Artifact and provenance boundary

Canonical Semantic Model wire data does not absorb execution policy. A
versioned Realization envelope must preserve its own revision, the referenced
semantic model/revision, the exact solver plan, problem requirements, target,
layout/partition artifact identities, and default-policy origin. Backend and
run provenance separately record resolved adapter/library versions, execution
topology, reduction policy, and device or MPI environment.
For deployment-bound linear paths, declared solver/execution provider releases
are typed L2 values carried by both binding and actual producer report; only
an exactly matched accepted receipt may be projected into Run provenance.

The opaque realization field in run manifest v1 is not widened in place. RFC
0013 adds `RealizationEnvelopeV1` and `RunManifestV2` with explicit DTOs,
decode limits, and cross-artifact target/layout/reduction validation. Typed
MPI/CUDA provenance remains distinct from executable backend support.

## Alternatives considered

### One universal vector trait from serial slices to MPI and GPU

This appears uniform, but either exposes backend-associated vector types or
reduces ownership, communication, and residency to hidden side effects. The
result makes global and local dimensions ambiguous. Rejected for v0. Shared
mathematical vocabulary is retained while execution data has explicit
backend-level contracts.

### Put worker, rank, and device facts on `LinearOperator`

This makes placement part of operator identity and duplicates the same
mathematics for every target. It also prevents comparing backends over the
same operator artifact. Rejected.

### Put Rayon, MPI, or faer types in Realization payloads

This reduces adapter code but ties stable policy and wire compatibility to
third-party APIs. Rejected. Adapters translate Eqiora-owned values at the
execution boundary.

### Use only backend-native residuals and reductions

This is faster and less code, but cannot independently detect adapter mistakes
or distinguish recursive residual drift from true convergence. Rejected for
verification. Performance runs may omit residual history, not the final true
residual acceptance check.

### Implement all production solvers and transports in Eqiora

This provides complete control but duplicates mature infrastructure and
weakens focus on semantic and realization contracts. Rejected. The reference
path remains intentionally small; production work is delegated through
isolated adapters.

## Compatibility and migration

`ConjugateGradientConfig` and the method-specific
`solve_conjugate_gradient(&LinearSystem, config)` entry point are removed while
the API is provisional. Callers pass the validated `SolverPlan` through the
common backend contract. Existing Poisson values, convergence orders, global
balance, and deterministic reference results must remain unchanged.

`SolverPlan` is a provisional v0 Rust API. Adding backend, preconditioner, and
reduction selection requires a new default-policy version if serialized or if
the project default changes. Default policy v0 continues to mean one-worker
reference CG until explicitly superseded.

No Semantic Kernel node or artifact wire version changes in this RFC. Future
Realization and run-provenance fields require their own versioned wire RFC.

## Verification

- Apply one CSR operator through both direct CSR and `LinearOperator`; require
  identical output and no per-apply allocation in the trait contract.
- Solve canonical Poisson through the compatibility and reference-backend
  paths; require unchanged solution/error/balance evidence.
- Compare reference and faer CG on the SPD system using independently
  recomputed true residuals.
- Solve a manufactured nonsymmetric system through faer BiCGSTAB, then falsify
  a CG selection for the same declared properties.
- Implemented: compare identity and Jacobi on a four-level coupled SPD
  `S T S` sequence with diagonal contrast through `10^6`; require nearly
  invariant reference Jacobi counts, increasing identity work, the same
  terminal ordering from faer, and independent true-residual acceptance for
  every solve (`numerics.preconditioner-stress`).
- Compare one-worker and multiple-worker reproducible CPU results exactly;
  check fast mode against the explicit residual tolerance.
- Reject a reference capability request for more than one host worker.
- For distributed execution, verify owner uniqueness, ghost consistency, halo
  exchange, global dot product, distributed CSR action, and solution residual
  on one, two, and at least four ranks.
- The implemented MPI slice verifies halo exchange, global dot product,
  distributed CSR action, Jacobi-preconditioned CG, and independently
  recomputed global solution residuals on one, two, and four ranks on one host.
- The same canonical case verifies functional execution with one rank on each
  of two distinct physical nodes; one-node placement under the two-node
  requirement fails before numerical work.
- Require the workspace MSRV, dependency policy, Clippy, docs, and all feature
  combinations for each graduated adapter.
- Assert that the canonical scalar-elliptic capability accepts only its bounded
  reference envelope: generated Cartesian FEM/FVM in `1D..=3D`, imported
  affine-simplex FEM in 2D, `f64`, replicated layout, and one host worker.
  Keep the lower-level runtime-dimensional contracts independently tested.

## Research basis

- [faer matrix-free operators](https://docs.rs/faer/0.24.4/faer/matrix_free/index.html)
  provide matrix-free CG and BiCGSTAB with explicit parallelism.
- [Rayon thread pools](https://docs.rs/rayon/latest/rayon/struct.ThreadPoolBuilder.html)
  permit execution-owned worker pools through `ThreadPool::install`.
- [mpi 0.8.2](https://docs.rs/mpi/0.8.2/mpi/) exposes communicator and
  collective adapters over a system MPI implementation.
- [PETSc vectors](https://petsc.org/release/manual/vec/) distinguish sequential,
  MPI, and ghosted distributed vectors and provide the principal comparison
  model for ownership and local/global assembly.
- [PETSc KSP](https://petsc.org/release/manual/ksp/) separates operator,
  preconditioner, solver method, and convergence policy.

These sources inform adapter and ownership boundaries; none defines Eqiora
model semantics.

## Security, safety, and governance

The reference and Rayon paths use safe Rust. Library panics must not cross an
FFI callback boundary. MPI and CUDA adapters isolate native handles, document
threading and lifetime invariants, and convert failures to stable diagnostics.
Untrusted artifacts cannot select arbitrary native libraries, kernel source,
hosts, or launch commands.

Backend support changes public capability claims and numerical reproducibility,
so graduation requires RFC/PR review and conformance evidence. Performance
numbers record hardware, worker/rank topology, library and driver versions,
precision, setup/transfer inclusion, reduction policy, and compiler flags.

## Unresolved questions

- Whether reproducible reduction uses a fixed tree, superaccumulator, or both
  as scale and accelerator evidence develops.
- The stable identity and wire representation of partition and halo artifacts.
- Whether the first production distributed solver is Eqiora Krylov over MPI or
  a PETSc adapter; both must consume the same ownership contract.
- MPI process-failure policy and whether ULFM-class recovery enters the first
  supported profile.
- NUMA placement and affinity representation after a multi-socket benchmark.
- Mixed precision and scalar-generic contracts after the first `f64` slices.
### Capability negotiation

Capability is a predicate over a plan and the admitted problem, not a set of
marketing labels. The first typed requirement contains:

```text
Problem requirements
    spatial dimension
    scalar type
    replicated | distributed vector layout

Backend capability
    exact set of (Realization context, Solver capability) pairs

Realization context
    spatial (method, mesh family, dimension envelope)
    replicated | distributed vector layout
    target kind and bounded local capacity
    exact Offline or exact RealTime { priority, deadline } request

Solver capability
    algorithm and asserted operator properties
    preconditioner and reduction policy
    scalar type
```

The scalar type has one authority in `SolverCapability`; the enclosing
Realization tuple delegates to it. A range of dimensions or maximum host
worker count is an evidence-bounded envelope inside one tuple, not permission
to recombine the other axes. Space family, polynomial order, quadrature, and
method-specific facts remain owned by their typed plan validators. This
capability set is admission data under the portable execution graph, not a
second scheduler and not persisted artifact identity.

The compatibility `RealizationPlan` does not carry an operator assertion, so
its first resolution retains the nonempty set of operator properties from its
otherwise exact candidate tuples. The equation-aware portable projection must
seal its property claim against that retained set. A legacy finalizer that
does not use this projection must require its known equation property against
the same retained set before materializing an operator. Field-wise plans carry
the assertion directly, and numerical finalizers still compare exact equation
identity and coefficients before execution. No wildcard operator property is
stored.

The current canonical scalar-elliptic reference envelope is generated
Cartesian FEM/FVM in `1D..=3D` plus imported affine-simplex FEM in 2D, all
`f64 + replicated + one host worker`. The dedicated legacy interval lowerer
remains one-dimensional. Runtime-dimensional topology, geometry, quadrature,
and space contracts are broader, but their generality does not widen these
bounded end-to-end claims.

`HostCpu { threads }` and `CudaGpu` remain request vocabulary, not executable
evidence. A capability without a working executor omits that worker count or
target, and resolution fails closed.
