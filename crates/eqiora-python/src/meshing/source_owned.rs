//! Private admission of deterministic Geometry v2 Mesh resources.

use eqiora::Diagnostic;
use eqiora::artifact::{GeometryMeshCorrespondenceEnvelopeV1, SimplicialMeshEnvelopeV1};
use eqiora::diagnostic::codes;
use eqiora::geometry::{CanonicalGeometryV1, EDGE_DIMENSION};
use eqiora::meshing::MeshQualityGate;

#[derive(Clone)]
pub(super) struct SourceOwnedPlan {
    pub(super) source: CanonicalGeometryV1,
    pub(super) mesh: SimplicialMeshEnvelopeV1,
    pub(super) correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    requested_max_boundary_error_m: f64,
    minimum_mean_ratio: f64,
    maximum_boundary_facets: usize,
}

impl SourceOwnedPlan {
    pub(super) fn resolve(
        source: &CanonicalGeometryV1,
        requested_max_boundary_error_m: f64,
        maximum_boundary_facets: usize,
        quality_gate: MeshQualityGate,
    ) -> Result<Self, Diagnostic> {
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
                source,
                requested_max_boundary_error_m,
                maximum_boundary_facets,
                quality_gate,
            )?;
        let plan = Self {
            source: source.clone(),
            mesh,
            correspondence,
            requested_max_boundary_error_m,
            minimum_mean_ratio: quality_gate.minimum_mean_ratio(),
            maximum_boundary_facets,
        };
        plan.boundary_facets()?;
        Ok(plan)
    }

    pub(super) fn revalidate(&self, source: &CanonicalGeometryV1) -> Result<(), Diagnostic> {
        if source != &self.source {
            return Err(invalid("MeshPlan belongs to a different exact Geometry"));
        }
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
                source,
                self.requested_max_boundary_error_m,
                self.maximum_boundary_facets,
                MeshQualityGate::new(self.minimum_mean_ratio)?,
            )?;
        if mesh != self.mesh || correspondence != self.correspondence {
            return Err(invalid(
                "source-owned Mesh resources differ from deterministic replay",
            ));
        }
        Ok(())
    }

    pub(super) fn boundary_facets(&self) -> Result<usize, Diagnostic> {
        let circle = self
            .source
            .entity_sets()
            .iter()
            .find(|set| set.dimension() == EDGE_DIMENSION && set.members() == [4])
            .ok_or_else(|| invalid("source-owned Geometry has no exact circular frontier"))?;
        self.correspondence
            .planar_circular_hole_v2_entity_set_entities(&self.source, circle.name())
            .map(|entities| entities.len())
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use eqiora::geometry::{CadAuthoredGraph, ConstrainedRectangleV1};

    use super::*;

    fn source() -> CanonicalGeometryV1 {
        let predecessor = CadAuthoredGraph::new(
            ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap(),
            1.0,
            1.0e-10,
        )
        .unwrap();
        let end_cap = predecessor.face_handle("end-cap").unwrap();
        let x_lower = predecessor.face_handle("profile-x-lower").unwrap();
        let x_upper = predecessor.face_handle("profile-x-upper").unwrap();
        let y_lower = predecessor.face_handle("profile-y-lower").unwrap();
        let y_upper = predecessor.face_handle("profile-y-upper").unwrap();
        let graph = predecessor
            .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
            .unwrap();
        let cut_wall = graph.face_handle("cut-wall").unwrap();
        graph
            .planar_result()
            .unwrap()
            .with_named_topology(&BTreeMap::from([
                ("fluid".to_owned(), vec![end_cap]),
                ("inlet".to_owned(), vec![x_lower]),
                ("outlet".to_owned(), vec![x_upper]),
                ("walls".to_owned(), vec![y_lower, y_upper]),
                ("cylinder".to_owned(), vec![cut_wall]),
            ]))
            .unwrap()
    }

    #[test]
    fn source_owned_plan_rejects_resource_replay_drift() {
        let source = source();
        let gate = MeshQualityGate::new(1.0e-5).unwrap();
        let plan = SourceOwnedPlan::resolve(&source, 1.0e-4, 50, gate).unwrap();
        plan.revalidate(&source).unwrap();

        let alternate = SourceOwnedPlan::resolve(&source, 2.0e-4, 50, gate).unwrap();
        let mut mesh_drift = plan.clone();
        mesh_drift.mesh = alternate.mesh.clone();
        assert!(mesh_drift.revalidate(&source).is_err());

        let mut correspondence_drift = plan.clone();
        correspondence_drift.correspondence = alternate.correspondence.clone();
        assert!(correspondence_drift.revalidate(&source).is_err());
    }
}
