use std::collections::BTreeSet;

use eqiora_core::Diagnostic;

use crate::{MeshEntity, MeshGeometry, MeshTopology, SimplicialMesh};

use super::{COMPONENTS, DIMENSION, invalid};

/// Numerical boundary meaning admitted by the bounded MINI realization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimplicialMiniStokesBoundaryCondition2d {
    /// Prescribe the complete vector-valued P1 velocity trace.
    EssentialVelocity,
    /// Prescribe one constant parent-outward traction vector on the facet.
    ConstantTraction {
        /// Dimensionless traction components in the ambient Cartesian frame.
        value: [f64; COMPONENTS],
    },
}

/// One exact mesh facet and its complete numerical boundary condition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimplicialMiniStokesBoundaryFacet2d {
    facet: MeshEntity,
    condition: SimplicialMiniStokesBoundaryCondition2d,
}

impl SimplicialMiniStokesBoundaryFacet2d {
    /// Bind one mesh-local facet to one boundary condition.
    #[must_use]
    pub const fn new(
        facet: MeshEntity,
        condition: SimplicialMiniStokesBoundaryCondition2d,
    ) -> Self {
        Self { facet, condition }
    }

    /// Exact mesh-local boundary facet.
    #[must_use]
    pub const fn facet(self) -> MeshEntity {
        self.facet
    }

    /// Condition applied over the complete facet.
    #[must_use]
    pub const fn condition(self) -> SimplicialMiniStokesBoundaryCondition2d {
        self.condition
    }
}

/// Complete, facet-derived boundary closure for one 2D simplicial mesh.
///
/// Every topological boundary facet occurs exactly once. Essential velocity is
/// evaluated later at the union of vertices belonging to essential facets;
/// consequently a corner shared by essential and traction facets is fixed in
/// the reduced trace space without discarding its full-system traction action.
#[derive(Debug, Clone, PartialEq)]
pub struct SimplicialMiniStokesBoundary2d {
    facets: Vec<SimplicialMiniStokesBoundaryFacet2d>,
}

