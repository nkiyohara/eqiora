# RFC 0058: Portable Realization and bound execution graphs

- Status: Implemented for the bounded portable-graph, host,
  single-device CUDA, and one-host MPI slices;
  [`numerics.threaded-cpu`](../verify/numerics/threaded-cpu/README.md),
  [`numerics.canonical-cartesian-poisson-mpi`](../verify/numerics/canonical-cartesian-poisson-mpi/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0009](0009-realization-graph-v0.md), [RFC
  0010](0010-execution-backend-contracts.md), [RFC
  0045](0045-fieldwise-mixed-realization-and-si-congruence.md), [RFC
  0050](0050-fixed-reference-monolithic-fsi.md), and [RFC
  0053](0053-discrete-block-system.md)

## Summary

Eqiora separates three values that the provisional `Target`-bearing plans
currently compress:

```text
resolved compatibility plan
  -> portable typed Realization DAG
  -> observed Deployment binding
  -> bounded run-materialized Execution DAG
  -> accepted Run evidence
```

The Realization DAG owns portable numerical choices and exact references to
Semantic identities. Deployment owns host, process, device, queue, runtime,
and transport assignments. The Execution DAG makes data movement and
completion dependencies visible. No layer uses an `MpiCuda` cross-product
variant or an arbitrary node/payload registry.

## Motivation

The accepted scalar, field-wise, coupled, and transient plans are useful
typed authoring contracts, but each repeats a tuple of spatial policy, solver,
`Target`, and schedule. Extending `Target::HostCpu | CudaGpu` with ranks,
devices per rank, staging modes, queues, and mixed host/device execution would
make capability logic a closed cross product. It would also place a local
CUDA ordinal in what should be portable Realization identity.

Replacing the tuples with an arbitrary graph store would be worse. It would
permit numerical meaning, runtime handles, buffers, evidence, and actions to
share one payload vocabulary and repeat the anything-box problem rejected by
RFC 0009.

The graph must therefore be typed by construction and introduced without
changing the frozen realization-envelope v1/v2/v3 bytes or creating a
short-lived transient wire.

## Phase A: portable Realization DAG

### Compatibility boundary

The existing plan families remain the authoring and frozen-wire compatibility
contracts during migration. Their current resolvers remain the only
compatibility validators. A successful resolved value is then normalized
losslessly into the common DAG; the graph does not independently reinterpret
an unvalidated legacy value.

Facts absent from an old compatibility plan enter as structurally typed claims
from an equation-aware lowerer. Graph validation proves reference closure and
solver compatibility, not the truth of caller-supplied Semantic identities or
operator properties. Before execution, the equation-aware finalizer must
compare every such claim with its exact accepted lowering. Only that composed
path may authorize a Run or registered evidence.

New execution paths consume the graph. Existing envelope versions permanently
retain `decode -> existing validation -> graph projection`. A graph-native
artifact family is considered only after scalar, field-wise, coupled, and
transient projections prove equal accepted behavior and unchanged old bytes.

### Typed layers

The minimum graph is a layered DAG:

```text
DomainDiscretization
        ^
FieldRepresentation
        ^
Transformation
        ^
AlgebraicSystem <- LinearSolve <- NonlinearSolve
                         |
              PlacementRequirement
```

Every reference has its own typed arena ID. There is no public `NodeId`,
`NodeKind`, untyped `Edge`, JSON payload, or adapter-defined extension map.
Typed direction makes cycles between layers unrepresentable; validation
rejects missing references, duplicates, orphan nodes, incomplete scaling, and
more than one connected accepted solve closure in Phase A.

`DomainDiscretizationNode` binds one exact Semantic Domain to coordinate
treatment, method, mesh policy, and quadrature. `FieldRepresentationNode`
binds one exact Semantic Field to a Domain node and discrete space. Algebraic
or reconstructed state is not duplicated as a Field-node flag: membership in
the system blocks or an elimination transformation determines the role.

Implemented transformations are added only when an accepted consumer exists:

- Backward Euler derivative of one exact Relation and state Field;
- energy-skew convection for one exact Relation and velocity Field;
- conforming trace quotient for one exact Connection; and
- Backward Euler state elimination for an exact state/rate/Relation binding.

The first two are implemented by the fixed-domain transient-flow projection.
The latter two are the next compatibility projections for the accepted
fixed-reference FSI path.
Essential elimination, local contribution packets, residual origins, CSR,
and coefficient storage remain private lowered-operator facts rather than
portable choices.

