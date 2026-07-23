# RFC 0025: Discrete field and external-import provenance

- Status: Shared contract, static caller-resolved XDMF import, and bounded native HDF5 file-image resolution implemented and verified; temporal XDMF and other formats pending
- Authors: Eqiora contributors
- Created: 2026-07-19

## Summary

Eqiora will represent an accepted mesh-associated array as one invariant-checked
`DiscreteFieldPayload`, give that payload a mesh-bound
`DiscreteFieldEnvelopeV1` content identity, and record how external syntax and
storage were asserted to produce accepted artifacts in a separate
`ExternalImportManifestV1`; only format-specific deterministic replay upgrades
that assertion to derivation evidence.

## Motivation

Field-bearing XDMF/HDF5 and VTU adapters need one shared destination before
either format is implemented. The two existing concepts named "field" do not
provide it:

- Semantic `FieldDef` is a scalar model unknown with physical dimension and
  optional initial value. It does not select a mesh or store discrete values.
- `ScalarFieldSummary` is a bounded L4 result projection containing location,
  count, minimum, and maximum. It deliberately cannot reconstruct the result
  array and is not an artifact authority.

Putting arrays in a format adapter would let XDMF and VTU assign different
meaning to the same values. Putting names, paths, HDF5 layout, and parser state
in the field artifact would instead make storage accidents part of numerical
content identity. A universal resource object would hide both mistakes behind
untyped metadata.

The required separation is:

```text
external syntax + storage
          |
          v
typed, bounded format adapter ----> ExternalImportManifestV1
          |                                  |
          |                         adapter-specific replay
          |                                  |
          |                                  v
          |                         verified lineage handle
          |
          v
MeshTopology + DiscreteFieldPayload
          |
          v
mesh artifact + DiscreteFieldEnvelopeV1
```

The lower path is accepted content. The side path is provenance. Neither is a
substitute for the other.

## Proposed design

### Ownership and layer boundary

`eqiora-meshing` will own `DiscreteFieldPayload` beside `MeshTopology`. This is
the lowest existing layer that can check vertex and top-dimensional cell
counts without depending on a file format, artifact schema, numerical method,
or Semantic Model. The type is immutable and owns its entity-major `f64`
buffer.

`eqiora-artifact` will own `DiscreteFieldEnvelopeV1` and
`ExternalImportManifestV1`. The former gives accepted affine-simplex field
content a portable identity; the latter records a typed lineage assertion.
Format adapters remain at L3 and expose only Eqiora-owned plans and payloads.
They do not depend sideways on the L3 artifact crate. The public L4 import
workflow composes the selected adapter with artifacts, owns the opaque verified
lineage handle, and is the only boundary that may claim a successful replay.
This preserves a directed dependency graph without moving format syntax into
artifact identity.

The general in-memory payload accepts any `MeshTopology`. The v1 portable
envelope is deliberately narrower: it binds only
`SimplicialMeshEnvelopeV1`, the sole typed mesh artifact family currently
available. Another mesh family requires its own typed artifact and an explicit
field-envelope version or closed mesh-reference variant; a generic digest is
not enough.

No new crate is justified by this first scalar/fixed-vector contract. A future
L2 field-data crate requires at least one additional independent consumer and
a boundary that cannot be expressed without making `eqiora-meshing` incoherent.
There is no `Resource { metadata, arbitrary_json }` escape hatch.

### `DiscreteFieldPayload`

The closed v1 vocabulary is:

```text
Association = Vertex | Cell

ComponentShape = Scalar
               | Vector { components: NonZeroU32 }

DiscreteFieldPayload = {
    association,
    component_shape,
    entity_count,
    values,            // flat, entity-major f64
}
```

`Cell` means a value associated with each top-dimensional mesh cell. It does
not assert geometric centering, finite-volume meaning, discontinuity, or a
quadrature location. `Vertex` means the canonical dimension-zero entity order.
For a topology of dimension `d`, construction requires respectively
`entity_count(0)` or `entity_count(d)` from the supplied `MeshTopology`.

