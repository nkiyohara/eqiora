# RFC 0062: Single-device fixed-mesh fluid--structure interaction

- Status: Implemented and verified for the bounded single-device 2D slice
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0019](0019-device-execution-contracts.md), [RFC
  0050](0050-fixed-reference-monolithic-fsi.md), and [RFC
  0058](0058-portable-realization-and-execution-graphs.md)

## Summary

Eqiora executes the existing fixed-reference monolithic 2D FSI operator on
one CUDA device. The physics finalizer remains the sole producer of the
dimensionless symmetric-indefinite CSR system. CUDA contributes only an exact
execution capability, explicit residency and completion evidence, and an
accepted candidate returned to the unchanged host and FSI acceptance paths.

```text
one canonical FSI model and numerical realization
  -> CPU-target and CUDA-target portable realization graphs
  -> the same equation-aware FSI finalizer
  -> bit-identical finalized CSR/RHS fingerprints
  -> one-device identity-preconditioned Fast MINRES
  -> independent serial-host residual acceptance
  -> unchanged fixed-reference FSI finish
```

The CPU reference Realization keeps reproducible MINRES. The CUDA Realization
selects native-fast MINRES explicitly. These are different execution policies
and therefore different Realization identities, but neither changes the
operator, physical Fields, pressure closure, scaling, time elimination, or
mesh meaning.

## Motivation

RFC 0050 already closes one fixed-reference FSI step through a finalized CSR,
reference MINRES, and physical acceptance. RFC 0019 already closes explicit
single-device CSR residency, cuSPARSE action, cuBLAS Krylov vector operations,
waited fences, transfer evidence, and independent host residual acceptance.
RFC 0058 binds that CUDA adapter to a portable Realization graph before device
allocation.

Those paths do not yet compose because the CUDA provider admits only CG over
SPD systems and BiCGSTAB over general systems. Relabelling the FSI matrix as
general and running BiCGSTAB would discard an accepted mathematical property
and substitute the solver policy. Giving FSI a CUDA-specific assembly or
physical finish would duplicate the numerical and physics authorities.

This RFC adds the missing MINRES capability and the narrow typed composition.
It does not add a GPU physics implementation.

## Decision

### One operator, two explicit execution policies

The semantic and numerical inputs are common:

- exact Model, semantic revision, and authenticated mesh;
- fluid velocity and pressure plus solid velocity and displacement Fields;
- conforming velocity trace quotient;
- Backward Euler displacement elimination;
- coherent-SI symmetric congruence;
- boundary-determined pressure closure;
- the same reduced/full assembly maps; and
- `LinearOperatorProperties::SymmetricIndefinite` with `f64` coefficients.

The CPU reference Realization selects:

```text
MinimumResidual + SymmetricIndefinite + Identity + Reproducible + f64
HostCpu { threads: 1 } + Replicated + Offline
```

The CUDA Realization selects:

```text
MinimumResidual + SymmetricIndefinite + Identity + Fast + f64
CudaGpu { device } + Replicated + Offline
```

The FSI plan constructor and exact-plan validator admit these as two closed
tuples. They do not accept independent capability axes whose Cartesian product
would imply unverified combinations. In particular, CUDA Jacobi MINRES,
reproducible CUDA MINRES, multi-device placement, distributed layout, and
real-time scheduling remain unrepresentable through this path.

`Fast` is an honest numerical policy, not a temporary spelling. RFC 0019
classifies the current cuBLAS dot and norm operations as backend-native
reductions. Disabling atomics and observing repeatable results on one device do
not provide Eqiora's placement-independent named reduction tree. A future
reproducible device reduction is a separate capability and must not silently
change this Realization.

The two finalizations must have equal
`CanonicalCsrAgreementFingerprintV1` values. Exact fingerprint agreement is
the proof that CUDA receives the CPU-finalized algebra rather than a second
target-specific lowering. Solution bits, iteration counts, Realization
digests, and Run digests are not required to agree across CPU and CUDA.

### Exact CUDA MINRES capability

The existing CUDA Krylov provider adds exactly:

