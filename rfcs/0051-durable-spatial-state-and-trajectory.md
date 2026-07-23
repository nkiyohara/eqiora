# RFC 0051: Durable spatial state and trajectory artifacts

- Status: Implemented and verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0013](0013-realization-and-run-provenance-wire.md),
  [RFC 0025](0025-discrete-field-and-import-provenance.md),
  [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md), and
  [RFC 0050](0050-fixed-reference-monolithic-fsi.md)

## Summary

Eqiora will record accepted spatial fields, complete multiphysics states, and
incrementally published trajectories as a small typed, content-addressed DAG.
The first implementation reuses `SimplicialMeshEnvelopeV1` and
`DiscreteFieldEnvelopeV1`; it does not introduce a second array meaning or a
universal scientific-data container.

The logical path is:

```text
SimplicialMeshEnvelopeV1
        ^
        |
DiscreteFieldEnvelopeV1[]
        ^
        |
FieldSnapshotEnvelopeV1[]
        ^
        |
SpatialStateEnvelopeV1[]
        ^
        |
SpatialTrajectorySegmentEnvelopeV1[]
        ^
        |
SpatialTrajectoryEnvelopeV1
        ^
        |
DatasetViewEnvelopeV1

RunManifestV2 -- output --> SpatialTrajectoryEnvelopeV1
```

Every edge is an exact digest reference checked again against separately
loaded typed content. Field values remain logical, coherent-SI values in exact
mesh entity order. Storage chunking is a separate, optional realization of
one canonical `DiscreteFieldEnvelopeV1` byte stream and never enters Field,
state, trajectory, or dataset-view identity.

The first evidence records two complete accepted states, `t1` and `t2`, from
two consecutive executions of RFC 0050's fixed-reference two-dimensional FSI
step. The initial `t0` input is not published as a complete state because it
has no accepted algebraic pressure Field; assigning it an invented zero
pressure would be a semantic fabrication.

## Decision boundary

This RFC defines durable physical observations. It does not define restart
state, storage-system object models, visualization metadata, or machine
learning semantics.

- A `FieldSnapshot` identifies one exact Semantic Field and the complete
  discrete coefficient blocks needed to reconstruct it in one exact
  Realization.
- A `SpatialState` identifies one accepted model-time point and the complete
  Field inventory selected by that Realization.
- A trajectory segment and root give immutable ordered publication and bounded
  partial retrieval.
- A `DatasetView` selects existing states and Fields without copying values or
  assigning feature, target, split, or normalization meaning.
- `ImplicitTimeCheckpointEnvelopeV1` remains a restart artifact in canonical
  lowering order. It is not a spatial observation and cannot substitute for a
  `SpatialState`.

The first wire generation is intentionally specific to the admitted shared
affine-simplex mesh, scalar or rank-one vector Fields, continuous P1, and
simplex P1-bubble spaces. New mesh families, tensor component grammars,
higher-order spaces, discontinuous spaces, ALE geometry states, and remesh
transitions require an explicit later wire version or closed typed variant.

## Existing authorities

The following contracts remain authoritative and are referenced rather than
copied:

- The sealed replayable-Model boundary reconstructs the exact Field definition,
  its coherent-SI dimension, mathematical shape, frame, and unique volume
  support without exposing one concrete envelope generation to consumers.
- `RealizationEnvelopeV3` identifies the exact Model, Domain/Field inventory,
  Field-to-space bindings, trace quotient, imported mesh, backward-Euler step,
  and execution policy.
- `GeometryIdentityEnvelopeV1` identifies the exact semantic bodies and
  boundaries in one geometry revision.
- `GeometryMeshCorrespondenceEnvelopeV1` proves their exact mesh cell and
  facet memberships.
- `SimplicialMeshEnvelopeV1` owns immutable affine coordinates, connectivity,
  acceptance policy, and recomputed quality evidence.
- `DiscreteFieldEnvelopeV1` owns one exact mesh digest, Vertex or Cell
  association, scalar or fixed-vector component shape, entity-major `f64`
  values, and its logical content digest.
- `RunManifestV2` owns exact Model, Realization, and execution provenance.

`ResolvedArrayV1` is not reused as a Field-value artifact. RFC 0025 defines it
as an external-import provenance reference with no mesh, Field association, or
storage authority. Promoting it to a general array would bypass the mesh-bound
invariants already present in `DiscreteFieldEnvelopeV1`.

## Canonical Field snapshot

### Multi-block representation