`Scalar` has one component. It remains distinct from `Vector { components: 1
}`: rank is part of shape and therefore part of artifact identity. Vector
components have no basis, variance, symmetry, unit, or tensor convention in
v1. Component `j` of entity `i` is stored at
`values[i * component_count + j]`.

Construction is valid only if:

1. the selected mesh stratum exists and its count is non-zero;
2. the component count is positive and representable as both `u32` and local
   `usize`;
3. `entity_count * component_count` succeeds under checked arithmetic;
4. `values.len()` equals that product exactly; and
5. every value is finite.

All zero values are normalized to positive zero before ownership is accepted.
No source grid name, attribute name, path, URI, dataset path, parser object,
layout, adapter identity, or run identity is retained by the payload.

The payload has no portable digest on its own. It was checked against a mesh
shape in memory, but only the envelope below binds it to one exact mesh
revision.

### `DiscreteFieldEnvelopeV1`

The wire schema identifier is `eqiora.discrete-field-envelope/v1`; its encoding
is `eqiora.canonical-json/v1`. Its private ordered DTO contains exactly:

```text
schema
encoding
mesh_sha256
association
component_shape
entity_count          // portable u64
values                // flat, entity-major f64
```

`association` is `vertex` or `cell`. `component_shape` is the closed tagged
form `{ "kind": "scalar" }` or
`{ "kind": "vector", "components": <positive u32> }`. Unknown fields,
variants, and scalar kinds fail closed.

Creation accepts a `SimplicialMeshEnvelopeV1` and an already checked
`DiscreteFieldPayload`. It rechecks the association count against that mesh and
stores the mesh envelope's exact digest. A decoded field envelope is not
admissible as an input or result until `validate_mesh_artifact` has checked:

- the supplied mesh envelope digest equals `mesh_sha256`;
- the supplied mesh's vertex or top-cell count equals `entity_count`; and
- the payload invariants still hold.

This follows the existing independently loaded Realization/mesh linkage: a
valid reference is not proof that referenced content is available.

The field digest is domain-separated SHA-256 over the schema identifier, a zero
byte, and the complete canonical envelope bytes. It therefore changes when the
mesh digest, association, component shape, entity count, or any canonical value
changes. It does not change when only an external name or storage layout
changes.

Finite `f64` values use the workspace's round-trip JSON path: encode, decode,
and re-encode must preserve the identical binary64 value and canonical bytes.
Because zero sign has no declared meaning in this schema, constructors
normalize `-0.0` to `+0.0`; a decoded wire containing negative zero is rejected
as non-canonical. NaN and infinities are rejected before artifact creation.

### Canonical resolved-array references

An import manifest distinguishes original sources, the arrays actually
resolved from them, and the artifacts accepted after Eqiora validation. A
resolved array is hashed from this exact private ordered DTO:

```text
schema    = "eqiora.resolved-array/v1"
encoding  = "eqiora.canonical-json/v1"
scalar    = "u64" | "f64"
shape     = non-empty positive u64 dimensions
values    = flat row-major JSON integers or numbers
```

The field order shown above is the canonical DTO order. Shape order is
meaningful and retained. The scalar tag selects exactly one value grammar:
`u64` admits JSON integers in `[0, 2^64 - 1]`; `f64` admits finite binary64
values through the round-trip JSON path and normalizes every zero to `+0.0`.
A decoded resolved-array DTO containing `-0.0`, a non-integer under `u64`, or a
value inconsistent with its scalar tag fails closed. Products and local
conversions are checked, and `values.len()` must equal `product(shape)`.

Its digest is SHA-256 over:

```text
UTF-8("eqiora.resolved-array/v1") || 0x00 || canonical DTO bytes
```

This identifies the normalized array presented to Eqiora, not HDF5 chunks,
VTK offsets, compression blocks, byte order, or a source dataset object. It is
a manifest reference, not a new general array artifact or large-field storage
claim.

### `ExternalImportManifestV1`

