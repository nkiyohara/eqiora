# RFC 0060: Distributed spatial ownership and owner-routed assembly

- Status: Implemented and verified for the bounded loopback and one-host MPI
  slices
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0010](0010-execution-backend-contracts.md), [RFC
  0018](0018-ordered-assembly-execution.md), [RFC
  0026](0026-distributed-spatial-layout-and-replication.md), [RFC
  0049](0049-geometry-identity-and-mesh-correspondence.md), [RFC
  0050](0050-fixed-reference-monolithic-fsi.md), [RFC
  0053](0053-discrete-block-system.md), and [RFC
  0058](0058-portable-realization-and-execution-graphs.md)

## Summary

Eqiora introduces one transport-neutral L2 seam between an exact accepted
mesh, deterministic local assembly, and the existing distributed algebra:

```text
accepted mesh revision + unique cell ownership
  -> derived owned/ghost entity layout + entity exchange plan
  -> cell-owned evaluation of the existing ordered AssemblyWork
  -> row-owner-routed deterministic accumulation
  -> owned-row shards + complete CPU reconstruction
  -> ordinary complete AssemblyResult + accepted in-memory receipt
```

The seam owns the association between spatial ownership and algebraic row
ownership. It does not move mesh meaning into `eqiora-distributed`, transport
meaning into `eqiora-meshing`, or physics-specific field support into
`eqiora-assembly`.

The first bounded consumer is the exact fixed-reference 2D FSI realization.
Its distributed path evaluates the same checked cell-local work and the same
reduced/full target maps as the CPU reference path. The complete CPU-finalized
CSR is an independent oracle, not the input to a purported distributed
assembly. A loopback protocol closes before the MPI adapter is allowed to
claim the same operation at one, two, and four ranks.

The distributed backends reconstruct and return an ordinary complete
`AssemblyResult` only after owned shards have passed their checks. That
complete result is the evidence/output of distributed assembly in this
bounded slice. It is never supplied as the input from which the shards are
derived.

## Motivation

The existing layers have clear but deliberately incomplete authorities:

- `eqiora-meshing` owns global mesh entities, incidence, orientation, affine
  geometry, and mesh-quality acceptance;
- `eqiora-realization` owns the exact content-addressed mesh selection;
- `eqiora-assembly` owns anonymous local contributions, constraint-aware
  local-to-global maps, ordered packets, and deterministic complete assembly;
- `eqiora-distributed` owns algebraic vector partitions, owned-row CSR,
  sparsity-derived vector ghosts and halo exchange, and transport-neutral
  collective policy; and
- `eqiora-numerics` owns discrete field support, FSI layout, local operators,
  physical interface checks, reconstruction, and coupled acceptance.

None of those authorities should absorb all the others. Adding cells, facets,
or physical interfaces to `eqiora-distributed` would widen the execution
backend RFC into mesh semantics. Adding algebraic row routing to
`eqiora-meshing` would make a topology crate own numerical accumulation.
Implementing the join privately in both `eqiora-numerics` and
`eqiora-backend-mpi` would duplicate the most important invariant precisely at
the transport boundary.

Partitioning a CPU-assembled CSR also does not satisfy this RFC. It verifies
distributed operator action, which RFC 0010 and RFC 0026 already cover, but it
does not prove that each local physical contribution was evaluated once,
that cross-partition interface contributions survived, or that row-owner
reduction preserved the reference accumulation order.

## Decision boundary

### One executable L2 composition seam

A new `eqiora-spatial-distribution` crate owns the executable composition:

```text
eqiora-meshing -----------\
eqiora-assembly ----------+--> eqiora-spatial-distribution
eqiora-distributed -------/          |                 |
                                     v                 v
                         loopback AssemblyBackend   eqiora-backend-mpi
                                     \                 /
                                      eqiora-numerics
                                  through AssemblyBackend
```

The same-layer dependencies are intentional and mechanically allowlisted
under this RFC. Dependencies remain one-way. The new crate does not become a
second owner of any input vocabulary and neither input crate depends back on
it.

The crate has two real consumers from its first completed slice:

1. the existing numerical finalizers consume its loopback implementation
   through `eqiora-assembly::AssemblyBackend`; and
2. `eqiora-backend-mpi` consumes the admitted layout/routing contract and
   implements the same `AssemblyBackend` boundary with physical transport.

