# Repository instructions

Keep this file a short routing map for recurring facts that code cannot reveal.

- [AI-authored platform strategy](docs/development/ai-authored-platform-strategy.md) — what to optimize and what not to build.
- [Evidence-development freeze](rfcs/0088-freeze-evidence-development.md) — preserve existing evidence; develop product behavior with focused tests.
- [High-risk and parallel development](docs/development/vertical-slice-development.md) — read only for high-risk capability work or explicitly requested parallel writes.
- [Local verification](docs/development/local-verification.md) — focused checks and gate tiers.

## Default: one local pass

Classify the actual delta, not its filename, hypothetical future use, or theoretical reach.
High risk is limited to changes in:

- governance, review, or evidence policy;
- scientific meaning, an oracle, expected value, tolerance, or falsifier;
- a public or versioned API, compatibility promise, or migration;
- a persisted schema or exact artifact;
- security, data integrity, release trust, or CI trust; or
- an architecture ceiling or debt entry.

Everything else uses one primary agent and one local pass: make the smallest change, run the
narrowest repository-owned check that can expose a plausible defect, self-review once, and
stop. A focused failure permits a focused correction and rerun, not broader ceremony.

Do not add an Issue, RFC, contract artifact, schema, registry, oracle, subagent, lane,
worktree, sealed handoff, or second derivation unless the actual delta or user request needs
it. Reuse accepted contracts and evidence. Prefer the cheap check that could disprove a
premise before work that assumes it.

## Integration flow

Opening a pull request is not completion. Keep one dependency-ordered integration queue and
actively move its earliest unmerged item through checks, review, correction, and merge while
independent implementation continues. Before adding another branch or worktree, inspect open
pull requests and merge every ready predecessor; if none is ready, record the exact failing
check or unresolved decision instead of silently accumulating work in progress. The primary
agent owns its pull request through merge and may review, approve, and merge its own change once
the required repository evidence is satisfied; a separate actor is never a merge prerequisite.
Dependent work may remain stacked, but it must not displace the earliest mergeable item from
the critical path. Delete or retarget merged stack branches promptly.

## Structure and context

Use the smallest conventional local tool and existing seam. Add a public abstraction only
for a current invariant with real consumers, not anticipated reuse. If a ceiling blocks work,
simplify or split the implementation rather than raising the ceiling or adding a bypass.

Parallelize only when requested or when independent work can shorten the critical path.
Read-heavy exploration is the safest use. Parallel writes need disjoint paths and separate
worktrees; one writer owns a shared central seam.

Do not load the whole repository history into a prompt. Start from this map, inspect the
nearest code and instructions, and fetch additional context only when the task needs it.

## Claims and evidence

Evidence development is frozen. Existing cases and oracles remain executable, read-only
verification inputs; run them, but do not add, extend, tune, regenerate, or replace evidence,
expected values, tolerances, falsifiers, exact inventories, evidence schemas, or evidence
infrastructure. A mismatch is a product, build-reproducibility, or claim-scope problem, never a
reason to change the evidence. Ordinary focused product and compatibility tests remain allowed.
Only an explicit owner instruction may unfreeze a named evidence scope.

When a change adds, removes, narrows, or extends an executable or user-visible capability,
update [the capability matrix](docs/capability-matrix.md) in the same pull request. The case
manifests under `verify/` remain authoritative; the matrix is their index.

- Assess contract, execution, verification, and maturity independently.
- Mark verification present only when a reproducible `verify/` case supports the exact claim.
- Leave new capability verification absent unless unchanged pre-freeze evidence already proves
  the exact claim.
- State the narrowest honest boundary and important non-claims.
- A pure refactor needs no status change, but check that the matrix remains truthful.
- Use `cargo run -p eqiora-verify -- index`; maintain no second evidence registry.

## Review by durable risk

High-risk deltas receive one explicit risk-focused review before integration, with enough
context to judge the claim, evidence, compatibility, or trust boundary. The writer or
integrator may perform that review and approve and merge its own change. External review is
welcome when it can change a concrete decision, but actor separation is never required.

Dependency-only updates with their lockfile and relevant check, non-governance documentation,
reproducible mechanical changes, private behavior-preserving refactors, and localized low-risk
corrections need only self-review unless an anomaly appears. A localized correction to reviewed
high-risk work receives focused self-review of the correction; changed claims, evidence,
compatibility, or scope reopen only that risk.

## Verification

Run the narrowest repository-owned focused check for the changed surface.

```bash
mise run fast      # fallback when no narrower repository-owned check covers the delta
mise run affected  # high-risk integration or genuinely uncertain dependency closure
```

Pass every semantically affected registered case with `--case`. Do not widen a focused check,
repeat a broad gate, or rerun whole-delta review solely because a head moved. High-risk deltas
wait for relevant hosted checks. Optional backends need their own case or matching environment.
Follow the verification guide for the narrow owner/admin bypass and optional implementation-
agent identifier check.

A number from one environment is not evidence about another. State the environment or omit the
number; reproduce the hosted profile before making a hosted decision. Run the repository gate,
not a hand-assembled substitute.

Put large build, candidate, and worktree scratch under home-backed `TMPDIR`, never OS `/tmp`.
Add no durable protocol to work around build, cache, sync, or editor-host limits.

## Instruction hygiene

Add a rule only after a recurring failure shows that code, a test, or a narrower document
cannot carry it. Delete or consolidate the rule it supersedes in the same change. This file has
a hard limit of 200 lines and should normally remain below 120.