A Field snapshot is not necessarily one array. RFC 0050's fluid MINI velocity
has a continuous P1 vertex block and one interior bubble coefficient per fluid
cell. Omitting the bubble block would produce a different function even when
the remaining vertex values looked plausible.

`FieldSnapshotEnvelopeV1` therefore references a nonempty, closed coefficient
block list:

```text
schema                  "eqiora.field-snapshot-envelope/v1"
encoding                "eqiora.canonical-json/v1"
model_sha256            ArtifactDigest
semantic_revision       u64
realization_sha256      ArtifactDigest
geometry_sha256         ArtifactDigest
correspondence_sha256   ArtifactDigest
mesh_sha256             ArtifactDigest
support_domain_ulid     ULID
field_ulid              ULID
physical                {
    unit_system         coherent-si
    dimension           seven signed SI base exponents
    value_shape         ordered positive u32 extents; [] is scalar
    frame               invariant | spatial-cartesian
}
representation          {
    scalar              f64
    ordering            canonical-mesh-entity-major
    blocks[]            {
        association     vertex | cell
        discrete_field_sha256 ArtifactDigest
    }
}
```

The repeated block association is a typed manifest role used for bounded
indexing and partial retrieval. It must equal the independently decoded
`DiscreteFieldEnvelopeV1` association; it is not an unchecked annotation.
Blocks are canonically ordered `vertex`, then `cell`, and associations are
unique in v1.

The exact Realization space determines the required block signature:

```text
ContinuousLagrange { degree: 1 }  -> [vertex]
SimplexP1Bubble                  -> [vertex, cell]
```

All other spaces fail closed in this wire generation. The Model Field's scalar
shape must match a scalar discrete payload. A rank-one shape must match a
vector payload with the exact component count. Rank-two and higher shapes are
not flattened into an anonymous vector.

### Support closure

`DiscreteFieldEnvelopeV1` deliberately covers a complete mesh stratum.
`FieldSnapshotEnvelopeV1` adds the exact semantic support without changing
that lower-level contract.

The active top-dimensional cells are exactly
`GeometryMeshCorrespondenceEnvelopeV1::body_cells(support)`. Active vertices
are the canonical vertex closure of those cells. Every coefficient outside
that support closure must be canonical positive zero. Shared interface
vertices belong to both adjacent closures and retain their physical values in
both endpoint snapshots.

For a conforming trace quotient, the L4 FSI projection derives the exact shared
facet set and requires the two endpoint Field snapshots to have bit-identical
vertex coefficients there before publication. They remain distinct Semantic
Fields and distinct snapshot identities; equality at the interface does not
flatten them into one anonymous array. The generic state artifact validates
identity, lineage, and completeness without acquiring FSI-specific physics.

### Construction and replay

The control plane first constructs one borrowed
`ValidatedFixedSpatialContextV1`. It replays the version-neutral Model boundary
and validates the exact Realization, geometry, correspondence, mesh, and
physics-neutral represented-Field inventory once. Field and state constructors
accept this narrow runtime proof plus their local values; it is neither a wire
artifact nor a universal context object.

The Field constructor derives the support Domain, all physical metadata, and
the block signature. Callers cannot supply a unit string, frame string,
support index list, basis name, or ordering claim.

After decoding, `validate_against` receives the same independently loaded
typed resources and discrete Field envelopes. It replays Model meaning,
Realization binding, mesh and geometry lineage, component shape, support-zero
closure, and the complete block signature before the snapshot is trusted.

## Narrow storage realization

Logical Field identity must remain stable when identical canonical Field bytes
are split differently for transport or recovery. The first slice needs one
narrow storage witness to falsify missing chunks and storage substitution; it
does not need an HDF5, Zarr, object-store, or universal array model.

`DiscreteFieldStorageEnvelopeV1` realizes exactly one canonical
`DiscreteFieldEnvelopeV1` byte stream:

```text
schema                  "eqiora.discrete-field-storage-envelope/v1"
encoding                "eqiora.canonical-json/v1"
logical_field_sha256    ArtifactDigest
storage_encoding        canonical-discrete-field-json-bytes
total_bytes             u64
chunks[]                {
    ordinal             contiguous u32 from zero
    offset              contiguous u64 byte offset
    length              positive u64
    bytes_sha256        raw SHA-256 of the exact chunk bytes
}
```

Chunks are external byte payloads addressed by their raw byte digest; no path,
URI, bucket, dataset name, file handle, compression map, or parser object is
part of this manifest. V1 admits no compression and no byte-order choice
because its payload is already canonical JSON bytes.

