//! Immutable planar result topology projected from an accepted authored-CAD build.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{
    CadAuthoredBuild, CadAuthoredFaceHandle, CadAuthoredGraph,
    CanonicalPlanarCircularHoleGeometryV2, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet,
};

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Opaque member of one exact accepted planar result topology.
///
/// The handle carries only its topological dimension and owner identity as
/// public observations. It has no canonical wire format, provider index, or
/// independent construction provenance.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CadAuthoredResultTopologyHandle {
    owner_graph_digest: [u8; 32],
    dimension: usize,
    member: usize,
}

impl CadAuthoredResultTopologyHandle {
    /// Topological dimension of this result member.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Exact authored-graph identity owning this result handle.
    #[must_use]
    pub const fn owner_graph_digest_bytes(&self) -> [u8; 32] {
        self.owner_graph_digest
    }
}

/// Accepted bounded planar result of one exact authored graph.
///
/// This owner contains the accepted build receipt and its complete
/// source-to-result relation. It is not a generic Boolean result or B-rep;
/// broader primitive/subtract result ergonomics remain outside this bounded
/// planar circular-hole surface.
#[derive(Clone, Debug, PartialEq)]
pub struct CadAuthoredPlanarResult {
    build: CadAuthoredBuild,
    bounds: [[f64; 2]; 2],
    circle_center: [f64; 2],
    circle_radius_m: f64,
}

impl CadAuthoredPlanarResult {
    pub(crate) fn from_graph(graph: &CadAuthoredGraph) -> Result<Self, Diagnostic> {
        let build = graph.build_analytic()?;
        if build.graph_digest_bytes() != graph.digest_bytes() {
            return Err(invalid(
                "accepted planar result build belongs to a foreign graph identity",
            ));
        }
        let Some((bounds, circle_center, circle_radius_m)) = graph.planar_cut_parts() else {
            return Err(invalid(
                "accepted planar result requires the admitted circular through-cut graph",
            ));
        };
        Ok(Self {
            build,
            bounds,
            circle_center,
            circle_radius_m,
        })
    }

    /// Exact authored-graph identity owning this result.
    #[must_use]
    pub const fn owner_graph_digest_bytes(&self) -> [u8; 32] {
        self.build.graph_digest_bytes()
    }

    /// Read-only accepted build receipt retained by this result.
    #[must_use]
    pub const fn build(&self) -> &CadAuthoredBuild {
        &self.build
    }

    /// Project one source-owned construction handle through accepted lineage.
    ///
    /// # Errors
    /// Returns `EQ0901` when the handle is foreign or stale, was deleted, or
    /// does not resolve to one unambiguous member of this result.
    pub fn project(
        &self,
        source: &CadAuthoredFaceHandle,
    ) -> Result<CadAuthoredResultTopologyHandle, Diagnostic> {
        let (dimension, member) = self.build.project_result_member(source)?;
        Ok(CadAuthoredResultTopologyHandle {
            owner_graph_digest: self.owner_graph_digest_bytes(),
            dimension,
            member,
        })
    }

