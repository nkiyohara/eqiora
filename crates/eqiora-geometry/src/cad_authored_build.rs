//! Accepted build evidence from Eqiora's bounded analytic CAD profiles.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::cad_authored_result_topology::CadAuthoredResultTopology;
use crate::cad_authored_selection::FaceKey;
use crate::{CadAuthoredFaceHandle, CadAuthoredGraph, CadRepairDispositionV1};

const RECTANGLE_PROFILE: &str = "eqiora.cad.analytic-rectangle-extrusion-v1";
const CIRCULAR_CUT_PROFILE: &str = "eqiora.cad.analytic-circular-through-cut-v1";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[derive(Clone, Debug, PartialEq)]
struct BuildObservation {
    provider_profile: &'static str,
    requested_modeling_tolerance_m: f64,
    requested_boolean_tolerance_m: Option<f64>,
    effective_boolean_tolerance_m: Option<f64>,
    maximum_position_discrepancy_m: f64,
    maximum_area_discrepancy_m2: f64,
    maximum_volume_discrepancy_m3: f64,
    repair: CadRepairDispositionV1,
    retained_unchanged: Vec<FaceKey>,
    retained_modified: Vec<FaceKey>,
    created: Vec<FaceKey>,
    deleted: Vec<FaceKey>,
    split: Vec<FaceKey>,
    merged: Vec<FaceKey>,
}

/// Complete accepted receipt from one bounded authored-CAD analytic build.
///
/// Provider/profile and effective policy are execution evidence, never graph
/// identity.  The type is constructible only after the graph owner validates
/// the complete observation and binds every lineage member to its exact graph
/// digest.
#[derive(Clone, Debug, PartialEq)]
pub struct CadAuthoredBuild {
    graph_digest: [u8; 32],
    provider_profile: &'static str,
    requested_modeling_tolerance_m: f64,
    requested_boolean_tolerance_m: Option<f64>,
    effective_boolean_tolerance_m: Option<f64>,
    maximum_position_discrepancy_m: f64,
    maximum_area_discrepancy_m2: f64,
    maximum_volume_discrepancy_m3: f64,
    repair: CadRepairDispositionV1,
    retained_unchanged: Vec<CadAuthoredFaceHandle>,
    retained_modified: Vec<CadAuthoredFaceHandle>,
    created: Vec<CadAuthoredFaceHandle>,
    deleted: Vec<CadAuthoredFaceHandle>,
    split: Vec<CadAuthoredFaceHandle>,
    merged: Vec<CadAuthoredFaceHandle>,
}

impl CadAuthoredBuild {
    pub(crate) fn from_graph(graph: &CadAuthoredGraph) -> Result<Self, Diagnostic> {
        Self::admit(graph, Self::expected_observation(graph))
    }

    fn admit(graph: &CadAuthoredGraph, observation: BuildObservation) -> Result<Self, Diagnostic> {
        let expected = Self::expected_observation(graph);
        if observation != expected {
            return Err(invalid(
                "analytic CAD build omitted or changed provider, tolerance, discrepancy, repair, or topology-lineage evidence",
            ));
        }
        Ok(Self {
            graph_digest: graph.digest_bytes(),
            provider_profile: observation.provider_profile,
            requested_modeling_tolerance_m: observation.requested_modeling_tolerance_m,
            requested_boolean_tolerance_m: observation.requested_boolean_tolerance_m,
            effective_boolean_tolerance_m: observation.effective_boolean_tolerance_m,
            maximum_position_discrepancy_m: observation.maximum_position_discrepancy_m,
            maximum_area_discrepancy_m2: observation.maximum_area_discrepancy_m2,
            maximum_volume_discrepancy_m3: observation.maximum_volume_discrepancy_m3,
            repair: observation.repair,
            retained_unchanged: bind(graph, &observation.retained_unchanged)?,
            retained_modified: bind(graph, &observation.retained_modified)?,
            created: bind(graph, &observation.created)?,
            deleted: bind(graph, &observation.deleted)?,
            split: bind(graph, &observation.split)?,
            merged: bind(graph, &observation.merged)?,
        })
    }

    fn expected_observation(graph: &CadAuthoredGraph) -> BuildObservation {
        if graph.is_cut() {
            BuildObservation {
                provider_profile: CIRCULAR_CUT_PROFILE,
                requested_modeling_tolerance_m: graph.requested_modeling_tolerance_m(),
                requested_boolean_tolerance_m: graph.requested_boolean_tolerance_m(),
                effective_boolean_tolerance_m: graph.requested_boolean_tolerance_m(),
                maximum_position_discrepancy_m: 0.0,
                maximum_area_discrepancy_m2: 0.0,
                maximum_volume_discrepancy_m3: 0.0,
                repair: CadRepairDispositionV1::None,
                retained_unchanged: vec![
                    FaceKey::profile_x_lower(),
                    FaceKey::profile_x_upper(),
                    FaceKey::profile_y_lower(),
                    FaceKey::profile_y_upper(),
                ],
                retained_modified: vec![FaceKey::start_cap(), FaceKey::end_cap()],
                created: vec![FaceKey::cut_wall()],
                deleted: Vec::new(),
                split: Vec::new(),
                merged: Vec::new(),
            }
        } else {
            BuildObservation {
                provider_profile: RECTANGLE_PROFILE,
                requested_modeling_tolerance_m: graph.requested_modeling_tolerance_m(),
                requested_boolean_tolerance_m: None,
                effective_boolean_tolerance_m: None,
                maximum_position_discrepancy_m: 0.0,
                maximum_area_discrepancy_m2: 0.0,
                maximum_volume_discrepancy_m3: 0.0,
                repair: CadRepairDispositionV1::None,
                retained_unchanged: Vec::new(),
                retained_modified: Vec::new(),
                created: FaceKey::V1_ALL.to_vec(),
                deleted: Vec::new(),
                split: Vec::new(),
                merged: Vec::new(),
            }
        }
    }

