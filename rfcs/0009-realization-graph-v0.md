# RFC 0009: Realization Graph v0

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Eqiora represents discretization, numerical solution, and deployment choices
as a typed Realization Graph that refers to—but cannot redefine—a separately
revisioned Semantic Model.

## Motivation

The same canonical Poisson relation must admit FEM, FVM, matrix-free, CPU,
GPU, offline, and real-time realizations without duplicating its equation,
unit, domain, or boundary meaning. Conversely, changing a boundary condition
must be a Semantic Model revision, not an innocent-looking solver option.

An untyped configuration map does not preserve that boundary. It permits a
model period, mesh size, physical unit, RTOS priority, and solver tolerance to
appear as interchangeable keys and makes contradictory selections fail late.
Embedding realization fields in every semantic node has the opposite failure:
one mathematical model becomes inseparable from a method and target.

## Proposed design

### Ownership boundary

The two layers share typed identity and revisions, not payload ownership:

| Semantic Model owns | Realization owns |
|---|---|
| equation and implicit residual | discrete method |
| Domain and boundary meaning | mesh policy and resolution |
| Field meaning, shape, and SI dimension | discrete function space |
| Parameter value and physical unit | quadrature policy |
| signal/conserving connection semantics | solver algorithm and tolerances |
| Activation and exact model-time ClockDomain | hardware target and deployment schedule |

A realization payload has no field for equations, quantities, boundary
conditions, model periods, event guards, or connection laws. It may identify
the semantic model it realizes and, once graph storage is added, individual
semantic entities through typed cross-graph edges.

### Pure v0 components

`eqiora-realization` is an L2 crate containing pure, immutable value contracts:

- `Space` declares an approximation family and order.
- `Discretization` declares method, mesh policy, and explicit quadrature.
- `SolverPlan` declares algorithm, finite tolerances, and an iteration limit.
- legacy `Target` declares compatibility deployment capacity; portable
  placement requirements contain resource counts but no environment-local
  device ordinal.
- `ExecutionSchedule` declares only offline or deployment-time
  priority/deadline constraints.
- `RealizationPlan` cross-validates those siblings without modifying any
  Semantic Kernel definition.

The L2 policy crate intentionally does not own a general graph store, wire
schema, or canonical numerical lowerer. RFC 0013 provides the separation point
for frozen compatibility wire DTOs in the L3 artifact crate. RFC 0058
supersedes the provisional claim that one tuple-shaped plan is itself the
complete graph: accepted compatibility plans now normalize after resolution
into a small typed portable DAG before environment binding.

### Revision provenance

`RealizationRequest` contains a typed `OntologyId<Model>` and a
`SemanticRevision`. An explicit plan additionally has a
`RealizationRevision`. The revision types are distinct newtypes and are not
implicitly convertible. A named default has no fabricated Realization Graph
revision; its provenance is a `DefaultPolicyVersion`.

This distinction permits, for example:

```text
semantic model M at semantic revision 41
  ├── default-policy/v0 → P1 FEM plan
  ├── realization revision 7 → the same explicit P1 FEM plan
  └── realization revision 8 → a cell-centred FVM plan
```

All three retain the same model identity and semantic revision. Numerical
evidence records which selection source was used.

### Selection and failure rules

Resolution is deterministic:

1. Resolve the exact requested default-policy version or take the exact
   explicit plan. Unknown default versions are errors.
2. Validate cross-component invariants. In v0 continuous Galerkin requires a
   continuous Lagrange space and Gauss--Legendre quadrature; cell-centred FVM
   requires a cell-constant space and centroid quadrature.
3. Check the declared method, target, and scheduling capabilities of the
   concrete lowerer/backend.
4. Return the plan and provenance, or one stable diagnostic.

An invalid or unsupported explicit plan never falls back to the default. This
is essential: fallback would produce a valid-looking artifact for an execution
the user did not request.

Default policy v0 is generated uniform-mesh P1 continuous Galerkin FEM, two
Gauss--Legendre points per axis, conjugate gradients, host CPU, and offline
execution. Mesh resolution and tolerances are realization data. The current
1D numerical lowerer remains responsible for proving that it can execute this
plan; the policy does not widen that lowerer's dimensional claim.

### Clock and schedule separation