```text
SolverCapability {
    algorithm: MinimumResidual,
    operator_properties: SymmetricIndefinite,
    preconditioner: Identity,
    reduction: Fast,
    scalar_type: F64,
}
```

The provider identity remains `eqiora.cuda.krylov`; the sole `SolverPlan`
distinguishes MINRES from CG and BiCGSTAB. No `CudaMinresConfig` or
physics-specific solver wrapper is introduced.

The implementation uses its own device-resident short-recurrence Lanczos and
orthogonal-rotation algorithm. It does not inherit the CPU reference provider's
retained-basis, full-H projection or dimension cap. The finalized CSR, right-hand
side, candidate, residual and Krylov vectors reside on the selected device for the solve. cuSPARSE supplies the CSR
action and cuBLAS supplies the existing level-one vector operations and native
scalar reductions. Identity preconditioning allocates and transfers no
diagonal. The generic CUDA execution admission accounts for the exact MINRES
vector workspace before the adapter creates a context or allocates memory.

Every recursive residual remains operational evidence only. After apparent
convergence, the adapter copies the complete candidate to the host and the
existing solver acceptance path recomputes the true residual using the exact
captured CSR and fixed-order host reduction. There is no fallback to CG,
BiCGSTAB, another preconditioner, another reduction policy, or the CPU solver.

### Typed execution composition

The CUDA FSI bridge is the ordinary graph-bound execution path:

1. resolve the explicit CUDA-target FSI plan against the selected provider and
   device capabilities;
2. finalize the exact FSI CSR through the unchanged equation-aware assembly;
3. bind the portable graph to one selected device and logical queue slot;
4. admit that exact finalized CSR and solver plan before runtime allocation;
5. execute the admission through the isolated CUDA adapter;
6. accept the candidate and complete CUDA trace through the generic execution
   receipt; and
7. consume the accepted `LinearSolution` through the existing FSI `finish`.

The bridge may make this ownership sequence convenient, but it may not expose
a second operator, take a caller-supplied solver plan, reconstruct a CUDA
matrix, or duplicate physical acceptance. `finish` remains the sole path that
reconstructs velocity, pressure and displacement Fields and checks residual,
incompressibility, kinematics, interface action, and energy.

### Residency, completion, and provenance

The existing CUDA execution trace remains authoritative for:

- the exact row-offset, column-index, value, right-hand-side, initial-value,
  optional-preconditioner, and output transfer plans;
- the selected runtime device and process-unique materialized queue;
- device-value generations;
- physically waited input, solve, and output completions;
- admitted resident-payload lower bound and observed cuSPARSE workspace; and
- the complete accepted host output and graph receipt.

MINRES with identity preconditioning has no inverse-diagonal transfer. Its
additional Krylov vectors change the admitted resident-payload lower bound but
not the transfer vocabulary or execution DAG.

`ExecutionProvenanceV1` records the observed CUDA runtime, adapter and solver
provider versions, exact device name and compute capability, driver version,
reduction policy, cudarc, binding-toolkit, cuSPARSE and cuBLAS versions. A
`RunManifestV2` binds that observation to the CUDA Realization. The generic
execution receipt remains an in-memory proof; this RFC does not create a
durable receipt or a new artifact schema.

### CPU/CUDA conformance

The registered evidence first finalizes and accepts the CPU reference result,
then independently resolves and finalizes the CUDA target. Before device
execution it requires exact equality of the two CSR/RHS fingerprints. After
both paths pass their native residual and physical acceptance gates, the case
compares corresponding Field identities, supports, coefficient order and
length, then compares dimensionless and physical values under one documented
absolute-plus-relative tolerance:

```text
|cuda - cpu| <= absolute + relative * max(|cuda|, |cpu|)
```

The evidence records the exact constants. It does not infer a general error
bound, cross-device bit identity, or performance claim from this fixture.

## Failure rules and falsifiers

The slice fails closed for at least:

- a CPU/CUDA finalized operator or right-hand-side fingerprint mismatch;
- any CUDA-specific FSI assembly, scaling, pressure closure, or Field-order
  drift;
