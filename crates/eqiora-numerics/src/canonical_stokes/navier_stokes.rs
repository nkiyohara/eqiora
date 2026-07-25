//! Method-neutral recognition of fixed-domain transient incompressible flow.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, RawId};
use eqiora_graph::EdgeKind;
use eqiora_ir::{
    OperatorApplicationProof, PureOperatorApplicationProof, PureOperatorDefinition,
    StandardPureOperator,
};
use eqiora_schema::kernel::typing::TypedResidual;
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, KernelNode, SymbolRef};
use eqiora_sem::KernelProgram;

use crate::canonical_boundary::BoundaryRelationBinding;
use crate::canonical_boundary::CartesianBoundaryInventory;
use crate::spatial_expression::{self, ScalarSpatialExpression};

use super::boundary::{self, LoweredStokesBoundary};
use super::expression::{
    is_divergence_of_field, load_definition_root, lower_newtonian_stress_viscosity,
};
use super::inertial::{parameters_referenced_by, require_positive_constant};
use super::recognize::{exact_fields_for_dimension, unique_box};
use super::support::{
    lowering_error, model_lowering_error, relation_expression, relations_on,
    require_continuous_relation, typed_relation, unique_root,
};

/// Exact fixed-domain transient incompressible Navier--Stokes meaning in `D` dimensions.
///
/// The admitted volume meaning is
///
/// ```text
/// density * derivative(velocity)
///   + div(density * outer_product(velocity, velocity))
///   - div(2 * viscosity * sym(grad(velocity)) - I * pressure)
///   - grad(force_potential) = 0
/// div(velocity) = 0
/// force_potential - expression = 0
/// ```
///
/// Density in inertia and convective flux is one identical revision-local
/// scalar expression. Every exact Cartesian side has one normalized boundary
/// disposition. Time integration, mesh motion, quadrature, spatial method,
/// assembly, nonlinear strategy, and linear solver choices are absent.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientIncompressibleNavierStokesCartesianModel<const D: usize> {
    domain: RawId,
    velocity: RawId,
    pressure: RawId,
    force_potential: RawId,
    bounds: [[f64; 2]; D],
    mass_density: ScalarSpatialExpression,
    dynamic_viscosity: ScalarSpatialExpression,
    force_potential_expression: ScalarSpatialExpression,
    force_potential_definition: RawId,
    momentum_relation: RawId,
    incompressibility_relation: RawId,
    boundary_inventory: CartesianBoundaryInventory<D>,
    boundary_relations: Vec<BoundaryRelationBinding>,
}

impl<const D: usize> TransientIncompressibleNavierStokesCartesianModel<D> {
    /// Canonical Cartesian fluid Domain.
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

    /// Canonical conservative-force potential Field.
    #[must_use]
    pub const fn force_potential(&self) -> RawId {
        self.force_potential
    }

    /// Physical Cartesian bounds in coherent SI coordinates.
    #[must_use]
    pub const fn bounds(&self) -> &[[f64; 2]; D] {
        &self.bounds
    }

    /// Positive constant mass density in coherent SI units.
    #[must_use]
    pub fn mass_density(&self) -> f64 {
        self.mass_density
            .constant_value()
            .expect("transient-flow lowerer retains a constant density tape")
    }

    /// Immutable density tape shared by inertia and conservative advection.
    #[must_use]
    pub const fn mass_density_expression(&self) -> &ScalarSpatialExpression {
        &self.mass_density
    }

    /// Positive constant dynamic viscosity in coherent SI units.
    #[must_use]
    pub fn dynamic_viscosity(&self) -> f64 {
        self.dynamic_viscosity
            .constant_value()
            .expect("transient-flow lowerer retains a constant viscosity tape")
    }

    /// Immutable viscosity tape retaining revision-local Parameter identity.
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

    /// Exact Relation witnessing transient conservative momentum balance.
    #[must_use]
    pub const fn momentum_relation(&self) -> RawId {
        self.momentum_relation
    }

    /// Exact Relation witnessing incompressibility.
    #[must_use]
    pub const fn incompressibility_relation(&self) -> RawId {
        self.incompressibility_relation
    }

