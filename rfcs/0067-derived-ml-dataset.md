# RFC 0067: Derived ML Dataset over durable spatial trajectories

- Status: Implemented
- Authors: Eqiora contributors
- Created: 2026-07-22
- Depends on: [RFC 0051](0051-durable-spatial-state-and-trajectory.md),
  [RFC 0065](0065-remeshing-correspondence-and-transfer.md), and
  [RFC 0066](0066-remeshing-trajectory-xdmf-hdf5-export.md)

## Summary

The first ML-facing Dataset is a typed, immutable derivation from one exact
remeshing-aware `SpatialTrajectoryEnvelopeV3`. It assigns feature and target
roles, selects deterministic windows, fixes disjoint time-ordered
train/validation/test partitions, records training-only normalization
statistics, and materializes bounded owned CPU arrays without hiding the
ragged topology created by remeshing.

It does not reinterpret XDMF, HDF5, framework tensors, solver output, or an
identity-only `DatasetViewEnvelopeV1` as Dataset meaning.

## Motivation

RFC 0051 deliberately made `DatasetViewEnvelopeV1` a fixed-spatial,
reference-only selection. It copies no values and assigns no ML roles, split,
or fitted preprocessing. Widening that artifact after remeshing support would
mix durable observation identity with one consumer's interpretation.

The missing seam is not a generic array container. It is an exact derivation:

```text
canonical V2 -> V3 trajectory and dependency graph
    -> typed feature/target and window contract
    -> time-ordered split lineage
    -> training-only normalization statistics
    -> explicit ragged owned CPU materialization
```

The remesh seam is essential. If the first Dataset contract assumes stable
array width, a later implementation must either pad unrelated degrees of
freedom, silently interpolate them, or replace the identity contract. None is
acceptable for an evidence-gated derivation.

## Decision

### A distinct derived artifact

`DatasetViewEnvelopeV1` remains unchanged. `MlDatasetEnvelopeV1` is a new
domain-separated artifact because feature/target roles, windows, partitions,
and fitted statistics are ML interpretation rather than spatial-state
identity.

The artifact references the exact `SpatialTrajectoryEnvelopeV3` and records
the exact state, snapshot, and logical coefficient-block identities selected
by every sample. It contains descriptors and statistics, but no copied field
values, external storage paths, framework types, or device placement.

### One exact V2-to-V3 replay profile

The application layer replays the existing closed V2-to-V3 dependency graph.
The source V2 tip and first V3 remesh target share `(step, time)`; the target
replaces the source tip in the Dataset frame sequence:

```text
source V2 frames before the tip
    + V3 remesh target
    + V3 continuation frames
```

The resulting frames must have strictly increasing time. This replacement is
part of the profile, not a general merge policy and not a new universal
trajectory trait.

### Typed descriptors and windows

Each descriptor has one closed role (`feature` or `target`), a nonnegative
window offset, and exact physical meaning derived from its referenced Field
snapshots:

- Semantic Field and support Domain identity;
- coherent-SI dimension;
- scalar or fixed-vector value shape;
- component frame; and
- canonical coefficient-block associations and component counts.

Descriptors are unique in `(role, offset, Field)` and ordered by role
(features first), offset, then Field identity. Every descriptor must be
present with unchanged meaning in every selected frame. The same Field may be
used in both roles or at several offsets, but duplicate descriptors fail.

The Dataset fixes one positive window length. Each sample records a start
frame ordinal and therefore selects exactly that many consecutive frames.
Every descriptor offset must lie within the window. Missing, repeated, or
nonmonotone frames fail rather than changing the meaning of an offset.

### Time-ordered split before preprocessing

Every sample has one closed partition: `training`, `validation`, or `test`.
All three are nonempty. Samples retain source-time order, and partition order
is monotone:

```text
training -> optional gaps -> validation -> optional gaps -> test
```

No state artifact may occur in more than one partition, including through
overlapping windows. This is deliberately stronger than requiring distinct
sample rows: a shared accepted state would leak the same observation across
an evaluation boundary.

