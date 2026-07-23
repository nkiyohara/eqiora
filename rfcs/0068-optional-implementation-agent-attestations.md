# RFC 0068: Optional implementation-agent attestations

- Status: Implemented
- Authors: Eqiora contributors
- Created: 2026-07-22

## Summary

An implementation-agent configuration is optional provenance. When a pull
request supplies one, the protected `main` revision owns
a machine-readable registry that binds its stable content identifier to exact
configuration scope, DeepSWE v1.1 evidence, score, validity, and maintainer
review provenance. The local checker fails closed for a supplied malformed,
unknown, stale, tampered, or below-threshold identifier. An absent identifier
does not block integration.

Repository-owned local verification, review, DCO, and registered capability
evidence remain the merge authority. Agent provenance is never evidence that a
particular patch is correct.

## Motivation

Exact configuration provenance can be useful, but the running development
environment does not necessarily expose an attestable model revision,
reasoning setting, harness revision, tool profile, or execution budget. Making
an empty registry mandatory would stop development or reward invented
self-attestation. Ignoring a supplied claim would be equally inconsistent with
an evidence-gated project.

The contract therefore distinguishes absence from invalid presence: absence is
allowed; a supplied claim must be exact and independently registered.

## Decision

### Protected-base registry

`governance/implementation-agent-qualifications.toml` is the sole registry. Its
header fixes the benchmark name and version and represents score as exact basis
points. Each accepted `[[configuration]]` entry contains:

- `id`, a content-derived `agent-config-v1:<64 lowercase hex>` identifier;
- exact model, revision, reasoning, harness, tools, budget, and evaluation
  fields;
- an HTTPS evidence URL and SHA-256 of the immutable evidence document;
- score in basis points; and
- maintainer acceptance, expiry, and `status = "accepted"`.

The identifier is SHA-256 over the UTF-8 JSON object formed from the nine
configuration fields, with sorted keys, no insignificant whitespace, and
direct UTF-8 encoding. Review, score, evidence, and validity are outside
configuration identity.

The checker validates the schema exhaustively, recomputes identifiers, rejects
duplicates and unknown fields, requires at least 7000 basis points, and checks
expiry in UTC calendar days. There is no fuzzy match, family inheritance, or
provider alias.

For a pull request, the checker reads the registry from the merge base supplied
by `--base`, not the candidate worktree. A pull request therefore cannot
authorize itself by adding and consuming the same entry.

### Optional presentation

The pull-request template contains:

```text
Implementation-agent configuration: not-supplied
```

An omitted, empty, or exact `not-supplied` value means only that exact
provenance is unavailable. If the environment exposes a registered identifier,
the author may replace that value with `agent-config-v1:<digest>`. A provider
or model-family name is never inferred or accepted.

The pre-merge command is:

```bash
python3 tools/ci/check_implementation_agent.py \
  --base origin/main \
  --pr-body-file /path/to/pr-body.md
```

When no identifier is supplied, the command reports `not-supplied`. When one
is supplied, every unknown state is an error. The checker is local and does not
add a hosted-Action dependency.

## Alternatives rejected

- **Mandatory empty registry.** It creates an admission deadlock without
  increasing patch correctness.
- **Trust a provider or model name.** It omits the evaluated harness, tools,
  reasoning setting, revision, and budget.
- **Let a pull request edit and consume the same registry.** That permits
  self-authorization.
- **Silently accept a bad supplied identifier.** Optionality is not permission
  to weaken claims that are actually made.

## Possible future strengthening

Governance may reconsider mandatory provenance only after a truthful
attestation path, populated registry, and falsifying evidence exist. That
later decision must not weaken or replace repository-owned verification.

## Nonclaims

- No currently running session is declared qualified.
- No agent identifier is required.
- Qualification is not correctness, safety, numerical, or capability evidence.
- This does not replace DCO, local verification, review, or registered cases.
- This does not add hosted Actions or disclose private execution metadata.