    /// Complete package-neutral meaning of all four exact Cartesian sides.
    #[must_use]
    pub const fn boundary_inventory(&self) -> &CartesianBoundaryInventory<D> {
        &self.boundary_inventory
    }

    /// Canonically ordered exact Relations admitted by boundary normalization.
    #[must_use]
    pub(crate) fn boundary_relations(&self) -> &[BoundaryRelationBinding] {
        &self.boundary_relations
    }

    /// Evaluate `grad(force_potential)` from the exact canonical scalar tape.
    ///
    /// # Errors
    /// Preserves the tape's shape and finite-evaluation diagnostics.
    pub fn conservative_body_force(&self, coordinates: &[f64]) -> Result<[f64; D], Diagnostic> {
        let zero_parameters = vec![0.0; self.force_potential_expression.parameter_fields().len()];
        let mut gradient = [0.0; D];
        for axis in 0..D {
            let mut direction = [0.0; D];
            direction[axis] = 1.0;
            gradient[axis] = self
                .force_potential_expression
                .evaluate_jvp(coordinates, &direction, &zero_parameters)?
                .1;
        }
        Ok(gradient)
    }
}

/// Two-dimensional compatibility name for canonical transient flow.
pub type TransientIncompressibleNavierStokesCartesianModel2d =
    TransientIncompressibleNavierStokesCartesianModel<2>;

/// Three-dimensional canonical transient incompressible flow meaning.
pub type TransientIncompressibleNavierStokesCartesianModel3d =
    TransientIncompressibleNavierStokesCartesianModel<3>;

/// Lower one exact whole-model fixed-domain transient incompressible fluid.
///
/// Recognition is identity-parametric and package-neutral. An explicit global
/// negation of the complete momentum residual is normalized; advective-form
/// shortcuts, relative/ALE velocity, and broader algebraic equivalence are
/// deliberately not inferred.
///
/// # Errors
/// Returns `EQ0703` unless the entire Model is exactly the admitted fluid and
/// its complete boundary network.
pub fn lower_transient_incompressible_navier_stokes_cartesian_2d(
    program: &KernelProgram,
) -> Result<TransientIncompressibleNavierStokesCartesianModel2d, Diagnostic> {
    lower_transient_incompressible_navier_stokes_cartesian::<2>(program)
}

/// Lower one exact whole-model 3D fixed-domain transient incompressible fluid.
///
/// # Errors
/// Returns `EQ0703` unless the complete model uses the same canonical relation
/// grammar as the 2D projection with exact three-component fields and six
/// Cartesian boundary sides.
pub fn lower_transient_incompressible_navier_stokes_cartesian_3d(
    program: &KernelProgram,
) -> Result<TransientIncompressibleNavierStokesCartesianModel3d, Diagnostic> {
    lower_transient_incompressible_navier_stokes_cartesian::<3>(program)
}

fn lower_transient_incompressible_navier_stokes_cartesian<const D: usize>(
    program: &KernelProgram,
) -> Result<TransientIncompressibleNavierStokesCartesianModel<D>, Diagnostic> {
    let (domain, bounds) = unique_box::<D>(program)?;
    let lowered =
        lower_transient_incompressible_navier_stokes_subdomain::<D>(program, domain, bounds)?;
    require_closed_model(
        program,
        &lowered.model,
        lowered.representation,
        &lowered.boundary,
    )?;
    Ok(lowered.model)
}