The schema identifier is `eqiora.external-import-manifest/v1`, with canonical
encoding `eqiora.canonical-json/v1`. Its private ordered top-level DTO contains
the fields below in exactly this order:

```text
schema                  "eqiora.external-import-manifest/v1"
encoding                "eqiora.canonical-json/v1"
adapter                 { id: text, version: text }
runtime_stack[]         {
    role: rust-binding | native-storage-library,
    implementation: text,
    version: text
}
selection               {
    grid: SelectedSourceEntityV1,
    attributes: SelectedSourceEntityV1[]
}
sources[]               {
    ordinal: u32,
    role: metadata-document | external-array-source,
    origin_selector: StructuralSelectorV1,
    display_locator: null | text,
    source_sha256: RawSourceSha256
}
resolved_arrays[]       {
    ordinal: u32,
    role: mesh-geometry | mesh-topology | field,
    source_ordinal: u32,
    origin_selector: StructuralSelectorV1,
    storage_display_selector: null | text,
    scalar: u64 | f64,
    shape: positive u64[],
    resolved_sha256: ArtifactDigest
}
accepted_artifacts[]    {
    ordinal: u32,
    role: mesh | field,
    artifact_sha256: ArtifactDigest
}

SelectedSourceEntityV1 = {
    selector: StructuralSelectorV1,
    display_name: null | text
}

StructuralSelectorV1 = { element_path: u32[] }
```

Text is bounded UTF-8 without control characters. Adapter IDs additionally use
stable lowercase dotted/kebab ASCII; versions are non-empty exact adapter
versions, not compatibility ranges. `runtime_stack` is empty for a pure
adapter. A native HDF5 resolver records the Rust binding and resolved native
HDF5 library in outer-to-inner call order. Duplicate `(role, implementation)`
entries are rejected. No library handle, ABI object, host name, or installation
path crosses the wire.

A structural selector is adapter-relative, not a source display name. Its path
indexes element children from the metadata document root in source order;
comments, whitespace, attributes, and text nodes do not consume indices. The
empty path denotes the complete metadata document and is valid only for source
ordinal zero. Every other selector is non-empty. The exact element tags
expected at each path are fixed by the named adapter version and rechecked by
replay. Thus an unnamed XDMF Grid/Attribute or VTK DataArray remains selectable,
and duplicate display names remain unambiguous. Reformatting XML cannot change
a selector; changing element order can, and also changes source provenance.

The selected grid is one structural selector. Attributes occur in explicit
caller selection order, not parser-map or lexical-name order. Repeating the
same attribute selector is rejected, while missing or duplicate display names
are retained as `null` or repeated text. A display name is inspectable
provenance only and never selects content.

`sources` has contiguous ordinals starting at zero. Ordinal zero is exactly one
`metadata-document` reference whose selector is the empty path. Remaining
`external-array-source` entries occur in first-use order of the normalized
resolved-array plan, one entry per external-reference occurrence. Repeated
source bytes or display locators are retained as separate ordinals rather than
deduplicated; their structural origin selectors must be distinct. All later
uses of that occurrence refer to its one ordinal.

`source_sha256` is raw SHA-256 of the complete original logical source byte
stream, with no schema prefix or domain separator. Eqiora computes it while
consuming a complete byte stream supplied through the source/resolver binding;
it never accepts an unchecked resolver-declared digest in place of bytes.
`RawSourceSha256` is a distinct Eqiora-owned 32-byte value serialized as 64
lowercase hexadecimal characters; it cannot be substituted for the
domain-separated `ArtifactDigest` type.
The resolver must couple array access and the complete raw-byte reader under
one binding. The manifest records that binding's result but does not establish
that an untrusted custom resolver described an authentic external object.
`display_locator` may be a redacted logical name, path, or URI, enters only
manifest identity, is never dereferenced during decoding, and never enters
accepted mesh or field identity.

`resolved_arrays` also has contiguous ordinals. The normalized import plan
orders mesh geometry first, mesh topology second, then selected field arrays in
the explicit attribute-selection order. Every entry records its closed role,
source ordinal, structural origin selector, optional display storage selector,
scalar, shape, and canonical resolved-array digest. Origin selectors must be
unique. A display storage selector such as an HDF5 dataset path or VTU appended
offset is provenance only; replay locates the declaring metadata element by its
structural selector and validates the format-native reference found there.

