# RFC 0061: MPI fixed-mesh fluid--structure interaction

- Status: Implemented and verified for the bounded one-host 2D slice
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0050](0050-fixed-reference-monolithic-fsi.md), [RFC
  0058](0058-portable-realization-and-execution-graphs.md), and [RFC
  0060](0060-distributed-spatial-ownership-and-assembly.md)

## Summary

Eqiora executes the existing fixed-reference monolithic 2D FSI step over MPI
by promoting RFC 0060's accepted reduced owner-row payloads directly into the
rank-local CSR/RHS storage used by the distributed algebra, deriving only the
solver-vector halo from their sparsity, and applying identity-preconditioned
reproducible MINRES to the same
finalized symmetric-indefinite operator used by CPU reference execution.

```text
accepted distributed mesh layout
  -> accepted owner-routed reduced + full FSI assembly
  -> accepted reduced owner-row payloads as the sole source of local CSR/RHS
  -> sparsity-derived solver-vector halo
  -> MPI identity-preconditioned reproducible MINRES
  -> explicit-index complete-vector gather on every rank
  -> complete-host residual reacceptance on every rank
  -> unchanged fixed-reference FSI finish
```

The full assembly target is not submitted to MINRES, but its identity remains
in the prepared FSI handoff. Its reconstructed pressure rows provide the
independent incompressibility residual; interface action and energy are
re-evaluated from accepted physical Fields and quadrature rather than inferred
from that target. No rank may
repartition the reduced system, repeat assembly, or substitute a separately
finalized operator between the accepted assembly receipt and solver
admission.

## Motivation

RFC 0050 proves one fixed-reference monolithic FSI step through a finalized
reduced CSR system, reproducible CPU MINRES, and independent physical
acceptance. RFC 0058 proves a transport-neutral distributed execution graph,
MPI halo/action/reduction, explicit-index gather, and accepted execution
receipt for bounded scalar SPD problems. RFC 0060 then proves that the exact
FSI cell packets can be evaluated and accumulated by their owners into
accepted reduced and full owner-row shards.

Those results do not yet compose. Reconstructing RFC 0060's complete CSR and
then choosing a new balanced or cyclic solver partition would create a second
ownership authority. Gathering the system before the numerical solve would
exercise MPI orchestration but not distributed FSI algebra. Changing the
indefinite operator into a positive-definite surrogate would change the
accepted Realization rather than execute it.

This RFC closes only that missing composition. It does not introduce new FSI
meaning, a second local operator, or a general MPI multiphysics framework.

## Decision

### One finalized FSI meaning and one distributed layout choice

The Semantic Model, exact Domain and Field identities, conforming trace
quotient, Backward Euler state elimination, coherent-SI congruence, pressure
closure, reduced/full assembly maps, and symmetric-indefinite property remain
those of RFC 0050.

The coupled Realization explicitly selects `VectorLayoutKind::Distributed`
for this execution path. Layout is an execution choice, not new physical
meaning. The portable graph therefore differs from the replicated CPU graph
only where it must describe distributed algebra and deployment. It retains
the same discretization, transformation identities, algebraic blocks,
operator property, solver plan, and one-host-worker-per-partition requirement.

The equation-aware FSI finalizer must retain the resolved vector-layout choice
beside the finalized system. It may not hard-code replicated result
acceptance and then special-case an MPI report after execution. The existing
topology validator already distinguishes a distributed producer from the
complete-host verifier; that ordinary contract is the route used here.

### Accepted assembly shards are the distributed operator

RFC 0060 produces one accepted in-memory evidence value containing:

- the exact distributed mesh-layout identity;
- a payload-bound assembly-plan and receipt identity;
- ordered reduced and full target identities;
- the unique owner of every target row; and
- one checked owner-row shard for every target and partition.

The MPI FSI bridge binds that accepted value without exposing a second
ownership input. For the reduced target it constructs the distributed linear system
directly from the accepted owner-row
shards and their exact row ownership. It validates global row coverage,
canonical column order, finite values and right-hand sides, scalar type,
partition count, and the reconstructed complete-system identity before the
system can enter solver admission.

