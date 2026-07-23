//! Versioned, deterministic Evidence & Artifact Graph payloads.
//!
//! Wire DTOs are deliberately separate from Semantic Kernel Rust types.
//! Decoding reconstructs validated definitions and commits one typed graph
//! transaction; deserialization never bypasses an existing invariant.

mod cad;
mod discrete_field;
mod distributed;
mod external_import;
mod geometry_identity;
mod geometry_mesh_correspondence;
mod geometry_revision_association;
mod geometry_state;
mod geometry_state_reference;
mod geometry_state_v2;
mod geometry_state_v3;
mod implicit_time;
mod implicit_time_lineage;
mod mesh;
mod mesh_revision_overlap;
mod model;
mod model_reference;
mod model_transaction;
mod model_transaction_v2;
mod model_transaction_v3;
mod model_transaction_v4;
mod model_transaction_v5;
mod model_transaction_v6;
mod model_v2;
mod model_v3;
mod model_v4;
mod model_v5;
mod model_v6;
mod physical_exposure;
mod realization;
mod realization_reference;
mod realization_v2;
mod realization_v3;
mod realization_v4;
mod realization_v5;
mod remesh_transfer;
mod resolved_array;
mod root_registration;
mod run_v2;
mod semantic_fingerprint;
mod spatial_data;
mod spatial_state_v2;
mod spatial_state_v3;
mod spatial_trajectory_v2;
mod spatial_trajectory_v3;
mod time;
mod xdmf_hdf5_trajectory_storage;

