# RFC 0087: One-pass development by default

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-08-22
- Amended: 2026-08-25 — one agent may own evidence, review, and integration
- Related: [RFC 0084](0084-contract-wave-capability-development.md) and
  [AI-authored platform strategy](../docs/development/ai-authored-platform-strategy.md)

## Summary

Make one local pass the default for every change outside a short durable-risk list. Contract
waves remain available for genuinely high-risk capability work and explicitly requested
parallel writes; they are no longer the default writable unit.

This RFC supersedes RFC 0084 as active development policy. Historical descriptions of
non-writer or non-implementer roles in older RFCs and evidence packages remain factual records,
but they do not impose actor separation on future implementation, evidence, review, or merge.

## Problem

The contract-wave model escaped its intended boundary. Ordinary changes acquired contract
artifacts, independent roles, path receipts, broad gates, and repeated review even when those
steps could not change a decision.

The alpha.2 integration pull request exposed the cost: hundreds of commits and files combined
release, documentation migration, CI tooling, generated content, and product work until the
integration envelope itself exceeded provider and review limits. The recovery work itself then
accumulated a 138,427-byte admission contract and a 12,988-byte review receipt while trying to
remove ceremony.

Adding a classifier framework or another coordination layer would preserve the failure in a new
form.

## Decision

Classify the actual delta. High risk is limited to:

- governance, review, or evidence policy;
- scientific meaning or risk-bearing evidence;
- public/versioned compatibility or migration;
- persisted schemas or exact artifacts;
- security, data integrity, release trust, or CI trust; and
- architecture ceilings or debt.

Every other change uses one primary agent, the smallest local implementation, the narrowest
repository-owned check, one self-review, and stop.

Ordinary work does not require an Issue, RFC, contract artifact, separate lane, independent
oracle, sealed handoff, worktree, subagent, or broad gate. Implementers may write focused tests
for low-risk behavior.

High-risk work retains only the evidence, review, environment, and hosted checks justified by
its concrete risk. Evidence must remain independently reproducible, but the implementation
writer may author it, perform the risk-focused review, approve the pull request, and merge it.
Scientific values are derived from the claim rather than tuned to observed output; new
formulations use two genuinely distinct derivation routes even when one agent owns both. A
mixed change reviews only its risky delta.

## Parallelism

Read-heavy independent exploration may run in parallel when it shortens the critical path.
Parallel writes require disjoint paths, independently integrable outputs, separate worktrees,
and one writer for each shared invariant. Capacity is not a reason to create a lane.

GitHub relations remain the only coordination graph. No in-repository activity ledger,
workflow classifier, contract schema, or autonomous integration queue is introduced.

## Documentation and migration

The following active documents carry the new default:

- `AGENTS.md`;
- `CONTRIBUTING.md`;
- `docs/development/ai-authored-platform-strategy.md`;
- `docs/development/vertical-slice-development.md`; and
- `docs/development/local-verification.md`.

Duplicated or contradictory prose is deleted rather than archived into another active manual.
Historical RFCs remain readable. New prompts and Issues use the one-pass default immediately
after this RFC is accepted; existing high-risk evidence and product claims are not widened.

## Non-goals

- no weakening of scientific, compatibility, exact-artifact, security, release, or CI trust;
- no repository rewrite, blanket crate split, or gate bypass;
- no mandatory metrics program before fixing an observed blocker; and
- no new process framework to implement this policy.

## Acceptance

The governance delta is accepted when:

- ordinary-work guidance has one unambiguous path across the active documents;
- documentation links and repository documentation checks pass;
- the complete governance delta receives a recorded risk-focused self-review;
- no product capability, scientific claim, schema, compatibility promise, or trust boundary is
  changed; and
- the Priority Zero tracker records the remaining release and backlog work.

This RFC adds no `case.toml`: it changes governance, not an executable or user-visible
capability.
