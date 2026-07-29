use std::collections::BTreeMap;
use std::ops::Deref;

use eqiora_core::RawId;
use eqiora_schema::kernel::BoundarySide;

use crate::canonical_boundary::BoundaryRelationBinding2d;
use crate::canonical_boundary::CartesianBoundaryEntry;
use crate::canonical_boundary::CartesianBoundaryInventory2d;
use crate::spatial_expression::ScalarSpatialExpression;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum StokesBoundaryKey2d {
    CartesianSide { axis: usize, side: BoundarySide },
    NamedEntitySet(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SteadyStokesBoundaryEntry2d {
    pub(super) boundary: RawId,
    pub(super) disposition: crate::canonical_boundary::PhysicalBoundaryDisposition,
}

/// One exact parent-outward normal-pressure boundary law.
///
/// The fluid traction is `-pressure * n_out`. A zero-traction Relation is the
/// exact zero tape with no coefficient Field or definition Relation.
#[derive(Debug, Clone, PartialEq)]
pub struct SteadyStokesNormalPressure2d {
    coefficient_field: Option<RawId>,
    definition_relation: Option<RawId>,
    expression: ScalarSpatialExpression,
}

impl SteadyStokesNormalPressure2d {
    pub(super) const fn zero(expression: ScalarSpatialExpression) -> Self {
        Self {
            coefficient_field: None,
            definition_relation: None,
            expression,
        }
    }

    pub(super) const fn field(
        coefficient_field: RawId,
        definition_relation: RawId,
        expression: ScalarSpatialExpression,
    ) -> Self {
        Self {
            coefficient_field: Some(coefficient_field),
            definition_relation: Some(definition_relation),
            expression,
        }
    }

    /// Exact immutable coefficient Field, absent for canonical zero traction.
    #[must_use]
    pub const fn coefficient_field(&self) -> Option<RawId> {
        self.coefficient_field
    }

    /// Exact Relation defining the coefficient tape, absent for zero traction.
    #[must_use]
    pub const fn definition_relation(&self) -> Option<RawId> {
        self.definition_relation
    }

    /// Immutable coherent-SI scalar pressure tape.
    #[must_use]
    pub const fn expression(&self) -> &ScalarSpatialExpression {
        &self.expression
    }
}

/// Exact, method-neutral 2D steady incompressible Stokes model.
///
/// The admitted volume Relations are
/// `q - expression = 0`,
/// `-div(2 mu sym(grad(u)) - isotropic_lift(p)) - grad(q) = 0`, and
/// `div(u) = 0`, plus one exact scalar definition for every distinct external
/// normal-pressure coefficient Field. Every exact boundary support carries one
/// explicit trace-zero, flux-zero, prescribed, or live field-physical
/// disposition. The support key is either an exact Cartesian side or an exact
/// geometry entity-set name; mesh membership remains a Realization concern.
/// The object retains semantic identity and immutable scalar tapes only:
/// pressure gauge, trace spaces, assembly, solver, and execution target remain
/// Realization concerns.
#[derive(Debug, Clone, PartialEq)]
pub struct SteadyIncompressibleStokesModel2d {
    pub(super) domain: RawId,
    pub(super) velocity: RawId,
    pub(super) pressure: RawId,
    pub(super) force_potential: RawId,
    pub(super) bounds: [[f64; 2]; 2],
    pub(super) dynamic_viscosity: ScalarSpatialExpression,
    pub(super) force_potential_expression: ScalarSpatialExpression,
    pub(super) force_potential_definition: RawId,
    pub(super) momentum_relation: RawId,
    pub(super) incompressibility_relation: RawId,
    pub(super) boundary_entries: BTreeMap<StokesBoundaryKey2d, SteadyStokesBoundaryEntry2d>,
    pub(super) boundary_relations: Vec<BoundaryRelationBinding2d>,
    pub(super) normal_pressures: BTreeMap<StokesBoundaryKey2d, SteadyStokesNormalPressure2d>,
    pub(super) normal_velocity_expressions: BTreeMap<StokesBoundaryKey2d, ScalarSpatialExpression>,
    pub(super) normal_velocity_coefficients: BTreeMap<StokesBoundaryKey2d, (RawId, RawId)>,
    pub(super) geometry_source_digest: Option<[u8; 32]>,
}

impl SteadyIncompressibleStokesModel2d {
    /// Canonical volume Domain.
    #[must_use]
    pub const fn domain(&self) -> RawId {
        self.domain
    }

    /// Canonical spatial-Cartesian velocity Field.
    #[must_use]
    pub const fn velocity(&self) -> RawId {
        self.velocity
    }

    /// Canonical invariant pressure Field.
    #[must_use]
    pub const fn pressure(&self) -> RawId {
        self.pressure
    }

    /// Canonical invariant conservative-force potential Field.
    #[must_use]
    pub const fn force_potential(&self) -> RawId {
        self.force_potential
    }

    /// Physical axis-aligned bounds in coherent SI coordinates.
    #[must_use]
    pub const fn bounds(&self) -> &[[f64; 2]; 2] {
        &self.bounds
    }

    /// Positive constant dynamic viscosity in coherent SI units.
    #[must_use]
    pub fn dynamic_viscosity(&self) -> f64 {
        self.dynamic_viscosity
            .constant_value()
            .expect("Stokes lowerer retains a constant dynamic-viscosity tape")
    }

    /// Immutable constant tape retaining dynamic-viscosity Parameter identity.
    #[must_use]
    pub const fn dynamic_viscosity_expression(&self) -> &ScalarSpatialExpression {
        &self.dynamic_viscosity
    }

    /// Immutable scalar tape defining the conservative-force potential.
    #[must_use]
    pub const fn force_potential_expression(&self) -> &ScalarSpatialExpression {
        &self.force_potential_expression
    }

    /// Exact Relation witnessing the force-potential definition.
    #[must_use]
    pub const fn force_potential_definition(&self) -> RawId {
        self.force_potential_definition
    }

    /// Exact Relation witnessing momentum balance.
    #[must_use]
    pub const fn momentum_relation(&self) -> RawId {
        self.momentum_relation
    }

    /// Exact Relation witnessing incompressibility.
    #[must_use]
    pub const fn incompressibility_relation(&self) -> RawId {
        self.incompressibility_relation
    }

    /// Canonically ordered exact Relation-to-Boundary support bindings.
    #[must_use]
    pub(crate) fn boundary_relations(&self) -> &[BoundaryRelationBinding2d] {
        &self.boundary_relations
    }

    pub(super) fn boundary_entries(
        &self,
    ) -> impl Iterator<Item = (&StokesBoundaryKey2d, &SteadyStokesBoundaryEntry2d)> {
        self.boundary_entries.iter()
    }

    pub(super) fn boundary_entry(
        &self,
        key: &StokesBoundaryKey2d,
    ) -> Option<&SteadyStokesBoundaryEntry2d> {
        self.boundary_entries.get(key)
    }

    pub(super) fn normal_pressure_for(
        &self,
        key: &StokesBoundaryKey2d,
    ) -> Option<&SteadyStokesNormalPressure2d> {
        self.normal_pressures.get(key)
    }

    pub(super) fn normal_pressures(&self) -> impl Iterator<Item = &SteadyStokesNormalPressure2d> {
        self.normal_pressures.values()
    }

    pub(super) fn normal_velocity_coefficients(&self) -> impl Iterator<Item = (RawId, RawId)> + '_ {
        self.normal_velocity_coefficients.values().copied()
    }

    pub(super) fn normal_velocity_expressions(
        &self,
    ) -> impl Iterator<Item = &ScalarSpatialExpression> {
        self.normal_velocity_expressions.values()
    }

    pub(super) fn prescribed_normal_velocity(
        &self,
        key: &StokesBoundaryKey2d,
        outward_normal: [f64; 2],
        coordinates: &[f64],
    ) -> Result<Option<[f64; 2]>, eqiora_core::Diagnostic> {
        let Some(expression) = self.normal_velocity_expressions.get(key) else {
            return Ok(None);
        };
        let normal_speed = expression.evaluate(coordinates)?;
        Ok(Some(
            outward_normal.map(|component| component * normal_speed),
        ))
    }

    pub(super) const fn geometry_source_digest(&self) -> Option<[u8; 32]> {
        self.geometry_source_digest
    }
}

/// Compatibility wrapper for the exact Cartesian steady-Stokes subset.
///
/// All method-neutral meaning lives in [`SteadyIncompressibleStokesModel2d`].
/// This wrapper retains the established four-side inventory without making
/// Cartesian side classification part of geometry-backed realization.
#[derive(Debug, Clone, PartialEq)]
pub struct SteadyIncompressibleStokesCartesianModel2d {
    pub(super) common: SteadyIncompressibleStokesModel2d,
    pub(super) boundary_inventory: CartesianBoundaryInventory2d,
}

impl Deref for SteadyIncompressibleStokesCartesianModel2d {
    type Target = SteadyIncompressibleStokesModel2d;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

impl SteadyIncompressibleStokesCartesianModel2d {
    /// Complete package-neutral meaning of the four exact Cartesian sides.
    #[must_use]
    pub const fn boundary_inventory(&self) -> &CartesianBoundaryInventory2d {
        &self.boundary_inventory
    }

    /// Parent-outward normal-pressure law on one exact Cartesian side.
    #[must_use]
    pub fn normal_pressure(
        &self,
        axis: usize,
        side: BoundarySide,
    ) -> Option<&SteadyStokesNormalPressure2d> {
        self.common
            .normal_pressure_for(&StokesBoundaryKey2d::CartesianSide { axis, side })
    }

    pub(super) fn from_common(
        common: SteadyIncompressibleStokesModel2d,
        entries: BTreeMap<(usize, BoundarySide), CartesianBoundaryEntry>,
    ) -> Self {
        Self {
            common,
            boundary_inventory: CartesianBoundaryInventory2d::new(entries),
        }
    }
}
