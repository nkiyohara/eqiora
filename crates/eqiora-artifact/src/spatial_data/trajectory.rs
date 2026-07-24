//! Immutable spatial trajectory manifests.

mod common;
mod root;
mod segment;

pub use root::SpatialTrajectoryEnvelopeV1;
pub use segment::SpatialTrajectorySegmentEnvelopeV1;

/// Semantic work budgets shared by immutable trajectory and derived-view artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrajectoryDecoderLimits {
    /// Common JSON syntax admission.
    pub json: crate::JsonDecoderLimits,
    /// Maximum Field references summarized by one trajectory state.
    pub max_spatial_state_fields: usize,
    /// Maximum v3 segments in one remeshing-aware trajectory root.
    pub max_remesh_trajectory_segments: usize,
    /// Maximum target states summarized by one remeshing-aware trajectory root.
    pub max_remesh_trajectory_states: usize,
    /// Maximum state references in one immutable trajectory segment.
    pub max_trajectory_segment_states: usize,
    /// Maximum immutable segments referenced by one trajectory root.
    pub max_trajectory_segments: usize,
    /// Maximum accepted states summarized by one complete trajectory root.
    pub max_trajectory_states: usize,
    /// Maximum Field selections in one derived Dataset view.
    pub max_dataset_view_fields: usize,
}

impl Default for TrajectoryDecoderLimits {
    fn default() -> Self {
        Self {
            json: crate::JsonDecoderLimits::default(),
            max_spatial_state_fields: 100_000,
            max_remesh_trajectory_segments: 100_000,
            max_remesh_trajectory_states: 1_000_000,
            max_trajectory_segment_states: 100_000,
            max_trajectory_segments: 100_000,
            max_trajectory_states: 1_000_000,
            max_dataset_view_fields: 100_000,
        }
    }
}