    /// Atomically bind complete result membership into canonical Geometry v2.
    ///
    /// Every group must be nonempty and dimension-homogeneous. Every result
    /// member must be owned by this result and occur exactly once across the
    /// complete mapping. No coordinates, proximity, provider IDs, mesh labels,
    /// or classification tolerance participate.
    ///
    /// # Errors
    /// Returns `EQ0901` for foreign, incomplete, duplicate, empty, or
    /// mixed-dimensional membership, or invalid semantic names.
    pub fn with_named_topology(
        &self,
        named_topology: &BTreeMap<String, Vec<CadAuthoredResultTopologyHandle>>,
    ) -> Result<CanonicalPlanarCircularHoleGeometryV2, Diagnostic> {
        let mut covered = BTreeSet::new();
        let mut entity_sets = Vec::with_capacity(named_topology.len());
        for (name, handles) in named_topology {
            let Some(first) = handles.first() else {
                return Err(invalid("named result-topology group must not be empty"));
            };
            let dimension = first.dimension();
            let mut members = Vec::with_capacity(handles.len());
            for handle in handles {
                if handle.owner_graph_digest_bytes() != self.owner_graph_digest_bytes() {
                    return Err(invalid(
                        "named result-topology handle belongs to a foreign owner identity",
                    ));
                }
                if handle.dimension() != dimension {
                    return Err(invalid(
                        "one result-topology name cannot group mixed dimensions",
                    ));
                }
                let identity = (handle.dimension, handle.member);
                if !covered.insert(identity) {
                    return Err(invalid(
                        "result-topology membership must be named exactly once",
                    ));
                }
                members.push(handle.member);
            }
            entity_sets.push(NamedEntitySet::new(name, dimension, members));
        }
        let expected = BTreeSet::from([
            (EDGE_DIMENSION, 0),
            (EDGE_DIMENSION, 1),
            (EDGE_DIMENSION, 2),
            (EDGE_DIMENSION, 3),
            (EDGE_DIMENSION, 4),
            (FACE_DIMENSION, 0),
        ]);
        if covered != expected {
            return Err(invalid(
                "named result topology must cover the complete planar result exactly once",
            ));
        }
        CanonicalPlanarCircularHoleGeometryV2::new(
            self.bounds,
            self.circle_center,
            self.circle_radius_m,
            entity_sets,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConstrainedRectangleV1;

    fn scaled_graph(scale: f64) -> (CadAuthoredGraph, CadAuthoredGraph) {
        let channel = CadAuthoredGraph::new(
            ConstrainedRectangleV1::new((0.0, 2.2 * scale), (0.0, 0.41 * scale), 0.0).unwrap(),
            scale,
            1.0e-10 * scale,
        )
        .unwrap();
        let fluid = channel
            .circular_through_cut([0.2 * scale, 0.2 * scale], 0.05 * scale, 1.0e-10 * scale)
            .unwrap();
        (channel, fluid)
    }

    fn named(
        channel: &CadAuthoredGraph,
        fluid: &CadAuthoredGraph,
        result: &CadAuthoredPlanarResult,
    ) -> BTreeMap<String, Vec<CadAuthoredResultTopologyHandle>> {
        let project = |name| result.project(&channel.face_handle(name).unwrap()).unwrap();
        BTreeMap::from([
            ("fluid".to_owned(), vec![project("end-cap")]),
            ("inlet".to_owned(), vec![project("profile-x-lower")]),
            ("outlet".to_owned(), vec![project("profile-x-upper")]),
            (
                "walls".to_owned(),
                vec![project("profile-y-lower"), project("profile-y-upper")],
            ),
            (
                "cylinder".to_owned(),
                vec![
                    result
                        .project(&fluid.face_handle("cut-wall").unwrap())
                        .unwrap(),
                ],
            ),
        ])
    }

    #[test]
    fn scale_family_projects_and_names_complete_geometry_v2() {
        for exponent in [-40, 0, 40] {
            let (channel, fluid) = scaled_graph(2.0_f64.powi(exponent));
            let result = CadAuthoredPlanarResult::from_graph(&fluid).unwrap();
            let geometry = result
                .with_named_topology(&named(&channel, &fluid, &result))
                .unwrap();
            assert_eq!(geometry.entity_set("fluid").unwrap().dimension(), 2);
            assert_eq!(geometry.entity_set("cylinder").unwrap().dimension(), 1);
        }
    }

    #[test]
    fn projection_and_atomic_naming_fail_closed() {
        let (channel, fluid) = scaled_graph(1.0);
        let result = CadAuthoredPlanarResult::from_graph(&fluid).unwrap();
        let (foreign, _) = scaled_graph(2.0);
        assert!(
            result
                .project(&foreign.face_handle("profile-x-lower").unwrap())
                .is_err()
        );
        assert!(
            result
                .project(&channel.face_handle("start-cap").unwrap())
                .is_err()
        );

        let mut incomplete = named(&channel, &fluid, &result);
        incomplete.remove("cylinder");
        assert!(result.with_named_topology(&incomplete).is_err());

        let mut duplicate = named(&channel, &fluid, &result);
        duplicate.get_mut("walls").unwrap().push(
            result
                .project(&fluid.face_handle("cut-wall").unwrap())
                .unwrap(),
        );
        assert!(result.with_named_topology(&duplicate).is_err());
    }

    #[test]
    fn registered_planar_circular_hole_geometry_v2_evidence() {
        crate::circular_hole::tests::independent_identity_witness_is_exact();
        crate::circular_hole_v2::tests::independent_ordinary_identity_witness_is_exact();
        crate::circular_hole_v2::tests::v1_and_v2_decoders_reject_each_others_wire();
        crate::cad_authored_build::tests::incomplete_or_substituted_build_observation_rejects();
        crate::cad_authored_build::tests::result_projection_requires_exact_source_handle_generation(
        );
        scale_family_projects_and_names_complete_geometry_v2();
        projection_and_atomic_naming_fail_closed();
        crate::circular_hole_v2::tests::strict_scale_independent_geometry_replays_without_tolerance(
        );
        crate::circular_hole_v2::tests::finite_increasing_positive_and_strict_clearance_predicates_fail_closed();
        crate::circular_hole_v2::tests::closed_v2_decoder_rejects_noncanonical_and_open_wire_mutants();
    }
}