The complete reduced CSR retained by the FSI finalizer is the independent
complete-host verifier for that same operator. It is not reshared to create
the local MPI matrices. `DistributedLinearSystem::from_complete` remains a
valid contract for other finalized-system paths, but it is not this
assembly-to-solve boundary.

Solver-vector ghosts and the halo plan are derived from the accepted reduced
shards' off-owner column indices. They are not copied from mesh entity
residency or assembly routes:

```text
cell ownership
  != lower-entity residency
  != assembly row routes
  != solver-vector halo
```

Only the first three are completed by RFC 0060. This RFC derives the fourth
from the already accepted algebraic sparsity and records its separate
identity.

The assembly-bound reduced system retains the whole two-target receipt and
selected reduced-target identity. The prepared FSI handoff composes it with
the independently checked full-target identity, row-partition identity, and
derived halo identity. A matching reduced system paired with a stale full
target is therefore not admissible to the FSI finish.

### Exact MPI MINRES capability

The MPI provider adds exactly this capability tuple:

```text
LinearSolver::MinimumResidual
  + LinearOperatorProperties::SymmetricIndefinite
  + PreconditionerPolicy::Identity
  + ReductionPolicy::Reproducible
  + ScalarType::F64
```

The implementation uses its own fixed-workspace, short-recurrence Lanczos and
orthogonal-rotation algorithm. It does not inherit the CPU reference provider's
retained-basis, full-H projection or dimension cap. It uses the existing
distributed owned-vector, halo action, synchronized vector update, and scalar
reduction contracts. The MPI provider uses the algorithm-neutral backend identity
`eqiora.mpi.krylov`; the exact `SolverPlan` and capability tuple distinguish
MINRES from CG. A MINRES execution therefore cannot report a CG plan even
though both algorithms share one transport/provider implementation.

The generic distributed algebra and execution layers encode the complete
solver plan and validate structural requirements. Concrete algorithm support
belongs to the selected provider's exact `SolverCapabilities`, avoiding a
second hard-coded capability table in the algebra container. Existing MPI
CG/SPD capability tuples remain unchanged while their historical
algorithm-specific provider identity migrates to the common Krylov identity.

Identity preconditioning is deliberate. Jacobi on an indefinite coupled
operator is neither inferred nor admitted, and no block or Schur-complement
preconditioner is introduced by this first proof.

### Reproducibility boundary

Within one admitted process-group shape, each rank performs local products in
the exact owned order and combines one rank contribution per logical rank in
rank order. All ranks must therefore agree exactly on the accepted complete
output and receipt.

Changing the rank count changes the ownership grouping and hence the
floating-point reduction tree. `Reproducible` does not imply bit-identical
MINRES iterates, solution values, or iteration counts between one, two, and
four ranks. Cross-rank-count verification instead requires:

- identical Semantic Model and finalized reduced/full operator meaning;
- the same solver plan and physical pressure policy;
- agreement with an independent CPU solution under explicit absolute and
  relative tolerances; and
- independent satisfaction of the existing FSI acceptance bounds.

Partition, halo, assembly-receipt, deployment, and Run identities are
expected to differ when the rank count differs. That is execution provenance,
not model drift.

### Gather, host reacceptance, and FSI finish

After distributed MINRES converges, every rank contributes only the global
indices it uniquely owns and the matching values. A paired explicit-index
gather reconstructs one complete candidate on every rank. Dense rank-order
concatenation is not accepted as an implicit global numbering.

Every rank then:

1. validates complete index coverage with no duplicate or missing owner;
2. reapplies the exact finalized reduced CSR on the host;
3. recomputes the true residual and the solver-plan target;
4. agrees the provider report and accepted output identity; and
5. passes the accepted `LinearSolution` into the unchanged RFC 0050 FSI
   finish.

The last step independently reconstructs physical velocity, pressure, and
solid displacement and rechecks incompressibility, kinematics, opposite
interface action, and the zero-work energy identity. The shared trace
quotient retains an exactly zero interface-velocity jump by construction.
MPI does not gain a physics-specific finish path.

### In-memory lineage and provenance

The typed in-memory composition binds:

- Model, semantic revision, Realization revision, and portable graph;
- the authenticated mesh artifact;
- RFC 0060 assembly receipt and ordered reduced/full target identities;
- reduced row ownership, distributed operator, and solver-vector halo
  identities;
