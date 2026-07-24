# Vertical-slice development

Eqiora optimizes for completed, falsifiable capabilities rather than lines,
commits, or simultaneously open branches. The smallest unit of delivery is a
closed vertical slice:

```text
bounded semantic claim
  -> typed contract
  -> lowering
  -> realization or adapter
  -> positive oracle and meaningful falsifier
  -> registered case.toml evidence
```

An implementation that stops earlier is useful work, but it is not a completed
capability. A verification fixture that bypasses the ordinary path is not a
vertical slice.

## Definition of done

A capability-changing pull request is complete only when all applicable items
below are true:

1. The narrow claim and important non-claims are explicit. One successful
   fixture cannot imply a broader dimension, field shape, backend, package,
   hardware, or performance claim.
2. The smallest typed contract that owns the invariant exists at the correct
   layer. Meaning is not reconstructed in an adapter or client.
3. One ordinary end-to-end path reaches an accepted result through lowering
   and realization.
4. Evidence has an independent positive oracle and at least one falsifier that
   would reject a plausible wrong implementation. Failure occurs before
   mutation or artifact acceptance where the contract is fail-closed.
5. A validated `verify/<area>/<case>/case.toml` registers the exact claim,
   reference strategy, structured evidence target, and claim boundary. The
   adjacent README explains only details that cannot be expressed by the
   manifest.
6. `docs/capability-matrix.md` states the same narrow product boundary. The
   machine-readable capability-to-evidence index is derived with
   `eqiora-verify index`; it is never transcribed into another registry.
7. The fast gate passes during development, the affected gate passes before
   integration, and any unavailable environment-dependent evidence is named
   as a limitation rather than treated as success.
8. Public API and new abstractions pass the budget below. Compatibility and
   migration effects are explicit.

Pure refactors need no new case, but they must preserve the registered claim
and run its existing evidence when the path is affected.

## Parallelization boundary

Central contracts have a high fan-out and therefore stay deliberately small,
reviewed, and normally serialized:

| Central boundary | Primary paths | Required review responsibility |
| --- | --- | --- |
| Semantic identity and canonical model meaning | `eqiora-core`, `eqiora-schema`, `eqiora-graph`, `eqiora-sem` | semantic contract owner |
| Language hierarchy, names, and elaboration | `eqiora-lang`, `eqiora-compiler` | language and semantic contract owners |
| Physical Port and field-interface meaning | schema, semantic, compiler, and the governing RFC | physical-interface contract owner |
| Artifact, package, version, and provenance wires | `eqiora-artifact`, `eqiora-package` | artifact/package contract owner |
| Cross-layer realization and capability admission | `eqiora-realization`, solver/device/distributed contracts | realization contract owner |

During bootstrap these responsibilities resolve to the bootstrap maintainer in
`CODEOWNERS`; they are responsibilities, not permission to bypass RFC or
evidence review. A central change should establish one invariant and its
conformance kit before downstream work fans out. A downstream feature should
not edit a central crate merely to obtain a convenient API: first show why the
existing contract cannot express the capability.

Once a contract is stable, the following lanes are independently owned and may
proceed in parallel:

- data and mesh adapters;
- solver, execution, and hardware adapters;
- model and physics packages;
- Python bindings and ergonomic APIs;
- Studio projections and workflows; and
- examples, benchmarks, and verification fixtures.

Data/mesh adapters, Python, and Studio are three immediately independent
outward lanes with disjoint primary paths. Each lane closes one registered
vertical slice without weakening a central contract. Cross-lane integration
happens only after each lane's affected gate passes.

### Contract-wave integration loop

The bootstrap loop has one primary AI integrator, but it need not have only one
active implementation. Parallelism expands after an invariant has one accepted
owner:

```text
contract cell
  -> reference path, oracle, and falsifier
  -> accepted contract revision
  -> disjoint adapter, backend, package, Python, Studio, and evidence lanes
  -> one integration queue
```

1. The integrator chooses a bounded claim, names its invariant owner, and
   identifies every central surface it may change.
2. One writer owns each affected central seam. An independent verifier derives
   a plausible wrong implementation and the falsifier that must reject it,
   preferably before reading the implementation explanation.
3. A central change closes one reference vertical slice and is accepted before
   dependent implementation fans out. A types-only foundation branch is not a
   contract freeze point.
4. Disjoint consumer branches start from the exact accepted contract revision.
   Writable work uses a separate worktree per mergeable slice; branch identity
   follows the slice, not the agent. Sibling branches do not merge one another.
5. The integrator reviews the contract, diff, independent verification, and
   abstraction budget; rebases the slice on the current integration head; runs
   the affected gate and explicit semantic cases; and merges and removes the
   branch promptly. Agent-reported completion is not acceptance evidence.

