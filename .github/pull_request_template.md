## Summary

- Bounded claim:
- Important non-claims:
- Invariant or user outcome:

## Design

- Related issue/RFC:
- Alternatives considered:
- Compatibility impact:
- Central contract touched (or `none`):

## Evidence

- Registered case(s):
- Independent positive oracle:
- Meaningful falsifier:
- `case.toml` capability/claim-boundary change:

## Abstraction budget

- New crate/public type/enum/trait/wire field/registry (or `none`):
- Two concrete consumers and invariant owner:
- Existing branch/type removed or simplified:
- Unknown/unsupported/stale value rejection:

## Verification

- [ ] Fast local gate (during development)
- [ ] Affected local gate (before integration)
- [ ] Formatting, tests, Clippy, and changed public Rustdoc
- [ ] Dependency-layer check when manifests or layer ownership changed
- [ ] Dependency policy when dependencies changed
- [ ] New or updated registered evidence when semantics changed
- [ ] Capability matrix synchronized to the exact bounded claim

Exact commands and environment limitations:

## Optional implementation-agent provenance

Implementation-agent configuration: not-supplied

When an exact configuration is attestable, replace `not-supplied` with its
`agent-config-v1:<digest>` from the protected-base registry. A supplied
identifier is validated locally; absence does not block integration.

## Contributor certification

- [ ] Every commit contains a valid DCO `Signed-off-by` trailer.