- the sole solver plan and MPI MINRES backend identity;
- logical process-group slot, actual rank count, and workers per rank;
- the actual normalized collective trace; and
- complete accepted output and receipt identities.

The registered case separately records and checks the geometry and
geometry--mesh correspondence fixture plus the observed MPI implementation,
library version, binding-library version, MPI standard version, and provided
thread support. All ranks agree a domain-separated summary of that typed
runtime observation. Those values are not fields of the in-memory execution
receipt, and this slice does not introduce a symmetric-indefinite distributed
Run artifact.

The L3 MPI adapter may compose the assembly and execution receipts in memory.
This RFC introduces no durable receipt, distributed Field, or new artifact
wire. A later persistence decision must preserve the typed constituent
identities rather than flatten them into an arbitrary provenance map.

## Failure rules and falsifiers

The typed boundaries fail closed for at least:

- a mesh-revision, assembly-receipt, reduced-target, full-target, operator, or
  solver-plan identity mismatch;
- a reduced row-owner array not taken from the accepted assembly evidence;
- a missing, duplicate, wrong-owner, malformed, or non-finite accepted shard;
- a shard reconstruction that differs from the finalized reduced operator;
- a stale full target even when the reduced operator still matches;
- an unsupported MPI solver tuple or an execution report that substitutes
  CG, Jacobi, `Fast`, scalar type, orientation, or tolerance policy;
- a halo request for a value without one admitted owner;
- a post-admission rank-local action, reduction, vector-update, report,
  gather, or host-verification failure while another rank can advance to the
  next collective;
- a gather with missing, duplicated, or out-of-range global indices;
- loss or duplication of constrained rows or physical interface
  contributions; or
- rank-count-dependent Semantic Model, pressure closure, or finalized
  operator meaning.

RFC 0060 supplies collective mesh-layout admission before variable-sized
assembly communication. The MPI execution adapter supplies collective
readiness agreement after execution admission. The noncollective FSI binder
does not claim to synchronize arbitrary rank-local allocation or lineage
failure, and the replicated physical finish runs only after MPI has returned
one rank-agreed accepted solution. Registered backend fault injection must
demonstrate a common stable diagnostic and bounded termination; each
subprocess owns a fresh communicator and exposes no partial accepted output.

## Verification

One registered environment-dependent case executes the existing exact
fixed-reference 2D FSI fixture with one, two, and four MPI ranks on one host.
For each rank count it must:

1. independently finalize, assemble, solve, and finish the CPU reference
   problem;
2. run RFC 0060 physical MPI owner-routed assembly for both reduced and full
   targets;
3. prove both reconstructed targets remain bit-identical to independent CPU
   assembly before numerical solving;
4. construct local distributed algebra only from the accepted reduced
   owner-row shards;
5. prove the solver partition is exactly the accepted assembly row ownership
   and that the halo is derived from those shards;
6. execute MPI identity-preconditioned reproducible MINRES;
7. reconstruct by explicit global indices and perform complete-host
   reacceptance on every rank;
8. require exact within-run agreement of output, trace, and receipt; and
9. compare dimensionless algebraic coefficients and physical Fields normalized
   by their exact Realization scales with the independent CPU result using
   `abs(a - b) <= 2e-10 + 2e-10 * max(abs(a), abs(b))`; and
10. require the CPU and MPI results independently to pass RFC 0050's native
    residual, incompressibility, kinematic, interface-action, and energy
    acceptance contracts.

The fixture deliberately gives interface velocity, fluid bubble, and pressure
rows different owners and requires nonempty halo traffic for multi-rank runs.
Expected row roles may be asserted by the case, but private numerical row
ordinals do not become a public FSI ABI.

At minimum, the composition case falsifies a foreign mesh revision, reduced
or full target drift, and any attempt to supply a second solver owner map.
The registered generic MPI prerequisite injects a post-admission MINRES
rank-local action failure and proves a common diagnostic without deadlock.
Existing RFC 0058 gather/host-verifier and RFC 0060 collective mesh/route
falsifiers remain prerequisites rather than being copied into a second
implementation.

## Alternatives considered

### Reconstruct complete CSR and repartition it for the solver