`eqiora-numerics` does not depend on the new crate. Its existing generic
assembly boundary receives either backend, so the spatial seam gains no
private FSI entry point and no reverse L3 dependency.

It also contains a one-process loopback executor, so it is not a
vocabulary-only crate. Its workspace-public types are not added to the
curated `eqiora` facade in this slice. A facade export or durable wire requires
a separate compatibility decision after another external consumer exists.

### Abstraction budget

The seam owns one invariant:

> One exact mesh-bound cell partition determines every local entity view and
> packet producer, while the actual target equation support determines every
> active row owner; no adapter may independently choose either mapping.

The implementation introduces no registry, provider enumeration, callback,
arbitrary payload, universal resource type, or plugin ABI. Borrowed local and
route views are preferred over copying each conceptual record into a public
owned type. Fixed-size identities are in-memory L2 agreement values, not
artifact schemas.

One supporting change belongs in `eqiora-assembly`: a packet-local
`AssemblyDelta` projection becomes the sole lowering of
`AssemblyMap + LocalContribution` into canonical global-row deltas. The
complete `CooAssembler` and the spatial owner-row accumulator consume that
same projection. This removes, rather than duplicates, constraint elimination
and per-packet scalar folding. The projection knows nothing about partitions,
meshes, MPI, or FSI.

`AssemblyWork` also carries one required `AssemblyPacketSetIdentityV1`. Local
reference/threaded work may declare the explicit `Unbound` state, but a
spatially distributed backend accepts only a content-bound identity equal to
the exact mesh revision in its layout. The resolved FSI path supplies that
identity only after mesh-envelope and Realization replay. Equal packet counts
therefore cannot make work from another mesh admissible.

## Exact mesh ownership

### Cell claims are the sole ownership input

The first contract accepts one opaque exact 32-byte mesh revision identity,
the corresponding accepted full-dimensional conforming mesh, a nonzero
partition count, and explicit claims:

```text
CellOwnershipClaim = (global top-dimensional MeshEntity, PartitionId)
```

Claims are canonicalized by global cell index. Every top-dimensional cell
must occur exactly once, every owner must lie in the declared partition
range, and every declared partition must own at least one cell in v1.
Duplicate and missing claims fail before a local layout is created.

The contract does not select a graph partitioner or claim a balancing policy.
Balanced, geometric, or application-informed partition selection is a
Realization policy outside this RFC. Once claims enter this contract, their
meaning is exact and independent of input order.

### Lower-dimensional ownership is derived

For a lower-dimensional mesh entity `e`, let its top-dimensional star be all
accepted cells incident to `e`. Define:

```text
residents(e) = sorted unique { owner(c) | c is in the top-cell star of e }
owner(e)     = minimum residents(e)
```

The minimum is a deterministic tie-break, not a load-balancing claim. Because
an accepted conforming mesh gives every lower entity a nonempty cell star,
the owner is total and is one of its residents.

For partition `p`:

```text
resident_p(e) <=> p is in residents(e)
owned_p(e)    <=> resident_p(e) and owner(e) == p
ghost_p(e)    <=> resident_p(e) and owner(e) != p
```

An entity is a **partition-boundary entity** exactly when it has more than one
resident partition. This term is deliberately distinct from a physical FSI
interface. A physical interface facet may lie within one process partition,
and a process-boundary facet need not separate two materials.

Each local stratum retains canonical global `MeshEntity` identities. Its
derived local order is:

```text
ascending owned global entities ++ ascending ghost global entities
```

No caller supplies a local-to-global permutation. The first slice retains
owned top cells and owned/ghost vertices, facets, and general lower strata. It
does not require ghost top cells because the admitted local operators are
cell-local and the complete cross-cell incidence remains available from the
accepted mesh revision.

### Mesh-bound identity

A domain-separated `DistributedMeshLayoutIdentityV1` covers at least:

- the exact opaque 32-byte mesh revision identity;
- topological dimension and entity counts;
- partition count and canonical cell-owner claims;
- every derived per-partition owned/ghost stratum; and
- every derived partition-boundary entity and entity exchange.

Counts use checked portable encoding. Construction owns all derived arrays,
so a caller cannot supply a layout whose identity agrees while its global
numbering differs.

