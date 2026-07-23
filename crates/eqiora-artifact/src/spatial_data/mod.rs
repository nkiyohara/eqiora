//! Durable, storage-independent spatial observations and trajectories.
//!
//! The logical graph is deliberately narrow: existing mesh-bound discrete
//! fields remain the numeric leaves; snapshots add exact Semantic Field and
//! Realization meaning; states, immutable segments, and roots add accepted
//! time identity. Storage realization is a separate typed projection.

mod context;
mod dataset;
mod field;
mod ml_dataset;
mod state;
mod storage;
mod trajectory;

pub use context::ValidatedFixedSpatialContextV1;
pub use dataset::DatasetViewEnvelopeV1;
pub use field::FieldSnapshotEnvelopeV1;
pub use ml_dataset::{
    MlDatasetChannelStatisticsV1, MlDatasetDescriptorRoleV1, MlDatasetEnvelopeV1,
    MlDatasetFieldDescriptorV1, MlDatasetObservationReferenceV1, MlDatasetSampleSplitV1,
    MlDatasetSampleV1, MlDatasetStateKindV1, MlDatasetStateReferenceV1,
};
pub use state::SpatialStateEnvelopeV1;
pub use storage::{DiscreteFieldStorageEnvelopeV1, StorageChunkSha256V1, StorageChunkV1};
pub use trajectory::{SpatialTrajectoryEnvelopeV1, SpatialTrajectorySegmentEnvelopeV1};
