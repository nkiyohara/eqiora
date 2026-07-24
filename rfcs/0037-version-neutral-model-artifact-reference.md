# RFC 0037: Version-neutral typed Model artifact reference

- Status: Accepted; bounded identity and replay slices implemented and verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0008](0008-canonical-artifact-wire-v1.md),
  [RFC 0013](0013-realization-and-run-provenance-wire.md)

## Summary

Two sealed, generation-neutral boundaries let an explicitly selected Model
v1--v6 artifact expose either exact identity alone or exact identity together
with validated Semantic Kernel content. The artifact owner accepts the
selected historical envelope into one opaque value. The identity-only surface
feeds unchanged downstream lineage. The replayable surface lets semantic
consumers inspect meaning without coupling to one concrete Model envelope
while retaining the selected artifact's exact wire-domain digest, ontology
identity, and semantic revision.

## Motivation

`RealizationEnvelopeV1` records Model identity rather than Model payload. Its
original constructor nevertheless accepted only `ModelEnvelopeV1`, so a
Model whose required meaning was available only through a later wire could
not enter otherwise version-independent Realization and Run v2 lineage. Adding
parallel constructors or a new Realization schema for every Model generation
would couple two independently versioned artifact families.

The missing boundary is a typed identity projection:

```text
explicit Model wire v1 | v2 | v3 | v4 | v5 | v6
              -> exact typed Model artifact reference
              -> unchanged Realization envelope v1
              -> unchanged Run manifest v2
```

This projection must not equate different Model wire domains, guess a decoder,
or turn identity linkage into an execution claim.

## Proposed design

### Sealed typed reference

`ModelArtifactReference` contains exactly:

- the selected Model artifact's domain-separated content digest;
- its typed Semantic Model ontology identity; and
- its semantic revision.

`CanonicalModelArtifact` is a sealed trait implemented by the artifact owner's
opaque `AcceptedModelArtifact`, its registered `ModelEnvelopeV1` through
`ModelEnvelopeV6` implementations, and the reference itself. Each envelope
derives the reference from its own validated state. Callers cannot implement a
permissive metadata adapter or construct a reference from three unrelated
values.

The reference is an in-memory typed contract, not a new serialized artifact.
It has no schema-sniffing, payload-decoding, conversion, or migration role.

### Sealed replayable content

Some consumers need validated Model meaning as well as identity. Geometry is
the first such consumer: it must inspect exact Domains and boundary-parent
relations, but must not depend on `ModelEnvelopeV4` merely because v4 is the
newest codec.

`ReplayableCanonicalModelArtifact` is a sealed extension implemented by the
opaque accepted artifact and its registered explicit Model v1--v6 envelopes.
`replay_model` invokes the selected envelope's ordinary codec-specific
`to_program` path and returns one `ReplayedCanonicalModel` containing both:

- the exact `ModelArtifactReference`; and
- the fully validated immutable `KernelProgram` reconstructed from those
  same selected bytes.

The combined value prevents a consumer from pairing metadata from one wire
generation with content replayed from another. It also checks Model identity
and semantic revision after reconstruction. `ModelArtifactReference`
deliberately does not implement replay: possessing identity is not possessing
content.

This is not a capability bag. Consumers inspect only the closed Semantic
Kernel vocabulary they declare. Unsupported required meaning still fails in
the ordinary codec or whole-model validator before a replayed value exists.

### Generation-neutral is not digest-neutral

The selected Model schema remains part of the digest domain. Equal semantic
meaning encoded as Model v1 and Model v2 therefore yields distinct artifact
digests and distinct references even when Model identity and semantic revision
match. A Realization sealed against one reference rejects another wire
generation of the same model.

`ModelArtifactReference::validate_artifact` and
`RealizationEnvelopeV1::validate_model_artifact` compare all three axes. A
matching ULID or revision cannot substitute for the exact selected artifact.

### Unchanged Realization wire

`RealizationEnvelopeV1::from_resolved` accepts the sealed Model artifact
contract and writes the same existing fields: Model digest, Model ULID, and
semantic revision. It still checks that the resolved Realization names the
same Model and revision. Its canonical JSON schema, field set, decoder,
domain-separated digest, and Run v2 linkage are unchanged.

