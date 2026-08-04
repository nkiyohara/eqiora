# RFC 0084: Contract-wave capability development

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-08-04
- Related: [AI-authored platform strategy](../docs/development/ai-authored-platform-strategy.md),
  [capability development guide](../docs/development/vertical-slice-development.md),
  and [RFC 0075](0075-fem-form-compiler-poisson-q1.md)

## Summary

Eqiora develops through dependency-ordered contract waves and accepts product
capabilities through ordinary-path evidence closure. Contract cells,
implementation lanes, evidence units, and capability closures are distinct:
parallel writers consume accepted seams, while every widened product claim
still reaches registered evidence and the capability matrix.

## Motivation

The current guide already serializes central contracts, closes one reference
slice, and then fans out disjoint consumers. It also separates compiler-class
conformance from per-instance witnesses. Its title and opening principle still
describe the whole method as vertical-slice development, which can be read as
requiring every writable lane to reconstruct a new meaning-to-runtime stack and
bespoke proof harness.

That reading does not scale under agent authorship. Writing another physics
module is cheap; independently auditing another formula, lowering, derivative,
and conformance harness is not. The accepted AI-authored platform strategy
therefore identifies separately hand-written capability paths as the wrong
growth curve and requires evidence to amortize where one derivation contract
genuinely owns a class.

Pure horizontal layer work is not an alternative. Without one ordinary path,
an agent can produce internally plausible contracts that never compose into a
falsifiable capability. This RFC changes the development vocabulary and work
boundaries, not the evidence required to widen a product claim.

## Proposed design

### Four distinct units

| Unit | Owner and completion condition |
| --- | --- |
| Contract cell | One invariant owner establishes the smallest central seam plus a reference capability path, independent oracle, and meaningful falsifier. It is normally serialized. |
| Implementation lane | One writer consumes an exact accepted revision in disjoint writable paths. The lane does not redesign its central seam and is not a product claim by itself. |
| Evidence unit | Evidence follows the owning contract. Compiler-derived instances may reuse class conformance and add an instance witness; adapters and execution providers keep their existing conformance form and exact capability-tuple evidence. |
| Capability closure | One bounded claim travels through the ordinary meaning-to-evidence path, registered evidence, and the capability matrix. It may reuse accepted contracts and conformance without creating a new full-stack architecture. |

The first capability path that accepts a central seam remains its **reference
slice**. The term is retained for that role and for historical capability
descriptions; it is no longer the name of every writable development unit.

### Contract-wave lifecycle

One writer owns each invariant-bearing central seam. A non-implementing route
precommits its oracle and falsifier where required. The central change closes a
reference capability path and merges before dependent consumers start. From
that exact revision, every lane with frozen decisions and disjoint writable
paths starts without waiting for unrelated lanes.

A fan-out consumer may only consume the accepted contract. If it discovers an
unexpressible requirement, it stops and returns the requirement to the contract
owner. It does not add a parallel DTO, validator, semantic interpretation, or
adapter-shaped central type.

### Integration boundary

Capability-directed implementation does not merge until capability closure. It
cannot be relabeled as a non-capability delta by omitting its claim, evidence,
or registration. Types-only foundations, dormant code paths, and partial
physics implementations are not integration envelopes.

Separately, a pull request may carry an accepted RFC or governance decision, or
one already admitted maintenance category that neither adds nor extends a
capability: dependency-only updates with their lockfile and relevant gate;
non-governance documentation; reproducible generated or mechanical output;
private behavior-preserving refactors; and localized corrections to reviewed
low-risk findings. Their existing risk review and affected-gate rules remain.

Integration-owned registration points remain singular. Parallel feature lanes
propose exact facade, dependency, registry, artifact-version, and claim deltas;
the per-envelope integrator reconciles them without granting any lane authority
to widen a shared seam.

### Evidence reuse boundary

Only a deriving compiler contract may split evidence into compiler-class
conformance and an instance witness. The compiler owns derivation rules, the
reference interpreter, mutant corpus, and primal/JVP/VJP consistency. Each
derived physics instance supplies the claim-specific manufactured or reference
solution, boundary data, norms, convergence order, conserved quantities,
tolerances, and nonclaims required by its registered case.

This split does not apply to execution providers or adapters. A new provider
capability tuple retains its own independent positive oracle, meaningful
falsifier, registered evidence, capability-matrix boundary, and recorded
environment. Reuse of a conformance kit never widens the tuple or turns one
environment's result into evidence for another.

## Alternatives considered

### Keep vertical-slice terminology as the universal label

Rejected. The detailed contract-wave rules are sound, but the universal label
conflates writable, evidence, and acceptance units and encourages repeated
full-stack work.

### Develop horizontal layers and verify only at system milestones

Rejected. Types and internal tests do not prove composition through the
ordinary product path. Central seams still require a reference capability path
before fan-out, and every widened claim still requires evidence closure.

### Replace per-claim evidence with broad class certification

Rejected. A compiler-class proof cannot establish that a submitted PDE is well
posed or physically appropriate. Provider correctness also depends on exact
operator, scalar, layout, method, target, topology, library, and environment
tuples. Class reuse is therefore limited to compiler-derived translation rules.

### Introduce one universal multiphysics or weak-form IR

Rejected. Audit reuse does not justify routing method-foreign meaning through
one abstraction. In particular, conservative finite-volume face fluxes remain
outside the FEM form compiler.

## Compatibility and migration

This RFC changes governance vocabulary and assignment boundaries only. It does
not change source syntax, Semantic Model meaning, public API, schema, artifact,
numerical formulation, oracle, tolerance, case manifest, or capability claim.
Existing accepted capabilities and historical uses of “vertical slice” remain
valid.

The existing `docs/development/vertical-slice-development.md` path is retained
so external and repository links do not break. Its title and owning references
change to “Contract-wave capability development.” Issues and new prompts use
the four units above; active branches do not need renaming.

## Verification

The governance delta is rejected if any of these mutants survives review:

- capability-directed code may merge before ordinary-path evidence closure;
- a provider tuple may substitute class conformance for its exact registered
  oracle, falsifier, environment, or capability boundary;
- a fan-out writer may redefine an accepted central seam;
- a capability claim may omit its registered case or capability-matrix update;
- the `AGENTS.md` 200-line ceiling, documentation links, or RFC index drift.

Repository verification runs the fast and affected local gates. This RFC adds
no `case.toml`: it changes governance and no executable or user-visible product
capability. A fresh-context non-writer reviews the complete governance delta
before integration.

## Security, safety, and governance

This governance change takes authority when its accepted policy delta merges.
Acceptance uses the public RFC process and the bootstrap decision rules in
`GOVERNANCE.md`. The writer cannot provide the required independent review of
this delta.

The decision grants no new merge, semantic, release, or evidence authority.
It preserves fail-closed admission, oracle independence, singular central-seam
ownership, and protected-branch checks.

## Deferred decision

- Renaming the existing guide file is deferred; preserving external links is
  preferred unless path ambiguity causes a measured failure.