Rejected for this composition. It would produce numerically equivalent local
matrices in successful cases, but it creates a second partitioning and
sharding authority after RFC 0060 already accepted exact owner-row shards.
It cannot prove that the solver consumed the accepted distributed assembly.

### Gather the assembled system and solve serially

Rejected. This is a useful independent oracle and is already part of the
verification strategy, but it does not execute a distributed vector, halo,
operator action, or Krylov recurrence and therefore cannot support an MPI FSI
solve claim.

### Form normal equations and reuse distributed CG

Rejected. Applying CG to `A^2 x = A b` squares the condition number and
changes the selected operator and solver semantics. It would evade rather
than implement the symmetric-indefinite MINRES contract.

### Use PETSc MINRES immediately

Deferred. PETSc is a credible later execution provider, especially when
scalable block preconditioning becomes the owning requirement. It adds a
second library lifecycle, option/configuration boundary, communicator
contract, and provenance surface without strengthening this first bounded
semantic composition proof. A future adapter must consume the same finalized
operator and exact capability contract.

### Add an FSI-specific MPI backend

Rejected. MPI owns transport and distributed solver execution, not fluid,
solid, pressure, interface, or energy meaning. The unchanged numerical FSI
finish is the only physics-aware consumer after generic linear acceptance.

## Compatibility and migration

This RFC changes no Semantic Kernel node, Standard Ontology meaning, package
contract, source language, Geometry Identity, mesh artifact, FSI equations,
assembly accumulation order, solver-plan type, canonical CSR identity, or
existing artifact wire.

Existing replicated CPU FSI, distributed CG meaning, loopback algebra, and
RFC 0060 assembly paths remain valid. The MPI provider identity changes from
`eqiora.mpi.cg` to `eqiora.mpi.krylov` because the provider now implements
more than one Krylov algorithm; no stable artifact wire has encoded the old
identity. The coupled FSI requirements and finalized
linear core must cease assuming replicated layout so that replicated and
distributed executions are both admitted through their ordinary typed
contracts. This is an execution-layout generalization, not a new numerical
lowering.

The first implementation may keep the assembly-bound bridge and composite
receipt workspace-public rather than adding it to the curated facade. Public
or durable exposure requires an independent consumer and compatibility
review.

## Nonclaims

This RFC does not claim:

- a multi-step or transient FSI trajectory;
- distributed physical `Field` storage, result persistence, checkpoint, or
  restart;
- multiple physical nodes, scalability, performance, NUMA placement, load
  balance, or process-failure recovery;
- nonblocking Krylov overlap, `Fast` reductions, Jacobi, block, Schur, AMG,
  or production FSI preconditioning;
- GPU-aware MPI, CUDA FSI, multiple GPUs, or MPI plus CUDA;
- distributed nonlinear solve, Navier--Stokes FSI, nonlinear structure,
  contact, nonconforming interfaces, or partitioned coupling;
- ALE, remeshing, moving geometry, shape optimization, or FSI sensitivity;
  or
- other dimensions, meshes, spaces, element families, PDEs, or arbitrary
  partition counts.

In particular, “MPI fixed-mesh FSI” means one fixed-reference implicit
monolithic step for the exact registered 2D fixture, not a general transient
or scalable FSI product claim.

## Security, safety, and governance

All shard, halo, gather, trace, and collective extents are checked and
reserved before their first corresponding communication. Native MPI handles
remain inside the L3 adapter, the application retains initialization and
finalization authority, and no partial local result can manufacture an
accepted assembly or execution receipt.

Unsupported solver tuples and collectively admitted mesh-layout disagreement
fail before variable-sized numerical traffic. After MPI execution admission,
every fallible adapter-local phase has a status-agreement boundary so one rank
cannot return while a peer blocks in a later collective. The noncollective FSI
binder makes no broader synchronized-failure claim.

Any durable wire, new provider library, fast reduction, preconditioner,
multi-node claim, or public facade exposure is a separate capability change
with its own falsifier and registered evidence.

## Unresolved questions

None for the bounded slice. Production preconditioning, external solver
providers, multi-node evidence, distributed physical results, and transient
FSI are deliberately deferred until their own consumers define the required
contracts.
