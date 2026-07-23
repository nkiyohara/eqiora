# RFC 0022: Exact package identity and resolution

- Status: Accepted; bounded v1 implementation
- Authors: Eqiora contributors
- Created: 2026-07-18
- Depends on: RFC 0021

## Summary

Eqiora model packages resolve only by exact content identity in v1, record an
exact locked dependency graph, and distinguish canonical semantic content from
the source bundle used for diagnostics and provenance.

## Motivation

Reusable declarations need distribution and reproducibility, but the current
proposal combines that problem with component semantics, execution providers,
and Studio workflow discovery. It also leaves two expensive questions vague:
what a compatible version means and which bytes a content digest covers.

Package resolution must not decide model semantics. RFC 0021 owns definitions,
instances, bindings, and flattening. This RFC owns only model-package identity,
storage, import resolution, and provenance. Execution providers, verification
bundles, and Studio workflows remain distinct systems until shared machinery
is justified by at least two implemented typed families.

## Proposed design

### Typed package boundary

A `ModelPackage` is a versioned, content-addressed unit of model-space
declarations. It owns one qualified namespace and may contain modules, types,
constants, pure operators, connector definitions, and component definitions.
All executable declarations elaborate through RFC 0021 and the ordinary
compiler path.

The author manifest has a closed, versioned schema with these logical fields:

```text
manifest schema version
qualified package name
declared package version
exact dependency identities
bounded source and resource inventory
```

The manifest cannot contain its own computed digest. After typed declaration
canonicalization, a separate closed `ModelPackageIdentityV1` combines the
qualified name, exact declared version, and computed semantic digest. A
`SourceBundleIdentityV1` combines that package identity with the computed source
bundle digest. These release identities are resolver inputs and outputs, not
author assertions accepted without recomputation.

Unknown schema versions, unknown fields, duplicate normalized paths, invalid
names, and unsupported declaration kinds fail closed. There is no arbitrary
payload field and no package build script.

### Identity is exact, not merely named

The executable identity of a package is:

```text
qualified name + declared version + canonical semantic digest
```

The declared version uses canonical SemVer syntax as a publisher assertion and
human coordination tool. It is not proof of source, binary, numerical, or
behavioral compatibility. In v1, dependencies contain an exact package name,
exact version, and exact semantic digest. The resolver does not accept version
ranges and does not infer compatibility from a major version.

SemVer build metadata is retained as part of exact identity. It has no ordering
or compatibility meaning in the resolver.

An authoring tool may later help select or update an exact identity, but every
accepted packaged compilation records the chosen identity. A separate bounded
v1 lineage edge may bind that compilation to one accepted `RunManifestV1`.
Range solving or public-surface compatibility requires a later versioned
contract with an explicit compatibility definition.

### Two digest domains

One digest cannot simultaneously ignore formatting and prove which source a
diagnostic displayed. Eqiora therefore uses two domain-separated digests.

The **canonical semantic digest** covers:

- a digest-domain tag and canonicalization version;
- manifest schema, qualified name, and declared version;
- every public and private executable declaration in canonical typed form;
- canonical module and declaration names, independent of physical file paths;
- exact dependency identities; and
- content digests of resources that affect model meaning.

It excludes comments, documentation, formatting, source spans, physical file
layout, archive ordering, timestamps, origin URLs, and signatures. Canonical
maps are sorted by encoded keys and encodings are length-delimited.

V1 canonicalization is structural, not an algebraic-equivalence prover. It
normalizes formatting, file placement, declaration/member permutations where
order is non-semantic, dependency aliases, and the explicitly documented
literal cases. Distinct operator trees such as `1 + 1` and `2`, or `m * m` and
`m ^ 2`, retain distinct semantic identities even when a later simplifier
could prove an equal value or dimension. This conservative boundary preserves
rounding, differentiation, and lowering intent. Widening the equivalence class
requires a new canonicalization version and never relabels a v1 release.

Because exact dependency identities are in this preimage, changing any declared
dependency always changes the root semantic digest in v1, whether or not a
particular imported declaration is referenced. Eqiora does not perform
reachability-dependent package identity.

The **source bundle digest** covers:

- a separate digest-domain tag and bundle schema version;
- the canonical semantic digest;
- the complete canonical author manifest, including dependency aliases and
  source inventory;
- normalized UTF-8 relative paths and exact file bytes;
- the source map and diagnostic metadata; and
- documentation or non-semantic resources included in the release bundle.

Relative paths use `/`, reject empty, absolute, parent, platform-prefix, NUL,
and normalization-colliding forms, and are sorted by encoded path. Archives,
compression, timestamps, permissions, and host directory order are transport
details outside the bundle digest.