/// Domain-scoped transient-flow recognition for exact multiphysics compositions.
pub(crate) fn lower_transient_incompressible_navier_stokes_subdomain<const D: usize>(
    program: &KernelProgram,
    domain: RawId,
    bounds: [[f64; 2]; D],
) -> Result<LoweredTransientIncompressibleNavierStokesSubdomain<D>, Diagnostic> {
    let (velocity, scalar_fields, representation) =
        exact_fields_for_dimension::<D>(program, domain)?;
    if scalar_fields.len() != 2 {
        return Err(lowering_error(
            domain,
            format!(
                "transient incompressible Navier--Stokes requires exactly pressure and force-potential scalar Fields, found {} pressure-valued scalars",
                scalar_fields.len()
            ),
        ));
    }
    let volume_relations = relations_on(program, domain);
    if volume_relations.len() != 3 {
        return Err(lowering_error(
            domain,
            format!(
                "transient incompressible Navier--Stokes requires exactly force, momentum, and incompressibility Relations, found {}",
                volume_relations.len()
            ),
        ));
    }
    for relation in &volume_relations {
        require_continuous_relation(program, *relation)?;
    }
    let typed_relations = volume_relations
        .iter()
        .map(|relation| Ok((*relation, typed_relation(program, *relation)?)))
        .collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;

    let incompressibility = volume_relations
        .iter()
        .copied()
        .filter(|relation| {
            typed_relations
                .get(relation)
                .and_then(|typed| {
                    unique_root(typed.expression(), *relation)
                        .ok()
                        .map(|root| (typed.expression(), root))
                })
                .is_some_and(|(expression, root)| {
                    is_divergence_of_field(expression, root, velocity)
                })
        })
        .collect::<Vec<_>>();
    if incompressibility.len() != 1 {
        return Err(lowering_error(
            domain,
            format!(
                "transient incompressible Navier--Stokes requires exactly one `div(velocity) = 0` Relation, found {}",
                incompressibility.len()
            ),
        ));
    }
    let incompressibility_relation = incompressibility[0];

    let mut candidates = Vec::new();
    for &force_potential in &scalar_fields {
        for &pressure in &scalar_fields {
            if pressure == force_potential {
                continue;
            }
            let definitions = volume_relations
                .iter()
                .copied()
                .filter_map(|relation| {
                    let expression = typed_relations.get(&relation)?.expression();
                    let root = unique_root(expression, relation).ok()?;
                    load_definition_root(expression, root, force_potential)
                        .map(|source| (relation, source))
                })
                .collect::<Vec<_>>();
            let mut momenta = Vec::new();
            for relation in &volume_relations {
                let typed = &typed_relations[relation];
                let root = unique_root(typed.expression(), *relation)?;
                if let Some(parts) = navier_stokes_momentum_parts(
                    typed,
                    root,
                    velocity,
                    pressure,
                    force_potential,
                    *relation,
                )? {
                    momenta.push((*relation, parts));
                }
            }
            if definitions.len() == 1
                && momenta.len() == 1
                && definitions[0].0 != momenta[0].0
                && definitions[0].0 != incompressibility_relation
                && momenta[0].0 != incompressibility_relation
            {
                candidates.push((pressure, force_potential, definitions[0], momenta[0]));
            }
        }
    }
    if candidates.len() != 1 {
        return Err(lowering_error(
            domain,
            format!(
                "pressure, force-potential definition, and exact conservative transient momentum must have one unique identity assignment, found {}",
                candidates.len()
            ),
        ));
    }
    let (
        pressure,
        force_potential,
        (force_potential_definition, source),
        (momentum_relation, momentum),
    ) = candidates.remove(0);
    let force_potential_expression = spatial_expression::lower(
        program,
        relation_expression(program, force_potential_definition)?,
        source,
        force_potential_definition,
        D,
    )?;
    let momentum_expression = relation_expression(program, momentum_relation)?;
    let momentum_typed = &typed_relations[&momentum_relation];
    let mass_density = spatial_expression::lower(
        program,
        momentum_expression,
        momentum.inertial_density,
        momentum_relation,
        D,
    )?;
    let advective_density = spatial_expression::lower(
        program,
        momentum_expression,
        momentum.advective_density,
        momentum_relation,
        D,
    )?;
    if mass_density != advective_density {
        return Err(lowering_error(
            momentum_relation,
            "inertia and conservative convective flux must use one identical density expression",
        ));
    }
    require_positive_constant(&mass_density, momentum_relation, "mass density")?;
    let dynamic_viscosity = lower_newtonian_stress_viscosity(
        program,
        momentum_typed,
        momentum.stress,
        velocity,
        pressure,
        momentum_relation,
    )?
    .ok_or_else(|| {
        lowering_error(
            momentum_relation,
            "fluid stress must be exactly `2 * mu * symmetric_part(grad(velocity)) - isotropic_lift(pressure)`",
        )
    })?;
    require_positive_constant(&dynamic_viscosity, momentum_relation, "dynamic viscosity")?;

    let boundary =
        boundary::lower_dimension::<D>(program, domain, velocity, pressure, &dynamic_viscosity)?;
    let model = TransientIncompressibleNavierStokesCartesianModel {
        domain,
        velocity,
        pressure,
        force_potential,
        bounds,
        mass_density,
        dynamic_viscosity,
        force_potential_expression,
        force_potential_definition,
        momentum_relation,
        incompressibility_relation,
        boundary_inventory: boundary.inventory.clone(),
        boundary_relations: boundary.boundary_relations.clone(),
    };
    Ok(LoweredTransientIncompressibleNavierStokesSubdomain {
        model,
        representation,
        volume_relations: [
            force_potential_definition,
            momentum_relation,
            incompressibility_relation,
        ],
        boundary,
    })
}