`accepted_artifacts` has contiguous ordinals with the mesh first and fields in
the same selected-field order. Every entry has the closed role `mesh` or
`field` and a canonical artifact digest. Equal field digests may occur more
than once when differently named selected attributes resolve to equal accepted
content; ordinals retain that provenance without changing either field
artifact.

Order is deliberately part of manifest identity because it records the typed
import plan. Parser map iteration is never such an order. Reordering records
without matching contiguous ordinals or cross-references is invalid; changing
the explicit selection/resolver order may produce a different manifest while
leaving all accepted artifact digests unchanged.

The manifest constructor validates all ordinal, selector, cardinality, role,
shape, and reference invariants. Given source streams, resolved DTOs, and
accepted artifacts, it computes their digests rather than accepting unchecked
digest/value pairs. After decoding, `validate_references` recomputes the same
digests from separately loaded dependencies. This proves only that the
manifest names those exact independent objects. Cross-wired source A, resolved
array B, and accepted artifact C can still satisfy independent digest checks;
neither construction nor `validate_references` is derivation proof.

Each format must therefore have a separately named deterministic replay
operation before its manifest can graduate as evidence. Conceptually, the L4
public import workflow exposes:

```text
verify_external_import_v1(
    manifest,
    exact metadata bytes,
    caller-owned resolver plan,
    exact accepted mesh/field artifacts,
    limits,
) -> VerifiedExternalImportV1
```

The public function is namespaced for XDMF or VTU and rejects a manifest for
another adapter/version. Its pure L3 format adapter re-parses metadata and
reconstructs the typed import result; the L4 workflow resolves every structural
selector and external-source occurrence, hashes the complete raw sources,
reconstructs every canonical resolved-array DTO, rebuilds the
`SimplicialMeshEnvelopeV1` and `DiscreteFieldEnvelopeV1` values through their
invariant-checking constructors, and requires every manifest list and accepted
digest to match exactly. Only the L4 composition can return an opaque,
non-serializable `VerifiedExternalImportV1` tied to the manifest digest. The
adapter does not depend on `eqiora-artifact`, and the manifest alone remains a
typed lineage assertion rather than proof that a source derived an artifact.

The manifest digest is SHA-256 over
`UTF-8("eqiora.external-import-manifest/v1") || 0x00 || canonical complete DTO
bytes`. It is an Evidence & Artifact Graph edge, not a container for the
referenced bytes. Replay proves deterministic derivation through the admitted
adapter/resolver contract, not source authorship, a malicious custom resolver's
honesty, or native-library safety.

### Content identity versus provenance identity

The contracts intentionally give two truthful answers to "is this the same?":

- Two arrays are the same accepted field content only when their canonical
  mesh-bound field envelopes have equal bytes and digest.
- Two manifests contain the same lineage assertion only when their canonical
  bytes and digest are equal.
- An import operation has verified derivation provenance only when the named
adapter has replayed that exact manifest and produced a
`VerifiedExternalImportV1` for its digest.

The XDMF implementation exposes two deliberately distinct L4 operations.
`import_xdmf_v1` freshly produces a non-proof artifact set for persistence.
`verify_xdmf_import_v1` accepts an independently loaded expected manifest,
mesh, and ordered fields together with the retained metadata plan and current
caller-owned responses. It derives once and issues the opaque handle only when
the complete expected and derived values match. Repeating one derivation with
the same in-memory inputs is not treated as independent replay.

The optional native composition mirrors that separation through
`import_xdmf_hdf5_v1` and `verify_xdmf_hdf5_import_v1`. It changes only how the
immutable XDMF requests obtain normalized values: one audited file image
replaces caller-supplied typed responses, and exact binding/native-runtime
entries make the two provenance manifests intentionally distinct. Accepted
mesh and Field identities remain governed by the same constructors.

