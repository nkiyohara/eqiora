#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora_core::{Diagnostic, RawId};
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh};

use crate::canonical_boundary::PhysicalBoundaryDisposition;
use crate::simplicial_stokes::{
    SimplicialMiniStokesBoundary2d, SimplicialMiniStokesBoundaryCondition2d,
    SimplicialMiniStokesBoundaryFacet2d,
};

use super::invalid;
use crate::canonical_stokes::navier_stokes::TransientIncompressibleNavierStokesModel2d;

const DIMENSION: usize = 2;
const BOUNDARY_NAMES: [&str; 4] = ["inlet", "outlet", "walls", "cylinder"];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CorrespondenceBoundary2d {
    entities: BTreeMap<String, Vec<MeshEntity>>,
}

impl CorrespondenceBoundary2d {
    pub(super) fn new(
        accepted: &AcceptedCircularHoleChordalRealizationV1,
    ) -> Result<Self, Diagnostic> {
        let mesh = accepted.mesh().mesh();
        let correspondence = accepted.correspondence();
        let geometry = accepted.realized_geometry();
        let mut entities = BTreeMap::new();
        for name in BOUNDARY_NAMES.into_iter().chain(["fluid"]) {
            entities.insert(
                name.to_owned(),
                correspondence
                    .region_entity_set_entities(geometry, name)
                    .map_err(|error| {
                        invalid(format!(
                            "correspondence cannot resolve required transient set `{name}`: {}",
                            error.message()
                        ))
                    })?,
            );
        }
        let expected_cells = (0..mesh.entity_count(DIMENSION).expect("2D mesh cells"))
            .map(|index| MeshEntity::new(DIMENSION, index))
            .collect::<Vec<_>>();
        if entities["fluid"] != expected_cells {
            return Err(invalid(
                "the exact `fluid` entity set does not realize every mesh cell exactly once",
            ));
        }
        let mut owned = BTreeSet::new();
        for name in BOUNDARY_NAMES {
            let facets = &entities[name];
            if facets.is_empty()
                || facets.iter().any(|facet| {
                    facet.dimension() != DIMENSION - 1
                        || mesh.is_boundary_entity(*facet) != Some(true)
                        || !owned.insert(*facet)
                })
            {
                return Err(invalid(
                    "correspondence-derived transient boundary sets must be nonempty, disjoint exterior facets",
                ));
            }
        }
        let exterior = (0..mesh.entity_count(DIMENSION - 1).expect("2D mesh edges"))
            .map(|index| MeshEntity::new(DIMENSION - 1, index))
            .filter(|facet| mesh.is_boundary_entity(*facet) == Some(true))
            .collect::<BTreeSet<_>>();
        if owned != exterior {
            return Err(invalid(
                "correspondence-derived transient boundary sets must cover the exterior exactly once",
            ));
        }
        Ok(Self { entities })
    }

    pub(super) fn require_dispositions(
        &self,
        model: &TransientIncompressibleNavierStokesModel2d,
        boundary_domains: &BTreeMap<String, RawId>,
    ) -> Result<(), Diagnostic> {
        let expected = [
            ("inlet", PhysicalBoundaryDisposition::TraceZero),
            ("outlet", PhysicalBoundaryDisposition::FluxZero),
            ("walls", PhysicalBoundaryDisposition::TraceZero),
            ("cylinder", PhysicalBoundaryDisposition::TraceZero),
        ];
        for (name, disposition) in expected {
            let domain = boundary_domains
                .get(name)
                .ok_or_else(|| invalid(format!("transient binding omits `{name}`")))?;
            if model.boundary_dispositions.get(domain) != Some(&disposition) {
                return Err(invalid(format!(
                    "transient boundary `{name}` has a disposition outside the exact-zero mixed partition"
                )));
            }
        }
        if model.boundary_dispositions.len() != expected.len() {
            return Err(invalid(
                "transient binding contains a boundary outside the exact four-name inventory",
            ));
        }
        Ok(())
    }

    pub(super) fn numerical_boundary(
        &self,
        model: &TransientIncompressibleNavierStokesModel2d,
        boundary_domains: &BTreeMap<String, RawId>,
        normalized: &SimplicialMesh,
    ) -> Result<SimplicialMiniStokesBoundary2d, Diagnostic> {
        let mut facets = Vec::new();
        for name in BOUNDARY_NAMES {
            let domain = boundary_domains[name];
            let condition = match model.boundary_dispositions.get(&domain) {
                Some(PhysicalBoundaryDisposition::TraceZero) => {
                    SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
                }
                Some(PhysicalBoundaryDisposition::FluxZero) => {
                    SimplicialMiniStokesBoundaryCondition2d::ConstantTraction {
                        value: [0.0; DIMENSION],
                    }
                }
                _ => {
                    return Err(invalid(
                        "transient numerical boundary differs from the admitted exact-zero mixed partition",
                    ));
                }
            };
            facets.extend(
                self.entities[name]
                    .iter()
                    .copied()
                    .map(|facet| SimplicialMiniStokesBoundaryFacet2d::new(facet, condition)),
            );
        }
        SimplicialMiniStokesBoundary2d::new(normalized, facets)
            .map_err(|error| invalid(error.message()))
    }
}