use std::collections::BTreeMap;
use std::fmt;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use cad::{CadBuildEvidenceEnvelopeV1, CadDesignEnvelopeV1};
pub use discrete_field::DiscreteFieldEnvelopeV1;
pub use distributed::{
    DistributedLayoutEnvelopeV1, LinearSystemEnvelopeV1, PartitionEnvelopeV1,
    validate_distributed_content_dag,
};
pub use eqiora_geometry::BodyAssociationCandidate;
pub use external_import::{
    ExternalAdapterIdentityV1, ExternalImportManifestV1, ExternalImportObservationV1,
    ExternalImportSelectionV1, ExternalImportSourceV1, ExternalRuntimeComponentV1,
    ExternalRuntimeRoleV1, RawSourceSha256, ResolvedImportArrayV1, SelectedSourceEntityV1,
    StructuralSelectorV1,
};
pub use geometry_identity::{
    CartesianGeometryBodyV1, CartesianGeometryBoundaryV1, GeometryEntityV1,
    GeometryIdentityEnvelopeV1,
};
pub use geometry_mesh_correspondence::{
    ConservingGeometryInterfaceV1, GeometryMeshCorrespondenceEnvelopeV1,
};
pub use geometry_revision_association::{
    GeometryAssociationArtifactError, GeometryRevisionAssociationEnvelopeV1,
};
pub use geometry_state::GeometryStateEnvelopeV1;
pub use geometry_state_reference::ReplayableFixedTopologyGeometryStateArtifact;
pub use geometry_state_v2::{
    GeometryStateEnvelopeV2, GeometryStateOriginKindV2, ValidatedRemeshGeometrySourceV2,
};
pub use geometry_state_v3::GeometryStateEnvelopeV3;
pub use implicit_time::{
    GeneralImplicitTimeLoweringEnvelopeV1, ImplicitTimeInitialDataEnvelopeV1,
    ImplicitTimeRunManifestV1,
};
pub use implicit_time_lineage::{ImplicitTimeCheckpointEnvelopeV1, ImplicitTimeRestartManifestV1};
pub use mesh::SimplicialMeshEnvelopeV1;
pub use mesh_revision_overlap::MeshRevisionOverlapEnvelopeV1;
pub use model::ModelEnvelopeV1;
#[allow(deprecated)]
pub use model_reference::{
    CanonicalModelArtifact, ModelArtifactReference, ModelArtifactReferenceV1,
    ReplayableCanonicalModelArtifact, ReplayedCanonicalModel,
};
pub use model_transaction::ModelTransactionEnvelopeV1;
pub use model_transaction_v2::ModelTransactionEnvelopeV2;
pub use model_transaction_v3::ModelTransactionEnvelopeV3;
pub use model_transaction_v4::ModelTransactionEnvelopeV4;
pub use model_transaction_v5::ModelTransactionEnvelopeV5;
pub use model_transaction_v6::ModelTransactionEnvelopeV6;
pub use model_v2::ModelEnvelopeV2;
pub use model_v3::ModelEnvelopeV3;
pub use model_v4::ModelEnvelopeV4;
pub use model_v5::ModelEnvelopeV5;
pub use model_v6::ModelEnvelopeV6;
pub use physical_exposure::{
    PhysicalExposureCatalogEnvelopeV1, PhysicalExposureContractV1,
    PhysicalExposureObservationBindingV1, PhysicalExposureProjectionV1, PhysicalExposureQuantityV1,
    PhysicalExposureSourceOriginV1, PhysicalExposureSourceSpanV1,
};
pub use realization::{LayoutArtifacts, LayoutArtifactsV1, RealizationEnvelopeV1};
#[allow(deprecated)]
pub use realization_reference::{
    CanonicalRealizationArtifact, RealizationArtifactReference, RealizationArtifactReferenceV1,
    ReplayableFixedTopologyAleRealizationArtifact,
};
pub use realization_v2::RealizationEnvelopeV2;
pub use realization_v3::RealizationEnvelopeV3;
pub use realization_v4::RealizationEnvelopeV4;
pub use realization_v5::RealizationEnvelopeV5;
pub use remesh_transfer::{
    BoundedRemeshDefectV1, FieldTransferReceiptV1, RemeshFieldRoleV1, RemeshIntegrationChartV1,
    RemeshNormalizationWitnessV1, RemeshProjectionActionV1, RemeshProjectionEvidenceEnvelopeV1,
    RemeshProjectionExecutionModeV1, RemeshTransferEvidenceV1, RemeshTransferLawV1,
    RemeshTransferReceiptEnvelopeV1,
};
pub use resolved_array::{ResolvedArrayScalarV1, ResolvedArrayV1};
pub use root_registration::RootRegistrationEnvelopeV1;
pub use run_v2::{
    DistributedTransportV1, ExecutionProvenanceFingerprintV1, ExecutionProvenanceV1,
    ExecutionTopologyV1, MpiThreadSupportV1, RunManifestV2,
};
pub use semantic_fingerprint::{
    SemanticFingerprintGeneration, StructuralSemanticFingerprint, structurally_equivalent,
};
pub use spatial_data::{
    DatasetViewEnvelopeV1, DiscreteFieldStorageEnvelopeV1, FieldSnapshotEnvelopeV1,
    MlDatasetChannelStatisticsV1, MlDatasetDescriptorRoleV1, MlDatasetEnvelopeV1,
    MlDatasetFieldDescriptorV1, MlDatasetObservationReferenceV1, MlDatasetSampleSplitV1,
    MlDatasetSampleV1, MlDatasetStateKindV1, MlDatasetStateReferenceV1, SpatialStateEnvelopeV1,
    SpatialTrajectoryEnvelopeV1, SpatialTrajectorySegmentEnvelopeV1, StorageChunkSha256V1,
    StorageChunkV1, ValidatedFixedSpatialContextV1,
};
pub use spatial_state_v2::{SpatialStateEnvelopeV2, ValidatedMovingSpatialContextV2};
pub use spatial_state_v3::{SpatialStateEnvelopeV3, SpatialStateOriginKindV3};
pub use spatial_trajectory_v2::{SpatialTrajectoryEnvelopeV2, SpatialTrajectorySegmentEnvelopeV2};
pub use spatial_trajectory_v3::{
    SpatialTrajectoryEnvelopeV3, SpatialTrajectorySegmentEnvelopeV3,
    SpatialTrajectorySegmentOriginKindV3,
};
pub use time::{TimeLoweringEnvelopeV1, TimeRunManifestV1};
pub use xdmf_hdf5_trajectory_storage::{
    TemporalStorageBlockPresentationV1, TemporalStorageStateKindV1, XdmfHdf5TrajectoryBlockV1,
    XdmfHdf5TrajectoryFieldV1, XdmfHdf5TrajectoryFrameV1, XdmfHdf5TrajectoryStorageEnvelopeV1,
};

const RUN_SCHEMA: &str = "eqiora.run-manifest/v1";
pub(crate) const CANONICAL_ENCODING: &str = "eqiora.canonical-json/v1";

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

/// A lowercase SHA-256 content identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactDigest(String);

