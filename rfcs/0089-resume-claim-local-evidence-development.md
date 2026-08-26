# RFC 0089: Resume claim-local evidence development

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-08-26
- Supersedes: [RFC 0088](0088-freeze-evidence-development.md)
- Related: [RFC 0087](0087-one-pass-development-default.md) and
  [AI-authored platform strategy](../docs/development/ai-authored-platform-strategy.md)

## Summary

Resume evidence development. Evidence evolves with the claim it tests, is derived independently
of the implementation output, and binds only the smallest semantic projection needed to expose a
plausible defect. It must not preserve obsolete product structure by pinning unrelated whole
files, package surfaces, generated inventories, or repository layout.

Before 1.0, migrate repository-owned consumers to the preferred API and schema atomically, then
delete displaced names, schemas, aliases, overloads, shims, and parallel paths. Historical release
bytes remain immutable records, but current authoring and decoding surfaces do not carry an
unaccepted compatibility burden.

## Motivation

RFC 0088 stopped evidence machinery from displacing product and gallery delivery. The freeze also
made accidental exact-byte witnesses capable of blocking a better API, workflow, or build design.
That preserved history in the active architecture, which is a worse outcome than a deliberate,
reviewed evidence migration.

The remedy is not unrestricted golden-file churn. Evidence remains an independent authority over
the claim. The change is that its scope and representation must be proportional to the mistake it
can catch, and obsolete evidence structure may be replaced alongside the product invariant it
observes.

## Decision

### Claim-local evidence

Every case or oracle names the exact scientific, semantic, compatibility, artifact, security, or
trust claim it can falsify. Its inputs and expectations contain only facts needed for that claim.

- Derive expected scientific values and relations independently of the implementation under test.
- Fix tolerances from the stated model, numerical method, precision, and acceptance rationale;
  never tune them until observed output passes.
- Prefer semantic projections and relations over whole-file bytes. Exact bytes are appropriate
  only when those bytes are themselves the public or persisted claim.
- Do not use a whole package root, source file, generated tree, broad inventory, or unrelated
  artifact digest as a proxy for a narrower public surface.
- Keep implementation, oracle, and falsifier routes genuinely distinct even when one contributor
  authors and reviews all of them.
- Replace an obsolete oracle in the same change that replaces its claim. Do not retain parallel
  evidence paths merely to document compatibility the project has not promised.

Evidence remains reproducible and fail-closed. A mismatch is investigated, not copied into an
expected file. Changing an accepted scientific meaning, tolerance, or result requires an explicit
risk-focused rationale and review of that exact change.

### Pre-1.0 convergence

Eqiora has no general backward-compatibility promise before 1.0. When a clearer API, schema, wire
epoch, or lifecycle replaces an existing design:

1. choose the coherent final surface;
2. migrate all repository-owned producers, consumers, examples, documentation, tests, and
   evidence in one dependency-closed change or stack;
3. delete the displaced API, schema, alias, overload, shim, decoder, and parallel lifecycle; and
4. state any deliberately unsupported historical input plainly.

Immutable release artifacts remain unchanged as historical objects. A current reader need not
continue accepting them unless an explicit stable or external interoperability contract says so.
Security and data-integrity checks are not weakened by a migration; they move to the new boundary.

### Development flow

Evidence work uses the same one-pass default as product work. Add or change a registered case only
when it can falsify a durable claim that focused product tests cannot adequately cover. High-risk
scientific, exact-artifact, security, release, and CI-trust evidence receives one explicit
risk-focused review; actor separation is not required.

Gallery candidates may now advance from product workflow to accepted scientific result when their
claim, independent derivation, falsifiers, environment, lineage, and publication projection are
complete. A candidate campaign is created for a real gallery or verification claim, not as a
parallel planning system.

## Migration

- Remove active references to the RFC 0088 freeze while retaining RFC 0088 as a historical record.
- Replace whole-file and whole-inventory witnesses with claim-local semantic checks when they block
  a product, API, build, or gallery change.
- Resume new and updated cases through the existing `verify/` authority and capability matrix; do
  not create a second evidence registry.
- Retire obsolete pre-1.0 compatibility paths together with their repository consumers and
  evidence.

## Non-goals

- no permission to tune evidence to implementation output;
- no claim that ordinary focused tests are scientific verification;
- no requirement to rewrite sound, claim-local accepted evidence;
- no blanket replacement of exact bytes when byte identity is the actual contract; and
- no new process framework, evidence registry, or permanent independent-actor ceremony.

## Acceptance

This policy is active when repository instructions and development, verification, roadmap, and
gallery guidance route new work through claim-local evidence rather than the global freeze, RFC
0088 is visibly historical, and documentation checks pass.