The implementation uses a closed `StorageChunkSha256V1` value for
`bytes_sha256`. It validates exactly 32 raw SHA-256 bytes and remains distinct
from both domain-separated `ArtifactDigest` and external-source
`RawSourceSha256`; those identities are not mutually substitutable merely
because their wire spelling is lowercase hexadecimal.

Validation requires a nonempty chunk list, contiguous ordinals and offsets,
checked total length, exact raw hashes, and complete concatenation. The
concatenated bytes must decode as `DiscreteFieldEnvelopeV1`, re-encode to the
same canonical bytes, and produce `logical_field_sha256`. Missing, duplicated,
overlapping, truncated, reordered, or substituted chunks fail before a Field
snapshot is reconstructed.

Changing chunk boundaries changes the storage-envelope digest while retaining
the same `logical_field_sha256`. No logical artifact references a storage
envelope. A caller may provide the canonical discrete Field bytes directly or
resolve them through a matching storage envelope; both paths reconstruct the
same logical artifact.

Format-specific heavy-data storage remains the responsibility of later I/O
adapters. Those adapters must reconstruct the same logical discrete Field and
record their own typed provenance rather than extending this narrow schema
with arbitrary codec or locator metadata.

## Complete spatial state

`SpatialStateEnvelopeV1` has this closed logical form:

```text
schema                  "eqiora.spatial-state-envelope/v1"
encoding                "eqiora.canonical-json/v1"
model_sha256            ArtifactDigest
semantic_revision       u64
realization_sha256      ArtifactDigest
geometry_sha256         ArtifactDigest
correspondence_sha256   ArtifactDigest
mesh_sha256             ArtifactDigest
accepted                { step: u64, time_s: finite nonnegative f64 }
fields[]                {
    support_domain_ulid ULID
    field_ulid          ULID
    snapshot_sha256     ArtifactDigest
}
```

Field entries are canonically sorted by exact Field ULID and unique. Their set
must equal `RealizationEnvelopeV3`'s complete, physics-neutral represented-Field
inventory. That inventory includes algebraic physical Fields and
represented-but-eliminated time-state Fields, but excludes constraint
multipliers. Every referenced snapshot must repeat the exact state lineage and
key. A valid digest reference alone is insufficient.

The first FSI state therefore contains exactly these four distinct physical
Fields:

- fluid velocity, with P1 vertex and fluid-cell bubble blocks;
- fluid pressure, with one P1 vertex block;
- solid displacement, with one P1 vertex block; and
- solid velocity, with one P1 vertex block.

Algebraic constraint multipliers, reduced solver vectors, residual workspaces,
and backend reports are not physical Fields and do not enter the state.

Model time is expressed in coherent SI seconds. Constructors normalize zero
to positive zero. Decoders reject negative zero, NaN, infinity, negative time,
and steps above the exact binary64 integer range. State construction and
context-validated segment replay both require `time = step * duration`;
ordering is a trajectory invariant.

## Immutable trajectory segments

`SpatialTrajectorySegmentEnvelopeV1` stores one nonempty ordered set of state
references under one exact fixed-reference lineage:

```text
schema                  "eqiora.spatial-trajectory-segment/v1"
encoding                "eqiora.canonical-json/v1"
model_sha256            ArtifactDigest
realization_sha256      ArtifactDigest
geometry_sha256         ArtifactDigest
correspondence_sha256   ArtifactDigest
mesh_sha256             ArtifactDigest
fields[]                { field_ulid, support_domain_ulid }
states[]                {
    step                u64
    time_s              finite nonnegative f64
    state_sha256        ArtifactDigest
}
```

Input declaration order is non-semantic. Construction receives the validated
fixed-spatial context, sorts by step, then requires strictly increasing step
and time, no duplicate state digest, exact summary equality with every
referenced state, and exact context lineage. For this fixed
backward-Euler v1, adjacent entries additionally satisfy

```text
time[j] - time[i]
  = (step[j] - step[i]) * realization.time_step.duration
```

in the canonical binary64 path. A later variable-step trajectory requires an
explicit time-step identity instead of weakening this check.

A segment does not embed state or Field bytes. Its ordered index permits a
consumer to locate one state and then one Field without loading unrelated
states.

## Immutable trajectory roots

`SpatialTrajectoryEnvelopeV1` publishes complete immutable segment prefixes:

```text
schema                  "eqiora.spatial-trajectory/v1"
encoding                "eqiora.canonical-json/v1"
generation              u64
previous_root_sha256    null | ArtifactDigest
model_sha256            ArtifactDigest
realization_sha256      ArtifactDigest
geometry_sha256         ArtifactDigest
correspondence_sha256   ArtifactDigest
mesh_sha256             ArtifactDigest
fields[]                { field_ulid, support_domain_ulid }
segments[]              {
    first_step          u64
    last_step           u64
    first_time_s        f64
    last_time_s         f64
    state_count         u64
    segment_sha256      ArtifactDigest
}
```

The genesis constructor accepts one segment. Extension accepts the previous
trajectory and exactly one new segment. It copies the complete prior segment
prefix, appends one strictly later nonoverlapping segment, sets
`generation + 1`, and records the exact previous-trajectory digest. There is
no public arbitrary-replacement or in-place append operation.

Validation checks every range summary against its segment, the complete prefix
against the previous trajectory, exact fixed resources and Field inventory,
and strict step/time ordering across each segment boundary. Segment ranges
permit bounded lookup before segment retrieval.

A completed `RunManifestV2` lists the final trajectory digest as an output.
Ordinary typed Run validation proves exact Model and Realization provenance;
output membership proves publication. No spatial-specific binding artifact is
added because it would repeat facts already owned by the Run and trajectory.
The trajectory never contains its Run digest, so the content graph remains
acyclic.

Interrupted publication can leave unreferenced chunks, Fields, snapshots,
states, or a segment, but cannot alter an already accepted root. The only
append-like operation is publishing a new root digest after every referenced
object has been validated.

## Derived dataset view

`DatasetViewEnvelopeV1` is a no-copy logical selection:

```text
schema                  "eqiora.dataset-view-envelope/v1"
encoding                "eqiora.canonical-json/v1"
trajectory_sha256       ArtifactDigest
window                  { first_step: u64, last_step: u64 }
states[]                { step, time_s, state_sha256 }
field_ulids[]           ULID
transformation          identity
normalization           none
split                   unpartitioned
```

The window is inclusive and nonempty, and its endpoints must exist in the
exact trajectory. The view records every selected state reference rather than
relying on an ambient resolver. Field ULIDs are canonically sorted and unique,
and each must exist in every selected state. Deterministic materialization
order is accepted step, then Field ULID, then each snapshot's canonical block
and entity-major order.

The view contains no copied scalar values. V1 admits only the identity
transform. Feature/target roles, normalization statistics, train/validation/
test splits, shuffling, framework tensors, and materialization backends belong
to the later ML Dataset adapter. They cannot be smuggled into this schema as
arbitrary transform JSON.

## Content identity and canonicalization

Each envelope digest is domain-separated SHA-256 over:

```text
UTF-8(schema identifier) || 0x00 || canonical envelope bytes
```

The complete canonical manifest is in the digest domain. References therefore
form a Merkle DAG: changing a coefficient changes its discrete Field digest,
snapshot, state, segment, root, and derived view. Repacking unchanged canonical
Field bytes changes only the optional storage-envelope and chunk-byte
identities.

Constructors normalize every non-semantic input order before bytes exist:

- snapshot blocks use the closed association order;
- state Fields and DatasetView Fields use exact typed identity order;
- segment states use accepted-step order; and
- root segments are introduced only by prefix-preserving extension.

Decoded wire data must already be in that canonical order. Duplicate entries,
unknown fields or variants, uppercase or malformed digests, malformed ULIDs,
negative zero, and non-finite values fail closed rather than being repaired.

## Bounded decoding and retrieval

Every `from_json` first applies the existing byte and nesting limits and then
applies independent family limits. `DecoderLimits` gains at least:

```text
max_field_snapshot_blocks
max_field_storage_chunks
max_spatial_state_fields
max_trajectory_segment_states
max_trajectory_segments
max_trajectory_states
max_dataset_view_fields
```

Existing value-shape rank/component limits and discrete-Field entity,
component, and scalar-value limits remain authoritative. All portable integer
conversions, products, byte offsets, aggregate Field counts, segment summaries,
and traversal totals use checked arithmetic.

Decoding performs no filesystem, network, object-store, or transitive artifact
lookup. It validates only the closed local DTO. Explicit `validate_against`
operations receive independently loaded typed dependencies and a traversal
budget. Root validation checks aggregate segment/state limits before loading
all descendants. DatasetView resolution visits only intersecting segments,
selected states, selected snapshots, and their selected coefficient blocks.

## First vertical slice

The first registered evidence extends RFC 0050's fixed-reference CPU reference
path without changing its finalized operator.

1. Execute the same accepted backward-Euler Realization twice.
2. Convert the first solution into the complete previous state for the second
   execution.