`AlgebraicSystemNode` owns canonical monolithic blocks, transformations,
congruence scaling, asserted operator properties, scalar policy, and
replicated/distributed partition requirement. `LinearSolveNode` consumes the
sole `SolverPlan`; `NonlinearSolveNode` refers to its exact linearization.
There is no second preconditioner or method-specific solver configuration.

### Portable placement

Phase A admits requirements, not discovered resources:

```text
HostWorkers { workers_per_partition }
CudaDevices { devices_per_partition }
```

A distributed system plus one CUDA device per partition expresses the
portable shape consumed by RFC 0063. The graph never contains an environment
device ordinal, MPI rank, communicator, queue ordinal, runtime handle, or
transport discovery result. Legacy CUDA plans retain their exact ordinal only
in their compatibility envelope and deployment projection.

Model-time activation remains in `ClockDomain`. `ExecutionSchedule` remains a
deployment scheduling requirement and cannot contain a model period. Repeated
step count remains a Run directive and is absent from Realization identity.

### Identity and canonicalization

Graph-local typed indices are references, not durable identity. Projection
sorts Domain and Field nodes by exact Semantic identity and gives every
accepted solve one connected root. A later graph artifact hashes canonical
content through the artifact layer's domain-separated encoding; this L2 crate
does not invent floating-point hashing.

`RealizationRevision` records independent editing/provenance revision and is
not a substitute for content identity. Model identity, semantic revision, and
default/explicit Realization source remain explicit lineage.

## Phase B: Deployment and Execution DAGs

Phase B introduces a small backend-neutral contract after all Phase-A
projections close:

```text
ExecutionRequirements
  -> DeploymentBinding
  -> run-materialized ExecutionDag
  -> accepted Run evidence
```

The first implemented bindings are deliberately linear. Host binding validates
one selected serial or Rayon backend/adapter, available worker capacity, and
the exact solver tuple before pool or numerical-system materialization. CUDA
binding validates one run-owned physical device, a logical `QueueSlot`, the
known minimum resident payload, and the exact CSR/property/CG/Jacobi/`Fast`
intersection before device allocation. Distributed binding validates one
transport-neutral `DistributedExecutorDescriptor`, a logical
`ProcessGroupSlot`, exact process count and one worker per partition, and the
bounded `Distributed`/`f64`/offline/SPD/CG/Jacobi/`Reproducible` intersection.
An opaque admitted execution then
borrows one finalized canonical CSR system and seals its fingerprint, operator
properties, sole solver plan, binding, and preallocated independent verifier.
For distributed execution it additionally seals the exact owner map, derived
layout/halo identity, and plan-inclusive distributed admission fingerprint.
The equation-aware scalar finalizer independently regenerates and retains the
exact portable graph beside that system; raw association is not exposed by the
curated facade. Neither contract exposes mutation or raw graph identity.

Deployment owns observed runtime placement rather than portable identity. A
`QueueSlot` selects the logical single-device deployment position before
allocation; the CUDA adapter later materializes one process-unique `QueueId`,
whose complete identity scopes submission order. The binding and producer
report carry Eqiora-owned typed provider descriptors: stable ID, exact
implementation version, and sorted dependency releases declared by that
provider. Receipt acceptance requires exact equality. Live CUDA
driver and vendor-library observations remain in paired adapter/Run evidence,
not in the execution receipt. MPI implementation/version and provided thread
support likewise remain live paired adapter/Run evidence; only the compiled
Rust binding belongs to provider identity. The application initializes MPI
and the L3 adapter validates thread support and duplicates the communicator
before it can form the observed process-group descriptor used by numerical
binding. The contract therefore makes no pre-communicator rejection claim.
Communicator handles, rank-local handles, limits, and future
partition-to-process-to-device assignments remain outside the portable graph
and L2 receipt.

The implemented host DAG has one closed shape:

```text
SolveWithNativeAcceptance -> ReplayTrueResidualOnHost -> AcceptHostComplete
```

It is exposed only as a read-only receipt view. Serial and Rayon share that
logical DAG and differ in their immutable binding and producer report. Exact
report agreement includes the complete solver/execution provider releases and
the contract-required verifier release, so a same-ID version or
dependency-release substitution fails closed. The first
step retains the solver-native verifier instead of pretending the solver
returned an unverified candidate. Acceptance then requires the complete
vector, exact report agreement, an additional independently recomputed
Eqiora-owned serial-host true residual, and an exact normalized output-vector
fingerprint.

The implemented CUDA DAG adds typed buffer slots, exact value generations, and
successful waits only for its executable consumer. Its closed shape is:

```text
TransferInputsToCuda -> AwaitCudaInputsReady -> SolveOnCuda
  -> AwaitCudaSolveCompletion -> TransferCandidateToHost
  -> AwaitHostVisibility -> AcceptWithNativeHostVerification
  -> ReplayTrueResidualOnHost -> AcceptHostComplete
```

Seven typed transfers cover CSR structure/values, right-hand side, Jacobi
diagonal, zero initial solution, and the complete returned solution. Distinct
real CUDA events are successfully waited after input transfer, solve, and
device-to-host transfer. The solution advances exactly one generation in its
device allocation, and the downloaded value names that exact solved
generation. The adapter records its external sparse workspace separately and
checks it with the known payload against total device capacity; this is not a
reservation or observation of currently free memory. Native serial-host
acceptance is retained, then a second serial
true-residual replay binds the immutable, complete-host-output receipt to its
normalized output fingerprint. A raw or device-resident value cannot escape
as accepted output.

The host-only reconstruction of committed CUDA evidence creates successful
synthetic fences solely to rebuild and validate this typed trace. It does not
re-attest physical event waits; only the physical collector obtains
`WaitedCompletion` values from real CUDA fences.

Iterative solves are not expanded to their maximum iteration count. Host
admission validates finite capability and reserves the exact replay workspace;
its three-node logical shape is static, while actual iteration count remains
only in `SolveReport`. The MPI adapter reserves a checked
`32 * maximum_iterations + 64` trace capacity before collective admission and
then records only the synchronized boundaries that actually occur, with dense
global ordinals and bounded iteration indices. A solver's reduction policy is
referenced from the sole `SolverPlan`, not copied into a second execution
configuration.

The implemented distributed macro DAG is fixed while its Krylov region owns
the bounded actual trace:

```text
AgreeDistributedAdmission -> SolveDistributedKrylov
  -> AgreeDistributedProducerReport -> GatherDistributedOwnedCandidate
  -> AcceptWithNativeHostVerification -> AgreeDistributedAcceptedResult
  -> ReplayTrueResidualOnHost -> AgreeDistributedReceipt
  -> AcceptHostComplete
```

The normalized trace exposes admission, halo readiness, owned action, owned
vector update, collective reduction, producer agreement, paired owner-gather
preparation/validation, native host acceptance, and accepted-result agreement.
It binds every observed step to the complete-system, partition, layout,
distributed-admission, logical process-group, rank/worker, and complete-gather
identities. The MPI adapter consumes the same opaque `AdmittedExecution` as
the host and CUDA adapters; it cannot reselect the graph, complete system,
layout, provider, process count, or solver plan. Every rank reconstructs and
natively accepts the complete host candidate, agrees the accepted result, and
the L2 receipt then performs its additional serial true-residual replay and
output fingerprinting. The L3 group finally all-gathers a domain-separated
fixed-size summary over operator, output, dimension, producer report,
partition/layout/admission/process-group identities, and the complete
normalized trace. Only byte-identical receipts pass
`AgreeDistributedReceipt` and escape as accepted output; its receive storage
is allocated when the communicator-backed group is constructed.

The bounded MPI plus CUDA slice composes partition-to-process and
process-to-device bindings with explicit host-staged transport; it does not
add a new target kind. Its v2 common summary retains the complete role-tagged
partition-local action provider release in addition to live CUDA observations,
so equal dependency versions cannot hide a different local-action identity.
Device-aware transport remains a separate capability gate.

## Failure rules

Projection, deployment, or execution admission fails before the allocation or
communication boundary owned by that phase when any of the following is
unknown, unsupported, duplicated, stale, or contradictory. MPI communicator
duplication and its small preparation-status storage precede numerical
deployment binding, as described above:

- exact Domain, Field, Relation, Connection, space, or block membership;
- transformation reference, connected-root reachability, or scale coverage;
- algorithm/operator/preconditioner/reduction/scalar capability tuple and
  deployment schedule;
- partition, process, device, queue, memory, or transport requirement;
- partition-to-process or process-to-device bijection;
- buffer type, extent, distribution, residency, producer, or generation;
- transfer endpoint/byte count, halo/layout identity, collective policy, or
  cross-queue completion without a fence; or
- accepted output without required movement, gathering, visibility,
  independent verification, and agreement.

An adapter may not perform hidden Eqiora-controlled address-space staging.
Vendor-driver and network-internal movement are outside this claim and are not
presented as observed evidence.

## Alternatives considered

### Make one universal graph node with an extensible payload