Renaming an XDMF Attribute, moving an HDF5 dataset, changing chunk placement,
using VTU instead of XDMF, or changing source bytes may change the manifest.
If the resolved values, association, shape, and accepted mesh digest remain
equal, the field envelope remains byte-for-byte equal. Conversely, equal source
digests do not permit a field artifact to bypass payload, mesh-linkage, or
adapter-replay validation.

### Resource limits and failure order

Artifact decoding extends the existing byte and nesting limits with independent
limits for field entities, components, scalar values, manifest text bytes,
sources, resolved arrays, array rank/scalars, runtime entries, and accepted
artifacts. Portable numeric extents convert to local `usize` through checked
arithmetic. Limits are caller policy with documented safe defaults; they are
not hidden wire-version maxima, so a trusted caller may raise them explicitly.

The manifest JSON decoder applies its global byte and nesting caps before
deserialization, then applies typed text, count, rank, and product limits
before artifact admission. Streaming format adapters additionally require
source-byte, XML node/depth/text, decompressed-byte, external-reference,
dataset, and decoded-work limits; their declaration-sized allocations require
checked counts and fallible reservation before allocation. Payload and manifest
validation does not make an upstream parser safe.

The XDMF 3 adapter produces a pure typed import plan from bounded XML and never
opens a path or URL. DTDs and external entities are disabled. External arrays
are obtained only through an explicit caller-owned resolver.

The optional `eqiora-io-hdf5` L3 adapter supplies the first native resolver
without making XDMF depend on HDF5. Its authority is one borrowed, complete,
caller-owned file image. It opens those bytes through HDF5's Core VFD
file-image facility and receives no path, directory, URL, or network
capability. The XDMF/HDF5 L4 composition currently requires every HDF
`DataItem` in one plan to name one common display locator and resolves the
complete request batch from that one image; the locator is checked for plan
coherence and is never dereferenced.

The native manifest adapter is the complete
`eqiora.xdmf-hdf5.file-image` composition, not its Rust binding. The dependency
boundary is exact `hdf5-metno` 0.13.0 with static bundled HDF5, whose current
observed runtime is 2.1.0. The binding and observed native runtime versions are
separate ordered entries in
`ExternalImportManifestV1.runtime_stack`, so changing either changes import
provenance rather than accepted mesh or Field identity. Native handles and
binding types do not cross the adapter boundary.

One serialized native operation fixes the native VOL, saves and disables HDF5
plugin loading, opens the in-memory image, audits and preflights, reads the
admitted batch, closes every child handle, and restores the prior plugin state.
Before the first value read, the resolver traverses the complete reachable
hard-link tree under independent source, link, object, dataset, name, rank,
declared-value, decoded-byte, request, and audit-work limits. It rejects
aliases and cycles; non-hard links; attributes and committed datatypes whether
linked or unlinked; virtual or externally stored datasets; filter pipelines;
and every datatype except exact transient standard `u64` or IEEE binary64
`f64`. Every requested path, scalar type, and positive shape is then checked
before any requested dataset is read.

This is an in-process authority and grammar boundary, not a native sandbox.
HDF5 effects caused by a hostile process environment before library
initialization, defects in HDF5 or its binding, and native internal work not
covered by Eqiora's explicit accounting remain nonclaims. Full hostile-file
containment requires a later isolated worker/process boundary. Temporal XDMF
Collections remain an independent format-adapter slice.

VTU is a later independent adapter over the same payload and manifest. Its
ASCII, inline-binary, appended, compression, multi-piece, and parallel variants
graduate separately.

## Alternatives considered

### Put field arrays in each format adapter

This has low initial implementation cost, but association, component shape,
zero handling, mesh linkage, and content identity would drift between XDMF and
VTU. It also prevents solver outputs from sharing the same accepted payload.
Rejected.

### Put `DiscreteFieldPayload` only in `eqiora-artifact`

This keeps wire code nearby but forces numerical producers and format adapters
to depend on an L3 provenance crate merely to construct an in-memory array.
It reverses the intended dependency from artifacts to invariant-checked L2
data. Rejected.

