# Contract-wave capability development

Eqiora optimizes for completed, falsifiable capabilities rather than lines,
commits, or simultaneously open branches. Development, evidence, and product
acceptance deliberately use different units. The default writable unit is a
bounded contract cell or disjoint implementation lane. The unit that may widen
a product claim is a closed capability path:

```text
bounded semantic claim
  -> typed contract
  -> lowering
  -> realization or adapter
  -> positive oracle and meaningful falsifier
  -> registered case.toml evidence
```

A **reference slice** is the first such path that freezes a central contract.
Downstream lanes consume that accepted contract; they do not create parallel
meaning or repeat accepted conformance evidence. A capability-changing lane
still reaches the ordinary path and adds the exact evidence required for its
new claim. A fixture that bypasses that path is not capability evidence.

An **outcome contract** freezes only what another layer, user, or verifier can
observe: the accepted result, at least one ordinary positive path, named
failure conditions, authority and identity boundaries where they matter, and
declared input and resource bounds. Landlock, seccomp, allocator behavior,
worklist lifetime, process layout, and other internal mechanisms remain
implementation choices unless the bounded product claim is specifically about
that mechanism. A contract may require fail-closed isolation or a resource
envelope without preselecting how the implementation supplies it.

## Working units

| Unit | Purpose | Boundary |
| --- | --- | --- |
| Contract-discovery lane | Resolve one frozen research question into an isolated contract candidate | No implementation, fixture, or tuned expected value; implementation remains stopped |
| Contract cell | Establish one invariant, owner, reference path, oracle, and falsifier | Normally serialized; not a fan-out assignment |
| Implementation lane | Consume frozen contracts in disjoint writable paths | Not a product claim by itself |
| Evidence unit | Attach evidence under the owning contract | Only compiler-derived instances split class proof from instance witness; providers retain exact-tuple evidence |
| Capability closure | Carry one bounded claim through the ordinary path into registered evidence | Does not require a new full-stack architecture |

An implementation that stops before capability closure can be useful branch
work, but it is not a completed capability and must not merge as dormant or
hidden capability code. A capability-directed implementation never becomes a
non-capability delta merely because it omits or defers its claim.

A pull request may separately carry an accepted RFC or governance decision, or
one of these bounded maintenance deltas that neither adds nor extends a
capability: a dependency-only update with its lockfile and relevant gate;
non-governance documentation; reproducible generated or mechanical output; a
private behavior-preserving refactor; or a localized correction to an
already-reviewed low-risk finding. The applicable review and gates below still
apply.

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
4. Evidence first proves one ordinary positive end-to-end path with an
   independent oracle, then runs at least one falsifier that would reject a
   plausible wrong implementation. The negative probe demonstrates
   non-vacuity: it cannot pass merely because an earlier unrelated denial made
   the capability unusable. Failure occurs before mutation or artifact
   acceptance where the contract is fail-closed.
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
outward lanes with disjoint primary paths. Each capability-changing lane closes
one registered capability path without weakening a central contract; it reuses
accepted upstream conformance instead of rebuilding its proof machinery.
Cross-lane integration happens only after each lane's affected gate passes.

### Contract-wave integration loop

The bootstrap loop has one primary AI integrator, but it need not have only one
active implementation. Parallelism expands after an invariant has one accepted
owner:

```text
contract cell
  -> reference capability path, oracle, and falsifier
  -> accepted contract revision
  -> disjoint adapter, backend, package, Python, Studio, and evidence lanes
  -> one integration queue
```

1. The integrator chooses a bounded outcome claim, names its invariant owner,
   and identifies every central surface it may change without freezing an
   unclaimed implementation mechanism.
2. One writer owns each affected central seam. An independent verifier fixes
   an ordinary positive probe, a plausible wrong implementation, and the
   non-vacuous falsifier that must reject it, preferably before reading the
   implementation explanation.
3. A central change closes one reference capability path and is accepted before
   dependent implementation fans out. This path is the reference slice; a
   types-only foundation branch is not a contract freeze point.
4. Disjoint consumer branches start from the exact accepted contract revision.
   Writable work uses a separate worktree per mergeable lane; branch identity
   follows the lane, not the agent. Sibling branches do not merge one another.
5. The integrator self-reviews the contract, complete diff, independent
   verification, and abstraction budget; obtains the risk-required independent
   review defined below; rebases the integration envelope on the current head;
   runs the affected gate and explicit semantic cases; marks the pull request
   ready; applies the risk-tiered hosted-wait rule below; and merges and removes
   the branch promptly. Agent-reported completion is not acceptance evidence.

