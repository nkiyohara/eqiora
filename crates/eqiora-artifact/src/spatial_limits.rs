use crate::JsonDecoderLimits;

/// Semantic work budgets for mesh, geometry, field, Realization, and remesh artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialDecoderLimits {
    /// Common JSON syntax admission.
    pub json: JsonDecoderLimits,
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
    /// Maximum positive-length retained-facet fragments in one remesh overlap artifact.
    pub max_mesh_overlap_facet_fragments: usize,
    /// Maximum associated entities in one discrete field envelope.
    pub max_discrete_field_entities: usize,
    /// Maximum components per entity in one discrete field envelope.
    pub max_discrete_field_components: usize,
    /// Maximum scalar values in one discrete field envelope.
    pub max_discrete_field_values: usize,
    /// Maximum eliminated physical exposures in one projection catalog.
    pub max_physical_exposure_projections: usize,
    /// Maximum retained Port identities summed across all exposure cuts.
    pub max_physical_exposure_cut_members: usize,
    /// Maximum complete source origins summed across one exposure catalog.
    pub max_physical_exposure_origins: usize,
    /// Maximum source-path bytes summed across one exposure catalog.
    pub max_physical_exposure_source_path_bytes: usize,
    /// Maximum exact Semantic Fields and Field-space bindings in one Realization envelope.
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
}

impl Default for SpatialDecoderLimits {
    fn default() -> Self {
        Self {
            json: JsonDecoderLimits::default(),
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
        }
    }
}