3. Record complete accepted states `t1` and `t2`; do not assign a fabricated
   pressure to the input-only `t0` state.
4. Expand fluid bubble coefficients from fluid-cell order into canonical full
   mesh Cell order, using positive zero outside the fluid support.
5. Expand fluid pressure from its exact fluid-vertex order into canonical full
   mesh Vertex order, using positive zero outside the fluid support.
6. Project the shared vertex velocity separately onto the fluid and solid
   support closures, retaining bit-identical interface values and positive zero
   elsewhere.
7. Record solid displacement on the solid closure and positive zero elsewhere.
8. Put `t1` and `t2` in two immutable one-state segments, publish a genesis
   trajectory for the first, extend it with the second, make the final
   trajectory an exact Run output, validate ordinary Run lineage and output
   membership, and build an identity-only DatasetView selecting the complete
   short accepted window.
9. Store at least one logical discrete Field under two different admitted
   chunk partitions and prove equal logical Field identity but distinct
   storage-envelope identity.

The execution evidence, not the artifact constructor, proves that the values
came from the finalized FSI operator. Artifact validation proves their exact
typed identity and lineage after publication.

## Falsifying verification

The registered case must reject or distinguish:

- stale or cross-wired Model, Realization, Run, geometry, correspondence, or
  mesh references;
- an unknown Field, wrong support Domain, changed SI dimension, shape, frame,
  or component count;
- a missing MINI bubble block, duplicate block association, wrong block order,
  or a block bound to another mesh;
- a nonzero coefficient outside the exact support closure;
- unequal fluid/solid endpoint values on the exact conforming trace quotient;
- an incomplete or duplicate multiphysics Field inventory;
- invented `t0` pressure presented as an accepted complete state;
- duplicate, nonmonotone, or fixed-step-inconsistent state identities;
- an overlapping, reordered, missing, or substituted segment;
- a root extension that drops or mutates an accepted prefix;
- missing, duplicated, overlapping, truncated, reordered, or substituted
  storage chunks;
- a storage manifest whose concatenated bytes do not reproduce the exact
  logical discrete Field;
- equal logical Field content stored under different chunk partitions being
  mistaken for different physical data; and
- a DatasetView with a stale root, absent Field/window endpoint, copied value,
  or any transform other than identity.

Declaration order changes for snapshot blocks, state Fields, and DatasetView
Fields must reproduce identical canonical bytes. Partial retrieval must obtain
one selected Field from one selected state without loading unrelated states or
Field values.

## Alternatives considered

### One discrete Field per snapshot

Rejected. It cannot represent the admitted MINI velocity because its vertex
and cell-bubble coefficients occupy different mesh strata. Dropping the bubble
block would make a lossy observation look like an exact Field state.

### Introduce a general N-dimensional array artifact

Rejected. A free array would duplicate mesh association, shape, scalar,
canonical-zero, and resource checks already owned by
`DiscreteFieldEnvelopeV1`, while inviting basis, unit, support, device, and
storage metadata into an anything-box.

### Embed values in states or trajectory segments

Rejected. Recursive embedding defeats deduplication, partial retrieval,
bounded validation, and immutable prefix publication. Digest references keep
each family independently decodable and testable.

### Put chunking, compression, and locations in FieldSnapshot

Rejected. Repacking would then change physical-data identity. The narrow
storage envelope is optional and references one unchanged logical Field.

### Publish the input state as `t0` with zero pressure

Rejected. The previous-state contract has no accepted algebraic pressure.
Zero is not implied by absence, so such a snapshot would invent physical data.

### Add a spatial-specific Run binding artifact

Rejected. The ordinary Run already owns exact Model/Realization provenance and
the final trajectory output reference. A second artifact would add no fact.
Storing the Run digest directly in the trajectory is also rejected because it
would create a digest cycle once the Run names the trajectory output.

## Nonclaims

This RFC does not claim general transient CFD, variable-step integration,
moving geometry, ALE, remeshing, AMR, transfer operators, restart from a
spatial state, production-scale chunk tuning, compression, memory mapping,
streaming telemetry, concurrent writers, distributed parallel I/O, HDF5,
Zarr, XDMF, VTU, object-store semantics, arbitrary component tensors,
higher-order or discontinuous spaces, long-term archival guarantees,
visualization conventions, ML features or targets, normalization, dataset
splits, framework tensors, or training pipelines.

Those capabilities may consume this logical DAG. None may reinterpret it or
move adapter-specific storage meaning into the Semantic Model.