The L2 constructor binds an accepted mesh to those fixed identity bytes; it
does not interpret or authenticate an artifact digest. The ordinary L3 FSI
composition path supplies the bytes only after replaying the existing model,
Geometry Identity, geometry--mesh correspondence, and mesh-envelope
validation from RFC 0049 and RFC 0050. A stale mesh, geometry, or
correspondence therefore fails before ownership or assembly. This keeps
`eqiora-realization` and `eqiora-artifact` outside the new L2 dependency graph.

## Three distinct communication contracts

The word *halo* must not merge three different relations.

### Entity exchange

Every ghost entity produces exactly one immutable owner-to-receiver record:

```text
(entity owner, receiver, entity dimension, ascending global entity IDs)
```

Records are ordered by owner, receiver, dimension, then global identity. This
is the entity-residency plan used to verify complete local closure. The first
slice does not claim distributed mesh loading or that coordinates originated
only on their owner.

### Assembly row reduction

Cell-local matrix and RHS contributions travel from their unique cell
producer to the unique owner of each global equation row. This is a reduction
of assembly contributions, not a vector halo. Its order is specified below.

### Solver vector halo

After owned rows have finalized, remote column indices induce the existing
`eqiora-distributed::LocalLayout` and `HaloPlan`. Those values are derived
from CSR sparsity and algebraic DOF ownership under RFC 0010. No mesh entity
exchange is converted into a solver halo by name or assumption.

The three plans may share partition IDs and agreement lineage. They do not
share a payload type or independently configurable index list.

## Target row ownership from actual test support

One distributed cell-assembly plan binds:

- the exact distributed mesh layout identity;
- one existing nonempty ordered `AssemblyPlan`;
- the invariant that packet index equals canonical cell index;
- the existing pure `AssemblyWork`, whose packet count must equal the
  top-cell count.

It does not accept a target row partition. Row ownership is derived from the
equation support already present in the actual `TargetAssemblyMap` values.
For target `t` and global row `r`, define:

```text
support_t(r) = {
  cell packet i |
  packet i has target t and its equation map contains global row r
}

owner_t(r) = minimum { owner(cell_i) | i is in support_t(r) }
```

Every row in the declared target dimension must have nonempty support. A row
with no candidate fails before routing or shard mutation. Duplicate
occurrences within one packet or across packets do not create multiple
owners; they contribute to the same support set and the minimum is unique.

This rule follows the actual test space without interpreting physics. In the
existing FSI maps, shared velocity rows occur in the fluid/solid packets that
test them, fluid pressure rows occur only in fluid packets, each MINI bubble
row occurs in its one fluid-cell packet, and full fixed rows occur wherever
the full map retains their equations. Reduced essential rows are absent from
the reduced target rather than represented by an invented owner.

The loopback backend evaluates every cell packet exactly once and caches the
validated packets. It derives all target row owners from those cached maps
before constructing any row route. It never calls a local operator again to
discover ownership.

The MPI backend evaluates only locally owned cell packets and caches them. It
forms one local owner-candidate array per target, using an out-of-range
sentinel for unseen rows and the local `PartitionId` for rows present in its
owned packet maps. An elementwise integer `MIN` collective derives the same
global owner array on every rank. Rows that remain sentinel fail collectively
before route payload exchange. The derived owner arrays are covered by the
assembly-plan/receipt agreement identity.

The collective owner matrix is not trusted merely because its shape and range
are valid. Every producer must issue an opaque local-route admission proving
both that (a) the admitted owner is no greater than each locally supported
candidate and (b) a row owned by that producer is actually supported there.
Once one such admission exists for every producer, these two facts prove that
the collective result is exactly the minimum supported producer. A target may
legitimately have no rows on one or more cell-owning partitions; assembly-row
ownership therefore does not reuse the later algebraic `Partition` contract,
which currently requires every solver partition to own an entry.

## Owner-routed deterministic assembly

### Producer and destination

For cell packet `i`, the producer is the exact owner of cell `i`. Only that
partition evaluates the packet. The adapter cannot supply a second packet
producer map.

For every target mapping and every present local equation row:

```text
destination = owner_target(global_equation_row)
```

Each `(packet, target, global row)` projection forms one route to that row's
owner. Its columns retain global DOF identities, including columns owned by
another partition. Fixed columns retain the same exact finite values and RHS
elimination rule as reference assembly.

No matrix entry is routed by column owner and no cell is evaluated on every
resident partition. Row ownership makes the destination unique while the
packet projection preserves the complete contribution to that row.

### Accumulation order

