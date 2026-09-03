/// Bounded resource policy for local source-unit identity construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSourceIdentityLimits {
    /// Maximum Connector, pure-operator, Component, and Model declarations combined.
    pub max_top_level_declarations: usize,
    /// Maximum declarations in one component or model body.
    pub max_members_per_container: usize,
    /// Maximum declarations summed across all component and model bodies.
    pub max_total_members: usize,
    /// Maximum expression nodes in the complete source unit.
    pub max_expression_nodes: usize,
    /// Maximum recursive expression depth.
    pub max_expression_depth: usize,
    /// Maximum residual roots in one Relation, with root order preserved.
    pub max_residuals_per_relation: usize,
    /// Maximum member paths in one Connection or Boundary declaration.
    pub max_connection_members: usize,
    /// Maximum named Parameter, spatial-support, and Field bindings in one instance.
    pub max_bindings_per_instance: usize,
    /// Maximum exact Boundary members in one complete-exterior set binding.
    pub max_boundary_set_members: usize,
    /// Maximum Boundary-set memberships summed across the source unit.
    pub max_total_boundary_set_memberships: usize,
    /// Maximum segments in one structured source path.
    pub max_path_segments: usize,
    /// Maximum UTF-8 bytes in one name or path segment.
    pub max_name_bytes: usize,
    /// Maximum UTF-8 name bytes summed across the source identity.
    pub max_total_name_bytes: usize,
    /// Maximum bytes in the complete canonical encoding.
    pub max_canonical_bytes: usize,
    /// Maximum bytes cumulatively materialized while canonical records are encoded for sorting.
    pub max_intermediate_bytes: usize,
}

impl Default for LocalSourceIdentityLimits {
    fn default() -> Self {
        Self {
            max_top_level_declarations: 65_536,
            max_members_per_container: 65_536,
            max_total_members: 1_000_000,
            max_expression_nodes: 1_000_000,
            max_expression_depth: 256,
            max_residuals_per_relation: 65_536,
            max_connection_members: 65_536,
            max_bindings_per_instance: 65_536,
            max_boundary_set_members: 65_536,
            max_total_boundary_set_memberships: 1_000_000,
            max_path_segments: 256,
            max_name_bytes: 4_096,
            max_total_name_bytes: 64 * 1_024 * 1_024,
            max_canonical_bytes: 128 * 1_024 * 1_024,
            max_intermediate_bytes: 512 * 1_024 * 1_024,
        }
    }
}