Random, grouped, and cross-validation policies are later typed adapters, not
open strings in the v1 wire.

### Active support and ragged blocks

Every selected snapshot retains its exact Vertex and/or Cell coefficient
blocks. Materialization includes only the active support closure:

- a Cell block visits the support Domain's cells; and
- a Vertex block visits the sorted vertex closure of those cells.

Full-mesh zeros outside that closure are validation padding in the durable
block, not observations. They do not enter statistics or materialized arrays.

Each materialized block is therefore an owned row-major `f64` array with
shape `[active entity, component]` and explicit lineage:

```text
sample / role / offset / Field / state / snapshot / block / mesh
association / active entity indices / component count / values
```

Blocks may have different entity counts after remeshing. V1 preserves this
truth as a collection of ragged owned blocks. It does not pad, interpolate,
coarsen, or pretend that an entity index on one mesh identifies an entity on
another mesh.

### Training-only population standardization

V1 has one transformation:

```text
training-population-standard-score
```

For each `(role, Field, block association, component)` channel, the producer
visits active training values in canonical sample/offset/entity order and
records count, physical mean, population standard deviation, applied scale,
and whether the channel was constant. Validation and test values never enter
the fit.

Population standard deviation uses `ddof = 0`. A zero-variance channel is not
an error: its applied scale is exactly `1`, the channel is marked constant,
and subtracting the mean produces zero. Counts, means, deviations, scales,
and constant markers are independently recomputed during replay. Non-finite
values or statistics fail.

The manifest names the `ordered-welford-binary64-v1` accumulator profile.
Values are visited only in the canonical order below. Constant classification
tracks equality of the finite input values separately from the computed
deviation; a nonconstant channel whose deviation underflows to zero fails
rather than being mislabeled constant. Cross-architecture bit identity remains
a nonclaim, but a changed accumulator is therefore an explicit new profile
rather than an unexplained identity drift.

The descriptor retains the input physical type; standardized values are
dimensionless.

### Canonical materialization order and bounds

Samples remain in strict source-time order. Within a sample, materialization
uses:

```text
feature before target
    -> window offset
    -> Field identity
    -> Vertex before Cell
    -> active entity identity
    -> component
```

Every output is an explicit owned copy. A caller supplies a maximum scalar
count; checked work accounting completes before any result-sized allocation.
There is no zero-copy, lazy I/O, hidden borrow, or stable dense-width claim.

### Storage independence

Only logical state, snapshot, mesh, and `DiscreteFieldEnvelopeV1` identities
enter the Dataset artifact. `DiscreteFieldStorageEnvelopeV1`, XDMF paths,
HDF5 dataset names, file hashes, chunking, compression, and container bytes do
not. Rechunking or repackaging equal logical blocks must reproduce the exact
Dataset artifact and materialized values.

An adapter may independently verify a storage projection before requesting
materialization, but successful storage replay grants no Dataset identity.

### Layer ownership

```text
eqiora-artifact  closed Dataset wire and bounded local validation
eqiora-api       exact trajectory replay, derivation, materialization, replay
eqiora           curated Dataset operations and artifact access
```

No new crate, universal resource registry, trajectory trait, or generic
ND-array abstraction is introduced. A standalone ML adapter crate is
justified only after a second execution backend needs a stable boundary that
cannot remain a small pure L4 projection.

## Prior art and differences

The design follows current scikit-learn guidance to split before fitting
preprocessing and learn normalization only from training data. Its ordered
partitions follow the purpose of `TimeSeriesSplit`, while Eqiora additionally
rejects shared state identity across partitions. Population standardization
and the constant-channel scale of one match `StandardScaler`.

MLCommons Croissant 1.1 models named Dataset splits and content checksums.
Eqiora retains the reproducibility goal but keeps distribution metadata and
file containers outside logical Dataset identity.

Apache Arrow specifies an efficient language-neutral memory format. Eqiora
v1 fixes only a small owned ragged CPU evidence layout; Arrow, DLPack, NumPy,
and framework tensors remain adapters over the same typed descriptors.