Read-only design, prior-art, oracle, and adversarial audits may scale beyond
writable lanes. More writers are added only for paths that consume a frozen
seam without redefining it. Semantic merge authority remains singular.

Contract discovery is the one writable pre-freeze exception. It may begin when
the research question, source authority, one isolated candidate output path,
non-claims, and decision and STOP criteria are frozen and disjoint from every
lane in flight. It writes only a contract candidate: no implementation, test
fixture, oracle value, tolerance, or falsifier may be authored or tuned there.
The candidate has no implementation authority, and implementation stays
stopped until the contract owner accepts or rejects and freezes the outcome
contract. Discovery lanes are for collapsing uncertainty, not speculative
foundation branches.

Fan-out means contract consumption, not delegated contract design. A consumer
that cannot express its bounded claim through the accepted seam stops and
returns the requirement to the contract owner. It must not add a parallel DTO,
configuration translation, validation path, adapter-shaped central type, or
client-owned interpretation as a temporary bypass.

### Integration-owned registration points

Parallel feature writers do not independently edit cross-lane registration
points. The integrator applies their proposed deltas to:

- crate-root `lib.rs` files and public facade inventories;
- workspace manifests, dependency policy, and lockfiles;
- the capability matrix, roadmap, and shared workflow registries; and
- artifact-family version registrars.

This is an ownership rule for parallel integration, not permission to defer the
registration. A capability-changing lane still includes its facade, evidence,
and matrix changes in the same pull request. The feature writer supplies the
exact proposed export, dependency, workflow, artifact-version, and claim
changes so that the integrator can reconcile them once.

### Contract and lane assignment

An Issue records a closable product claim, not an agent roster or permanent
project plan. Before writable work starts, its body or implementation prompt
should identify:

- the predecessor and exact starting revision;
- the bounded claim, important non-claims, and invariant owner;
- the externally observable outcome, ordinary positive path, failure
  conditions, authority or identity boundary, and input and resource bounds;
- the primary writable paths and central paths that must not change;
- the ordinary execution path and existing contract being consumed;
- an independent positive oracle and plausible wrong implementation;
- the required falsifier, registered case, and capability-matrix row;
- any environment-specific limitation; and
- the condition that stops the lane when another public abstraction, wire, or
  central seam becomes necessary.

The prompt must not freeze an internal isolation, allocation, scheduling, or
worklist mechanism unless that mechanism is itself part of the bounded claim.

Prefer lane, oracle, derivation, comparator, and evaluator names that expose,
where applicable, the role's distinguishing method, source authority, or
observation. Avoid opaque labels such
as `Route A`, `Oracle B`, or `Method 2` when a short semantic name is available;
accepted historical identifiers may still be quoted without forcing a rename.
A semantic name does not establish agent or provider independence; independence
still comes from role separation, fresh context, sealed inputs, and no
writer-scratch sharing.

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

Every oracle or falsifier package starts with at least one ordinary positive
end-to-end probe. A negative probe must show that its targeted gate, rather
than an earlier unrelated denial, distinguishes the admitted path from the
rejected one. A package that can pass while the capability is unusable is
vacuous and cannot close a claim.

An RFC's verification section maps its acceptance invariants to registered
cases and kit IDs. Adding an enum variant or adapter name without such a path
does not extend conformance.

### Compiler-class proof and instance witness

Where one compiler contract derives many instances from a single translation
rule, the kit divides in two.

- **Compiler-class conformance** is owned by the deriving compiler contract: the
  derivation rules, a reference interpreter, the mutant corpus, and
  primal/JVP/VJP consistency. It is proved once for the class.
- **Instance witness** is supplied per instance and contains only data: a
  manufactured or reference solution, boundary data, norm, expected convergence
  order, conserved quantity, tolerances, and nonclaims.

A compiler-derived instance may not add its own kernel, Jacobian, or
conformance harness when the class can already express it. If the class cannot
express a discovered requirement, the instance stops and returns that
requirement to the contract owner instead of adding a local workaround.

This division applies only to compiler-derived instances. Adapter and provider
conformance kits keep their existing form and are not forced into the witness
model.

## Rigor and mechanism budget

The durable-risk classes listed under branch and integration discipline keep
their independent evidence and fresh-context review gates. For all other work,
process must not become a larger product than the seam it protects:

- adapters, application surfaces, and private glue use typed boundaries plus
  focused positive and falsifier tests; they do not automatically acquire an
  RFC, schema, digest, dual derivation, registry, or security sandbox;
- release-trust claims remain durable risk, but the first design is simple
  authority separation or disposable validated copies. Freeze an OS isolation
  mechanism only when the public claim promises that exact mechanism;
- resource admission uses raw input caps and a deterministic,
  implementation-independent abstract cost or step function. Live allocation,
  allocator behavior, queue depth, or worklist lifetime is not an oracle unless
  resource residency itself is the public claim;
- total abstract work, peak simultaneous residency, durable retained storage,
  and cumulative I/O are separate quantities. Charge a witness to peak memory
  or persistent storage only when the public claim requires its overlap or
  later availability; otherwise bounded streaming, aggregation, release, and
  deterministic recomputation remain valid evidence strategies;
- do not materialize every state, certificate, or mutant merely because it can
  be enumerated. Require exhaustive generation only when completeness of that
  exact finite set is part of the claim or no structural argument discharges
  the obligation; otherwise combine the analytic or structural proof with
  bounded representative and adversarial instances; and
- oracle independence normally means role separation, fresh context, sealed
  inputs, and no writer-scratch sharing. Stable actor or control-plane identity
  and lifetime receipts are required only when an actual adversarial
  multi-party provenance or trust claim needs them, not for scientific
  independence by default.

If an oracle becomes more complex than the product seam, or needs a new OS
trust mechanism to test a claim that does not expose one, stop and simplify
the outcome contract or separate authority before adding machinery. This does
not relax a gate: an inconsistent oracle still stops the lane, and a claim
narrows to what independent evidence shows. When the excess resource envelope
comes from oracle bookkeeping rather than the claimed product operation, the
terminal is oracle redesign, not a declaration that the target or product is
resource-infeasible.

This calibration is prospective. It does not retroactively widen an accepted
claim or reinterpret its existing evidence. New lanes and any reopened or
previously rejected lane use these rules when their contract is next frozen.

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

An anticipated consumer is not a real consumer. Keep the first internal use
private; extract a shared trait, configuration, wire, crate, or registry only
when a second independent use exists, or when the private abstraction
demonstrates audit compression as defined below. A public end-user surface may
itself be the bounded product claim, but it does not justify a generic
implementation abstraction ahead of two implementations.

### Audit compression

The two-consumer rule exists because speculative generality is expensive to
undo. Under agent authorship the dominant cost is audit rather than
refactoring, so a **private** abstraction may be introduced with one consumer
when it demonstrably compresses what has to be audited. All of the following
must hold:

- an independent agent owns a class-level mutant and falsifier suite;
- the count of invariant-bearing hand-written formula sites does not increase;
- at least two hand-written implementations, or one primal/JVP/VJP triple, are
  deleted;
- a new instance requires only witness data, not executable formulas; and
- no public type, wire, or registry is added.

**Public or durable API keeps the stricter rule**: two external consumers, or
the public surface is itself the product claim. Compatibility audit is the
dominant cost there, and agent authorship does not reduce it.

## Fan-out wave closure

When sibling lanes reunite, the integrator performs one duplication audit
before accepting their composition. This is a merge-boundary check, not a
calendar review. Check that:

- equivalent configuration conversions or helper types were not copied across
  lanes;
- clients did not reimplement central validation or structured diagnostics;
- adapter-specific types did not escape into semantic or realization layers;
- exhaustive branching did not spread from one owner into every consumer;
- root re-exports, universal contexts, or option bags did not grow merely to
  simplify registration; and
- semantic identity, execution provenance, and presentation state remain
  separate.

If duplication reveals one missing invariant owner, return only that invariant
to a narrow contract cell. Do not solve wave-level duplication with a universal
utility crate, DTO, context, or plugin interface.

## Issue queue discipline

Issue intake is continuous, while implementation remains dependency-ordered.
Refresh the complete open queue at three natural boundaries:

1. before selecting the next contract cell or capability closure;
2. after accepting a central-contract or authoritative dependency-spine
   change; and
3. immediately before integrating a completed envelope.

Classify each newly observed Issue before changing course:

- **prerequisite** — it corrects or invalidates an invariant required by the
  active lane; stop at a clean boundary and resolve it first;
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