### Add a general resource or array crate now

A universal JSON resource loses type and invariant closure. A dedicated array
crate could become appropriate for chunked, distributed, tensor, or device
storage, but the first contract would contain one mesh-associated `f64` type
and no second independent abstraction. Rejected until evidence demonstrates a
real boundary.

### Include source names and units in field identity

Names are mutable presentation/provenance. Units and Semantic `FieldDef`
binding are meaningful but require conversion, dimension, and model-revision
contracts not present here. Including either now would conflate storage,
discrete values, and model meaning. Rejected from v1 rather than represented by
untyped optional metadata.

## Compatibility and migration

This RFC adds two independent artifact families and one L2 payload. It changes
no existing Semantic Model, mesh, Realization, run, checkpoint, transaction,
or Python wire bytes. `ScalarFieldSummary::CellCenter` is not reinterpreted as
`Association::Cell`; a future conversion must state why a cell-associated
array is summarized at a center.

Both v1 schemas are closed. Unknown fields, variants, scalar kinds, and major
versions fail closed. Future units, Semantic `FieldDef` binding, time axes,
tensor shapes, basis conventions, chunked storage, or distributed ownership
require a new schema identifier and explicit migration. New provenance cannot
be appended to v1 in a way that changes its canonical bytes.

`DiscreteFieldEnvelopeV1` permanently means a field linked to
`SimplicialMeshEnvelopeV1`. A future Cartesian, mixed-cell, surface, adaptive,
or distributed mesh artifact is not smuggled in under the same digest field;
it requires an explicit new field-envelope version or closed typed reference.

Migration always decodes and validates the old envelope against its referenced
mesh, constructs the new typed payload, and emits new bytes. A reader never
guesses a new association or shape from value count. Pre-release Rust type
layout may change independently; the accepted v1 wire meaning and production
rule do not.

## Verification

The registered
[`artifacts.discrete-field-import-provenance`](../verify/artifacts/discrete-field-import-provenance/README.md)
case verifies the shared contract through the public facade: Vertex scalar and
Cell vector identity, content/provenance separation, exact reference
cross-wiring rejection, invariant failures, and bounded decoding. The manifest
is verified only as a lineage assertion. It is not derivation proof and does
not graduate any format adapter.

The registered
[`artifacts.xdmf-hdf5-native-import`](../verify/artifacts/xdmf-hdf5-native-import/README.md)
case verifies the native file-image slice through the public facade: exact
batch resolution and independently persisted replay, exact binding/native
runtime provenance, accepted mesh/Field equality with caller-resolved XDMF,
and fail-closed rejection of forbidden metadata/storage features and explicit
resource-limit excess. It does not graduate temporal XDMF or native-process
containment.

The shared-contract and later adapter slices use the following
machine-readable falsifiers at their respective graduation gates:

1. node scalar and cell vector payloads round-trip to identical canonical
   bytes and digests under repeated construction;
2. source `-0.0` and `+0.0` normalize to the same field identity, while a wire
   containing negative zero, NaN, or infinity fails closed;
3. `DiscreteFieldPayload` accepts an independent `MeshTopology`, while v1
   artifact construction and linkage accept only the exact referenced
   `SimplicialMeshEnvelopeV1`;
4. different source names and storage layouts with equal resolved values
   produce identical field bytes/digests and distinct import manifests/source
   digests;
5. changing only the linked mesh digest changes field identity, and a forged
   mesh digest or equal-count wrong mesh is rejected by linkage validation;
6. swapping vertex/cell association is rejected when counts differ and creates
   a distinct valid artifact when counts happen to be equal;
7. scalar and one-component vector shapes remain distinct;
8. source SHA-256 agrees with a raw-byte oracle without a domain prefix, while
   resolved `u64` and `f64` array digests agree with exact ordered-DTO/domain
   oracles and reject scalar-tag, shape, order, and negative-zero drift;
