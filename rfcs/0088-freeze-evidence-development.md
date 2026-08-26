# RFC 0088: Freeze technical evidence development

- Status: Superseded by [RFC 0089](0089-resume-claim-local-evidence-development.md)
- Authors: Eqiora contributors
- Created: 2026-08-25
- Supersedes in part: [RFC 0087](0087-one-pass-development-default.md), evidence-authoring policy
- Related: [AI-authored platform strategy](../docs/development/ai-authored-platform-strategy.md)

> Historical record only. The repository-wide freeze ended on 2026-08-26. RFC 0089 is the
> active evidence-development policy; none of the prohibitions below governs current work.

## Summary

Freeze technical evidence development while continuing product, API, solver, and gallery work.
Accepted evidence remains immutable and runnable. New behavior uses focused product tests and is
reported as unverified unless unchanged pre-freeze evidence already proves the exact claim.

This RFC supersedes every active instruction to add, extend, tune, regenerate, or replace an
evidence case, scientific or exact-artifact oracle, expected value, tolerance, falsifier, exact
inventory, evidence projection, evidence schema, or evidence infrastructure. Historical RFCs and
accepted evidence remain readable records, not authorization for new evidence work.

## Motivation

Evidence work grew into a second product: exact inventories, projections, candidate campaigns,
oracle lanes, and long hosted gates repeatedly displaced the user-visible Python, solver, and
gallery path. The maintainer therefore chose a strict development freeze instead of another
attempt to optimize evidence throughput.

The freeze must not turn into a product freeze. Eqiora may implement and publish bounded behavior
without claiming that behavior is scientifically verified. Contract, execution, verification,
and maturity remain separate facts.

## Decision

### Frozen scope

The following existing material is read-only unless a later explicit maintainer instruction
unfreezes a named scope:

- `verify/` cases, manifests, expected data, references, oracles, and falsifiers;
- scientific expected values, acceptance tolerances, convergence bands, and mutant criteria;
- exact-artifact and exact-inventory expectations used as evidence;
- evidence schemas, registries, catalog projections, candidate campaigns, and admission logic;
- new infrastructure whose purpose is to create, expand, rewrite, or optimize that evidence.

Running existing evidence unchanged is allowed. A mismatch is resolved by fixing the product or
build determinism, narrowing the claim, or leaving the capability unverified. The evidence is not
changed to follow observed output. Removal or semantic repair also requires a scoped unfreeze.

### Work that continues

Product code, public APIs, solvers, geometry, scaling, notebooks, documentation, and gallery
examples continue. They use ordinary focused positive, compatibility, failure, and regression
tests. Those tests are not registered as evidence and do not upgrade verification status.

Capability-matrix updates remain truthful:

- contract and execution may advance when the product surface exists and runs;
- verification advances only when unchanged pre-freeze evidence proves the exact claim;
- otherwise verification remains absent and the important non-claim is explicit.

Existing affected evidence and hosted trust checks may still run unchanged. Build and CI defects
may be corrected with focused ordinary tests when the correction does not alter evidence meaning,
expectations, inventory, or admission policy.

### Gallery publication

The gallery admits two clearly separated publication classes:

1. **Unverified product example.** A real bounded Eqiora workflow with focused tests, result
   lineage, units, reproducible invocation, accessible presentation, and an explicit unverified
   label. It makes no verified or validated scientific claim and needs no new evidence dossier.
2. **Accepted scientific result.** An unchanged projection of a pre-freeze accepted Result and
   its existing evidence. Its original claim and non-claims remain fixed.

An unverified example may be public and useful; "unverified" does not mean synthetic,
non-publishable, or test-only. It means the publication is a product demonstration rather than
scientific acceptance evidence.

## Migration

- Close evidence-only Issues and pull requests as not planned.
- Remove post-freeze evidence additions from mixed product pull requests; retain product code and
  focused tests.
- Do not push or integrate prepared but unmerged oracle/evidence branches.
- Update active agent, contributor, development, solver/library, roadmap, benchmark, and gallery
  guidance. Historical RFCs and archived notes remain intact, with supersession notices where an
  accepted active RFC would otherwise direct new evidence work.

## Alternatives rejected

### Optimize evidence generation first

Rejected. It continues investing in the frozen subsystem and delays the product/gallery path.

### Require evidence before any public capability

Rejected. This collapses implementation and verification into one status and would make the
freeze a product freeze.

### Delete all existing evidence

Rejected. Existing accepted evidence still supports its exact bounded claims and remains useful
as an immutable regression and verification input.

## Acceptance

The policy is integrated when:

- active repository and contributor guidance has no path that requires or authorizes new
  evidence development;
- product, benchmark, and gallery guidance provides an explicit unverified path;
- existing evidence remains runnable and unchanged;
- evidence-only in-flight work is closed and mixed work is scheduled for product-only restacking;
- documentation checks and a fresh non-writer governance review pass; and
- the exact DCO sign-off is present.

This RFC adds no `case.toml` and changes no executable capability, scientific claim, schema,
oracle, expected value, tolerance, falsifier, or trust boundary.
