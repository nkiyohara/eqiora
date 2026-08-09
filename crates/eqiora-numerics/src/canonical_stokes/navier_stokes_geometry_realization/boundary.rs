#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora_core::{Diagnostic, RawId};
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh};

use crate::canonical_boundary::{PhysicalBoundaryDisposition, PhysicalBoundaryQuantity};
use crate::canonical_stokes::expression::IncompressibleStressForm;
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

pub(super) struct PreparedCorrespondenceBoundary2d {
    boundary: SimplicialMiniStokesBoundary2d,
    fixed_velocity: Vec<Option<[f64; DIMENSION]>>,
}

impl PreparedCorrespondenceBoundary2d {
    pub(super) const fn boundary(&self) -> &SimplicialMiniStokesBoundary2d {
        &self.boundary
    }

    pub(super) fn essential_velocity(
        &self,
        mesh: &SimplicialMesh,
        coordinate: [f64; DIMENSION],
    ) -> Result<[f64; DIMENSION], Diagnostic> {
        mesh.vertices()
            .iter()
            .position(|candidate| candidate.as_slice() == coordinate)
            .and_then(|vertex| self.fixed_velocity[vertex])
            .ok_or_else(|| {
                invalid(
                    "essential velocity callback received a vertex outside the correspondence-owned trace",
                )
            })
    }
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
        stress_form: IncompressibleStressForm,
    ) -> Result<(), Diagnostic> {
        let inlet = match stress_form {
            IncompressibleStressForm::SymmetricNewtonian => PhysicalBoundaryDisposition::TraceZero,
            IncompressibleStressForm::DfgNonsymmetric => {
                let domain = boundary_domains
                    .get("inlet")
                    .ok_or_else(|| invalid("transient binding omits `inlet`"))?;
                match model.boundary_dispositions.get(domain).copied() {
                    Some(PhysicalBoundaryDisposition::Prescribed(law))
                        if law.quantity() == PhysicalBoundaryQuantity::Trace =>
                    {
                        PhysicalBoundaryDisposition::Prescribed(law)
                    }
                    _ => {
                        return Err(invalid(
                            "DFG transient `inlet` must own one prescribed trace law",
                        ));
                    }
                }
            }
        };
        let expected = [
            ("inlet", inlet),
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
        physical: &SimplicialMesh,
        normalized: &SimplicialMesh,
        velocity_scale: f64,
    ) -> Result<PreparedCorrespondenceBoundary2d, Diagnostic> {
        if !velocity_scale.is_finite() || velocity_scale <= 0.0 {
            return Err(invalid(
                "correspondence boundary requires a positive finite velocity scale",
            ));
        }
        let mut facets = Vec::new();
        let mut fixed_velocity = vec![None; normalized.vertices().len()];
        let mut nonzero_inlet = false;
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
                Some(PhysicalBoundaryDisposition::Prescribed(law))
                    if law.quantity() == PhysicalBoundaryQuantity::Trace =>
                {
                    SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity
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
            if condition == SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity {
                for facet in &self.entities[name] {
                    let outward = facet_outward_unit_normal(physical, *facet)?;
                    for vertex in physical
                        .entity_vertices(*facet)
                        .expect("accepted correspondence facet owns vertices")
                    {
                        let value = match model.boundary_dispositions[&domain] {
                            PhysicalBoundaryDisposition::TraceZero => [0.0; DIMENSION],
                            PhysicalBoundaryDisposition::Prescribed(_) => {
                                let expression = model
                                    .normal_velocity_expressions
                                    .get(&domain)
                                    .ok_or_else(|| {
                                        invalid(
                                            "prescribed correspondence trace has no retained scalar expression",
                                        )
                                    })?;
                                let normal_speed =
                                    expression.evaluate(&physical.vertices()[vertex.index()])?;
                                outward.map(|normal| normal * normal_speed / velocity_scale)
                            }
                            _ => unreachable!("condition match admits only essential meaning"),
                        };
                        if name == "inlet" && value[0] > 0.0 {
                            nonzero_inlet = true;
                        }
                        let slot = &mut fixed_velocity[vertex.index()];
                        if slot.is_some_and(|prior| prior != value) {
                            return Err(invalid(
                                "correspondence-owned essential traces disagree at a shared vertex",
                            ));
                        }
                        *slot = Some(value);
                    }
                }
            }
        }
        if model.stress_form == IncompressibleStressForm::DfgNonsymmetric && !nonzero_inlet {
            return Err(invalid(
                "DFG inlet trace has no strictly positive correspondence-owned vertex",
            ));
        }
        let boundary = SimplicialMiniStokesBoundary2d::new(normalized, facets)
            .map_err(|error| invalid(error.message()))?;
        Ok(PreparedCorrespondenceBoundary2d {
            boundary,
            fixed_velocity,
        })
    }

    pub(super) fn boundary_facet_count(&self) -> usize {
        BOUNDARY_NAMES
            .iter()
            .map(|name| self.entities[*name].len())
            .sum()
    }

    pub(super) fn outlet_facet_count(&self) -> usize {
        self.entities["outlet"].len()
    }
}

fn facet_outward_unit_normal(
    mesh: &SimplicialMesh,
    facet: MeshEntity,
) -> Result<[f64; DIMENSION], Diagnostic> {
    let vertices = mesh
        .entity_vertices(facet)
        .ok_or_else(|| invalid("correspondence facet is absent from the bound mesh"))?;
    let adjacent = mesh
        .incidence(facet, DIMENSION)
        .ok_or_else(|| invalid("correspondence facet has no cell incidence"))?;
    let ([left, right], [cell]) = (vertices.as_slice(), adjacent.as_slice()) else {
        return Err(invalid(
            "boundary facet requires two vertices and exactly one adjacent fluid cell",
        ));
    };
    let a = &mesh.vertices()[left.index()];
    let b = &mesh.vertices()[right.index()];
    let tangent = [b[0] - a[0], b[1] - a[1]];
    let length = tangent[0].hypot(tangent[1]);
    if !length.is_finite() || length <= 0.0 {
        return Err(invalid("boundary facet has non-positive finite length"));
    }
    let cell_vertices = mesh
        .entity_vertices(cell.entity)
        .expect("validated adjacent cell owns vertices");
    let mut centroid = [0.0; DIMENSION];
    for vertex in cell_vertices {
        centroid[0] += mesh.vertices()[vertex.index()][0] / 3.0;
        centroid[1] += mesh.vertices()[vertex.index()][1] / 3.0;
    }
    let midpoint = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
    let mut normal = [tangent[1] / length, -tangent[0] / length];
    if normal[0] * (midpoint[0] - centroid[0]) + normal[1] * (midpoint[1] - centroid[1]) < 0.0 {
        normal = [-normal[0], -normal[1]];
    }
    Ok(normal)
}