The public `ModelDocument` exposes a reference only for its retained exact
codec. Compatibility replay is `ExactModelCodec::replay`; the decoder is the
receiver rather than an argument hidden among ordinary authoring operations.
No path tries one codec after another or upgrades old bytes.

### Ownership and dependency direction

- Model envelope implementations, the sealed reference, and the sealed replay
  boundary live in `eqiora-artifact`;
- the accepted v1--v6 envelope set and all encode/decode/identity/replay
  dispatch are generated from one registry in `eqiora-artifact`;
- `eqiora-api` retains the public caller policy `ExactModelCodec`, maps its
  selected generation into that registry, and keeps one opaque accepted
  artifact in `ModelDocument`;
- Realization consumes only the typed reference surface and retains its
  existing wire; and
- Run v2 continues to consume the resulting Realization, not the Model
  payload.

The artifact registry owns historical-envelope mechanics, not public
generation selection. Adding a generation therefore updates both the single
artifact-owned dispatch registration and `ExactModelCodec`. Generation-neutral
CAD and Geometry consumers remain outside either historical match.

The Semantic Kernel, Model v1/v2/v3/v4/v5/v6 bytes, Geometry Identity v1 bytes,
Realization v1 bytes, and Run v2 bytes do not change.

## Alternatives considered

### Add one Realization constructor per Model wire

Rejected. It would repeat identical linkage checks and make each new Model
generation an API event throughout downstream artifact families.

### Convert every Model to v1 before realization

Rejected. Later wires carry meaning that v1 cannot represent. Even for a common
subset, conversion would replace the exact artifact named by provenance.

### Compare only Model identity and semantic revision

Rejected. That would admit a different content artifact and erase the selected
wire's digest domain.

### Add a schema tag to Realization v1

Rejected. The existing domain-separated Model digest already identifies the
exact artifact, and changing a stable consumer wire is unnecessary.

## Compatibility and migration

This change widens only the typed Rust construction boundary before 1.0.
Existing Model, Realization, and Run canonical bytes and digest preimages are
unchanged. Existing Model v1 callers continue through the same contract.

A future Model wire requires its exact envelope and decoder, one entry at the
artifact owner's historical-envelope dispatch registration point, and an
explicit update to the public caller policy `ExactModelCodec` before callers
can select it for identity or replay. Generation-neutral CAD and Geometry
consumers remain unchanged; Realization and Run continue to consume only the
typed identity boundary. The contract does not authorize wire auto-detection,
fallback, implicit upgrade, cross-generation digest equivalence, or permissive
replay of unknown required semantics.

## Verification

The registered
[`artifacts.model-reference-lineage`](../verify/artifacts/model-reference-lineage/README.md)
case must:

1. compile one spatial Model through explicit wire v1, one scalar-physical
   Model through explicit wire v2, and one field-boundary Model through
   explicit wire v3;
2. derive the sealed typed reference for each selected artifact;
3. construct an unchanged Realization v1 and linked Run v2 for each of those
   three selected generations, then replay the explicitly selected Model
   bytes;
4. preserve exact Model digest, ontology identity, and semantic revision
   through that lineage; and
5. encode and decode one semantic graph through every artifact-owner-registered
   v1--v6 generation, rejecting every wrong-generation decoder choice; and
6. prove that the same semantic graph encoded in different Model digest
   domains cannot substitute for the artifact selected by a Realization.

The case proves artifact identity composition only. It does not execute,
lower, or numerically accept the scalar-physical or field-boundary Models.

The registered
[`geometry.fixed-reference-interface-identity-2d`](../verify/geometry/fixed-reference-interface-identity-2d/README.md)
case separately replays one common geometry-capable Model through explicit
v1, v2, v3, and v4 decoders. It proves equal decoded Domains and memberships,
distinct exact wire-domain digests, rejection of cross-wire substitution, and
ordinary fail-closed rejection of unsupported or stale semantic content.

## Security and safety

Sealing prevents untrusted crates from implementing either identity or replay
projection.
Every serializable endpoint still uses its own bounded, closed decoder and
typed validation. A digest remains content addressing rather than authenticity
or trust evidence.

## Nonclaims

This RFC does not define Model wire auto-detection, schema upgrade or
migration, cross-schema digest equality, a universal artifact reference,
execution-provider discovery, execution support, numerical acceptance, or
physical result storage. It does not widen any Model, Realization, or Run wire.