impl ArtifactDigest {
    /// Encode complete SHA-256 bytes as canonical lowercase hexadecimal.
    #[must_use]
    pub fn from_sha256(bytes: [u8; 32]) -> Self {
        let mut encoded = String::with_capacity(64);
        for byte in bytes {
            use fmt::Write as _;
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(encoded)
    }

    /// Parse a canonical 64-character lowercase SHA-256 digest.
    ///
    /// # Errors
    /// Returns `EQ0901` for any other representation.
    pub fn from_hex(value: impl Into<String>) -> Result<Self, Diagnostic> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid_artifact(
                "artifact digest must be 64 lowercase hexadecimal SHA-256 characters",
            ));
        }
        Ok(Self(value))
    }

    /// Canonical lowercase hexadecimal form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Complete SHA-256 bytes.
    #[must_use]
    pub fn sha256_bytes(&self) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        for (index, pair) in self.0.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
        }
        bytes
    }

    pub(crate) fn compute(domain: &[u8], bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update([0]);
        hasher.update(bytes);
        let digest = hasher.finalize();
        Self::from_sha256(digest.into())
    }
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("ArtifactDigest always contains validated lowercase hexadecimal"),
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Reproducible inputs and outputs of one execution.
///
/// Wall-clock timestamps and host paths are intentionally absent. A caller may
/// attach those as provenance metadata without changing run identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunManifestV1 {
    wire: WireRunManifestV1,
}

impl RunManifestV1 {
    /// Start a run manifest for one semantic artifact revision and executor.
    ///
    /// # Errors
    /// Returns `EQ0901` for empty executor identity.
    pub fn new(
        model: ArtifactDigest,
        semantic_revision: u64,
        executor: impl Into<String>,
        executor_version: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        let executor = executor.into();
        let executor_version = executor_version.into();
        validate_text("executor", &executor)?;
        validate_text("executor version", &executor_version)?;
        Ok(Self {
            wire: WireRunManifestV1 {
                schema: RUN_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                model_sha256: model.0,
                semantic_revision,
                realization_sha256: None,
                executor,
                executor_version,
                numerical_settings: BTreeMap::new(),
                output_sha256: Vec::new(),
            },
        })
    }

    /// Reference an independently versioned Realization artifact.
    #[must_use]
    pub fn with_realization(mut self, realization: ArtifactDigest) -> Self {
        self.wire.realization_sha256 = Some(realization.0);
        self
    }

    /// Add one deterministic numerical setting.
    ///
    /// # Errors
    /// Returns `EQ0901` unless the key is lowercase dotted/kebab ASCII and the
    /// value is non-empty without control characters.
    pub fn with_numerical_setting(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        let key = key.into();
        let value = value.into();
        validate_setting_key(&key)?;
        validate_text("numerical setting", &value)?;
        if self
            .wire
            .numerical_settings
            .insert(key.clone(), value)
            .is_some()
        {
            return Err(invalid_artifact(format!(
                "duplicate numerical setting `{key}`"
            )));
        }
        Ok(self)
    }

    /// Add a content-addressed output artifact.
    #[must_use]
    pub fn with_output(mut self, output: ArtifactDigest) -> Self {
        self.wire.output_sha256.push(output.0);
        self.wire.output_sha256.sort();
        self.wire.output_sha256.dedup();
        self
    }

