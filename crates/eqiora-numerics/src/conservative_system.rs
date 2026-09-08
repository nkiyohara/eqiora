//! Bounded cell-average conservative action. Physics supplies flux, not scatter.

pub(crate) mod contract;
pub(crate) mod euler;
mod receipt;
#[cfg(test)]
mod tests;

use eqiora_artifact::CartesianMeshEnvelopeV1;
use eqiora_core::{Diagnostic, DimExponents};
use eqiora_meshing::MeshTopology;
use eqiora_realization::{RealizationLineage, Space};
use eqiora_schema::kernel::BoundarySide;

use crate::cartesian_fvm_geometry::{
    CartesianCellMetrics, CartesianFacetAdjacency, CartesianFacetMetrics, cartesian_fvm_geometry,
};
pub(crate) use contract::{CellValueMeaning, Component, ConservativePhysics};
use contract::{Identity, invalid, reserve, validate_work};
pub(crate) use receipt::{ConservativeActionReceipt, FaceActionPacket};

/// This numerical choice belongs to the independently revisioned Realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RusanovPolicy {
    lineage: RealizationLineage,
}

impl RusanovPolicy {
    pub(crate) const fn new(lineage: RealizationLineage) -> Self {
        Self { lineage }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CellAverageState {
    identity: Identity,
    values: Vec<f64>,
}

/// Explicitly different from coefficients of the cell-constant basis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CellInventories {
    identity: Identity,
    dimensions: Vec<DimExponents>,
    values: Vec<f64>,
}

impl CellInventories {
    pub(crate) fn values(&self) -> &[f64] {
        &self.values
    }
    pub(crate) fn dimensions(&self) -> &[DimExponents] {
        &self.dimensions
    }
}

#[derive(Debug, Clone, PartialEq)]
enum BoundaryMeaning {
    ExteriorState(Vec<f64>),
    /// Physical flux density with the exact boundary's outward orientation.
    OutwardFlux(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConservativeBoundary {
    identity: Identity,
    boundary: eqiora_core::RawId,
    meaning: BoundaryMeaning,
}

/// One action owns each interface packet. There is no cell-local flux callback.
#[derive(Debug, Clone)]
pub(crate) struct ConservativeSystemAction<L, const D: usize> {
    physics: L,
    identity: Identity,
    cells: Vec<CartesianCellMetrics<D>>,
    faces: Vec<CartesianFacetMetrics<D>>,
}

impl<L: ConservativePhysics<D>, const D: usize> ConservativeSystemAction<L, D> {
    pub(crate) fn new(
        physics: L,
        mesh: &CartesianMeshEnvelopeV1,
        policy: RusanovPolicy,
        components: &[Component],
        space: Space,
    ) -> Result<Self, Diagnostic> {
        let semantic = physics.contract();
        semantic.validate::<D>()?;
        if semantic.components != components || space != Space::cell_constant() {
            return Err(invalid(
                "conservative component order/type or cell-constant space differs",
            ));
        }
        if policy.lineage.model() != semantic.model.model()
            || policy.lineage.semantic_revision() != semantic.model.semantic_revision()
        {
            return Err(invalid(
                "numerical-flux policy references another Model revision",
            ));
        }
        let mesh_data = mesh.mesh();
        if mesh_data.topological_dimension() != D {
            return Err(invalid(
                "conservative action requires the exact spatial dimension",
            ));
        }
        for (axis, bounds) in semantic.bounds.iter().enumerate() {
            let coordinates = mesh_data
                .axis_coordinates(axis)
                .ok_or_else(|| invalid("missing mesh axis"))?;
            if coordinates.first() != Some(&bounds[0]) || coordinates.last() != Some(&bounds[1]) {
                return Err(invalid("Mesh bounds differ from the exact semantic Domain"));
            }
        }
        validate_work::<D>(
            components.len(),
            mesh_data.entity_count(D).unwrap_or(0),
            mesh_data.entity_count(D - 1).unwrap_or(0),
        )?;
        let identity = Identity {
            semantic: semantic.clone(),
            mesh: mesh.artifact_reference()?,
            policy,
            space,
        };
        let (cells, faces) = cartesian_fvm_geometry::<D>(mesh_data)?;
        Ok(Self {
            physics,
            identity,
            cells,
            faces,
        })
    }

    pub(crate) fn cell_averages(
        &self,
        components: &[Component],
        meaning: CellValueMeaning,
        values: Vec<f64>,
    ) -> Result<CellAverageState, Diagnostic> {
        if components != self.identity.semantic.components || meaning != CellValueMeaning::Average {
            return Err(invalid(
                "cell values must be exact ordered conservative cell averages",
            ));
        }
        if values.len() != self.cells.len() * components.len() {
            return Err(invalid(
                "cell-average layout is incomplete; scalar broadcasting is not admitted",
            ));
        }
        for state in values.chunks_exact(components.len()) {
            self.admit_state(state)?;
        }
        Ok(CellAverageState {
            identity: self.identity.clone(),
            values,
        })
    }

    pub(crate) fn inventories(
        &self,
        state: &CellAverageState,
    ) -> Result<CellInventories, Diagnostic> {
        self.require_state(state)?;
        let k = self.width();
        let mut values = reserve(state.values.len())?;
        for (cell, state) in self.cells.iter().zip(state.values.chunks_exact(k)) {
            for value in state {
                values.push(finite(value * cell.measure, "cell inventory")?);
            }
        }
        let volume = contract::length_dimension()
            .pow(D as i32, 1)
            .ok_or_else(|| invalid("volume dimension overflow"))?;
        let dimensions = self
            .identity
            .semantic
            .components
            .iter()
            .map(|c| {
                c.value_type
                    .dimension()
                    .mul(volume)
                    .ok_or_else(|| invalid("inventory dimension overflow"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CellInventories {
            identity: self.identity.clone(),
            dimensions,
            values,
        })
    }

    pub(crate) fn exterior_state(
        &self,
        boundary: eqiora_core::RawId,
        components: &[Component],
        values: Vec<f64>,
    ) -> Result<ConservativeBoundary, Diagnostic> {
        self.require_boundary(boundary)?;
        if components != self.identity.semantic.components {
            return Err(invalid(
                "boundary state component order or dimensions differ",
            ));
        }
        self.admit_state(&values)?;
        Ok(ConservativeBoundary {
            identity: self.identity.clone(),
            boundary,
            meaning: BoundaryMeaning::ExteriorState(values),
        })
    }

    pub(crate) fn outward_flux(
        &self,
        boundary: eqiora_core::RawId,
        dimensions: &[DimExponents],
        values: Vec<f64>,
    ) -> Result<ConservativeBoundary, Diagnostic> {
        self.require_boundary(boundary)?;
        if dimensions != self.flux_dimensions()?.as_slice() || values.len() != self.width() {
            return Err(invalid(
                "boundary physical normal flux requires exact component dimensions and width",
            ));
        }
        for value in &values {
            finite(*value, "boundary physical normal flux")?;
        }
        Ok(ConservativeBoundary {
            identity: self.identity.clone(),
            boundary,
            meaning: BoundaryMeaning::OutwardFlux(values),
        })
    }

    pub(crate) fn evaluate(
        &self,
        state: &CellAverageState,
        boundaries: &[ConservativeBoundary],
    ) -> Result<ConservativeActionReceipt, Diagnostic> {
        self.require_state(state)?;
        if boundaries.len() != 2 * D {
            return Err(invalid("boundary inventory is incomplete or duplicated"));
        }
        for (index, boundary) in boundaries.iter().enumerate() {
            self.require_boundary(boundary.boundary)?;
            if boundary.identity != self.identity
                || boundaries[..index]
                    .iter()
                    .any(|old| old.boundary == boundary.boundary)
            {
                return Err(invalid(
                    "boundary occurrence, Model, closure, Mesh or policy cross-wire",
                ));
            }
        }
        let mut packets = reserve(self.faces.len())?;
        for (face, geometry) in self.faces.iter().enumerate() {
            let mut normal = [0.0; D];
            let (owner, neighbor, boundary, left, right, prescribed) = match geometry.adjacency {
                CartesianFacetAdjacency::Interior { lower, upper, .. } => {
                    normal[geometry.normal_axis] = 1.0;
                    (
                        lower,
                        Some(upper),
                        None,
                        self.cell(state, lower),
                        Some(self.cell(state, upper)),
                        None,
                    )
                }
                CartesianFacetAdjacency::Boundary { cell, side, .. } => {
                    normal[geometry.normal_axis] = if side == BoundarySide::Lower {
                        -1.0
                    } else {
                        1.0
                    };
                    let boundary_id = self.identity.semantic.boundaries
                        [2 * geometry.normal_axis + usize::from(side == BoundarySide::Upper)];
                    let meaning = &boundaries
                        .iter()
                        .find(|b| b.boundary == boundary_id)
                        .ok_or_else(|| invalid("missing exact boundary occurrence"))?
                        .meaning;
                    let (right, prescribed) = match meaning {
                        BoundaryMeaning::ExteriorState(values) => (Some(values.as_slice()), None),
                        BoundaryMeaning::OutwardFlux(values) => (None, Some(values.as_slice())),
                    };
                    (
                        cell,
                        None,
                        Some(boundary_id),
                        self.cell(state, cell),
                        right,
                        prescribed,
                    )
                }
            };
            let (normal_flux, wave_bound) = if let Some(prescribed) = prescribed {
                (prescribed.to_vec(), None)
            } else {
                let (flux, speed) =
                    self.rusanov(left, right.expect("exterior state or neighbor"), &normal)?;
                (flux, Some(speed))
            };
            let integrated = normal_flux
                .iter()
                .map(|value| finite(value * geometry.measure, "area-integrated face packet"))
                .collect::<Result<Vec<_>, _>>()?;
            packets.push(FaceActionPacket {
                face,
                owner,
                neighbor,
                boundary,
                normal: normal.to_vec(),
                area: geometry.measure,
                normal_flux,
                wave_bound,
                integrated,
            });
        }
        receipt::assemble(&self.identity, &self.cells, packets)
    }

    /// Replay exact packet derivation and all accounting from the accepted input.
    /// This also detects stale wave bounds, omitted faces and divergent local copies.
    pub(crate) fn replay(
        &self,
        state: &CellAverageState,
        boundaries: &[ConservativeBoundary],
        receipt: &ConservativeActionReceipt,
    ) -> Result<(), Diagnostic> {
        if &self.evaluate(state, boundaries)? != receipt {
            return Err(invalid(
                "conservative face-action replay differs from exact input/packet/accounting",
            ));
        }
        Ok(())
    }

    fn rusanov(
        &self,
        left: &[f64],
        right: &[f64],
        normal: &[f64; D],
    ) -> Result<(Vec<f64>, f64), Diagnostic> {
        let (left_flux, left_speed) = self.checked_sample(left, normal)?;
        let (right_flux, right_speed) = self.checked_sample(right, normal)?;
        let speed = left_speed.max(right_speed);
        let mut flux = reserve(self.width())?;
        for component in 0..self.width() {
            flux.push(finite(
                0.5 * left_flux[component] + 0.5 * right_flux[component]
                    - 0.5 * speed * (right[component] - left[component]),
                "Rusanov normal flux",
            )?);
        }
        Ok((flux, speed))
    }

    fn checked_sample(
        &self,
        state: &[f64],
        normal: &[f64; D],
    ) -> Result<(Vec<f64>, f64), Diagnostic> {
        self.admit_state(state)?;
        let (flux, speed) = self.physics.normal_flux_and_wave_bound(state, normal)?;
        if flux.len() != self.width()
            || flux
                .iter()
                .zip(self.flux_dimensions()?)
                .any(|(value, dimension)| !value.value().is_finite() || value.dim() != dimension)
            || !speed.value().is_finite()
            || speed.value() < 0.0
            || speed.dim() != contract::velocity_dimension()
        {
            return Err(invalid(
                "semantic adapter returned invalid physical normal flux or wave bound",
            ));
        }
        Ok((
            flux.into_iter().map(|value| value.value()).collect(),
            speed.value(),
        ))
    }
    fn admit_state(&self, state: &[f64]) -> Result<(), Diagnostic> {
        if state.len() != self.width() || state.iter().any(|x| !x.is_finite()) {
            return Err(invalid(
                "physical state has wrong width or nonfinite component",
            ));
        }
        self.physics.admit(state)
    }
    fn require_state(&self, state: &CellAverageState) -> Result<(), Diagnostic> {
        if state.identity != self.identity || state.values.len() != self.cells.len() * self.width()
        {
            return Err(invalid(
                "state Model, closure, occurrence, Mesh, space or policy cross-wire",
            ));
        }
        Ok(())
    }
    fn require_boundary(&self, boundary: eqiora_core::RawId) -> Result<(), Diagnostic> {
        if !self.identity.semantic.boundaries.contains(&boundary) {
            return Err(invalid("foreign boundary occurrence"));
        }
        Ok(())
    }
    fn flux_dimensions(&self) -> Result<Vec<DimExponents>, Diagnostic> {
        self.identity
            .semantic
            .components
            .iter()
            .map(|c| {
                c.value_type
                    .dimension()
                    .mul(contract::velocity_dimension())
                    .ok_or_else(|| invalid("flux dimension overflow"))
            })
            .collect()
    }
    fn width(&self) -> usize {
        self.identity.semantic.components.len()
    }
    fn cell<'a>(&self, state: &'a CellAverageState, cell: usize) -> &'a [f64] {
        &state.values[cell * self.width()..(cell + 1) * self.width()]
    }
}

fn finite(value: f64, role: &str) -> Result<f64, Diagnostic> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid(format!("{role} overflowed")))
    }
}
