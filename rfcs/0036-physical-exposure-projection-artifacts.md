# RFC 0036: Physical exposure projection artifacts

- Status: Accepted; bounded v1 slice implemented and verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0013](0013-realization-and-run-provenance-wire.md),
  [RFC 0022](0022-exact-package-identity-and-resolution.md),
  [RFC 0033](0033-hierarchical-conserving-connection-sets.md),
  [RFC 0035](0035-field-valued-boundary-interfaces.md)

## Summary

An eliminated public physical Port is observed through one versioned artifact
which preserves its exact Connection cut, nominal contract, and complete
package source lineage, followed by a separate value-free binding from one
closed projected quantity to an output already named by an exact Run.

## Motivation

RFC 0033 correctly removes ownerless public Ports before the flat Semantic
Model is sealed. Retaining such a Port would invent an unowned physical
unknown; replacing it with an alias Relation would invent an equation. The
resulting flat Connection, however, does not retain which proper subset of its
members lay inside one eliminated occurrence. A raw Connection ID can recover
the common across quantity, but cannot distinguish the net through quantity of
two different public cuts through the same maximal class.

The compiler sidecar supplies the missing mathematical cut, but an in-memory
sidecar alone cannot be cited by a durable result. The artifact boundary must
therefore preserve all of the following without changing Model meaning:

```text
exact package compilation + exact Model
  -> eliminated exposure + maximal Connection + interior cut + contract
  -> one durable projection identity
  -> one post-run quantity/output binding
```

The output payload remains independently typed. A projection artifact is not a
container for arrays, mesh indices, samples, or solver-specific values.

## Proposed design

### The projection is a cut, not an alias

One `PhysicalExposureProjectionV1` contains:

- the full identity of the eliminated public Port occurrence;
- the full identity of the final maximal conserving Connection;
- a sorted, nonempty proper subset of retained Port full identities inside the
  eliminated occurrence;
- either the exact scalar physical Connector identity, or the exact
  field-valued Connector and boundary Domain identities;
- a presentation selector; and
- every complete source origin as one definition span, one instance span, and
  its complete ordered binding-span set.

The exposure identity must not project to a retained Kernel Port. The
Connection must be a retained conserving Connection. Every cut member must be
a retained member of that Connection and must agree with the exact nominal
Connector and, for a field boundary, the exact boundary support. The cut is a
proper subset because the exposure separates an interior from an exterior; a
whole-Connection cut would have no such boundary meaning.

No Kernel entity, source alias, equation, result value, or mesh object is
created.

### Projection identity and catalog identity are distinct

The domain-separated projection digest is computed from exactly:

```text
exposure full identity
+ maximal Connection full identity
+ ordered interior Port full identities
+ exact scalar or field-boundary contract
```

The presentation selector, source spans, Model artifact, semantic revision,
and package-compilation digest are deliberately absent from that digest
preimage. A source partition or relocation may therefore preserve projection
meaning. The selector is useful for authoring and inspection, but cannot become
a durable semantic lookup key.

The enclosing `PhysicalExposureCatalogEnvelopeV1` owns the contextual
identity. It binds the complete canonically ordered projection set to the exact
Model digest, Model ULID, semantic revision, and
`PackageCompilationRecordV2` digest. Complete provenance remains in each
projection and therefore contributes to catalog bytes and catalog identity.
Two package compilations may produce the same Model and projection meanings
while correctly producing different catalogs when their source lineage
differs.

The catalog is nonempty and bounded independently by encoded bytes, projection
count, total cut members, total source origins, and source-path bytes. Unknown
wire fields and unknown schema variants fail closed.

### Structural replay and exact package replay

Artifact-level replay validates local canonical form, exact Model identity and
revision, package-compilation identity, projected Kernel IDs, conserving
Connection membership, proper-cut structure, and the scalar Connector or
field Connector/boundary contract.

That check is necessary but cannot reconstruct an occurrence cut from the flat
Kernel graph. `PackagedModelDocument::validate_physical_exposure_catalog`
therefore performs the stronger public replay:

1. replay the exact resolution record against the package compilation;
2. use the admitted compilation's retained compiler projections and complete
   package-qualified provenance to seal the expected catalog again;
3. perform the artifact-level structural checks; and
4. require exact equality with the reconstructed catalog.

This is the only supported durable sealing path in the bounded v1 slice. A
caller cannot make an arbitrary proper subset authoritative merely because it
is structurally valid in a flat Model.

### Quantities and post-run binding

The closed `PhysicalExposureQuantityV1` has two variants:

- `Common`: the common scalar across value, or the common field trace on the
  maximal Connection class;
- `NetOutward`: the sum of scalar through values, or parent-outward field flux,
  over the exact retained interior cut.

`PhysicalExposureObservationBindingV1` is a separate, value-free lineage edge.
It contains exact Model and revision identity, catalog digest, projection
digest, quantity, one explicitly tagged `RunManifestV1` or `RunManifestV2`
digest, and one result digest. Construction and replay require:

- the projection to exist in the exact catalog;
- the Run to name the catalog's exact Model and semantic revision;
- the Run schema to be one of the two closed variants; and
- the result digest to already occur in that Run's output set.

The observation binding is constructed after the Run and is not inserted into
the same Run's outputs. This avoids a Run/binding content-digest cycle. The
edge says which existing output a producer designates for one projection
quantity; it does not validate the result artifact's schema, bytes, numerical
acceptance, or physical accuracy.