impl SimplicialMiniStokesBoundary2d {
    /// Validate one complete boundary-facet partition in canonical facet order.
    ///
    /// # Errors
    /// Returns `EQ0801` for a non-facet, interior, missing, duplicate, or
    /// non-finite boundary entry.
    pub fn new(
        mesh: &SimplicialMesh,
        facets: impl IntoIterator<Item = SimplicialMiniStokesBoundaryFacet2d>,
    ) -> Result<Self, Diagnostic> {
        if mesh.topological_dimension() != DIMENSION {
            return Err(invalid(
                "MINI Stokes boundary closure requires a two-dimensional mesh",
            ));
        }
        let facet_count = mesh
            .entity_count(DIMENSION - 1)
            .expect("2D simplex mesh owns edge entities");
        let expected = (0..facet_count)
            .filter(|index| {
                let facet = MeshEntity::new(DIMENSION - 1, *index);
                mesh.is_boundary_entity(facet)
                    .expect("mesh owns every edge boundary classification")
            })
            .collect::<Vec<_>>();
        let mut facets = facets.into_iter().collect::<Vec<_>>();
        facets.sort_by_key(|entry| (entry.facet.dimension(), entry.facet.index()));
        if facets.windows(2).any(|pair| pair[0].facet == pair[1].facet) {
            return Err(invalid(
                "MINI Stokes boundary closure contains a duplicate facet",
            ));
        }
        for entry in &facets {
            if entry.facet.dimension() != DIMENSION - 1
                || entry.facet.index() >= facet_count
                || !mesh
                    .is_boundary_entity(entry.facet)
                    .expect("validated facet index belongs to the mesh")
            {
                return Err(invalid(
                    "MINI Stokes boundary closure contains a non-boundary facet",
                ));
            }
            if matches!(
                entry.condition,
                SimplicialMiniStokesBoundaryCondition2d::ConstantTraction { value }
                    if value.iter().any(|component| !component.is_finite())
            ) {
                return Err(invalid("MINI Stokes prescribed traction must be finite"));
            }
        }
        let actual = facets
            .iter()
            .map(|entry| entry.facet.index())
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(invalid(
                "MINI Stokes boundary closure must cover every boundary facet exactly once",
            ));
        }
        Ok(Self { facets })
    }

    /// Construct the legacy complete-essential boundary closure.
    ///
    /// # Errors
    /// Preserves complete-boundary validation failures.
    pub fn all_essential(mesh: &SimplicialMesh) -> Result<Self, Diagnostic> {
        let facet_count = mesh
            .entity_count(DIMENSION - 1)
            .ok_or_else(|| invalid("MINI Stokes mesh has no edge stratum"))?;
        Self::new(
            mesh,
            (0..facet_count).filter_map(|index| {
                let facet = MeshEntity::new(DIMENSION - 1, index);
                mesh.is_boundary_entity(facet)
                    .filter(|boundary| *boundary)
                    .map(|_| {
                        SimplicialMiniStokesBoundaryFacet2d::new(
                            facet,
                            SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity,
                        )
                    })
            }),
        )
    }

    /// Canonically facet-ordered complete boundary inventory.
    #[must_use]
    pub fn facets(&self) -> &[SimplicialMiniStokesBoundaryFacet2d] {
        &self.facets
    }

    pub(crate) fn prepare<B>(
        &self,
        mesh: &SimplicialMesh,
        essential_velocity: &B,
    ) -> Result<PreparedBoundary2d, Diagnostic>
    where
        B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
    {
        let mut essential_vertices = BTreeSet::new();
        let mut traction_facets = Vec::new();
        for entry in &self.facets {
            let vertices = mesh
                .entity_vertices(entry.facet)
                .expect("validated boundary edge owns vertices");
            match entry.condition {
                SimplicialMiniStokesBoundaryCondition2d::EssentialVelocity => {
                    essential_vertices.extend(vertices.into_iter().map(MeshEntity::index));
                }
                SimplicialMiniStokesBoundaryCondition2d::ConstantTraction { value } => {
                    traction_facets.push(PreparedTractionFacet2d {
                        facet: entry.facet,
                        value,
                    });
                }
            }
        }
        let has_essential = !essential_vertices.is_empty();
        let has_traction = !traction_facets.is_empty();
        if !has_essential && has_traction {
            return Err(invalid(
                "MINI Stokes mixed-boundary slice requires at least one essential facet to remove velocity rigid modes",
            ));
        }
        if !has_essential && !has_traction {
            return Err(invalid("MINI Stokes boundary closure is empty"));
        }
        let mut fixed_velocity = vec![None; mesh.vertices().len()];
        for vertex in essential_vertices {
            let coordinates = &mesh.vertices()[vertex];
            let value = essential_velocity([coordinates[0], coordinates[1]])?;
            if value.iter().any(|component| !component.is_finite()) {
                return Err(invalid("MINI Stokes essential velocity is non-finite"));
            }
            fixed_velocity[vertex] = Some(value);
        }
        let pressure_reference = if has_traction {
            PressureReferenceKind2d::BoundaryTraction
        } else {
            PressureReferenceKind2d::ZeroIntegral
        };
        Ok(PreparedBoundary2d {
            fixed_velocity,
            traction_facets,
            pressure_reference,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PressureReferenceKind2d {
    ZeroIntegral,
    BoundaryTraction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreparedTractionFacet2d {
    pub(crate) facet: MeshEntity,
    pub(crate) value: [f64; COMPONENTS],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedBoundary2d {
    pub(crate) fixed_velocity: Vec<Option<[f64; COMPONENTS]>>,
    pub(crate) traction_facets: Vec<PreparedTractionFacet2d>,
    pub(crate) pressure_reference: PressureReferenceKind2d,
}

impl PreparedBoundary2d {
    pub(super) fn integrated_traction(
        &self,
        mesh: &SimplicialMesh,
    ) -> Result<[f64; COMPONENTS], Diagnostic> {
        let mut integrated = [0.0; COMPONENTS];
        for facet in &self.traction_facets {
            let geometry = mesh
                .geometry_map(facet.facet)
                .expect("validated boundary facet owns affine geometry");
            let length = geometry.measure_scale();
            for (result, value) in integrated.iter_mut().zip(facet.value) {
                *result += length * value;
            }
        }
        if integrated.iter().any(|component| !component.is_finite()) {
            return Err(invalid(
                "MINI Stokes integrated boundary traction is non-finite",
            ));
        }
        Ok(integrated)
    }
}