    /// Exact graph identity this accepted build realizes.
    #[must_use]
    pub const fn graph_digest_bytes(&self) -> [u8; 32] {
        self.graph_digest
    }

    /// Compile-time Eqiora analytic provider/profile identity.
    #[must_use]
    pub const fn provider_profile(&self) -> &'static str {
        self.provider_profile
    }

    /// Identity-bearing base modeling tolerance, never substituted into the Boolean.
    #[must_use]
    pub const fn requested_modeling_tolerance_m(&self) -> f64 {
        self.requested_modeling_tolerance_m
    }

    /// Requested Boolean tolerance, absent for the rectangle-only history.
    #[must_use]
    pub const fn requested_boolean_tolerance_m(&self) -> Option<f64> {
        self.requested_boolean_tolerance_m
    }

    /// Effective Boolean tolerance reported by the selected profile.
    #[must_use]
    pub const fn effective_boolean_tolerance_m(&self) -> Option<f64> {
        self.effective_boolean_tolerance_m
    }

    /// Maximum exact positional discrepancy observed by the analytic profile.
    #[must_use]
    pub const fn maximum_position_discrepancy_m(&self) -> f64 {
        self.maximum_position_discrepancy_m
    }

    /// Maximum exact face-area discrepancy observed by the analytic profile.
    #[must_use]
    pub const fn maximum_area_discrepancy_m2(&self) -> f64 {
        self.maximum_area_discrepancy_m2
    }

    /// Maximum exact volume discrepancy observed by the analytic profile.
    #[must_use]
    pub const fn maximum_volume_discrepancy_m3(&self) -> f64 {
        self.maximum_volume_discrepancy_m3
    }

    /// Explicit repair disposition.
    #[must_use]
    pub const fn repair_disposition(&self) -> CadRepairDispositionV1 {
        self.repair
    }

    /// Faces retained without changing their analytic boundary.
    #[must_use]
    pub fn retained_unchanged(&self) -> &[CadAuthoredFaceHandle] {
        &self.retained_unchanged
    }

    /// Faces retaining provenance while gaining an inner boundary loop.
    #[must_use]
    pub fn retained_modified(&self) -> &[CadAuthoredFaceHandle] {
        &self.retained_modified
    }

    /// Faces created by the last admitted operation.
    #[must_use]
    pub fn created(&self) -> &[CadAuthoredFaceHandle] {
        &self.created
    }

    /// Faces deleted by the last admitted operation.
    #[must_use]
    pub fn deleted(&self) -> &[CadAuthoredFaceHandle] {
        &self.deleted
    }

    /// Faces split by the last admitted operation.
    #[must_use]
    pub fn split(&self) -> &[CadAuthoredFaceHandle] {
        &self.split
    }

    /// Faces merged by the last admitted operation.
    #[must_use]
    pub fn merged(&self) -> &[CadAuthoredFaceHandle] {
        &self.merged
    }

    /// Project this accepted build's complete lineage into immutable result topology.
    ///
    /// # Errors
    /// Returns `EQ0901` when the build and graph identities differ, the graph
    /// is not the admitted circular through-cut, or lineage is incomplete,
    /// mutated, deleted, split, merged, or ambiguous.
    pub fn result_topology(
        &self,
        graph: &CadAuthoredGraph,
    ) -> Result<CadAuthoredResultTopology, Diagnostic> {
        CadAuthoredResultTopology::from_build(graph, self)
    }
}

fn bind(
    graph: &CadAuthoredGraph,
    selections: &[FaceKey],
) -> Result<Vec<CadAuthoredFaceHandle>, Diagnostic> {
    selections
        .iter()
        .copied()
        .map(|selection| graph.face_handle_for(selection))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConstrainedRectangleV1;

    fn cut_graph() -> CadAuthoredGraph {
        CadAuthoredGraph::new(
            ConstrainedRectangleV1::new((-0.04, 0.04), (-0.025, 0.025), 0.0).unwrap(),
            0.02,
            1.0e-10,
        )
        .unwrap()
        .circular_through_cut([0.02, 0.0], 0.008, 1.0e-9)
        .unwrap()
    }

    #[test]
    fn incomplete_or_substituted_build_observation_rejects() {
        let graph = cut_graph();
        let mut observation = CadAuthoredBuild::expected_observation(&graph);
        observation.effective_boolean_tolerance_m = Some(1.0e-10);
        assert!(CadAuthoredBuild::admit(&graph, observation).is_err());

        let mut observation = CadAuthoredBuild::expected_observation(&graph);
        observation.created.clear();
        assert!(CadAuthoredBuild::admit(&graph, observation).is_err());

        let mut observation = CadAuthoredBuild::expected_observation(&graph);
        observation.repair = CadRepairDispositionV1::None;
        observation.provider_profile = "truck";
        assert!(CadAuthoredBuild::admit(&graph, observation).is_err());
    }
}
