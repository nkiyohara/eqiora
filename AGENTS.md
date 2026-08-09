# Repository instructions

These repository-wide instructions state only what code cannot reveal. Longer rules live in
their authoritative owners:

- [AI-authored platform strategy](docs/development/ai-authored-platform-strategy.md) — optimization and amendments.
- [Contract-wave capability development](docs/development/vertical-slice-development.md) — closure, lanes, conformance, abstractions, Issues, and branches.
  **Read it before capability or parallel work.**
- [Local verification](docs/development/local-verification.md) — the gate tiers.

## One primary agent, independent roles

Codex GPT-5.6 Sol is the default primary; pick each lane's agent by fit, not habit. The
maintainer directs without reading diffs; roles stay independent.

A writer never authors or tunes its oracle. Durable risk gets complete-delta review by a
fresh-context non-writer; same-provider lineage is permitted. Bind handoffs by frozen claim,
paths, non-claims, sealed inputs, and no scratch sharing. Stable actor identity or lifetime
receipts are only for actual adversarial multi-party trust, not scientific independence.

Provider diversity is optional escalation, not a gate. Use Opus for bounded derivation,
mutation search, or disagreement; Fable for cross-cutting, visual, or long-horizon review.
Confidence is not evidence; every result states what was not checked.

## Run every lane that can run

Wall-clock is scarce; tokens and agent count are not. Start a lane only when both hold:

- its writable paths are disjoint from every lane in flight, and
- an implementation lane's outcome contract is frozen — externally observable
  result, ordinary positive path, failure conditions, relevant authority and
  identity, bounds, non-claims, oracle, writable paths, and STOP decisions.

Contract discovery may precede that freeze only with a frozen question, source authority,
isolated candidate path, non-claims, and decision/STOP criteria. It writes no implementation,
fixture, or tuned evidence; implementation stays stopped.

Starting without them costs a cycle. Never invent work to fill a slot; idle is correct when
no result can change a decision or shorten the critical path.

Recheck on lane finish and dependency merge, even mid-task; an idle startable lane loses time.

Prefer the cheap check that would falsify a premise over the work that assumes
it.

## Structure outranks speed, every time

Parallelism is the accepted speed because it spends agents rather than structure. A slow lane
costs once; a structure that burdens every later lane compounds. More lanes sharpen the risk:
each sees its local cost and may widen something shared.

When a predicate, budget, or oracle blocks a lane, the lane changes, not the gate:

- a file over its ceiling is **split**, then the ceiling ratchets down to match;
- a nameable glob re-export is **replaced by named items**, not re-registered;
- an unsatisfiable oracle is **returned with the argument**, not relaxed;
- a claim is **narrowed to what was shown**, never widened to what was built;
- a path a frozen whole-tree sweep predates is **admitted by exact path**, never
  by a glob, a directory, or a suffix rule, and admission is a permission that
  joins no frozen set rather than a claim the path was always there.

## Measure the thing you are reasoning about

A number from one environment is not evidence about another. Local wall-clock does not
predict hosted, development does not predict `opt-level=1`, and an aborted run is not complete.
State the environment or omit the number; reproduce the hosted profile per
[local verification](docs/development/local-verification.md) before a hosted decision.

Run the repository's own gate, not an equivalent you assembled. What a
hand-written command list omits — packaged-tree behaviour, the interpreter
matrix, the CI contracts — is where the defects that reach CI live.

## Improve these instructions, at constant size

When review or CI teaches something code cannot reveal, write it here in the same change;
a lesson left in a pull-request body is lost.

This file has a **hard budget of 200 lines**. Adding requires removing a line whose deletion
no longer causes error; otherwise put the lesson in its owning document and link it.

## Claims are part of the implementation

When a change adds, removes, narrows, or extends an executable or user-visible
capability, update [`docs/capability-matrix.md`](docs/capability-matrix.md) in
the same pull request. The case manifests under `verify/` remain the authority;
the matrix is their index.

- Assess contract, execution, verification, and maturity independently. Code
  presence is neither verification nor maturity.
- Mark verification present only when a reproducible case under `verify/`
  supports the exact claim.
- State the narrowest honest boundary and keep important non-claims. Never
  widen a row from one fixture to a general product claim.
- Add a row when no existing capability describes the change.
- A pure refactor needs no status change, but still check that a moved or
  renamed contract has not made the matrix misleading.

Use `cargo run -p eqiora-verify -- index` rather than maintaining a second
capability-to-evidence list.

## Evidence must be independent of the implementer

An implementing agent must not author, tune, or relax the oracle, expected
values, tolerances, or falsifiers for its own implementation. Wiring a
pre-committed fixture is permitted; owning the evidence content is not. Where
an implementer believes a pre-committed oracle is wrong, it stops and returns
the proof rather than adjusting the implementation to match.
An exact-artifact oracle replays producer ordering; geometric equivalence is not byte
evidence when local order changes quality or digest. Every evidence package proves an
ordinary positive path first; unrelated earlier denial cannot count as rejection.

