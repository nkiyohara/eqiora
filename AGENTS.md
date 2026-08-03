# Repository instructions

These repository-wide instructions state only what an agent cannot infer from the code.
Everything longer lives in the document that owns it, and those documents are authoritative:

- [AI-authored platform strategy](docs/development/ai-authored-platform-strategy.md)
  — what Eqiora optimizes for, and which rules the agent-authorship premise amends.
- [Vertical-slice development](docs/development/vertical-slice-development.md)
  — definition of done, parallelization, conformance kits, abstraction budget,
  Issue queue, branch discipline. **Read it before starting capability or
  parallel work.**
- [Local verification](docs/development/local-verification.md) — the gate tiers.

## One primary agent, independent roles

Codex GPT-5.6 Sol is the default for contract ownership, implementation,
end-to-end integration, review, and repository gates. The maintainer directs
without reading diffs; independence comes from separated roles and fresh context.

A writer never authors or tunes its own oracle and never reviews its own diff.
A fresh-context non-implementer may own the oracle or review the complete diff
even with the same provider lineage. Bound each handoff by the frozen claim,
writable paths, and non-claims; never share writer scratch with oracle or reviewer.

Provider diversity is optional escalation, not a gate. Use Opus for bounded
derivation, mutation search, or review when agents disagree, an oracle appears
wrong, or a scientific claim is unusually consequential. Use Fable for
cross-cutting, visual, or long-horizon review when fresh-context Sol review is
insufficient. Confidence is not evidence; every result states what was not checked.

## Run every lane that can run

Wall-clock to a working platform is the scarce resource; tokens and agent count
are not. Start a lane when, and only when, both hold:

- its writable paths are disjoint from every lane in flight, and
- its contract is frozen — bounded claim, non-claims, pre-committed oracle,
  writable-path allowlist, and the decisions the implementer must not revisit.

Those two conditions are what keep rework out; a lane started without them costs
a cycle, not a saving. Beyond them nothing is a reason to wait.

Check at the two moments the answer changes: when a lane finishes, and when a
dependency merges. Not when your own hands are free — you will be mid-task both
times, and a lane left idle through your task is wall clock nobody gets back.

Prefer the cheap check that would falsify a premise over the work that assumes
it.

## Structure outranks speed, every time

Parallelism is the only accepted speed, because it spends agents rather than
structure. A slow slice costs twice as long once, visibly. A structure that
makes every later slice harder compounds forever and bills nobody — the agent
taking the shortcut is not the one who pays. More lanes sharpen this: each sees
only its own cost, so the locally cheapest move is to widen something shared.

When a predicate, budget, or oracle blocks a lane, the lane changes, not the gate:

- a file over its ceiling is **split**, then the ceiling ratchets down to match;
- a nameable glob re-export is **replaced by named items**, not re-registered;
- an unsatisfiable oracle is **returned with the argument**, not relaxed;
- a claim is **narrowed to what was shown**, never widened to what was built;
- a path a frozen whole-tree sweep predates is **admitted by exact path**, never
  by a glob, a directory, or a suffix rule, and admission is a permission that
  joins no frozen set rather than a claim the path was always there.

## Measure the thing you are reasoning about

A number from one environment is not evidence about another. Local wall-clock
does not predict hosted, a development profile does not predict `opt-level=1`,
and an aborted run is not a completed one. State the environment beside the
number or omit the number, and reproduce the hosted profile with the prefix in
[local verification](docs/development/local-verification.md) before quoting a
timing that informs a hosted decision.

Run the repository's own gate, not an equivalent you assembled. What a
hand-written command list omits — packaged-tree behaviour, the interpreter
matrix, the CI contracts — is where the defects that reach CI live.

## Improve these instructions, at constant size

When cross-review, a gate, or CI teaches something an agent could not have
inferred, write it here in the same change. A lesson left in a pull-request body
is lost.