Rejected. It removes compile-time ownership, admits orphan metadata, and
mixes portable choices with runtime and evidence.

### Replace each tuple field by a vector or option

Rejected. This produces a graph-shaped bag without typed dependency or
reachability and moves validation branching into every consumer.

### Make the private discrete block system public

Rejected. Relation contributions, support packets, CSR materialization, and
assembly incidence are lowered operator facts, not portable numerical
selection.

### Add `Mpi`, `MpiCuda`, and staging variants to `Target`

Rejected. Distribution, compute resource, and transport are orthogonal and
must compose.

### Create a graph-native artifact immediately

Rejected. It would duplicate current validators and create another wire before
lossless scalar, field-wise, coupled, and transient migration is proven.

## Verification

Phase A closes in four projections:

1. Fixed-domain transient flow: exact Domain/Fields, Backward Euler and
   energy-skew
   transformations, monolithic blocks, nonlinear-to-linear role, and
   ordinal-free host placement drive the unchanged two-step reference.
2. Scalar serial/Rayon: equation-aware Domain/Field claims are compared with
   the accepted scalar lowering and produce unchanged solution, reports, and
   frozen v1 bytes.
3. Field-wise Stokes: exact mixed spaces, constraint, scaling, and v2 bytes are
   unchanged.
4. Coupled FSI/discrete blocks: exact Domain/Field inventory, quotient,
   eliminated state,
   monolithic block identity, accepted operator, and v3 bytes are unchanged.

The bounded Phase-A implementation closes all four projections. Each accepted
execution path consumes the common graph, and registered evidence owns the
structural and equation-aware comparisons. Separate registered artifact
evidence with committed golden fixtures owns v1/v2/v3 byte stability. This
does not promote the provisional in-memory graph to a public artifact wire.

Malformed reference, duplicate Field, orphan Domain/transformation, and
unsupported placement fail during graph validation. Wrong Relation or
Domain/Field identity and eliminated-state role drift fail during the
equation-aware finalizer's exact comparison, before execution.

The host Phase-B slice verifies pre-pool capability rejection with an
allocation spy, exact graph/operator/plan/report substitution rejection,
independent serial true-residual replay, and one common serial/Rayon logical
DAG with distinct bindings. The registered `numerics.threaded-cpu` application
case owns the ordinary public-path evidence. The CUDA Phase-B implementation
provides one Q1/TPFA Jacobi-CG/`Fast`, implicit-zero, run-owned
single-device/single-queue path with pre-device-allocation binding and admission
and waited input/solve/output fences. The MPI Phase-B slice is closed by the
registered canonical Q1/TPFA case at one, two, and four ranks on one host. It
verifies the exact distributed binding and admission identities, bounded
actual collective trace, fixed nine-step macro DAG, explicit-index owner
gather, native host acceptance, accepted-result agreement, additional receipt
replay, final receipt-summary agreement, and cross-rank
operator/output/partition/layout/admission agreement. The ordinary physical
gate uses live MPI processes on one host; it is not a simulated transport.
Recorded two-node physical evidence remains limited to the earlier lower-level
halo/reduction/CG algebra test; the graph-bound generic/canonical bridge does
not inherit it. Physical bridge placement, performance, and scale remain
separate evidence. With host, CUDA, and MPI gates closed, the bounded portable
execution-graph slice is complete and RFC 0060 is the next distributed
dependency.

## Compatibility and nonclaims

Realization envelopes v1/v2/v3 and Run manifest v2 remain frozen. The first
Phase-A graph is in-memory and provisional. It is not a dynamic plugin API,
universal scheduler, load balancer, fault-tolerance policy, real-time
certification, graph wire, multi-GPU implementation, or scale claim.
Host, CUDA, and distributed execution receipts are in-memory and do not imply
a durable execution artifact. The CUDA slice does not expose a curated public
graph API
or claim arbitrary initial guesses, free-memory reservation, persistent
residency, multiple streams/queues, GPU assembly, matrix-free execution,
reproducible device reductions, general CUDA, general FSI/MINRES, multi-GPU, MPI plus
CUDA beyond the bounded host-staged composition, performance, scale, or
hardware attestation. The distributed slice does not claim general
distributed mesh or assembly, a sharded result Field, physical
multi-node graph-bound bridge, hybrid rank/thread execution, arbitrary initial
guess, native-fast reduction, general FSI/MINRES, checkpoint/restart,
process-failure recovery, dynamic process groups, general MPI plus CUDA,
durable trace/receipt wire, general distributed PDE execution, performance,
scale, or attestation.
