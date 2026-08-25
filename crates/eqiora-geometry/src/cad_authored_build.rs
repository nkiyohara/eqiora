//! Accepted build evidence from Eqiora's bounded analytic CAD profiles.

use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::cad_authored_result_topology::{
    CadAuthoredResultTopology, CadAuthoredResultTopologyHandle,
};
use crate::cad_authored_selection::FaceKey;
use crate::{CadAuthoredFaceHandle, CadAuthoredGraph, CadRepairDispositionV1, CanonicalGeometryV1};

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
    predecessor_graph_digest: Option<[u8; 32]>,
    result_topology: Option<CadAuthoredResultTopology>,
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
        let mut build = Self {
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
            predecessor_graph_digest: graph.predecessor_digest_bytes(),
            result_topology: None,
        };
        if graph.is_cut() {
            build.result_topology = Some(CadAuthoredResultTopology::from_build(graph, &build)?);
        }
        Ok(build)
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

    fn project_source(
        &self,
        topology: &CadAuthoredResultTopology,
        source: &CadAuthoredFaceHandle,
    ) -> Result<CadAuthoredResultTopologyHandle, Diagnostic> {
        let key = source.face_key();
        let retained_source = source.is_v1()
            && Some(source.graph_digest_bytes()) == self.predecessor_graph_digest
            && key != FaceKey::cut_wall();
        let created_source = !source.is_v1()
            && source.graph_digest_bytes() == self.graph_digest
            && key == FaceKey::cut_wall();
        if !retained_source && !created_source {
            return Err(invalid(
                "construction handle is foreign or stale for this accepted build result",
            ));
        }
        let current = self
            .retained_unchanged
            .iter()
            .chain(&self.retained_modified)
            .chain(&self.created)
            .find(|handle| handle.face_key() == key)
            .ok_or_else(|| invalid("construction handle is absent from this accepted result"))?;
        topology.project(current)
    }

    /// Atomically bind complete named source lineage into common Geometry.
    ///
    /// Retained handles must belong to this graph's exact predecessor and
    /// created handles to this exact result graph. Every group must be nonempty
    /// and dimension-homogeneous, and every result member must occur exactly
    /// once. Projection and naming are one operation; no coordinate classifier
    /// or public result-topology handle participates.
    ///
    /// # Errors
    /// Returns `EQ0901` for a non-planar build or foreign, incomplete,
    /// duplicate, empty, or mixed-dimensional named membership.
    pub fn with_named_topology(
        &self,
        named_topology: &BTreeMap<String, Vec<CadAuthoredFaceHandle>>,
    ) -> Result<CanonicalGeometryV1, Diagnostic> {
        let topology = self
            .result_topology
            .as_ref()
            .ok_or_else(|| invalid("accepted build has no planar circular-hole result topology"))?;
        let projected = named_topology
            .iter()
            .map(|(name, handles)| {
                handles
                    .iter()
                    .map(|handle| self.project_source(topology, handle))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|handles| (name.clone(), handles))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        topology
            .canonical_geometry_v2(&projected)
            .map(CanonicalGeometryV1::from_planar_circular_hole_v2)
    }

    pub(crate) const fn has_planar_result(&self) -> bool {
        self.result_topology.is_some()
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