References:

- [scikit-learn common pitfalls and data leakage](https://scikit-learn.org/stable/common_pitfalls.html)
- [scikit-learn `TimeSeriesSplit`](https://scikit-learn.org/stable/modules/generated/sklearn.model_selection.TimeSeriesSplit.html)
- [scikit-learn `StandardScaler`](https://scikit-learn.org/stable/modules/generated/sklearn.preprocessing.StandardScaler.html)
- [MLCommons Croissant 1.1 specification](https://docs.mlcommons.org/croissant/docs/croissant-spec-1.1.html)
- [Apache Arrow columnar format](https://arrow.apache.org/docs/format/Columnar.html)

## Alternatives considered

### Widen DatasetView v1

Rejected. It would retroactively turn a fixed-spatial identity selection into
an ML policy object and weaken the RFC 0051 boundary.

### Read XDMF/HDF5 as Dataset authority

Rejected. RFC 0066 is a storage projection with deliberately narrower
presentation semantics. Reading it would discard canonical support, basis,
and hidden coefficient meaning and invert the authority boundary.

### Require one stable dense width

Rejected. It would make the first ML contract unable to express its immediate
remeshing dependency and force a later identity-breaking revision.

### Pad every remeshed state to one maximum width

Rejected. Array shape would depend on an arbitrary collection and padded
indices would not denote the same physical degrees of freedom.

### Store arbitrary transform JSON

Rejected. It creates an anything-box whose units, leakage behavior,
invertibility, and replay rules cannot be checked.

### Randomly split windows

Rejected for the first temporal profile. It is order-sensitive and can place
the same source state on both sides of an evaluation boundary.

## Compatibility and migration

No existing Model, Realization, Field, state, trajectory, Dataset view,
storage, XDMF, or HDF5 bytes change. V1 is a new domain-separated artifact and
additive facade surface. Future transforms, interpolation policies, split
policies, or storage adapters require new versioned contracts and cannot
widen v1.

## Verification

The first registered case uses the existing remeshing fixture's three strict
time frames: source pre-tip, V3 remesh target, and V3 continuation. Singleton
windows become training, validation, and test samples. Fluid velocity is the
feature, including its active Vertex and MINI Cell-bubble blocks; fluid
pressure is the target.

The case must prove:

- exact V2/V3 root, segment, state, snapshot, block, mesh, and descriptor
  replay;
- remesh target replacement and strict source-time order;
- declaration-order invariance for dependency catalogs and descriptors;
- nonempty ordered partitions with no shared state across partitions;
- statistics agree with an independent two-pass training-raw-value oracle,
  including constant channels, while population counts exclude held-out data;
- exact active-support entity sequences and complete ragged normalized values
  agree with an independent fixture oracle;
- the hidden cell-bubble block remains present despite XDMF presentation;
- no external storage identity occurs in the Dataset wire or derivation input,
  while the sibling HDF5 audit retains the same hidden logical bubble; and
- bounded decoding and materialization before result allocation.

It must reject stale roots, missing or duplicate dependencies, nonmonotone or
out-of-range windows, overlapping partition state identity, normalization
leakage, physical type/support/block drift, non-finite values, full-mesh
padding treated as observations, and substituted expected arrays or
manifests.

Registered evidence:

- [`artifacts.ml-dataset-remeshing-2d`](../verify/artifacts/ml-dataset-remeshing-2d/README.md)

## Nonclaims

- interpolation, padding, pooling, coarsening, reference-grid projection,
  geometry features, or differential feature generation;
- dynamic/ragged batching into one dense tensor;
- arbitrary transforms, categorical data, masks, weights, or missing values;
- random/grouped/stratified/cross-validation split policies;
- training pipelines, model registries, metrics, or experiment tracking;
- Arrow, Parquet, Croissant emission, NumPy zero-copy, DLPack, JAX, PyTorch,
  GPU loaders, distributed shuffling, or online learning;
- licensing, consent, retention, access control, or trust signatures; and
- production scale or cross-architecture bit-identical floating statistics.