9. unnamed and duplicate-named source entities remain unambiguous by structural
   selector; repeated selection of one selector fails, while repeated external
   source bytes/locators retain distinct deterministic occurrence ordinals;
10. relative to one independently persisted expected manifest, mesh, and field
    set, any source/selector/array/artifact substitution prevents a verified
    lineage handle; a caller resolver may instead rebind same-shaped values and
    create a distinct valid fresh import, because source-to-array derivation and
    resolver honesty are explicit nonclaims;
11. zero components, count/product overflow, shape mismatch, value-length
   mismatch, non-finite values, unknown variants/fields, dangling source
   ordinals, non-contiguous order, and every decoder-limit excess fail without
   panic or partial artifact admission; and
12. the implemented caller-resolved XDMF fixture and native-HDF5 fixture, plus
    future VTU fixtures, containing equivalent admitted data produce the same
    accepted mesh and field artifact identities but distinct format provenance
    manifests, each verified only by its own adapter replay.

Graduation gates are independent:

- **Contract:** L2 payload, both artifact schemas, public Eqiora-owned facade
  types, bounded decoders, and positive/negative unit tests exist. Only then may
  the matrix contract gate become present.
- **Execution:** one named adapter reaches accepted mesh and field artifacts
  through the public facade. This does not graduate another format.
- **Verification:** a reproducible case under `verify/` supports the exact
  named adapter/encoding claim, deterministic replay, cross-wire rejection,
  and hostile-input boundary. A manifest round-trip is insufficient.
- **Maturity:** multiple formats, large-field storage, stable compatibility,
  broad platform evidence, and operational security exist. No first slice
  satisfies this gate.

## Security, safety, and governance

All artifact, XML, and storage input is untrusted. Canonical digests provide
content addressing, not authorship, authenticity, malware scanning, or trust.
Display locators and source selectors can contain sensitive project structure;
callers may supply redacted logical identifiers before persisting a manifest.
Decoding a manifest never performs I/O.

The payload/envelope layer claims no containment of hostile XML, VTU binary
blocks, compressed data, native-library defects, or resource use outside its
explicit accounting. The native HDF5 adapter rejects the admitted metadata and
storage escape hatches described above and disables plugin loading during its
serialized operation, but it is not a native sandbox. Complete hostile
native-HDF5 containment is not claimed without a future isolated worker/process
boundary. Arbitrary filesystem traversal, network retrieval, implicit external
links, callbacks, and code execution are outside v1.

Public wire changes require RFC review and DCO-signed implementation. Format
dependencies remain isolated, optional, pinned by policy, and subject to the
repository's license, MSRV, and native-CI gates.

## Nonclaims

The first metadata implementation is deliberately limited to one bounded XDMF
3 Uniform Tri3/Tet4 plan. It supports both explicit caller-resolved HDF
references and the optional bounded native file-image resolver described
above. This RFC does not implement VTU, export, time series, collections,
multiple HDF5 source images in one native plan, mixed/high-order/curved cells,
physical units, Semantic
`FieldDef` binding, tensor or basis conventions, quadrature-point data, sparse
fields, interpolation, cell subentity data, chunking, compression,
lazy/partial reads, memory mapping, zero-copy, GPU residency, distributed
ownership, MPI I/O, or large-field durable storage.

It also does not claim hostile native-library containment, safety against a
hostile process environment established before HDF5 initialization, source
authenticity, display-locator reachability, or semantic equivalence beyond
exact canonical content identity. `ExternalImportManifestV1` construction,
round-trip, digest linkage, and reference validation alone do not prove that a
source derived an array or artifact. Only a separately verified, named adapter
replay supports that bounded claim; replay does not prove a malicious custom
resolver honest.

## Unresolved questions

- Which isolated-worker protocol will contain native HDF5 defects and process
  ambient authority without making native handles or paths part of the public
  import contract.
- Whether a later chunked field artifact uses a Merkle tree or an external
  content-addressed blob store; neither may reinterpret the v1 inline envelope.
- Which future contract binds a discrete payload to Semantic `FieldDef`, units,
  basis, and model revision without making those concerns optional metadata.
