# Repository instructions

These instructions apply to the entire repository.

## Capability claims are part of the implementation

When a change adds, removes, narrows, or materially extends an executable or
user-visible capability, update
[`docs/capability-matrix.md`](docs/capability-matrix.md) in the same pull
request. Treat the relevant matrix row and its exact boundary as part of the
feature's definition of done.

- Assess the contract, execution, verification, and maturity gates
  independently. Code presence alone is not verification or maturity.
- Mark verification present only when a reproducible case under `verify/`
  supports the exact claim. Link or name that evidence where useful.
- State the narrowest honest current boundary and retain important
  non-claims. Never widen a row from one fixture to a general product claim.
- Add a row when no existing capability describes the change; do not hide a
  new concept inside a vaguely related row.
- Pure refactors need no status change, but still check that moved or renamed
  contracts have not made the matrix misleading.

The case manifests under `verify/` remain authoritative. The matrix is their
whole-product index, not an independent source of evidence.

## Close vertical slices

Capability work follows
[`docs/development/vertical-slice-development.md`](docs/development/vertical-slice-development.md).
Do not report a feature complete until its bounded claim travels through the
typed contract, ordinary lowering/realization path, meaningful falsifier, and
registered evidence. Keep central semantic and identity changes narrow; fan
out independent adapter, package, Python, Studio, and fixture work only after
the owning contract is stable. Parallel work follows a contract wave: one
writer owns an invariant-bearing central seam until its reference slice is
accepted, then disjoint consumers start from that exact accepted revision.
Writable branches belong to mergeable slices rather than agents and use
separate worktrees; an independent agent should derive the falsifier, while one
integrator retains the final semantic and merge decision.

A fan-out lane consumes its accepted central contract; it does not extend that
contract for local convenience. If the contract cannot express a discovered
requirement, stop the lane without adding a workaround and return the
requirement to the contract owner. During parallel waves, the integrator alone
applies cross-lane registration changes to crate roots, public facades,
workspace manifests and lockfiles, the capability matrix and roadmap, shared
workflow registries, and artifact version registrars. Feature agents return the
proposed registration delta with their implementation.

Use `cargo run -p eqiora-verify -- index` instead of maintaining another
capability-to-evidence list. Apply the abstraction/public-API budget before
adding a crate, public type, enum variant, trait, wire field, or registry.

Use `python3 tools/ci/local_verify.py fast` during implementation and
`python3 tools/ci/local_verify.py affected` before integration. Pass every
semantically affected registered case explicitly with `--case`; automatic
Cargo closure is conservative assistance, not claim ownership. The affected
planner validates every case manifest but runs only changed and explicitly
named cases; optional backend features require their evidence case or a
matching environment-specific check.
Refresh the open Issue queue when choosing a slice, after a central contract or
dependency-spine change, and immediately before integration. A newly filed
Issue does not preempt accepted work merely because it is newer: place it at
the earliest dependency-safe gate, reuse an existing tracker where possible,
and interrupt the active slice only when the new information invalidates an
owning prerequisite or exposes an urgent security, correctness, or data-loss
risk. Follow the queue rules in the vertical-slice guide; do not create a
calendar review or an activity ledger.

## Apply rigor in proportion to durable risk

Reserve full vertical-slice ceremony for scientific meaning, public or
versioned interfaces, persisted data, compatibility, release trust, and other
failures that would invalidate a user-visible claim. Adapters and application
surfaces need ordinary typed boundaries and focused tests, but they do not
automatically require a new RFC, schema, digest, registry, or evidence case.

Developer convenience stays outside the product architecture. Prefer the
smallest conventional local tool that works, keep maintainer-specific hosts
and paths out of the repository, and do not add a protocol or durable contract
for a build, cache, synchronization, or editor-host workaround. Promote such a
tool only when it acquires an actual public compatibility, data-integrity,
scientific-evidence, or release-trust boundary.

If a pull request supplies an implementation-agent configuration identifier,
validate it before merging with
`python3 tools/ci/check_implementation_agent.py --base origin/main
--pr-body-file <path>` against its final body. The identifier is optional during
ordinary contribution, but a supplied value must resolve to a current entry already
present in the protected-base qualification registry. Do not infer or invent an
identifier from a visible model/provider name, and do not consume a registry
entry introduced by the same pull request.
