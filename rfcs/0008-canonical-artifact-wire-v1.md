# RFC 0008: Canonical artifact wire v1

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Eqiora serializes validated Semantic Models and reproducible run inputs through
explicit, versioned wire DTOs whose canonical JSON bytes have domain-separated
SHA-256 content identities.

## Motivation

A graph snapshot in process memory is not yet a portable artifact. Evidence
cannot be reproduced unless another process can recover the same typed model,
exact clock periods, quantities, expression DAG, and execution inputs without
depending on Rust layout, insertion order, host paths, or wall-clock time.

Artifact decoding is also a trust boundary. Deriving `Serialize` directly on
every in-memory enum would make a refactor into an accidental file-format
change and could construct values without passing the constructors and atomic
graph transaction that enforce Semantic Kernel invariants.

## Proposed design

### Wire and memory are separate contracts

`eqiora-artifact` owns two v1 envelopes:

- `ModelEnvelopeV1` contains one validated canonical Semantic Model.
- `RunManifestV1` identifies the semantic artifact, semantic revision,
  executor, numerical settings, optional opaque realization artifact, and
  output artifacts for one reproducible run.

Each uses a private wire DTO. No public Semantic Kernel type promises that its
Rust representation is serializable. A model decoder converts every DTO
through its typed constructor, stages one `Transaction`, obtains an immutable
snapshot, and finally builds a `KernelProgram`. Failure exposes no partially
committed graph.

### Model envelope

The model schema identifier is `eqiora.model-envelope/v1`. It represents:

- the model ULID and source graph revision;
- stable kebab-case entity kinds and typed ULIDs;
- every current Semantic Kernel node definition;
- dimensions as exact integer SI exponents;
- finite SI quantities as IEEE-754 values plus dimensions;
- rational time as an exact integer numerator and non-zero denominator;
- expression nodes as an explicitly indexed DAG;
- semantic graph edges and the model boundary.

Nodes, values, edges, and boundary IDs are sorted by their semantic keys and
duplicates are rejected. Input array order is therefore not meaning. The
source revision is provenance: it is serialized but excluded from the model
content digest. Model ULID remains part of content identity, so two separately
named models with equal equations do not silently become the same artifact.

### Run manifest

The run schema identifier is `eqiora.run-manifest/v1`. Numerical setting keys
are a sorted map of stable lowercase names to non-empty textual values. Output
digests are sorted and unique. Wall-clock timestamps, working directories,
absolute paths, and machine names are deliberately absent from run identity;
an evidence layer may record them separately as provenance.

The realization reference is opaque in v1. This keeps the artifact contract
independent from the provisional Realization Graph contract. A later schema
version may add typed realization fields without retroactively changing v1.
RFC 0013 subsequently does so through a separate Realization envelope and
run-manifest/v2; neither v1 schema changes.

### Canonical bytes and digest

The canonical encoding identifier is `eqiora.canonical-json/v1`. Canonical
bytes use compact UTF-8 JSON emitted from the ordered wire DTO after validation
and normalization; arbitrary input whitespace and input array ordering are not
preserved. Every admitted finite IEEE-754 value must survive
serialize/decode/serialize as the identical value and canonical byte sequence;
the workspace therefore enables serde_json's `float_roundtrip` parser path.
Schemas normalize negative zero where sign carries no declared meaning and
reject non-finite values before serialization.

Content identity is lowercase SHA-256 over:

```text
schema-domain UTF-8 bytes || 0x00 || canonical-content bytes
```

The schema-domain separator prevents equal byte strings from being confused
across artifact kinds. A digest identifies bytes under one named canonical
contract; it is not a claim of mathematical equivalence.

### Decoder limits and compatibility

Before deserialization, the decoder rejects inputs over the byte limit or JSON
nesting-depth limit. Before constructing graph state, it rejects excess node,
edge, and expression-node counts, duplicate IDs, dangling references, unknown
entity/edge variants, malformed quantities and rational times, and unsupported
schema or canonical-encoding identifiers.

Unknown major versions are errors, never a request to guess or silently fall
back. New optional provenance belongs in a new compatible schema only if it
does not alter canonical v1 bytes; semantic additions require a new schema
identifier and explicit migration.

## Alternatives considered

### Derive serialization on public kernel types

This is initially concise but couples artifacts to Rust enum layout, admits
invariant bypass, and makes harmless implementation refactors compatibility
events. Rejected.

### Hash arbitrary input JSON after parsing

This would assign different identities to key order, whitespace, and graph
insertion order. Rejected because those are not semantic distinctions.

### Use timestamps or host paths in run identity

Those fields are useful provenance but make reproducible runs receive unequal
identities on different machines. Rejected from the manifest digest boundary.

### Serialize executable callbacks

Closures and backend handles have no portable semantic meaning and expand the
trust boundary. Rejected; artifacts contain declarative model data and stable
executor identity.

## Compatibility and migration

This is the first artifact schema and has no legacy wire compatibility burden.
Rust API stability and wire stability are independent. Pre-1.0 readers may add
new schema decoders, but the meaning and byte production rule of the v1 schema
identifier remain fixed. Migration must decode an old version, validate it,
and explicitly emit a new version; it must not reinterpret old bytes in place.

## Verification

- Round-trip canonical Poisson and sampled periodic-controller models through
  envelope, transaction, snapshot, and `KernelProgram` validation.
- Require the second encoding to equal the first byte-for-byte and by digest.
- Exercise a nontrivial finite `f64` whose default fast decimal parser may
  otherwise select an adjacent representable value.
- Permute input node and edge arrays and require identical canonical output.
- Reject duplicate and dangling IDs before graph mutation.
- Reject byte, nesting-depth, node, edge, and expression resource-limit
  violations without panic.
- Reject unknown schema, encoding, entity kind, and malformed digest data.
- Require run settings and output ordering to be deterministic.

## Security, safety, and governance

JSON input is untrusted. V1 uses safe Rust, bounded bytes/nesting/counts, finite
quantity validation, checked typed-ID conversion, and one transactional graph
commit. SHA-256 provides content addressing, not author authentication;
signatures and trust policy belong to an evidence or distribution layer.

Public wire changes require RFC review. Dependency versions follow their
official APIs: [`serde_json`](https://docs.rs/serde_json/latest/serde_json/)
provides JSON encoding and [`sha2`](https://docs.rs/sha2/latest/sha2/) provides
the SHA-256 implementation.

## Unresolved questions

- Whether a future evidence bundle signs individual digests or a Merkle root.
- Whether large expression DAGs gain a streaming binary encoding alongside,
  but never ambiguously in place of, canonical JSON v1.
- Which typed fields enter the next run-manifest version after the Realization
  Graph contract is accepted.
