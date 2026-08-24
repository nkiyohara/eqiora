# Repository instructions

Keep this file a short routing map for recurring facts that code cannot reveal.

- [AI-authored platform strategy](docs/development/ai-authored-platform-strategy.md) — what to optimize and what not to build.
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

When an oracle, evidence package, or review reaches a third version, stop: do not retry in
the same session or thread. Re-scope the task, falsify its premise, or return it to its
owner with the argument.

Do not add an Issue, RFC, contract artifact, schema, registry, oracle, subagent, lane,
worktree, sealed handoff, or second derivation unless the actual delta or user request needs
it. Reuse accepted contracts and evidence. Prefer the cheap check that could disprove a
premise before work that assumes it.

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

When a change adds, removes, narrows, or extends an executable or user-visible capability,
update [the capability matrix](docs/capability-matrix.md) in the same pull request. The case
manifests under `verify/` remain authoritative; the matrix is their index.

- Assess contract, execution, verification, and maturity independently.
- Mark verification present only when a reproducible `verify/` case supports the exact claim.
- State the narrowest honest boundary and important non-claims.
- A pure refactor needs no status change, but check that the matrix remains truthful.
- Use `cargo run -p eqiora-verify -- index`; maintain no second evidence registry.

Implementers may write focused tests and assertions for ordinary low-risk behavior. When a
high-risk change introduces or changes scientific evidence, an exact-artifact oracle, expected
values, tolerances, or falsifiers, the implementer must not author, tune, or relax that
evidence. New scientific formulations, expected values, or tolerances use two fresh independent
derivations. Application surfaces and adapters use focused tests, not derivation ceremony.

Every evidence package proves an ordinary positive path first. An unrelated earlier denial
cannot count as rejection of the intended mutant. If an oracle becomes more complex than the
seam or needs unrelated OS trust, simplify the claim or separate authority; never relax an
inconsistency.

## Review by durable risk

Fresh-context non-writer review is required before integration only for the high-risk delta
listed above, plus enough context to judge it. Mixed changes do not send unrelated low-risk
work through that review. The writer or integrator cannot approve its own high-risk delta.

Dependency-only updates with their lockfile and relevant check, non-governance documentation,
reproducible mechanical changes, private behavior-preserving refactors, and localized low-risk
corrections need only self-review unless an anomaly appears. A localized correction to reviewed
high-risk work receives focused review of the correction; changed claims, evidence,
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