This file is loaded into every session and competes with the work for attention,
so it holds a **hard budget of 200 lines**. Adding requires removing: find the
line whose deletion would no longer cause a mistake, and delete it. If nothing
qualifies, the lesson belongs in the document that owns the topic, linked from
here rather than restated.

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
An exact-artifact oracle replays the producer's exact ordering: a geometrically
equivalent fixture is not byte evidence when local order changes quality or digest.

Every change is reviewed **before integration** by a fresh-context non-writer;
the same provider lineage is permitted. **The integrator's own work is not
exempt**: holding acceptance makes self-acceptance cost nothing. Review may be
brief; absence is a defect, and review after acceptance is not this gate.

Cross-review catches what one agent missed, not what both assumed. Only a new
scientific formulation, expected value, or tolerance carries a **dual
independent oracle gate**: two fresh-context agents derive it from the public
claim by different analytic and numerical or symbolic routes, without reading
implementation, writer scratch, or fixtures. They may use the same provider;
separation is recommended only on disagreement or consequential claims. Durable
schemas and exact artifacts require a pre-committed non-implementer oracle, not
dual derivation unless they introduce science. Adapters and application surfaces
need focused tests and non-writer review, not derivation ceremony.

## Slice ownership

One writer owns an invariant-bearing central seam until its reference slice is
accepted; disjoint consumers then start from that exact accepted revision. A
fan-out lane consumes its accepted contract and does not extend it for local
convenience — if the contract cannot express a discovered requirement, stop the
lane and return the requirement to the contract owner.

Writable branches belong to mergeable slices rather than to agents, and use
separate worktrees. The integrator is a per-slice role, not a standing one.

During parallel waves the integrator alone edits crate roots, public facades,
workspace manifests and lockfiles, the capability matrix and roadmap, shared
workflow registries, and artifact version registrars. A feature agent returns
its proposed registration delta instead.

Do not create a durable activity ledger inside the repository. Non-authoritative
coordination state outside it is permitted: agent, slice, base revision, branch
or worktree, current lock, and handoff only. It stays disposable, never becomes
the authority for a Model, claim, or evidence, never shadows `verify/`, the
Issue queue, or the roadmap, and is never committed.

## Rigor in proportion to durable risk

Reserve full vertical-slice ceremony for scientific meaning, public or
versioned interfaces, persisted data, compatibility, and release trust. Adapters
and application surfaces need ordinary typed boundaries and focused tests, not
automatically a new RFC, schema, digest, registry, or evidence case.

Apply the abstraction and public-API budget before adding a crate, public type,
enum variant, trait, wire field, or registry. Structural predicates are checked,
not judged: an ordinary pull request may only move `cargo xtask
check-architecture` numbers down. Raising a ceiling or adding a debt entry is an
architecture change, permitted but reviewed as one, and it carries a reason and
a deletion condition.

Prefer the smallest conventional local tool. Put large build, candidate, and worktree scratch under
home-backed `TMPDIR`, never OS `/tmp`. Keep maintainer-specific hosts/paths out of the repository.
Add no protocol or durable contract to work around build, cache, sync, or editor-host limits.

## Gates

```bash
python3 tools/ci/local_verify.py fast      # during implementation
python3 tools/ci/local_verify.py affected  # before integration
```

Pass every semantically affected registered case explicitly with `--case`.
Automatic Cargo closure is conservative assistance, not claim ownership. An
evidence package is an executor, not semantic ownership.

Both tiers use default features. Optional MPI, CUDA, Diffsol, or other backend code is
not covered by a passing default gate: it requires its own case or environment-specific check.
Hosted media evidence installs native tools explicitly, and a long concurrent `uv run`
owns a target-private cache; a binary or uncontended cache on one host proves neither.

If a pull request supplies an implementation-agent configuration identifier,
validate it before merging with `python3 tools/ci/check_implementation_agent.py
--base origin/main --pr-body-file <path>` against its final body. The identifier
is optional; a supplied value must resolve to a current entry already present in
the protected-base registry. Do not infer one from a visible model or provider
name, and do not consume an entry introduced by the same pull request.