The existing reference assembler first forms each packet's sparse entry/RHS
deltas in deterministic local order and then accumulates packets by increasing
logical index. Distributed assembly preserves exactly that expression tree:

```text
target ordinal
  -> owner-local global packet index
       -> existing per-packet sparse delta fold
```

Transport arrival order, rank scheduling, and message batching are not
mathematical order. An owner validates its complete inbox, sorts by the sealed
route ordinals, and only then mutates its row-subset accumulator. Duplicate,
missing, wrong-producer, wrong-target, wrong-destination, or out-of-order
logical routes fail without exposing a partial shard.

The owner-row accumulator in `eqiora-spatial-distribution` consumes the exact
`AssemblyDelta` emitted by `eqiora-assembly`. It therefore shares constraint
elimination, local duplicate folding, finite checks, and the per-packet
expression tree with complete reference assembly. It adds only packet-ordered
owner-row reduction and final zero filtering. A separate MPI map/scatter
algorithm is forbidden.

### Result and receipt

Successful execution yields, for every target:

- one owned-row shard per declared partition;
- complete unique global row coverage;
- strictly ordered global columns and finite values/RHS;
- an ordered row-reduction plan;
- a complete reconstructed CPU `LinearSystem`; and
- its exact property-free storage identity over dimensions, canonical CSR,
  and right-hand-side bits.

One fixed-size `DistributedAssemblyReceiptV1` binds the mesh-layout identity,
target shapes, derived target row-owner arrays, packet ownership, entity
exchanges, the complete sealed row-route inventory and payload identities,
exactly one opaque local-route admission per producer, reconstructed
property-free system identities, and the complete accepted execution
topology. Operator properties remain with the solver handoff; the assembly
layer neither accepts nor invents them. The receipt is internal
in-memory execution evidence used before the backend returns an ordinary
`AssemblyResult`, not a durable artifact, a second assembly return type, or a
replacement for Run provenance.

## FSI composition and complete reconstruction

The fixed-reference FSI path currently creates one cell packet per triangle
and maps it to two targets:

```text
target 0: reduced free-DOF monolithic system used by the solve
target 1: complete uneliminated system used by residual/reaction evidence
```

Both targets participate in distributed assembly and exact reconstruction.
Comparing only the reduced solve target would not prove that reaction and
physical-interface action evidence survived the partition.

The existing `AssemblyBackend` trait remains the only numerical integration
boundary. The loopback and MPI implementations are ordinary
`AssemblyBackend` implementations: `assemble(plan, work)` validates the
cell-indexed shape, caches the cell-owned packets, derives target row owners,
forms and verifies owner shards, reconstructs the complete targets, and only
then returns the existing complete `AssemblyResult` with distributed
`AssemblyReport` placement.

The current FSI finalizer, layout, local operators, and result acceptance do
not gain a public prepare/finish API. They continue to create the same
`AssemblyPlan` and `AssemblyWork` and consume the same `AssemblyResult` through
the already implemented explicit-backend path. The private discrete-block
wrapper checks the same work and returned materialization for reference,
Rayon, loopback-distributed, and MPI-distributed backends; no adapter receives
a public physics block IR.

The reconstructed reduced and full CSR indices, floating-point bits, and RHS
bits must equal independently executed CPU reference assembly. The reduced
fingerprint must equal the sole CPU-finalized operator fingerprint. The
ordinary loopback evidence then solves the reconstructed reduced system on
the existing host reference backend and passes the result through the
unchanged FSI residual, incompressibility, kinematic, interface-action, and
energy acceptance path.

The reconstructed primary system, exact row-owner array, and accepted receipt
are the in-memory inputs available to RFC 0061. This slice deliberately stops
before constructing a `DistributedLinearSystem`, deriving its solver-vector
halo, or executing distributed MINRES. RFC 0061 owns that later composition
gate; it must consume the checked output of owner-routed assembly rather than a
separately assembled or repartitioned operator.

## Loopback before MPI

### Transport-neutral oracle

The loopback executor simulates every declared partition in one process. It
must evaluate each packet exactly once under its cell owner, cache every
validated packet, derive all target row owners from the cached equation maps,
execute only the resulting row routes, accumulate each owner inbox in
canonical order, and reconstruct the complete systems solely from accepted
owned-row shards.

The same exact FSI fixture runs with one, two, and four partitions. The
four-partition ownership assigns every partition at least one fluid cell so
the existing nonempty algebraic partition contract remains intact. The
fixture deliberately places both physical FSI interface facets across process
partitions.