A formatting-only edit or dependency-alias rename preserves the semantic digest
while changing the source bundle digest. Compile provenance records both. A
package release may therefore be traced to exact source and author spelling
without making either part of model meaning.

V1 closed DTOs use canonical JSON and lowercase hexadecimal SHA-256 digests.
Every digest is computed as `SHA-256(domain-tag || 0x00 || canonical-bytes)`
with distinct tags for package semantics, source bundles, resolution records,
and package compilation. Typed wrappers prevent interchange between domains.
Documentation and other non-semantic files are optional, but every bundled file
is inventoried and contributes its exact bytes to the source bundle digest.

### Exact resolution record

The resolver consumes a root manifest, an exact resolution record, and a
content-addressed local store. The record contains:

- root package identity;
- every exact dependency identity and source bundle digest;
- directed dependency edges with the declaring package and local alias.

The resolver separately receives non-semantic source locators sufficient to
retrieve or audit the exact expected bundles.

Resolution never depends on import order, directory search order, environment
variables, or the first matching version. The compiler performs no network
access. Every bundle is loaded by expected digest, parsed under declared
limits, and verified before name resolution or graph mutation. Missing,
ambiguous, duplicate, cyclic, or digest-mismatched graphs fail atomically.

The canonical resolution-record preimage contains identities, source bundle
digests, declaring-package/alias/target edges, and no locator. Mirror paths or
store layout therefore cannot change the exact resolved graph. Locators belong
to resolution provenance and are checked only as caller-owned means of loading
the exact expected bundle.

Imports select a declaration through the exact locked package identity and a
qualified public path. They cannot expose private declarations. Standard and
third-party packages use the same store, resolver, namespace rules,
elaboration, and artifact path; a bootstrap built-in path must be explicit and
temporary.

### Artifact and evidence connection

The bounded v1 package-compilation sidecar records the canonical Model digest,
root semantic package identity, exact resolution-record digest, source bundle
digests, and compiler/canonicalization versions. Existing Model v1/v2 bytes do
not change.

`PackageRunBindingV1` is a separate canonical lineage edge. It records the
shared Model digest, exact package-compilation digest, closed Run schema, and
canonical digest of one caller-designated `RunManifestV1`. The application
boundary checks Model digest and semantic revision before construction; replay
also revalidates the resolution-record identity and inventory plus the complete
compilation identity. The binding changes neither Model nor Run bytes and does
not prove that execution occurred or that numerical outputs were accepted. A
registered evidence producer constructs it only after those separate checks.
Typed Realization lineage and Model-v2-neutral `RunManifestV2` package lineage
are deferred from this RFC and later closed by
[RFC 0032](0032-typed-package-execution-lineage.md) as a separate edge.

Changing any exact dependency changes both the resolution-record digest and
the root semantic digest. Prior evidence remains valid for its recorded
identities and is visibly historical rather than silently relabelled.

## Prior art and deliberate differences

Cargo provides useful prior art for exact lock records, locked operation, and
source checksums:

- [Cargo dependency and lock-file guide](https://doc.rust-lang.org/cargo/guide/dependencies.html)
- [`--locked` behavior](https://doc.rust-lang.org/cargo/commands/cargo.html#manifest-options)
- [Source replacement and checksums](https://doc.rust-lang.org/cargo/reference/source-replacement.html)

Eqiora borrows the separation between authoring metadata and an exact resolved
graph. V1 deliberately does less: no version ranges, feature unification,
build scripts, source replacement, native code, or online resolution during
compile.

Modelica informs qualified package names, encapsulation, and hierarchical
libraries through its [package specification](https://specification.modelica.org/maint/3.7/packages.html).
Eqiora does not adopt tool-dependent ordered `MODELICAPATH` lookup or
`package.order`; exact content identity and canonical ordering replace both.

## Alternatives considered

### Digest the source bundle only

This gives exact source provenance but makes comments, formatting, and archive
layout alter semantic identity. Rejected.

### Digest only lowered declarations

This gives stable meaning but cannot prove which source, spans, documentation,
or diagnostics accompanied a release. Rejected as the sole digest; retained as
the semantic half of the two-digest design.

### Resolve SemVer ranges in v1

This is familiar but leaves compatibility undefined and makes a resolver
policy part of the first semantic supply chain. Deferred. Exact identities are
sufficient for deterministic import, artifacts, and evidence.

### One universal distributable resource manifest

Sharing an arbitrary payload across packages, backends, workflows, and
verification would erase authority boundaries before reuse is demonstrated.
Rejected. Small identity primitives may be extracted later from concrete typed
families without merging their schemas.

## Compatibility and migration

The first package wire is new and pre-release. It does not change canonical
model nodes or existing source without imports. Manifest, semantic-canonical,
source-bundle, and resolution-record versions are independent and explicit.

Built-in declarations migrate only after an ordinary package reproduces their
canonical output, diagnostics, and verification. Existing artifacts without a
package root remain valid under their current wire version; they are not
retroactively assigned a fictional package identity.

## Implemented v1 boundary

The bounded implementation provides closed author, semantic, source-bundle,
resolution, and package-compilation DTOs; typed domain-separated SHA-256
identities; exact offline in-memory and handle-relative directory stores;
package-local and direct alias-qualified public Connector/Component lookup;
compiler source re-canonicalization before elaboration; package-qualified
in-memory source provenance; the ordinary Model v1/v2 artifact path; and a
closed `PackageRunBindingV1` for model-matched `RunManifestV1` identity lineage.

The public Rust preparation seam admits one bounded author manifest/file
inventory plus a caller-supplied complete exact dependency release closure.
It accepts no author semantic payload. The application layer derives semantic
content through the ordinary compiler, constructs the candidate release,
derives an exact resolution record only from release identities and manifests,
then replays the full graph through the ordinary resolver and compiler under
the final exact namespaces. Missing, duplicate, ambiguous, cyclic,
unreachable, or source/semantic-mismatched inputs fail before a root release is
returned. This seam performs no filesystem discovery, version selection,
network access, or publication.

V1 source bundles contain the complete canonical author manifest, UTF-8 model
source, and arbitrary documentation bytes. Semantic resources and a separately
serialized source-map payload are deferred. Diagnostic locations are
reconstructed from the exact package namespace, normalized bundle path, and
source span, then retained in the compiler provenance sidecar. Canonical JSON
decoders accept structurally equivalent field/whitespace order while rejecting
unknown fields; canonical emit and every digest preimage use the normalized
representation.

[`packages.offline-model-package`](../verify/packages/offline-model-package/README.md)
resolves `Eqiora.Electrical.Basic` and `org.example.parallel` through the same
store, resolver, compiler, hierarchy, artifact, and analytic solve path. There
is no standard-package enum or privileged lookup path.

## Verification

1. Permute imports, files, archive entries, and internal insertion order;
   canonical semantic bytes and digest must remain unchanged.
2. Reformat and relocate source without changing declarations; the semantic
   digest must remain stable and the source bundle digest must change.
3. Change one executable declaration or exact dependency; the relevant
   semantic and resolution identities must change.
4. Round-trip manifest, bundle, and resolution record to identical canonical
   bytes and reconstruct the exact dependency graph offline.
5. Derive release semantics through the compiler from admitted author sources,
   prove dependency input order does not change release identity, and replay a
   transitive exact closure under final content-addressed namespaces.
6. Reject missing, ambiguous, duplicate, cyclic, unreachable, wrong-version, or
   digest-mismatched dependencies before model mutation.
7. Reject a dependency whose carried semantic content disagrees with its exact
   source before returning the root release.
8. Prove two packages can own the same local declaration name without
   collision and that private declarations cannot cross a public import.
9. Reject absolute, parent, platform-prefixed, NUL-containing,
   normalization-colliding, duplicate, overlong, or over-budget bundle paths.
10. Resolve one project-maintained and one third-party-shaped package through
   the identical store, resolver, elaboration, and artifact path.
11. Record both package digests and the resolution-record digest in compilation
   provenance and show prior evidence remains tied to the old graph after an
   update. Bind one accepted `RunManifestV1` through a separate lineage edge and
   reject changed resolution, compilation, Model, revision, schema, or Run
   digest identities.

## Security, safety, and governance

Packages are untrusted data, never code to execute during resolution. Parsing,
decompression, path normalization, graph construction, and declaration
elaboration enforce byte, file, declaration, edge, depth, and expansion limits
before allocation or model mutation. Resolvers do not follow symlinks or write
bundle paths into the workspace.

Signatures, transparency logs, revocation, registries, and publisher authority
are important but separate from content identity. They require a later supply
chain RFC before public package publication.

## Nonclaims

This RFC does not define a public registry, version-range solver,
compatibility inference, package signing, trust policy, build scripts, native
or WebAssembly plugins, execution-provider distribution, Studio workflow
discovery, filesystem/CLI project authoring, Python/Studio release preparation,
editable vendoring, collaborative publishing, automatic source rewriting,
execution attestation, typed Realization package lineage, or `RunManifestV2`
package lineage.

## Deferred questions

The local content-store layout and garbage-collection policy cannot affect
resolution semantics and remain implementation policy. Public registries,
signing, trust, range solving, and other nonclaims require separate RFCs.
