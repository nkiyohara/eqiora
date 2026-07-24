use crate::JsonDecoderLimits;

/// Semantic work budgets for resolved arrays, import/export, and trajectory artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataExchangeDecoderLimits {
    /// Common JSON syntax admission.
    pub json: JsonDecoderLimits,
    /// Maximum rank of one canonical resolved-array reference.
    pub max_resolved_array_rank: usize,
    /// Maximum scalar values in one canonical resolved-array reference.
    pub max_resolved_array_values: usize,
    /// Maximum dynamic UTF-8 text bytes summed across one external-import manifest.
    pub max_import_manifest_text_bytes: usize,
    /// Maximum native runtime components in one external-import manifest.
    pub max_import_runtime_entries: usize,
    /// Maximum selected attributes in one external-import manifest.
    pub max_import_selection_attributes: usize,
    /// Maximum source occurrences in one external-import manifest.
    pub max_import_sources: usize,
    /// Maximum normalized array references in one external-import manifest.
    pub max_import_resolved_arrays: usize,
    /// Maximum accepted artifact references in one external-import manifest.
    pub max_import_accepted_artifacts: usize,
    /// Maximum runtime components in one external-export storage envelope.
    pub max_trajectory_storage_runtime_entries: usize,
    /// Maximum frames in one external-export storage envelope.
    pub max_trajectory_storage_frames: usize,
    /// Maximum Field entries summed across one external-export envelope.
    pub max_trajectory_storage_fields: usize,
    /// Maximum coefficient blocks summed across one external-export envelope.
    pub max_trajectory_storage_blocks: usize,
    /// Maximum dynamic UTF-8 text bytes in one external-export envelope.
    pub max_trajectory_storage_text_bytes: usize,
    /// Maximum complete XDMF document bytes asserted by one trajectory-storage envelope.
    pub max_xdmf_storage_bytes: u64,
    /// Maximum complete HDF5 file-image bytes asserted by one trajectory-storage envelope.
    pub max_hdf5_storage_bytes: u64,
    /// Maximum v3 segments in one remeshing-aware trajectory root.
    pub max_remesh_trajectory_segments: usize,
    /// Maximum target states summarized by one remeshing-aware trajectory root.
    pub max_remesh_trajectory_states: usize,
    /// Maximum accepted state references in one immutable trajectory segment.
    pub max_trajectory_segment_states: usize,
    /// Maximum immutable segments referenced by one trajectory root.
    pub max_trajectory_segments: usize,
    /// Maximum accepted states summarized by one complete trajectory root.
    pub max_trajectory_states: usize,
    /// Maximum Field selections in one derived Dataset view.
    pub max_dataset_view_fields: usize,
    /// Maximum Field references summarized by one trajectory state.
    pub max_spatial_state_fields: usize,
}

impl Default for DataExchangeDecoderLimits {
    fn default() -> Self {
        Self {
            json: JsonDecoderLimits::default(),
            max_resolved_array_rank: 8,
            max_resolved_array_values: 16_000_000,
            max_import_manifest_text_bytes: 1024 * 1024,
            max_import_runtime_entries: 32,
            max_import_selection_attributes: 100_000,
            max_import_sources: 100_000,
            max_import_resolved_arrays: 100_002,
            max_import_accepted_artifacts: 100_001,
            max_trajectory_storage_runtime_entries: 32,
            max_trajectory_storage_frames: 16_384,
            max_trajectory_storage_fields: 1_000_000,
            max_trajectory_storage_blocks: 2_000_000,
            max_trajectory_storage_text_bytes: 64 * 1024 * 1024,
            max_xdmf_storage_bytes: 16 * 1024 * 1024,
            max_hdf5_storage_bytes: 512 * 1024 * 1024,
            max_remesh_trajectory_segments: 100_000,
            max_remesh_trajectory_states: 1_000_000,
            max_trajectory_segment_states: 100_000,
            max_trajectory_segments: 100_000,
            max_trajectory_states: 1_000_000,
            max_dataset_view_fields: 100_000,
            max_spatial_state_fields: 100_000,
        }
    }
}