Loopback evidence closes the typed contract, cancellation-sensitive
determinism, complete-interface preservation, and CPU reconstruction before
an MPI feature is introduced.

### MPI adapter

Only after loopback passes may `eqiora-backend-mpi` consume the admitted local
and route views. The adapter:

1. agrees the fixed-size mesh layout, assembly shape, and packet-count identity
   before variable-sized communication;
2. evaluates and caches only the current rank's owned cell packets;
3. synchronizes local preparation or numerical failure before any rank can
   advance to row-owner derivation;
4. computes local target-row candidates and performs the elementwise integer
   `MIN` collective;
5. collectively rejects every sentinel row and agrees the resulting derived
   row-owner identity;
6. forms and exchanges the exact L2 route payloads to those row owners;
7. presents each complete owner inbox to the L2 canonical-order fold;
8. gathers owned shards for bounded complete-CPU reconstruction;
9. agrees the exact typed receipt identity before returning the complete
   `AssemblyResult`.

The registered evidence, outside the adapter, independently compares that
result with complete CPU reference assembly and then executes the unchanged
host FSI acceptance path.

MPI communicators, datatypes, requests, implementation/version, and provided
thread support remain in the L3 adapter and Run provenance. The MPI adapter
does not inspect a facet as a physical interface, infer Field support,
independently select a row owner, change an assembly mapping, or choose a
solver.

## Failure rules and falsifiers

The first implementation fails closed for at least:

- duplicate, missing, non-top-dimensional, or out-of-range cell ownership;
- a partition with no owned cell under the v1 contract;
- rank-local disagreement in mesh revision identity, ownership, entity layout,
  or plan identity;
- stale model, geometry, mesh, or correspondence at the ordinary FSI
  composition boundary;
- an unbound packet set or a same-cell-count packet set from another mesh;
- caller-supplied or mutated local/global numbering;
- a ghost entity without exactly one owner-to-receiver exchange;
- loss of a physical FSI interface facet, one of its adjacent material cells,
  or its exact global orientation/incidence;
- target shape, partition count, scalar type, local candidate, collective
  row-owner, or missing-row disagreement;
- a collective row owner below unsupported space or above a supporting
  producer, or a missing/duplicate local-route admission;
- evaluation of a cell packet by a non-owner;
- a missing, duplicated, wrong-producer, wrong-target, or wrong-destination
  row route;
- an unowned, multiply owned, empty, malformed, or non-finite global row;
- a rank returning an error while a peer can enter a later exchange or
  collective;
- reconstructed CSR/RHS or canonical fingerprint drift; and
- different floating-point output under different legal transport arrival
  orders in the reproducible policy.

Tests must contain cancellation-sensitive shared entries; equal results on a
strictly positive fixture alone do not falsify arrival-order accumulation.

## Verification

The transport-neutral registered case proves:

1. the exact existing accepted 2D affine-simplex FSI mesh and correspondence
   reach the ordinary checked local-operator path;
2. one, two, and four cell partitions derive complete owned/ghost vertices,
   facets, partition-boundary entities, and entity exchanges;
3. both physical FSI interface facets remain complete when split across
   partitions;
4. each cell packet is evaluated exactly once by its owner and its validated
   target maps are cached rather than replayed for ownership discovery;
5. every reduced and full row owner is derived from the minimum owner of the
   cells whose actual equation maps contain that row, while an unsupported row
   fails before routing;
6. reduced and full targets reconstruct byte-for-byte equal CSR and RHS values
   to independent CPU reference assembly;
7. route arrival permutations leave cancellation-sensitive results
   bit-identical;
8. the reconstructed reduced operator retains the CPU canonical fingerprint;
   and
9. the gathered host solve passes the unchanged coupled FSI acceptance.

A separate environment-dependent registered MPI case uses the same admitted
plan at one, two, and four ranks on one host. It verifies physical row routing,
synchronized failure, complete shard gathering, CPU reconstruction, and final
receipt agreement. The MPI case does not inherit physical multi-node,
performance, or scaling claims from earlier algebra evidence.

The capability matrix names distributed spatial ownership/assembly separately
from generic distributed algebra and from MPI FSI solve. The loopback and MPI
case manifests remain the authoritative evidence registry.

## Alternatives considered

### Add mesh ownership to `eqiora-distributed`

