# Control v2 compile/check oracle

Staged, pre-committed evidence for the `eqiora.control/v2` compile/check
contract frozen by
[RFC 0083](../../../../../rfcs/0083-current-model-artifact-epoch.md) §"Frozen
compile/check v2 contract". Nothing here is live: the schema is staged rather
than published to `schemas/control/`, no case manifest or registry names these
files, and the v1 case beside them is unchanged. The implementation writer
wires these values and may not author, tune, or relax them.

## Contents

| Path | Role |
| --- | --- |
| `schema/compile-v2.schema.json` | complete v2 request/response JSON Schema, `$id` `urn:eqiora:schema:control:compile-v2` |
| `models/accepted-v2.json` | accepted witness, request ID `shared-accepted-v2` |
| `models/rejected-source-v2.json` | empty-source witness, request ID `shared-rejected-source-v2` |
| `models/retired-v1.json` | the previous accepted v1 request, byte-for-byte |
| `models/unknown-protocol-v2.json` | accepted witness with protocol `eqiora.control/unknown-test` |
| `models/unknown-command-v2.json` | accepted witness with command `model.unknown-test` |
| `models/forbidden-model-wire-v2.json` | accepted witness plus `modelWire: "v8"` |
| `models/forbidden-required-features-v2.json` | accepted witness plus `requiredFeatures` |
| `expected/contract.json` | expected contract, schema `eqiora.verify.control-plane-compile-check/v2` |
| `expected/historical/compile-v1.schema.json` | the previous live v1 schema, byte-for-byte |

## What is frozen

- The complete v2 schema: closed request `protocol, command, requestId,
  filename, source`; closed response `protocol, command, requestId, outcome`;
  accepted Model descriptor `schema, transactionSchema, digest, modelId,
  semanticRevision`; and the retained `diagnostic`, `sourceSpan`, and `patch`
  shapes with the v2 numeric tightenings.
- The accepted schema facts `eqiora.model-envelope/v8` and
  `eqiora.model-transaction-envelope/v8` as observed output constants, semantic
  revision 1, and the identity-shape predicates on `digest` and `modelId`.
- The positive relation: two `execute_compile_v2` invocations and one ordinary
  `ModelDocument::compile` of the same source have pairwise-distinct `modelId`
  and `digest`, revision 1, and equal generation-v2 structural semantic
  fingerprints, while one same-execution response links to the `ModelDocument`
  it returned.
- Diagnostic source, severity, and code for every rejection, the four exact
  message values quoted from RFC 0083, and the stage precedence that makes the
  retired v1 request fail the dispatch prelude with `EQ0001` even though it
  also carries `modelWire` and `requiredFeatures`.
- In a `rejections` entry, `requestId` is the identity the response echoes, and
  it is null exactly when `standaloneDiagnostic` is true: a pre-admission
  diagnostic is not a protocol response, carries no request ID, and is never
  wrapped in a synthetic v2 response.
- The dispatch-prelude bound of 128 characters and 128 UTF-8 bytes for each of
  `protocol` and `command`, its `EQ0901` overflow, and the boundary case where
  a 128-character protocol is admitted by the prelude and then rejected as an
  unsupported identity with `EQ0001`.
- The response diagnostic bounds and the overflow response: exactly one
  control-source `EQ0901` error with the frozen message and null `graphPath`,
  `span`, and `patch`, with no partial or truncated kernel diagnostic.
- The four generated resource boundaries and, for each, the specimen rule that
  makes it exceed exactly one bound.

## What is deliberately not frozen

- The accepted `digest` and `modelId`. Flat source compilation allocates fresh
  occurrence ULIDs, so independent compilations correctly differ; only the
  shape predicates and the inequality relation are evidence.
- Any structural fingerprint digest. RFC 0073's registered evidence owns the
  fingerprint algorithm; this oracle owns only the equality relation and its
  generation.
- Parser-, compiler-, and DTO-owned wording. `EQ0602` and every `EQ0901`
  witness freezes source, severity, code, and a nonempty message only.
- The prelude-overflow message. It is required nonempty and required not to
  echo caller content; `dispatchPrelude.contentMarker` is the string a
  generated oversized specimen embeds so that "does not echo" is checkable.

## Derivations, and where each value comes from

- The witness source is the retired v1 request's `source` member, unchanged:
  130 bytes, SHA-256 `3be494c6…41bda9`, one trailing line feed. It is byte-
  identical in all six requests that carry it, so no witness silently drifts.
- Revision 1 is not copied from a producer run. `ModelDocument::compile`
  crosses one `InMemoryGraphStore::new()` at revision 0 followed by exactly one
  `commit`, and `InMemoryGraphStore::commit` advances the federation revision by
  one, so a flat compilation of one model declaration observes revision 1.
- The schema is derived from RFC 0083 §"Frozen compile/check v2 contract"
  alone, not by editing the v1 document. Two consequences are explicit:
  `model.schema` is a `const`, never the v1 document's inconsistent v1–v6
  enumeration; and `modelWire`, `requiredFeatures`, and every `model-wire/*`
  feature are absent from both directions.
- The retained value shapes reuse the v1 spellings because RFC 0083 keeps the
  v1 resource policy verbatim. The filename rule "nonempty and no control
  character" is encoded as `minLength: 1` only, matching v1: the control-
  character rule is DTO admission behaviour reported as `EQ0901`, and RFC 0083
  supplies an explicit `pattern` for `requestId` but none for `filename`.
- The schema file is byte-formatted as `serde_json::to_string_pretty` output
  with a trailing line feed, the form the committed v1 schema already uses, so
  a generator beside the v2 DTO constants can reproduce it exactly.

## Checks run when this oracle was written

Every staged request and a synthesized accepted, rejected, and overflow
response were validated against the staged schema with a draft 2020-12
validator; each of the five negative requests fails the top-level `oneOf`;
mutants covering `modelWire`, `requiredFeatures`, a v7 Model schema, a missing
`transactionSchema`, an uppercase digest, empty diagnostics, 1025 diagnostics,
and every diagnostic bound were confirmed rejected while the at-bound values
were confirmed admitted. The standalone diagnostic validates against
`#/$defs/diagnostic` and is confirmed not to be a top-level member. The two
byte-for-byte copies were compared with `cmp`. The four generated resource
falsifiers were constructed and confirmed to exceed exactly one bound each.

Not checked here: no Rust control v2 code exists yet, so no value in this
oracle has been observed from a running v2 dispatcher, and the frozen
relations are contract-level predicates the implementation must satisfy.

## Promotion

The atomic implementation moves `schema/compile-v2.schema.json` to
`schemas/control/compile-v2.schema.json`, the seven `models/` requests into the
case's `models/`, `expected/contract.json` over the v1 expected contract, and
`expected/historical/compile-v1.schema.json` into the case as retired-protocol
evidence, all byte-for-byte; `expected/contract.json` names request files
without directories so it survives that move unchanged. It then deletes the
live `schemas/control/compile-v1.schema.json` and the v1 rejected-source and
unsupported-protocol specimens, and returns the case-manifest and registry
delta to its integrator. Historical copies are never generated, registered,
packaged, or dispatched.
