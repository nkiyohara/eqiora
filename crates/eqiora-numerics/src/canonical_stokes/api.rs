use std::collections::BTreeMap;

use eqiora_core::RawId;
use eqiora_schema::kernel::BoundarySide;

use crate::canonical_boundary::BoundaryRelationBinding2d;
use crate::canonical_boundary::CartesianBoundaryInventory2d;
use crate::spatial_expression::ScalarSpatialExpression;

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
/// normal-pressure coefficient Field. Every side of the exact Cartesian box
/// carries one explicit trace-zero, flux-zero, prescribed, or live
/// field-physical boundary disposition. The object retains semantic identity
/// and immutable scalar tapes only: mesh, pressure gauge, trace spaces,
/// assembly, solver, and execution target remain Realization concerns.
#[derive(Debug, Clone, PartialEq)]
pub struct SteadyIncompressibleStokesCartesianModel2d {
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
    pub(super) boundary_inventory: CartesianBoundaryInventory2d,
    pub(super) boundary_relations: Vec<BoundaryRelationBinding2d>,
    pub(super) normal_pressures: BTreeMap<(usize, BoundarySide), SteadyStokesNormalPressure2d>,
}

impl SteadyIncompressibleStokesCartesianModel2d {
    /// Canonical Cartesian volume Domain.
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

    /// Physical Cartesian bounds in coherent SI coordinates.
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

    /// Complete package-neutral meaning of the four exact Cartesian sides.
    #[must_use]
    pub const fn boundary_inventory(&self) -> &CartesianBoundaryInventory2d {
        &self.boundary_inventory
    }

    /// Canonically ordered exact Relation-to-Boundary support bindings.
    #[must_use]
    pub(crate) fn boundary_relations(&self) -> &[BoundaryRelationBinding2d] {
        &self.boundary_relations
    }

    /// Parent-outward normal-pressure law on one exact side, when present.
    ///
    /// `Some` covers both an explicit pressure coefficient and canonical zero
    /// traction. `None` denotes a trace condition or unresolved live Port.
    #[must_use]
    pub fn normal_pressure(
        &self,
        axis: usize,
        side: BoundarySide,
    ) -> Option<&SteadyStokesNormalPressure2d> {
        self.normal_pressures.get(&(axis, side))
    }
}
