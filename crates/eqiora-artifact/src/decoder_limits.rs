/// Limits applied before and immediately after JSON decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderLimits {
    /// Maximum encoded bytes accepted.
    pub max_bytes: usize,
    /// Maximum JSON object/array nesting accepted before deserialization.
    pub max_nesting_depth: usize,
    /// Maximum Semantic Kernel nodes in one model envelope.
    pub max_nodes: usize,
    /// Maximum graph edges in one model envelope.
    pub max_edges: usize,
    /// Maximum expression nodes summed across one model envelope.
    pub max_expression_nodes: usize,
    /// Maximum expression roots summed across one model or transaction
    /// envelope.
    pub max_expression_roots: usize,
    /// Maximum pure-operator definitions summed across Relation residuals and
    /// Activation guards in one model or transaction envelope.
    pub max_pure_operator_definitions: usize,
    /// Maximum pure-operator formals summed across all expression-local
    /// definitions in one model or transaction envelope.
    pub max_pure_operator_formals: usize,
    /// Maximum exact component-calculus nodes summed across all
    /// expression-local definitions in one model or transaction envelope.
    pub max_pure_operator_calculus_nodes: usize,
    /// Maximum ordered arguments summed across all generic pure-operator
    /// applications in one model or transaction envelope.
    pub max_pure_operator_application_arguments: usize,
    /// Maximum Semantic Model members summed across model-view edit
    /// operations, or implied by one complete model envelope.
    pub max_model_view_members: usize,
    /// Maximum model-root boundary Ports summed across model-view edit
    /// operations, or stored by one complete model envelope.
    pub max_model_boundary: usize,
    /// Maximum rank of one exact Semantic Model value shape.
    pub max_value_shape_rank: usize,
    /// Maximum checked scalar components in one Semantic Model value shape.
    pub max_value_shape_components: usize,
    /// Maximum ordered operations in one model transaction envelope.
    pub max_transaction_ops: usize,
    /// Maximum atomic preconditions in one model transaction envelope.
    pub max_transaction_preconditions: usize,
    /// Maximum state dimension for exact rational rank replay in one time
    /// lowering envelope.
    pub max_exact_rank_dimension: usize,
    /// Maximum state dimension in a residual-native time artifact.
    pub max_time_state_dimension: usize,
    /// Maximum scalar root callbacks in one root registration envelope.
    pub max_root_functions: usize,
    /// Maximum vertices in one imported mesh artifact.
    pub max_mesh_vertices: usize,
    /// Maximum top-dimensional cells in one imported mesh artifact.
    pub max_mesh_cells: usize,
    /// Maximum coordinate scalars summed across an imported mesh artifact.
    pub max_mesh_coordinate_values: usize,
    /// Maximum connectivity indices summed across an imported mesh artifact.
    pub max_mesh_connectivity_indices: usize,
    /// Maximum body and boundary entities in one geometry identity artifact.
    pub max_geometry_entities: usize,
    /// Maximum cell and facet memberships in one geometry correspondence.
    pub max_geometry_mesh_memberships: usize,
    /// Maximum body decisions in one cross-revision geometry association.
    pub max_geometry_revision_associations: usize,
    /// Maximum positive-area fragments in one remesh overlap artifact.
    pub max_mesh_overlap_cell_fragments: usize,
    /// Maximum positive-length retained-facet fragments in one remesh overlap
    /// artifact.
    pub max_mesh_overlap_facet_fragments: usize,
    /// Maximum associated entities in one discrete field envelope.
    pub max_discrete_field_entities: usize,
    /// Maximum components per entity in one discrete field envelope.
    pub max_discrete_field_components: usize,
    /// Maximum scalar values in one discrete field envelope.
    pub max_discrete_field_values: usize,
    /// Maximum rank of one canonical resolved-array reference.
    pub max_resolved_array_rank: usize,
    /// Maximum scalar values in one canonical resolved-array reference.
    pub max_resolved_array_values: usize,
    /// Maximum dimension of one decoded distributed algebra artifact.
    pub max_distributed_dimension: usize,
    /// Maximum partitions in one decoded unique-owner map.
    pub max_distributed_partitions: usize,
    /// Maximum nonzeros in one decoded complete CSR system.
    pub max_distributed_nonzeros: usize,
    /// Maximum owner-map entries in one decoded partition artifact.
    pub max_distributed_owner_entries: usize,
    /// Maximum owned and ghost indices summed across one layout artifact.
    pub max_distributed_local_indices: usize,
    /// Maximum halo records in one decoded layout artifact.
    pub max_distributed_halo_records: usize,
    /// Maximum halo indices summed across one decoded layout artifact.
    pub max_distributed_halo_indices: usize,
    /// Maximum aggregate scalar work admitted before distributed artifact
    /// reconstruction.
    pub max_distributed_aggregate_work: usize,
    /// Maximum dynamic UTF-8 text bytes summed across one external-import
    /// manifest.
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
    /// Maximum complete XDMF document bytes asserted by one trajectory-storage
    /// envelope.
    pub max_xdmf_storage_bytes: u64,
    /// Maximum complete HDF5 file-image bytes asserted by one
    /// trajectory-storage envelope.
    pub max_hdf5_storage_bytes: u64,
    /// Maximum eliminated physical exposures in one projection catalog.
    pub max_physical_exposure_projections: usize,
    /// Maximum retained Port identities summed across all exposure cuts.
    pub max_physical_exposure_cut_members: usize,
    /// Maximum complete source origins summed across one exposure catalog.
    pub max_physical_exposure_origins: usize,
    /// Maximum source-path bytes summed across one exposure catalog.
    pub max_physical_exposure_source_path_bytes: usize,
    /// Maximum exact Semantic Fields and Field-space bindings in one
    /// field-wise Realization envelope.
    pub max_realization_fields: usize,
    /// Maximum algebraic constraints in one field-wise Realization envelope.
    pub max_realization_constraints: usize,
    /// Maximum scaled algebraic blocks in one field-wise Realization envelope.
    pub max_realization_blocks: usize,
    /// Maximum coefficient blocks in one logical Field snapshot.
    pub max_field_snapshot_blocks: usize,
    /// Maximum raw canonical-byte chunks in one Field snapshot storage manifest.
    pub max_field_storage_chunks: usize,
    /// Maximum exact Field references in one accepted spatial state.
    pub max_spatial_state_fields: usize,
    /// Maximum Field-aware entries in one remesh transfer receipt.
    pub max_remesh_transfer_fields: usize,
    /// Maximum component solves in one typed remesh projection evidence.
    pub max_remesh_projection_solves: usize,
    /// Maximum v3 segments in one remeshing-aware trajectory root.
    pub max_remesh_trajectory_segments: usize,
    /// Maximum target states summarized by one remeshing-aware trajectory
    /// root.
    pub max_remesh_trajectory_states: usize,
    /// Maximum accepted state references in one immutable trajectory segment.
    pub max_trajectory_segment_states: usize,
    /// Maximum immutable segments referenced by one trajectory root.
    pub max_trajectory_segments: usize,
    /// Maximum accepted states summarized by one complete trajectory root.
    pub max_trajectory_states: usize,
    /// Maximum Field selections in one derived Dataset view.
    pub max_dataset_view_fields: usize,
    /// Maximum typed Field descriptors in one derived ML Dataset.
    pub max_ml_dataset_descriptors: usize,
    /// Maximum samples in one derived ML Dataset.
    pub max_ml_dataset_samples: usize,
    /// Maximum state references summed across all ML Dataset windows.
    pub max_ml_dataset_window_states: usize,
    /// Maximum selected snapshot references summed across all ML Dataset samples.
    pub max_ml_dataset_observations: usize,
    /// Maximum coefficient-block references summed across one ML Dataset.
    pub max_ml_dataset_blocks: usize,
    /// Maximum population-normalization channels in one ML Dataset.
    pub max_ml_dataset_normalization_channels: usize,
}