Fresh-context non-writer review is required before integration only for the complete delta that
changes: governance, review, or evidence policy; scientific meaning or an oracle; a public or versioned API, compatibility, or migration;
a persisted schema or exact artifact; security, data integrity, release or CI trust; or an
architecture ceiling or debt entry.
For mixed changes, review only that risky delta, with enough context to judge it. Outside those
boundaries, the integrator may self-review and run gates for dependency-only updates (including lockfile and relevant automated gate), non-governance documentation, generated or mechanical changes, private behavior-preserving refactors,
and localized corrections to low-risk findings. These need no fresh reviewer absent an anomaly.
A strictly localized correction to a reviewed high-risk finding gets focused fresh review of the correction,
not whole-diff rereview. If it changes claim, evidence, or compatibility, widens scope, or otherwise reopens
accepted risk, review the reopened risky delta plus needed context as a new high-risk change. The integrator's own high-risk work is not exempt; post-integration review does not satisfy this gate.
A missed historical review remains noncompliant; never relabel it. Repair may use a new
exact-current-head re-admission envelope with complete fresh risky-delta review and rerun
evidence, accepted only from that head. Revert only when current state cannot be safely
reviewed/revalidated or migration or persisted-state semantics require it.

Independent derivation catches what one agent missed, not what both assumed. Only a new scientific
formulation, expected value, or tolerance carries a **dual independent oracle gate**: two fresh-context
agents derive it from the public claim by different analytic and numerical or symbolic routes, without
reading implementation, writer scratch, or fixtures. They may use the same provider; separation is
recommended only on disagreement or consequential claims. Durable schemas and exact artifacts require
a pre-committed non-implementer oracle, not dual derivation unless they introduce science. Adapters and
application surfaces need focused tests, not derivation ceremony.

## Contract and lane ownership

One writer owns an invariant-bearing central seam until its reference capability path is
accepted; disjoint consumers then start from that exact accepted revision. A
fan-out lane consumes its accepted contract and does not extend it for local
convenience — if the contract cannot express a discovered requirement, stop the
lane and return the requirement to the contract owner.

Evidence precommitted against an implementation under isolation needs its exact consumed interface frozen in the contract, or its wiring waits for acceptance; frozen type names alone let isolated writers diverge until integration.

Writable branches belong to mergeable lanes rather than to agents, and use
separate worktrees. The integrator is a per-integration-envelope role, not a standing one.

During parallel waves the integrator alone edits crate roots, public facades,
workspace manifests and lockfiles, the capability matrix and roadmap, shared
workflow registries, and artifact version registrars. A feature agent returns
its proposed registration delta instead.

Do not create a durable activity ledger inside the repository. Non-authoritative
coordination state outside it is permitted: agent, lane, base revision, branch
or worktree, current lock, and handoff only. It stays disposable, never becomes
the authority for a Model, claim, or evidence, never shadows `verify/`, the
Issue queue, or the roadmap, and is never committed.

## Rigor in proportion to durable risk

Every check, review, lane split, artifact, and wait must name a decision it can change or a
plausible defect it can expose — the chain, not each link; once a choice cannot change what evidence catches, collapse the rest. For low-risk work, reuse accepted evidence only after proving
the complete delta byte-identical and the base change disjoint from its gate, closure, toolchain,
authority, inputs, and behavior; patch-id alone is not proof. Use focused checks and merge promptly.
Do not repeat broad gates or whole-delta review solely because a head moved. Durable risk retains
independent evidence and hosted waiting; low-risk bypass retains exact-head mise and CI tracking.
Outcome contracts freeze observable results and bounds, not internals; local glue needs no ceremony.

Resource gates use raw input caps and deterministic implementation-independent abstract
cost, not live allocation or worklist lifetime unless residency is claimed. If an oracle
outgrows the seam or needs new OS trust, simplify the contract or separate authority;
never relax inconsistency. Existing accepted claims are not widened:
new, reopened, and rejected lanes use this calibration at their next contract freeze.

Apply the abstraction/API budget before any crate, public type, enum, trait, wire field, or registry; ordinary changes only lower architecture numbers, and raising a ceiling or debt is reviewed with reason and deletion condition.

Prefer the smallest conventional local tool. Put large build, candidate, and worktree scratch under home-backed
`TMPDIR`, never OS `/tmp`. Keep maintainer-specific hosts/paths out of the repository. Add no protocol or durable contract to work around build, cache, sync, or editor-host limits.

## Gates

```bash
mise run fast      # during implementation
mise run affected  # before integration
```

Pass every semantically affected case with `--case`; Cargo closure is conservative
assistance, and an evidence package is an executor rather than semantic owner.

High-risk deltas wait for relevant hosted checks. Only the exact low-risk class in
[hosted topology](docs/development/ci-topology.md) may deliberately use the live owner/admin
bypass after exact-head mise gate, scope/DCO, and review; prefer base trust, record actor and
reason, track CI post-merge, and repair or assess rollback on failure. Required contexts and
the ruleset remain unchanged.

Default tiers exclude optional backends, which need their own case or environment check.
Hosted media installs native tools; one cache or host proves neither contention nor portability.

Before merging a pull request that supplies an implementation-agent configuration identifier, run the
manual protected-base check in [local verification](docs/development/local-verification.md); no CI job performs it.