The Run v2 constructor and replay path are present. An end-to-end physical
Model-v2/v3 Run v2 is not yet constructible because
`RealizationEnvelopeV1` accepts only `ModelEnvelopeV1`; the version-neutral
identity and replay boundary is owned by
[RFC 0037](0037-version-neutral-model-artifact-reference.md). The registered
scalar physical case therefore exercises an output-bearing Run v1 rather than
fabricating a v2 Realization.

### Field-valued boundary

The field projection stores the exact specialized nominal Connector and exact
boundary Domain rather than copying shape, frame, parent, or orientation into
a second payload. Structural replay follows those retained Domain identities
back to the ordinary RFC 0035 Port contract and rejects a coincident peer
boundary substituted for the occurrence's actual support.

The current field evidence proves durable identity, cut, Connector/support,
provenance, canonical round-trip, and exact API replay. It does not store or
sample field values and does not define a discrete trace space or transfer
operator.

### Ownership and dependency direction

- `eqiora-compiler` derives the occurrence cut and in-memory source provenance;
- `eqiora-artifact` owns the bounded catalog and observation-binding wire,
  local validation, and domain-separated digests;
- `eqiora-api` is the only layer that can compose compiler projections,
  package compilation, Model artifact, and exact resolver replay; and
- numerical result families remain separate artifacts owned by their own
  contracts.

The Semantic Kernel, Model v1/v2/v3 wires, package release, package compilation,
and Run v1/v2 bytes do not change.

## Alternatives considered

### Preserve the public Port in the Semantic Model

Rejected. It creates an unowned physical unknown and changes the equation
system solely to preserve a source-level name.

### Alias the exposure to a Connection or retained Port

Rejected. A Connection denotes the whole equivalence class, while
`NetOutward` depends on one exact cut. Choosing a retained Port is arbitrary
and wrong for N-ary and nested boundaries.

### Put selector and source provenance in projection identity

Rejected. Presentation spelling and source placement do not change the
mathematical cut. They belong to the contextual catalog identity, where exact
package lineage remains replayable.

### Put values in the catalog

Rejected. Scalar samples, field arrays, GPU buffers, and distributed fields
have different layout and ownership contracts. A universal result payload
would turn the catalog into an anything-box and couple model-facing identity
to one realization.

### Add the observation binding to the producing Run outputs

Rejected. The binding names the Run digest, so making the Run name the binding
would create an identity cycle. The post-run edge is the same natural shape as
other orthogonal provenance bindings.

## Compatibility and migration

Both wires are new optional pre-release artifacts. Existing Model,
Transaction, Realization, Run, package, compilation, and package-lineage bytes
and digests are unchanged. A package compilation with no eliminated physical
exposure produces no catalog rather than an empty artifact.

The schemas are closed v1 contracts. Supporting another source-lineage family,
Run schema, projection quantity, or contract kind requires an explicit version
decision; a free-form schema string cannot widen v1.

## Verification

The registered
[`packages.hierarchical-physical-boundary`](../verify/packages/hierarchical-physical-boundary/README.md)
case must:

1. seal both eliminated scalar terminals of one exact dependency-defined
   wrapper;
2. prove N-ary and partitioned source forms preserve each projection digest and
   interior cut while their exact source-lineage catalogs remain distinct;
3. round-trip canonical catalog bytes and replay the exact package resolution,
   compilation, Model, compiler cut, and provenance;
4. reject a catalog from the other exact package compilation even when Model
   and projection meaning agree;
5. bind both `Common` and `NetOutward` to one existing output of an exact
   Model-matched Run v1 and prove distinct binding identities; and
6. reject a result digest absent from the Run output set.

The registered
[`packages.field-valued-boundary-interface`](../verify/packages/field-valued-boundary-interface/README.md)
case must:

1. eliminate two 2D `[2]` field-valued wrapper exposures;
2. retain distinct exact boundary supports with the shared nominal Connector;
3. round-trip and exactly replay the package-qualified catalog; and
4. reject substitution of the coincident peer boundary support.

Contract validation additionally rejects malformed identities, noncanonical
ordering, unknown fields and variants, retained exposure IDs,
absent/nonconserving Connections, nonmember or whole-Connection cuts,
Connector/support mismatch, resource-limit overflow, and wrong
Model/revision/catalog/projection/Run links. The binding wire admits only the
closed Run v1/v2 variants; the registered physical end-to-end evidence is v1
for the version-neutral identity reason above.

## Security, safety, and governance

Decoders are byte- and count-bounded, deny unknown fields, and perform no file,
network, package, result, or code loading. Source paths are inert provenance,
bounded UTF-8 without control characters; they grant no filesystem authority.
Every externally supplied catalog remains untrusted until exact API replay has
reconstructed it from the admitted package compilation.

An observation binding is lineage, not an execution attestation, acceptance
record, trust signature, or proof that its output contains the named quantity.

## Nonclaims

This RFC does not define numerical value payloads, array ownership, sampling
location, time axes, mesh/facet IDs, discrete trace spaces, quadrature, field
transfer, nonmatching interfaces, interpolation, units presentation,
visualization, execution attestation, or numerical acceptance. It does not
claim a field-valued solve, elasticity, Stokes flow, or FSI.

## Unresolved questions

The first typed scalar and field result payloads should bind this identity
without copying its semantic contract. End-to-end physical Run v2 remains
blocked on the versioned Realization/Model boundary owned by
[RFC 0037](0037-version-neutral-model-artifact-reference.md). Mesh-associated
field observations and conservative transfer require separate
Realization-owned contracts.