    /// Decode and validate a run manifest.
    ///
    /// # Errors
    /// Returns `EQ0901` for oversized, malformed, unknown-version, duplicate,
    /// or non-canonical field data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        check_wire_limits(bytes, limits)?;
        let mut wire: WireRunManifestV1 = serde_json::from_slice(bytes)
            .map_err(|error| invalid_artifact(format!("invalid run manifest JSON: {error}")))?;
        if wire.schema != RUN_SCHEMA || wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported run-manifest schema or canonical encoding",
            ));
        }
        ArtifactDigest::from_hex(wire.model_sha256.clone())?;
        if let Some(realization) = &wire.realization_sha256 {
            ArtifactDigest::from_hex(realization.clone())?;
        }
        validate_text("executor", &wire.executor)?;
        validate_text("executor version", &wire.executor_version)?;
        for (key, value) in &wire.numerical_settings {
            validate_setting_key(key)?;
            validate_text("numerical setting", value)?;
        }
        for output in &wire.output_sha256 {
            ArtifactDigest::from_hex(output.clone())?;
        }
        let original_output_count = wire.output_sha256.len();
        wire.output_sha256.sort();
        wire.output_sha256.dedup();
        if wire.output_sha256.len() != original_output_count {
            return Err(invalid_artifact(
                "run manifest contains duplicate output artifact digests",
            ));
        }
        Ok(Self { wire })
    }

    /// Deterministic canonical JSON bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire)
            .map_err(|error| invalid_artifact(format!("cannot serialize run manifest: {error}")))
    }

    /// Domain-separated SHA-256 identity of canonical manifest bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            RUN_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Referenced semantic-model digest.
    #[must_use]
    pub fn model(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.model_sha256.clone())
    }

    /// Semantic revision executed within the referenced Model artifact.
    #[must_use]
    pub const fn semantic_revision(&self) -> u64 {
        self.wire.semantic_revision
    }

    /// Sorted content-addressed output artifacts.
    #[must_use]
    pub fn outputs(&self) -> Vec<ArtifactDigest> {
        self.wire
            .output_sha256
            .iter()
            .cloned()
            .map(ArtifactDigest)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRunManifestV1 {
    schema: String,
    encoding: String,
    model_sha256: String,
    semantic_revision: u64,
    realization_sha256: Option<String>,
    executor: String,
    executor_version: String,
    numerical_settings: BTreeMap<String, String>,
    output_sha256: Vec<String>,
}

pub(crate) fn check_wire_limits(bytes: &[u8], limits: DecoderLimits) -> Result<(), Diagnostic> {
    if bytes.len() > limits.max_bytes {
        return Err(invalid_artifact(format!(
            "artifact has {} bytes, exceeding the {} byte decoder limit",
            bytes.len(),
            limits.max_bytes
        )));
    }

    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.checked_add(1).ok_or_else(|| {
                    invalid_artifact("artifact JSON nesting depth overflowed usize")
                })?;
                if depth > limits.max_nesting_depth {
                    return Err(invalid_artifact(format!(
                        "artifact JSON nesting exceeds the {} level decoder limit",
                        limits.max_nesting_depth
                    )));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_setting_key(key: &str) -> Result<(), Diagnostic> {
    if key.is_empty()
        || !key.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        return Err(invalid_artifact(
            "numerical setting keys must be non-empty lowercase dotted/kebab/snake ASCII",
        ));
    }
    Ok(())
}

pub(crate) fn validate_text(label: &str, value: &str) -> Result<(), Diagnostic> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(invalid_artifact(format!(
            "{label} must be non-empty and contain no control characters"
        )));
    }
    Ok(())
}

pub(crate) fn invalid_artifact(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_manifest_is_ordered_versioned_and_content_addressed() {
        let model = ArtifactDigest::compute(b"model", b"one");
        let output = ArtifactDigest::compute(b"output", b"two");
        let manifest = RunManifestV1::new(model.clone(), 7, "eqiora-reference", "0.1.0")
            .unwrap()
            .with_numerical_setting("solver.max-step", "0.01 s")
            .unwrap()
            .with_output(output.clone())
            .with_output(output);
        let bytes = manifest.canonical_json().unwrap();
        let decoded = RunManifestV1::from_json(&bytes, DecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), bytes);
        assert_eq!(decoded.model(), model);
        assert_eq!(decoded.digest().unwrap(), manifest.digest().unwrap());
    }

    #[test]
    fn malformed_digest_and_duplicate_setting_are_rejected() {
        assert_eq!(
            ArtifactDigest::from_hex("ABC").unwrap_err().code(),
            codes::INVALID_ARTIFACT
        );
        assert_eq!(
            ArtifactDigest::from_hex("01".repeat(32))
                .unwrap()
                .sha256_bytes(),
            [1; 32]
        );
        let model = ArtifactDigest::compute(b"model", b"one");
        let manifest = RunManifestV1::new(model, 1, "executor", "v1")
            .unwrap()
            .with_numerical_setting("step", "one")
            .unwrap();
        assert_eq!(
            manifest
                .with_numerical_setting("step", "two")
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );
    }

    #[test]
    fn explicit_json_depth_limit_ignores_delimiters_inside_strings() {
        let limits = DecoderLimits {
            max_nesting_depth: 2,
            ..DecoderLimits::default()
        };
        check_wire_limits(br#"{"text":"[[{{"}"#, limits).unwrap();
        assert_eq!(
            check_wire_limits(br#"{"nested":{"too":{"deep":true}}}"#, limits)
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );
    }
}