Branches are short-lived and scoped to one contract cell, capability closure,
or independent outward lane. A high-fan-out predecessor merges before its
consumer branches begin; composition evidence begins only after all of its
parent capability paths are accepted.
Rebase the current integration head once before final local verification. The
integrator records the exact local commands and limitations, merges only a
passing affected closure with any required independent review, and deletes the
merged branch. Hosted checks validate the exact proposed or integrated revision
but complement rather than replace repository-owned local acceptance.

A pull request is the exact integration envelope for a closed capability, an
accepted RFC or governance decision, or one of the bounded maintenance deltas
enumerated above. It is not a queue for routine human approval. Draft status is
used only while that envelope or its verification is incomplete. Before
marking it ready, the integrator self-reviews:

- the complete diff against the current base and the absence of unrelated
  changes;
- the bounded claim, non-claims, invariant owner, and central surfaces touched;
- the independent oracle, plausible mutant, falsifier, and affected evidence;
- every facade, dependency, registry, artifact-version, and capability claim
  applied at an integration-owned registration point;
- the wave-closure duplication audit when sibling lanes reunite; and
- the exact local commands, results, and environment limitations recorded in
  the pull request.

The self-review classifies the delta by durable risk. A fresh-context
non-writer must independently review before integration the complete risky
delta that changes any of:

- governance, review, or evidence policy;
- scientific meaning, an oracle, expected value, tolerance, or falsifier;
- a public or versioned API, compatibility promise, or migration;
- a persisted schema or exact artifact;
- security, data integrity, release trust, or CI trust; or
- an architecture ceiling or debt entry.

This review covers only the risky delta, plus enough surrounding context to
judge it, rather than every unrelated low-risk change in the integration
envelope. The writer or integrator cannot supply the independent review for
its own high-risk delta. Implementer/oracle independence remains mandatory
regardless of this review classification.

Outside those boundaries, integrator self-review and repository gates are
sufficient for a dependency-only update when it includes the lockfile and
relevant automated gate; non-governance documentation; reproducible generated
or mechanical output; a private behavior-preserving refactor; and a localized
correction to an already-reviewed low-risk finding. No fresh reviewer is
required unless an anomaly appears. Anomalies include ambiguous risk
classification, scope drift, an unexplained gate change or failure,
non-reproducible generated output, or a change to the accepted contract or
evidence.

A strictly localized correction to an already-reviewed high-risk finding gets
focused fresh-context non-writer review of the correction only, not whole-diff
rereview merely because the reviewed head changed. If the correction changes
the claim, evidence, or compatibility, widens scope, or otherwise reopens the
accepted risk, review the reopened risky delta plus needed context as a new
high-risk change.

A historical pre-integration review miss remains noncompliant and must not be
relabeled. Repair may be prospective without a mandatory revert: create a new
exact-current-head re-admission envelope, obtain complete fresh-context review
of its risky delta, rerun all affected evidence, and begin acceptance only at
that new head. Revert or removal is required when current state cannot be
safely reviewed and revalidated, or migration or persisted-state semantics
demand it. This replaces destructive ceremony, not review or evidence.

Hosted waiting follows the same durable-risk boundary. A delta in the high-risk
list above waits for every relevant hosted check before merge. A localized
low-risk change in the exact class enumerated by
[the hosted topology](ci-topology.md) may deliberately use the existing
auditable owner/admin bypass after its exact-head mise gate, scope and DCO
audit, and any required review pass. Prefer the base-owned trust context,
record the actor, reason, head, commands, and results, and track running CI as a
post-merge signal; failure triggers immediate repair or rollback assessment.
The ruleset and its required contexts remain unchanged.

Once the applicable review and hosted-wait condition are closed, mark the pull
request ready and merge promptly. Stop when protection requires another action,
an anomaly remains unresolved, or mandatory high-risk review has not accepted
its bounded delta.

There is no calendar review or activity ledger. Revisit the development model
only when an operational anomaly appears:

- the same verification failure occurs twice in succession;
- central-contract rework repeatedly invalidates downstream work;
- parallel lanes produce increasing merge conflicts;
- the same revision receives redundant broad hosted verification;
- a ready capability waits behind integration for longer than one broad gate;
- a plausible mutant survives the registered falsifier;
- one lane expands into more than one independently owned central seam;
- public APIs or crates grow faster than accepted capabilities; or
- evidence registration itself becomes the implementation bottleneck.

The review ends when the concrete recurring cause is removed. It must not add
standing reports, meetings, dashboards, or activity metrics to otherwise
healthy flow.