#[derive(Debug)]
pub(crate) struct LoweredTransientIncompressibleNavierStokesSubdomain<const D: usize> {
    pub(crate) model: TransientIncompressibleNavierStokesCartesianModel<D>,
    pub(crate) representation: RawId,
    pub(crate) volume_relations: [RawId; 3],
    pub(crate) boundary: LoweredStokesBoundary<D>,
}

#[derive(Debug, Clone, Copy)]
struct NavierStokesMomentumParts {
    inertial_density: ExprId,
    advective_density: ExprId,
    stress: ExprId,
}

fn navier_stokes_momentum_parts(
    residual: &TypedResidual<RawId>,
    root: ExprId,
    velocity: RawId,
    pressure: RawId,
    force_potential: RawId,
    owner: RawId,
) -> Result<Option<NavierStokesMomentumParts>, Diagnostic> {
    if let Some(ExprNode::Neg(inner)) = residual.expression().node(root) {
        return navier_stokes_momentum_parts_oriented(
            residual,
            *inner,
            velocity,
            pressure,
            force_potential,
            owner,
        );
    }
    navier_stokes_momentum_parts_oriented(
        residual,
        root,
        velocity,
        pressure,
        force_potential,
        owner,
    )
}

fn navier_stokes_momentum_parts_oriented(
    residual: &TypedResidual<RawId>,
    root: ExprId,
    velocity: RawId,
    pressure: RawId,
    force_potential: RawId,
    owner: RawId,
) -> Result<Option<NavierStokesMomentumParts>, Diagnostic> {
    let expression = residual.expression();
    let Some(ExprNode::Sub(internal_balance, load)) = expression.node(root) else {
        return Ok(None);
    };
    let Some(ExprNode::Gradient(load_argument)) = expression.node(*load) else {
        return Ok(None);
    };
    if !is_exact_field(expression, *load_argument, force_potential) {
        return Ok(None);
    }
    let Some(ExprNode::Sub(inertial_and_convective, divergence)) =
        expression.node(*internal_balance)
    else {
        return Ok(None);
    };
    let Some(ExprNode::Divergence(stress)) = expression.node(*divergence) else {
        return Ok(None);
    };
    let Some(ExprNode::Add(inertia, advection)) = expression.node(*inertial_and_convective) else {
        return Ok(None);
    };
    let Some(inertial_density) = inertia_density(expression, *inertia, velocity) else {
        return Ok(None);
    };
    let Some(advective_density) =
        conservative_advection_density(residual, *advection, velocity, owner)?
    else {
        return Ok(None);
    };

    // Prove pressure identity before coefficient lowering so a candidate
    // assignment cannot be selected merely by the scalar Field count.
    let Some(ExprNode::Sub(_, isotropic_pressure)) = expression.node(*stress) else {
        return Ok(None);
    };
    let Some(pressure_proof) = OperatorApplicationProof::classify(
        residual,
        *isotropic_pressure,
        StandardPureOperator::IsotropicLift,
    )
    .map_err(|error| {
        lowering_error(
            owner,
            format!(
                "isotropic_lift calculus proof failed at expression node {}: {error}",
                isotropic_pressure.index()
            ),
        )
    })?
    else {
        return Ok(None);
    };
    if !is_exact_field(expression, pressure_proof.operand(), pressure) {
        return Ok(None);
    }
    Ok(Some(NavierStokesMomentumParts {
        inertial_density,
        advective_density,
        stress: *stress,
    }))
}

