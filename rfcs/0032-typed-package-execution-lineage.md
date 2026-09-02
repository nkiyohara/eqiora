# RFC 0032: Typed package execution lineage

- Status: Accepted; bounded v1 slice implemented and verified
- Authors: Eqiora contributors
- Created: 2026-07-19
- Depends on: RFC 0013, RFC 0022

## Summary

Exact Model Package compilation composes with typed Realization and Run v2
through one separate content-addressed lineage edge which changes none of the
linked artifacts and introduces no execution-provider or plugin abstraction.

## Motivation

RFC 0022 deliberately stopped after `PackageRunBindingV1`: an exact package
compilation could name a caller-designated, model-matched Run v1 digest, but it
could not name a typed Realization or Run v2. RFC 0013 already makes a Run v2
validate exact Model, semantic revision, Realization, solver policy, topology,
layout, and reduction coherence. Copying any of those payloads into the package
wire would create a second source of truth.

The missing object is therefore a lineage edge, not another manifest:

```text
PackageCompilationRecordV2
  + RealizationEnvelopeV1
  + RunManifestV2
  -> PackageExecutionBindingV1
```

Package-free Model, Realization, and Run artifacts remain valid. Package
lineage is optional provenance and does not become model meaning.

## Design pass

### Current best formulation

`eqiora-package` owns one closed, dependency-minimal wire containing only
package identities, exact external artifact digests, schema tags, and semantic
revision. `eqiora-api` is the sole layer that sees the concrete package,
compiler, Model, Realization, and Run families and the sole validated path that
composes or replays those concrete artifacts. `eqiora-package` exposes only the
closed identity-level constructor and validator over strongly typed external
digests.

This formulation preserves the existing dependency direction and gives every
artifact exactly one owner. It adds constant-time identity checks after the
concrete artifact validators have run; it does not copy numerical payloads.

### Alternatives considered

1. **Extend `PackageRunBindingV1` with optional Realization fields.** Rejected.
   It would change an emitted v1 schema and permit invalid combinations of Run
   v1, Run v2, and optional Realization identity.
2. **Add package fields to `RealizationEnvelopeV1` or `RunManifestV2`.**
   Rejected. Existing artifacts are package-neutral and byte identity must not
   change when optional provenance is attached.
3. **Create a universal package/provider/plugin resource.** Rejected. Model
   packages, solver adapters, transports, device runtimes, data adapters, and
   Studio workflows have different payload and loading authority. Common
   identity discipline does not justify a common arbitrary payload.

## Proposed design

### Closed wire

`PackageExecutionBindingV1` contains exactly:

- `eqiora.package-execution-binding.v1` schema and canonical JSON encoding;
- canonical Model digest from the package compilation;
- exact semantic revision;
- package-compilation digest;
- closed `eqiora.realization-envelope/v1` tag and external digest;
- closed `eqiora.run-manifest/v2` tag and external digest.

Its own digest uses the independent
`eqiora.package-execution-binding.sha256.v1` domain. External Model,
Realization, and Run digest wrappers validate lowercase SHA-256 shape but do
not redefine those artifacts' digest preimages.

The wire has no optional fields, arbitrary schema string, execution settings,
package locator, source path, backend payload, or numerical result.

The repeated Model digest and semantic revision are deliberate agreement axes,
not alternate owners of Model meaning. Construction derives them from the
admitted Model and compilation; concrete replay recomputes them through the
Realization and Run validators before accepting the edge. They may improve
inspection and mismatch diagnostics, but an untrusted binding can never
override any endpoint. The endpoint artifacts and their domain-separated
digests remain authoritative.

### Construction barrier

`PackagedModelDocument::bind_execution_v2` performs the following checks before
the edge exists:

1. the admitted Model digest equals the package-compilation Model digest;
2. the typed Realization references that exact Model digest;
3. the Realization semantic revision and Model ontology equal the admitted
   packaged Model;
4. `RunManifestV2::validate_against` accepts the complete Realization link and
   execution-policy coherence;
5. the application boundary derives exact external Realization and Run
   digests; and
6. only then does the package crate construct the closed binding.

The constructor does not claim that an execution occurred. Evidence producers
must call it only after their independent numerical acceptance checks.

### Independent replay

`validate_execution_v2_binding` first replays the complete resolution graph
against the package compilation. It then repeats the concrete Model,
Realization, and Run validation above and compares every derived identity with
the binding. A content match at only one layer is insufficient.

Changing source documentation may preserve package semantic and Model identity
while changing source, resolution, compilation, and binding identity. Changing
Realization policy changes Realization, Run, and binding identity while leaving
package and Model identity unchanged. Changing execution provenance or output
identity changes Run and binding only. These are intended domain boundaries.

### Ownership and dependencies

- `eqiora-package` owns the closed binding DTO, external digest wrappers, and
  domain-separated binding digest. It gains no artifact, compiler, or backend
  dependency.
- `eqiora-artifact` remains unaware of packages and retains sole ownership of
  Model, Realization, and Run bytes and validation.
- `eqiora-api` composes concrete artifacts and package compilation. It is the
  only dependency layer which imports both concrete families; the package
  crate's public constructor sees typed identities only.
- the public `eqiora` facade re-exports the typed contract without adding a
  second implementation.

No same-layer dependency exception or Kernel/wire change is introduced.

## Compatibility and migration

This is a new optional pre-release wire. Existing Model v1/v2,
Realization-envelope v1, Run-manifest v1/v2, package-compilation v1, and
package-run-binding v1 bytes are unchanged. Existing package-free execution
remains valid and is not assigned fictional lineage.

Supporting a future Realization or Run schema requires a new explicit binding
contract or version. V1 does not accept arbitrary schema identifiers.

## Verification

The registered `packages.typed-execution-lineage` case must:

1. prepare and resolve one exact third-party-shaped scalar elliptic package;
2. compile it through the ordinary locked package and Model v1 path;
3. resolve and execute one host-serial typed Q1 Realization through the shared
   scalar elliptic application API;
4. construct Run v2 from observed solver/backend/topology/reduction evidence
   only after true-residual and continuous-balance acceptance;
5. round-trip and independently replay package compilation, Model,
   Realization, Run, and binding identities;
6. prove source-file insertion order cannot change any identity;
7. prove a documentation-only source change keeps package semantics and Model
   identity while invalidating compilation lineage;
8. reject a different valid Realization, output set, or execution producer as
   a substitute for the bound artifacts; and
9. freeze every digest domain in a reviewed expected-identity record.

Unit tests additionally reject unknown schema variants, unknown fields,
uppercase/malformed digests, oversized wires, changed revision, and changed
resolution/source/toolchain compilation identities.

## Security, safety, and governance

The binding decoder is byte-bounded and denies unknown fields. It loads no
package, artifact, library, or code and performs no network or filesystem
operation. External artifacts remain untrusted until their own bounded
decoders and typed validators succeed. A binding is provenance, not an
execution attestation or trust signature.

## Nonclaims

This RFC does not define provider distribution, a dynamic library ABI,
registry discovery, version ranges, signing, publisher trust, build scripts,
execution attestation, numerical-result schemas, or acceptance by identity
alone. It does not make Model Packages execution plugins.

## Unresolved questions

Typed durable result artifacts and signed evidence bundles remain separate
contracts. A provider package family may be proposed only after concrete
distribution requirements from multiple provider classes justify a shared
identity subset without merging their payload schemas.