A `ClockDomain` answers when a relation is activated in model time. An
`ExecutionSchedule` answers how an already-derived deployment task is assigned
priority and deadline. A deadline is not a model period, and a clock tick is
not an RTOS priority.

The v0 crates expose no conversion or shared builder between these types.
Future lowering may derive task constraints from clocked partitions, but that
derivation must produce a separate Realization Graph revision and must not add
deployment facts to the Semantic Model.

## Alternatives considered

### Put mesh and solver fields on Semantic Kernel nodes

This simplifies one end-to-end solver but makes method changes alter model
meaning and prevents one model revision from supporting comparable FEM/FVM
evidence. Rejected.

### Use a string-keyed options map

This is extensible at the syntax level but cannot make revision kinds,
clock/schedule ownership, or method/space compatibility type-visible. Unknown
keys and implicit fallback also weaken reproducibility. Rejected as the
canonical contract; adapters may translate external option maps into validated
typed values.

### Make the current lowered `(x,z,q,...)` form the realization contract

That form is useful compiler output but classifies state and execution too
early and does not naturally own mesh, space, and deployment choices. Rejected
as the graph-level contract.

### One graph containing model, plan, artifacts, and actions

Shared IDs are convenient, but mutation history and evidence would then appear
to be mathematical meaning. Rejected in favour of Graph Federation layers with
typed cross-graph references.

## Compatibility and migration

This is a provisional v0 Rust API, not a stable wire schema. It adds no
Semantic Kernel nodes and does not change existing model source. The current
`solve_default_scalar_elliptic_1d` entry point can later become an adapter that
resolves default policy v0 and lowers the returned plan; its numerical behavior
must remain covered by the existing verification case during migration.

RFC 0008 owns artifact wire v1. That run manifest continues to refer to
realization data opaquely; this RFC does not retroactively change it. RFC 0013
adds a separate typed Realization envelope and run-manifest/v2 while preserving
the component and revision boundary decided here.

## Verification

- Resolve default-policy/v0 and an equal explicit FEM plan for the same model
  and semantic revision; require equal plans and distinct provenance.
- Resolve explicit FVM for that same model revision; require a different valid
  plan without semantic mutation.
- Reject continuous Galerkin paired with a cell-constant space.
- Reject an unsupported CUDA target without default fallback.
- Reject an unknown default-policy version without fallback.
- Compile-fail when a `RealizationRevision` is passed as a
  `SemanticRevision`, when a model period is assigned to `ExecutionSchedule`,
  or when deployment priority is assigned to `ClockDomainDef`.
- In integration, compile the canonical Poisson fixture once, resolve default
  FEM and explicit FVM, and require both existing convergence evidence paths to
  consume the same semantic model revision.

## Research basis

The separation is consistent with, but deliberately sharper than, established
interfaces:

- The [Modelica synchronous-language specification](https://specification.modelica.org/master/synchronous-language-elements.html)
  treats clocks as equation/variable activation semantics and notes that task
  placement can remain an implementation concern.
- [FMI 3.0 Scheduled Execution](https://fmi-standard.org/docs/3.0.2/)
  exposes model partitions associated with clocks while leaving their external
  activation schedule to the importer, illustrating a boundary between model
  partition semantics and deployment control.
- [PETSc KSP](https://petsc.org/main/manual/ksp/) treats relative tolerance,
  absolute tolerance, divergence tolerance, and iteration limit as solver
  policy rather than equation meaning.
- RFC 0006 records the method-neutral mesh, quadrature, local-operator, and
  assembly contracts consumed after realization selection.

## Security, safety, and governance

The prototype uses safe Rust and validates finite tolerance and compatibility
invariants before selection. Capability failure is closed: unsupported targets
and unknown policy versions are errors. No execution, graph mutation, or
artifact deserialization occurs in this crate.

Making a new default affects reproducibility and therefore requires a new
`DefaultPolicyVersion` and RFC review. Existing versions must not change in
place. A deployment adapter may interpret priority only under an explicit
target contract; v0 does not claim portable RTOS priority semantics.

## Unresolved questions

- Stable payload schema and graph transaction definitions for each Realization
  entity kind.
- Typed references for imported meshes and generated realization artifacts.
- Capability negotiation for mixed methods, adaptivity, nonlinear solvers,
  accelerators, distributed partitions, and matrix-free execution.
- The exact lowering that converts semantic ClockDomains into deployable task
  partitions while preserving, rather than conflating, both layers.