Read-only design, prior-art, oracle, and adversarial audits may scale beyond
writable lanes. More writers are added only for paths that consume a frozen
seam without redefining it. Semantic merge authority remains singular.

### Slice assignment

An Issue records a closable product claim, not an agent roster or permanent
project plan. Before writable work starts, its body or implementation prompt
should identify:

- the predecessor and exact starting revision;
- the bounded claim, important non-claims, and invariant owner;
- the primary writable paths and central paths that must not change;
- the ordinary execution path and existing contract being consumed;
- an independent positive oracle and plausible wrong implementation;
- the required falsifier, registered case, and capability-matrix row;
- any environment-specific limitation; and
- the condition that stops the slice when another public abstraction, wire, or
  central seam becomes necessary.

Do not copy this information into another machine-readable planning registry.
The roadmap owns durable dependency order, Issues own transient closable work,
and registered evidence owns executable claims.

## Conformance kits

A central contract should publish the smallest reusable fixture and assertion
vocabulary needed by at least two consumers. A kit names invariants and failure
modes; it does not encode one adapter's implementation or numerical oracle.
Its fixtures remain under `verify/`, and its consuming cases declare the kit in
their manifests. A private shared test helper is preferred until independent
external consumers justify a public conformance crate.

An RFC's verification section maps its acceptance invariants to registered
cases and kit IDs. Adding an enum variant or adapter name without such a path
does not extend conformance.

## Abstraction and public-API budget

Every new crate, public type, enum variant, trait, wire field, or registry must
answer these questions in review:

1. Which invariant has exactly one owner after this addition?
2. What are the two real consumers? If there is only one, why is a private
   implementation insufficient?
3. Which existing type or branch can be removed or made simpler?
4. Can the concept travel along the existing meaning → lowering → realization
   → adapter → evidence path without a new layer?
5. What rejects an unknown value, unsupported combination, or stale mapping?
6. Is it semantic identity, execution provenance, or presentation state? It
   must not silently occupy more than one of these roles.
7. What compatibility promise and deletion condition does the API create?

A small crate is not automatically a good boundary. An abstraction that only
renames data, duplicates configuration, or moves branching to every consumer
does not meet this budget.

## Issue queue discipline

Issue intake is continuous, while implementation remains dependency-ordered.
Refresh the complete open queue at three natural boundaries:

1. before selecting the next vertical slice;
2. after accepting a central-contract or authoritative dependency-spine
   change; and
3. immediately before integrating a completed slice.

Classify each newly observed Issue before changing course:

- **prerequisite** — it corrects or invalidates an invariant required by the
  active slice; stop at a clean boundary and resolve it first;
- **urgent fault** — it reports a credible security, correctness, or data-loss
  risk in an accepted path; triage immediately under the repository's normal
  evidence and exception rules;
- **later dependent** — place it at the earliest gate whose predecessors own
  all required contracts;
- **independent outward lane** — it may proceed in parallel only after its
  central contract is stable and its primary paths do not overlap; or
- **duplicate or broad tracker** — link the existing Issue and move only the
  smallest independently closable claim into implementation.

Creation time and Issue number are never priority signals by themselves. Do
not reopen a closed predecessor merely to absorb adjacent scope, and do not
create a universal Issue when the roadmap already supplies an owning parent.
Record a dependency change in `docs/roadmap.md`; keep transient queue state on
GitHub rather than copying the open Issue list into repository documents.
This audit is a transition check, not a calendar ceremony or activity metric.

## Branch and integration discipline

Branches are short-lived and scoped to one slice or one independent outward
lane. A high-fan-out predecessor merges before its consumer branches begin;
composition evidence begins only after all of its parent slices are accepted.
Rebase the current integration head once before final local verification. The
integrator records the exact local commands and limitations, merges only a
passing affected closure, and deletes the merged branch. Required hosted checks
validate the exact proposed merge on protected public branches; they complement
rather than replace the repository-owned local acceptance decision.

There is no calendar review or activity ledger. Revisit the development model
only when an operational anomaly appears:

- the same verification failure occurs twice in succession;
- central-contract rework repeatedly invalidates downstream work;
- parallel lanes produce increasing merge conflicts;
- the same revision receives redundant broad hosted verification;
- a ready slice waits behind integration for longer than one broad gate;
- a plausible mutant survives the registered falsifier;
- one slice expands into more than one independently owned central seam;
- public APIs or crates grow faster than completed vertical slices; or
- evidence registration itself becomes the implementation bottleneck.

The review ends when the concrete recurring cause is removed. It must not add
standing reports, meetings, dashboards, or activity metrics to otherwise
healthy flow.
