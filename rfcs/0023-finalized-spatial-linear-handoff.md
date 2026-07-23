# RFC 0023: Finalized spatial linear handoff

- Status: Implemented for Cartesian scalar elliptic reference paths
- Authors: Eqiora contributors
- Created: 2026-07-19

## Summary

A resolved spatial realization hands execution adapters one opaque finalized
problem containing its immutable linear system, asserted properties, exact
`SolverPlan`, assembly evidence, and private method-native reconstruction
state.

## Motivation

Q1 FEM and TPFA previously assembled, selected a backend, solved, and rebuilt a
field inside separate method functions. A device or distributed adapter could
consume the CSR only by duplicating canonical-plan logic or adding a
CUDA-shaped entry point to numerics. Both choices would couple mathematical
realization to placement.

The seam must not mistake an accepted report for durable problem provenance.
It independently reapplies the finalized operator and policy, rejecting a
borrowed vector only when that vector fails the current problem's numerical
acceptance. A vector satisfying two systems is valid for both; origin identity
requires a future artifact contract.

## Proposed design

`FinalizedScalarEllipticCartesianProblem` is opaque except for:

- `LinearSystem` and `LinearOperatorProperties`;
- the exact, sole `SolverPlan`;
- resolved method identity and `AssemblyReport`; and
- construction of a borrowed `LinearProblem`.

FEM retains eliminated-boundary and full-reaction state privately. TPFA
retains facet flux and dual-grid reconstruction state privately. `finish`
consumes an accepted `LinearSolution`, rechecks its exact plan, normal
orientation, iteration limit, producer topology, residual target, and true
residual against this finalized CSR/RHS, then reconstructs the method-native
field and balance. Host producers may use no more workers than the resolved
host target; a CUDA producer must name the exact resolved device. Verification
placement remains independent. These checks are independent numerical
reacceptance, not proof that this finalized problem originally produced the
vector.

`SolveReport` retains the complete `SolverPlan`, rather than only algorithm,
preconditioner, and reduction projections. This makes tolerance and iteration
identity testable without introducing a second solver-control type.

No vendor or runtime-library type enters canonical semantics, assembly,
numerics, or the public handoff. The handoff intentionally retains Eqiora's
own `Target`, and accepted evidence retains Eqiora's `ExecutionTopology`, so
placement policy can be checked without importing a CUDA or other vendor API.
Artifact wire remains unchanged.

## Alternatives considered

A generic `FinalizedLinearProblem<Reconstruction>` exposes method-specific
bookkeeping in the public type and makes heterogeneous orchestration harder.
A boxed reconstruction callback obscures state equality and validation. A
CUDA-specific numerical entry point couples placement to method lowering.
The opaque enum is the smallest boundary that retains native FEM/FVM recovery
while presenting identical algebra to every backend.

## Compatibility and migration

Existing one-call Cartesian solve APIs remain and now execute
`finalize -> backend -> finish`; verified values and reports are unchanged.
`SolveReport::accepted` now receives the exact `SolverPlan`. There is no
Semantic Model, Realization, artifact-wire, or CUDA API change.

## Verification

The `numerics.finalized-spatial-handoff` case proves exact one-call CPU
regression for Q1 FEM and TPFA. Its FEM/FVM systems have the same shape and
residual target, while the cross-wired vector fails a fresh residual against
the receiving system. It separately rejects different-plan and
wrong-producer-topology evidence. This fixture does not claim that every
cross-system vector must fail.

## Security, safety, and governance

The handoff contains immutable owned arrays and no raw pointers, callbacks,
vendor/runtime handles, or unsafe code. Eqiora-owned target and execution
topology values are intentionally retained. Widening it to nonlinear,
distributed-vector, imported-mesh, or artifact-persistent problems requires
separate evidence and an explicit compatibility review.

## Unresolved questions

- A general spatial problem family may later factor the common algebraic
  envelope from scalar-elliptic reconstruction, after a second physics family
  demonstrates the abstraction.
- Durable system identity and origin provenance for an in-flight finalized
  problem are deferred to artifact persistence and remain outside v0.