Rejected. That crate owns algebraic vector and operator distribution under
RFC 0010. Cells, facets, physical interfaces, and mesh incidence would widen
the transport/algebra contract into a second meshing authority.

### Split ownership between `eqiora-meshing` and `eqiora-assembly`

Rejected. Mesh-local views and row-owner routing could each be implemented in
their apparent source crate, but no single contract would own the identity
binding between them. Every consumer would have to reproduce the composition
and its stale-input checks.

### Keep the implementation private to `eqiora-numerics`

Rejected as the completed design. It is sufficient for a one-process
experiment, but the L3 MPI adapter cannot consume an L3 sibling without either
an illegal dependency or duplicated ownership/routing semantics.

### Assemble globally and partition the resulting CSR

Rejected. This reuses the already verified distributed algebra path and
cannot falsify duplicate local evaluation, missing interface contributions,
or order-dependent owner reduction.

### Accept ownership for every entity stratum

Rejected. Vertex, facet, and interface owner inputs can contradict cell
incidence and each other. Cell ownership plus a closed deterministic
lower-entity rule has fewer states and one authority.

### Accept a target-row `Partition` from `eqiora-numerics`

Rejected. The actual `TargetAssemblyMap` equation support is already the
authority for which cells test each row. A separately supplied partition
would duplicate that support as physics-specific layout data, could drift
from reduced/full constraint maps, and would make the generic backend depend
on a caller's independent ownership choice. Deriving the minimum supporting
cell owner has one input authority and fails closed for unsupported rows.

### Accumulate in message arrival order

Rejected for the reproducible policy. It makes network scheduling part of the
floating-point model. A later fast policy requires separate numerical and
evidence gates and cannot weaken this result.

### Define one universal halo type

Rejected. Entity residency, assembly row reduction, and solver vector ghosts
have different derivations, payloads, and failure modes. Sharing a name would
hide rather than remove those distinctions.

## Compatibility and nonclaims

This RFC changes no Semantic Kernel node, Standard Ontology meaning,
canonical language, package contract, Geometry Identity, mesh artifact wire,
Realization envelope, Run manifest, or existing CSR agreement identity. It
does not add a portable execution-graph node: assembly placement remains
separate `AssemblyReport`/receipt evidence before the finalized operator enters
the RFC 0058 execution DAG.

The first verified numerical claim is one fixed, conforming, intrinsic-2D,
affine-simplex monolithic FSI fixture with the existing MINI-fluid/P1-solid
spaces and reproducible `f64` assembly. Runtime-dimensional entity derivation
does not imply verified distributed PDE assembly in other dimensions,
methods, element families, or physics.

The slice does not claim:

- adaptive repartitioning, load balancing, ALE, or remeshing;
- distributed mesh generation, distributed CAD, sharded mesh persistence,
  parallel input/output, XDMF/HDF5, or checkpoint/restart;
- a distributed result `Field` or durable distributed assembly artifact;
- MPI MINRES, MPI FSI solve, general distributed nonlinear execution, or
  solver scalability;
- GPU assembly, CUDA FSI, GPU-aware MPI, multiple GPUs, or MPI plus CUDA;
- process-failure recovery, elastic process groups, fault tolerance, or
  topology optimization; or
- performance, memory-scaling, multi-node bridge, or production
  preconditioning claims.

In particular, this RFC implements neither the fixed-mesh CUDA path in
RFC 0062 nor the MPI FSI bridge in RFC 0061. It produces the exact
transport-neutral prerequisite that RFC 0061 must later consume.

## Security, safety, and governance

All counts, products, route extents, and receive capacities are checked before
allocation or communication. Fixed-size agreement precedes variable-sized
MPI traffic. A failed packet or malformed route exposes no partial accepted
shard. Native handles remain in the adapter, and no unsafe transport buffer is
allowed to manufacture an L2 receipt without replaying its complete shape and
identity.

The new L2 same-layer dependency exceptions and any facade exposure require
explicit review. A future wire, partitioner, fast reduction, field family, or
device path is a capability change with its own falsifier and registered
evidence; it cannot enter by adding an optional payload to this contract.

## Unresolved questions

None for the bounded slice. Durable distributed mesh/assembly artifacts,
nonreplicated mesh ingestion, fast accumulation, and adaptive repartitioning
are deliberately deferred until independent consumers make their exact
requirements concrete.