impl Default for DecoderLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_nesting_depth: 64,
            max_nodes: 100_000,
            max_edges: 1_000_000,
            max_expression_nodes: 1_000_000,
            max_expression_roots: 1_000_000,
            max_pure_operator_definitions: 100_000,
            max_pure_operator_formals: 1_000_000,
            max_pure_operator_calculus_nodes: 4_000_000,
            max_pure_operator_application_arguments: 4_000_000,
            max_model_view_members: 100_000,
            max_model_boundary: 100_000,
            max_value_shape_rank: 8,
            max_value_shape_components: 4_096,
            max_transaction_ops: 1_000_000,
            max_transaction_preconditions: 100_000,
            max_exact_rank_dimension: 128,
            max_time_state_dimension: 128,
            max_root_functions: 4_096,
            max_mesh_vertices: 1_000_000,
            max_mesh_cells: 2_000_000,
            max_mesh_coordinate_values: 4_000_000,
            max_mesh_connectivity_indices: 8_000_000,
            max_geometry_entities: 1_000_000,
            max_geometry_mesh_memberships: 16_000_000,
            max_geometry_revision_associations: 1_000_000,
            max_mesh_overlap_cell_fragments: 16_000_000,
            max_mesh_overlap_facet_fragments: 16_000_000,
            max_discrete_field_entities: 2_000_000,
            max_discrete_field_components: 64,
            max_discrete_field_values: 16_000_000,
            max_resolved_array_rank: 8,
            max_resolved_array_values: 16_000_000,
            max_distributed_dimension: 4_000_000,
            max_distributed_partitions: 65_536,
            max_distributed_nonzeros: 32_000_000,
            max_distributed_owner_entries: 4_000_000,
            max_distributed_local_indices: 16_000_000,
            max_distributed_halo_records: 4_000_000,
            max_distributed_halo_indices: 16_000_000,
            max_distributed_aggregate_work: 96_000_000,
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
            max_physical_exposure_projections: 100_000,
            max_physical_exposure_cut_members: 1_000_000,
            max_physical_exposure_origins: 1_000_000,
            max_physical_exposure_source_path_bytes: 64 * 1_024 * 1_024,
            max_realization_fields: 100_000,
            max_realization_constraints: 100_000,
            max_realization_blocks: 200_000,
            max_field_snapshot_blocks: 8,
            max_field_storage_chunks: 1_000_000,
            max_spatial_state_fields: 100_000,
            max_remesh_transfer_fields: 100_000,
            max_remesh_projection_solves: 2,
            max_remesh_trajectory_segments: 100_000,
            max_remesh_trajectory_states: 1_000_000,
            max_trajectory_segment_states: 100_000,
            max_trajectory_segments: 100_000,
            max_trajectory_states: 1_000_000,
            max_dataset_view_fields: 100_000,
            max_ml_dataset_descriptors: 100_000,
            max_ml_dataset_samples: 1_000_000,
            max_ml_dataset_window_states: 16_000_000,
            max_ml_dataset_observations: 16_000_000,
            max_ml_dataset_blocks: 32_000_000,
            max_ml_dataset_normalization_channels: 6_400_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DecoderLimits;

    #[test]
    fn decoder_limit_defaults_are_frozen() {
        assert_eq!(
            DecoderLimits::default(),
            DecoderLimits {
                max_bytes: 16 * 1024 * 1024,
                max_nesting_depth: 64,
                max_nodes: 100_000,
                max_edges: 1_000_000,
                max_expression_nodes: 1_000_000,
                max_expression_roots: 1_000_000,
                max_pure_operator_definitions: 100_000,
                max_pure_operator_formals: 1_000_000,
                max_pure_operator_calculus_nodes: 4_000_000,
                max_pure_operator_application_arguments: 4_000_000,
                max_model_view_members: 100_000,
                max_model_boundary: 100_000,
                max_value_shape_rank: 8,
                max_value_shape_components: 4_096,
                max_transaction_ops: 1_000_000,
                max_transaction_preconditions: 100_000,
                max_exact_rank_dimension: 128,
                max_time_state_dimension: 128,
                max_root_functions: 4_096,
                max_mesh_vertices: 1_000_000,
                max_mesh_cells: 2_000_000,
                max_mesh_coordinate_values: 4_000_000,
                max_mesh_connectivity_indices: 8_000_000,
                max_geometry_entities: 1_000_000,
                max_geometry_mesh_memberships: 16_000_000,
                max_geometry_revision_associations: 1_000_000,
                max_mesh_overlap_cell_fragments: 16_000_000,
                max_mesh_overlap_facet_fragments: 16_000_000,
                max_discrete_field_entities: 2_000_000,
                max_discrete_field_components: 64,
                max_discrete_field_values: 16_000_000,
                max_resolved_array_rank: 8,
                max_resolved_array_values: 16_000_000,
                max_distributed_dimension: 4_000_000,
                max_distributed_partitions: 65_536,
                max_distributed_nonzeros: 32_000_000,
                max_distributed_owner_entries: 4_000_000,
                max_distributed_local_indices: 16_000_000,
                max_distributed_halo_records: 4_000_000,
                max_distributed_halo_indices: 16_000_000,
                max_distributed_aggregate_work: 96_000_000,
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
                max_physical_exposure_projections: 100_000,
                max_physical_exposure_cut_members: 1_000_000,
                max_physical_exposure_origins: 1_000_000,
                max_physical_exposure_source_path_bytes: 64 * 1_024 * 1_024,
                max_realization_fields: 100_000,
                max_realization_constraints: 100_000,
                max_realization_blocks: 200_000,
                max_field_snapshot_blocks: 8,
                max_field_storage_chunks: 1_000_000,
                max_spatial_state_fields: 100_000,
                max_remesh_transfer_fields: 100_000,
                max_remesh_projection_solves: 2,
                max_remesh_trajectory_segments: 100_000,
                max_remesh_trajectory_states: 1_000_000,
                max_trajectory_segment_states: 100_000,
                max_trajectory_segments: 100_000,
                max_trajectory_states: 1_000_000,
                max_dataset_view_fields: 100_000,
                max_ml_dataset_descriptors: 100_000,
                max_ml_dataset_samples: 1_000_000,
                max_ml_dataset_window_states: 16_000_000,
                max_ml_dataset_observations: 16_000_000,
                max_ml_dataset_blocks: 32_000_000,
                max_ml_dataset_normalization_channels: 6_400_000,
            }
        );
    }
}