fn conservative_advection_density(
    residual: &TypedResidual<RawId>,
    value: ExprId,
    velocity: RawId,
    owner: RawId,
) -> Result<Option<ExprId>, Diagnostic> {
    let expression = residual.expression();
    let dyadic_product = PureOperatorDefinition::dyadic_product().map_err(|error| {
        lowering_error(
            owner,
            format!("canonical dyadic-product definition is invalid: {error}"),
        )
    })?;
    let Some(ExprNode::Divergence(flux)) = expression.node(value) else {
        return Ok(None);
    };
    let Some(ExprNode::Mul(left, right)) = expression.node(*flux) else {
        return Ok(None);
    };
    for (outer_product, density) in [(*left, *right), (*right, *left)] {
        let Some(proof) =
            PureOperatorApplicationProof::classify(residual, outer_product, &dyadic_product)
                .map_err(|error| {
                    lowering_error(
                        owner,
                        format!(
                            "outer_product calculus proof failed at expression node {}: {error}",
                            outer_product.index()
                        ),
                    )
                })?
        else {
            continue;
        };
        if proof.arguments().len() == 2
            && proof
                .arguments()
                .iter()
                .all(|operand| is_exact_field(expression, *operand, velocity))
        {
            return Ok(Some(density));
        }
    }
    Ok(None)
}

fn inertia_density(expression: &ExprDag, value: ExprId, velocity: RawId) -> Option<ExprId> {
    let ExprNode::Mul(left, right) = expression.node(value)? else {
        return None;
    };
    for (derivative, density) in [(*left, *right), (*right, *left)] {
        if matches!(expression.node(derivative), Some(ExprNode::Symbol(SymbolRef::Derivative(field))) if field.erase() == velocity)
        {
            return Some(density);
        }
    }
    None
}

fn is_exact_field(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    matches!(expression.node(value), Some(ExprNode::Symbol(SymbolRef::Field(candidate))) if candidate.erase() == field)
}

fn require_closed_model<const D: usize>(
    program: &KernelProgram,
    model: &TransientIncompressibleNavierStokesCartesianModel<D>,
    representation: RawId,
    boundary: &LoweredStokesBoundary<D>,
) -> Result<(), Diagnostic> {
    let mut domains = BTreeSet::from([model.domain]);
    domains.extend(
        model
            .boundary_inventory
            .entries()
            .map(|(_, entry)| entry.boundary()),
    );
    domains.extend(boundary.connector_domains.iter().copied());
    let mut relations = BTreeSet::from([
        model.force_potential_definition(),
        model.momentum_relation(),
        model.incompressibility_relation(),
    ]);
    relations.extend(
        model
            .boundary_relations()
            .iter()
            .map(|binding| binding.relation()),
    );
    let activations = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Activates && relations.contains(&edge.to()))
        .map(|edge| edge.from())
        .collect::<BTreeSet<_>>();
    let parameters = parameters_referenced_by(program, &relations);
    let fields = BTreeSet::from([model.velocity, model.pressure, model.force_potential]);
    for node in program.nodes() {
        let admitted = match node {
            KernelNode::Domain(value) => domains.contains(&value.id().erase()),
            KernelNode::Representation(value) => value.id().erase() == representation,
            KernelNode::Field(value) => fields.contains(&value.id().erase()),
            KernelNode::Parameter(value) => parameters.contains(&value.id().erase()),
            KernelNode::Relation(value) => relations.contains(&value.id().erase()),
            KernelNode::Activation(value) => activations.contains(&value.id().erase()),
            KernelNode::Port(value) => boundary.ports.contains(&value.id().erase()),
            KernelNode::Connection(value) => boundary.connections.contains(&value.id().erase()),
            _ => false,
        };
        if !admitted {
            return Err(model_lowering_error(
                program,
                format!(
                    "closed transient Navier--Stokes lowering would ignore unexpected {:?} node {}",
                    node.kind(),
                    node.id()
                ),
            ));
        }
    }
    Ok(())
}