- a graph, target, device, queue, scalar, layout, schedule, operator-property,
  solver, preconditioner, reduction, tolerance, or iteration-limit
  substitution;
- an unsupported or stale device capability snapshot before allocation;
- malformed, non-finite, oversized, or unsupported CSR/index storage;
- an admitted resident-payload lower bound that omits MINRES workspace;
- an implicit or missing input/output transfer, wrong buffer identity,
  skipped generation, foreign queue, or unwaited completion;
- CUDA library failure, non-finite recurrence, Lanczos or rotation breakdown,
  iteration exhaustion, or false recursive convergence;
- a candidate that fails independent host true-residual acceptance;
- a candidate that passes algebraically but fails FSI incompressibility,
  kinematic, interface-action, or energy acceptance; or
- CPU/CUDA Field identity, support, shape, ordering, length, or tolerance
  disagreement.

No failure path may publish a partial accepted execution or silently execute
on the host.

## Alternatives considered

### Relabel the operator as general and use BiCGSTAB

Rejected. The accepted operator is symmetric indefinite and the FSI
Realization selects MINRES. Property and solver substitution would make CUDA a
different numerical realization while pretending otherwise.

### Advertise reproducible CUDA reductions

Rejected. The current device reductions use the vendor-native tree. cuBLAS
documents repeatability only under a narrower runtime, toolkit and device
shape boundary; that is not Eqiora's placement-independent reproducible
contract.

### Keep the host Realization and replace only its deployment binding

Rejected. A host placement requirement cannot honestly bind a CUDA device.
The CUDA target and Fast reduction are explicit Realization choices even
though the finalized algebra remains identical.

### Add CUDA-specific FSI lowering or finish code

Rejected. Target-specific physics code would create a second authority for
the interface, pressure, state elimination and energy balance. CUDA consumes
the finalized algebra and returns to the existing physical finish.

### Add a durable CUDA FSI receipt

Rejected for this slice. Existing typed Realization and Run artifacts already
record stable inputs and observed execution provenance. The in-memory graph
receipt is sufficient to prove the bounded execution without freezing a new
wire family.

## Compatibility

No Semantic Kernel, canonical Model, package, transaction, mesh, Realization
wire, Run wire, or Field artifact schema changes. Existing CPU, MPI, CUDA CG
and CUDA BiCGSTAB tuples remain exact and unchanged. The provisional Rust FSI
plan surface may add an explicit CUDA constructor while retaining the existing
CPU-reference constructor for source compatibility.

The CUDA dependency remains optional and isolated at L3. Default and MSRV
builds do not load CUDA. A build with the CUDA feature but without a selected
physical device compiles the contract tests and skips only the explicit
hardware case.

## Verification

1. Unit-test exact capability admission and reject MINRES with SPD/general,
   Jacobi, reproducible reduction, or non-`f64` substitutions.
2. Unit-test the exact MINRES workspace lower bound and existing CG/BiCGSTAB
   bounds.
3. Run a physical CUDA symmetric-indefinite solve and independently accept its
   host true residual.
4. Build CPU and CUDA FSI Realizations from the same Model, mesh and previous
   state; require exact finalized CSR/RHS fingerprint equality.
5. Exercise the graph-bound CUDA admission and require the exact transfer,
   generation, fence, workspace, report and output receipt.
6. Reject a locally coherent but fingerprint-substituted operator before
   physical execution.
7. Pass the CUDA result through the unchanged FSI finish and compare its
   normalized Fields with the independently accepted CPU result under the
   declared tolerance.
8. Register the hardware-dependent case and update the capability matrix
   without widening the claim beyond one fixed 2D mesh and one device.

## Nonclaims

This RFC does not claim GPU assembly, matrix-free FSI, transient FSI beyond the
single fixed-reference step, reproducible device reductions, CUDA-specific
model meaning, durable execution receipts, pinned or unified host memory,
multiple streams, multi-GPU, MPI plus CUDA, physical scale, performance,
checkpoint/restart, ALE, remeshing, shape sensitivity, or FSI adjoints.
