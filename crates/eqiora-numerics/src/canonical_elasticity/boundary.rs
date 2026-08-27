//! Package-neutral normalization of exact elasticity boundary meaning.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, RawId};
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef};
use eqiora_sem::KernelProgram;

use crate::canonical_boundary::{
    BoundaryRelationBinding, CartesianBoundaryEntry, CartesianBoundaryInventory,
    PhysicalBoundaryDisposition, exact_cartesian_boundaries, normalize_field_physical_interface,
};
use crate::spatial_expression::ScalarSpatialExpression;

use super::{
    lower_isotropic_stress_coefficients, lowering_error, relation_expression, relations_on,
    require_continuous_relation, typed_relation,
};

#[derive(Debug)]
pub(crate) struct LoweredElasticityBoundary<const D: usize> {
    pub(crate) inventory: CartesianBoundaryInventory<D>,
    pub(crate) relations: BTreeSet<RawId>,
    pub(crate) boundary_relations: Vec<BoundaryRelationBinding>,
    pub(crate) ports: BTreeSet<RawId>,
    pub(crate) connections: BTreeSet<RawId>,
    pub(crate) connector_domains: BTreeSet<RawId>,
    pub(crate) uninterpreted_live_relations: BTreeSet<RawId>,
}

pub(crate) type LoweredElasticityBoundary2d = LoweredElasticityBoundary<2>;

pub(super) fn lower(
    program: &KernelProgram,
    domain: RawId,
    trace_field: RawId,
    stress_displacement: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
) -> Result<LoweredElasticityBoundary2d, Diagnostic> {
    lower_with_boundaries(
        program,
        domain,
        trace_field,
        stress_displacement,
        volume_two_mu,
        volume_lambda,
        exact_cartesian_boundaries::<2>(program, domain)?,
    )
}

pub(super) fn lower_with_boundaries(
    program: &KernelProgram,
    domain: RawId,
    trace_field: RawId,
    stress_displacement: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
    exact_boundaries: BTreeMap<(usize, eqiora_schema::kernel::BoundarySide), RawId>,
) -> Result<LoweredElasticityBoundary2d, Diagnostic> {
    lower_dimension_with_boundaries::<2>(
        program,
        domain,
        trace_field,
        stress_displacement,
        volume_two_mu,
        volume_lambda,
        exact_boundaries,
    )
}

pub(super) fn lower_dimension<const D: usize>(
    program: &KernelProgram,
    domain: RawId,
    trace_field: RawId,
    stress_displacement: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
) -> Result<LoweredElasticityBoundary<D>, Diagnostic> {
    let exact_boundaries = exact_cartesian_boundaries::<D>(program, domain)?;
    lower_dimension_with_boundaries(
        program,
        domain,
        trace_field,
        stress_displacement,
        volume_two_mu,
        volume_lambda,
        exact_boundaries,
    )
}

pub(crate) fn lower_dimension_with_boundaries<const D: usize>(
    program: &KernelProgram,
    _domain: RawId,
    trace_field: RawId,
    stress_displacement: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
    exact_boundaries: BTreeMap<(usize, eqiora_schema::kernel::BoundarySide), RawId>,
) -> Result<LoweredElasticityBoundary<D>, Diagnostic> {
    let mut entries = BTreeMap::new();
    let mut admitted_relations = BTreeSet::new();
    let mut boundary_relations = BTreeSet::new();
    let mut admitted_ports = BTreeSet::new();
    let mut admitted_connections = BTreeSet::new();
    let mut connector_domains = BTreeSet::new();
    let mut uninterpreted_live_relations = BTreeSet::new();

    for ((axis, side), boundary) in exact_boundaries {
        let relations = relations_on(program, boundary);
        for relation in &relations {
            require_continuous_relation(program, *relation)?;
        }

        let mut direct = Vec::new();
        for relation in &relations {
            if let Some(disposition) = direct_disposition(
                program,
                *relation,
                trace_field,
                stress_displacement,
                volume_two_mu,
                volume_lambda,
            )? {
                direct.push((*relation, disposition));
            }
        }
        let (disposition, side_relations) = if direct.len() == 1 && relations.len() == 1 {
            (direct[0].1, BTreeSet::from([direct[0].0]))
        } else {
            if !direct.is_empty() {
                return Err(lowering_error(
                    boundary,
                    "direct elasticity boundary meaning is ambiguous with additional Relations",
                ));
            }
            let normalized = normalize_physical_interface(
                program,
                boundary,
                trace_field,
                stress_displacement,
                volume_two_mu,
                volume_lambda,
                &relations,
            )?;
            let side_relations = normalized.relations.clone();
            admitted_ports.extend(normalized.ports);
            admitted_connections.insert(normalized.connection);
            connector_domains.extend(normalized.connector_domains);
            uninterpreted_live_relations.extend(normalized.uninterpreted_live_relations);
            (normalized.disposition, side_relations)
        };
        admitted_relations.extend(side_relations.iter().copied());
        boundary_relations.extend(
            side_relations
                .into_iter()
                .map(|relation| BoundaryRelationBinding::new(boundary, relation)),
        );
        entries.insert(
            (axis, side),
            CartesianBoundaryEntry::new(boundary, disposition),
        );
    }

    Ok(LoweredElasticityBoundary {
        inventory: CartesianBoundaryInventory::new(entries),
        relations: admitted_relations,
        boundary_relations: boundary_relations.into_iter().collect(),
        ports: admitted_ports,
        connections: admitted_connections,
        connector_domains,
        uninterpreted_live_relations,
    })
}

fn direct_disposition(
    program: &KernelProgram,
    relation: RawId,
    trace_field: RawId,
    stress_displacement: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
) -> Result<Option<PhysicalBoundaryDisposition>, Diagnostic> {
    let expression = relation_expression(program, relation)?;
    let [root] = expression.roots() else {
        return Ok(None);
    };
    match expression.node(*root) {
        Some(ExprNode::Trace(value)) if is_field(expression, *value, trace_field) => {
            Ok(Some(PhysicalBoundaryDisposition::TraceZero))
        }
        Some(ExprNode::NormalComponent(stress)) => {
            require_matching_stress(
                program,
                expression,
                *stress,
                stress_displacement,
                relation,
                volume_two_mu,
                volume_lambda,
            )?;
            Ok(Some(PhysicalBoundaryDisposition::FluxZero))
        }
        _ => Ok(None),
    }
}

fn normalize_physical_interface(
    program: &KernelProgram,
    boundary: RawId,
    trace_field: RawId,
    stress_displacement: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
    boundary_relations: &[RawId],
) -> Result<crate::canonical_boundary::NormalizedFieldPhysicalInterface, Diagnostic> {
    let mut interfaces = Vec::new();
    for relation in boundary_relations {
        if let Some(port) = interface_port(
            program,
            *relation,
            trace_field,
            stress_displacement,
            volume_two_mu,
            volume_lambda,
        )? {
            interfaces.push((*relation, port));
        }
    }
    if interfaces.len() != 1 {
        return Err(lowering_error(
            boundary,
            format!(
                "elasticity boundary requires one direct law or one exact mechanical interface, found {} interface candidates",
                interfaces.len()
            ),
        ));
    }
    let (interface_relation, interface_port) = interfaces[0];
    normalize_field_physical_interface(
        program,
        boundary,
        boundary_relations,
        interface_relation,
        interface_port,
    )
}

fn interface_port(
    program: &KernelProgram,
    relation: RawId,
    trace_field: RawId,
    stress_displacement: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
) -> Result<Option<RawId>, Diagnostic> {
    let expression = relation_expression(program, relation)?;
    let [first, second] = expression.roots() else {
        return Ok(None);
    };
    let mut trace_port = None;
    let mut flux_port = None;
    for root in [*first, *second] {
        let Some(ExprNode::Sub(left, right)) = expression.node(root) else {
            continue;
        };
        if matches!(expression.node(*left), Some(ExprNode::Trace(value)) if is_field(expression, *value, trace_field))
        {
            trace_port = port_trace(expression, *right);
            continue;
        }
        if let Some(ExprNode::NormalComponent(stress)) = expression.node(*left)
            && let Some(port) = port_flux(expression, *right)
        {
            require_matching_stress(
                program,
                expression,
                *stress,
                stress_displacement,
                relation,
                volume_two_mu,
                volume_lambda,
            )?;
            flux_port = Some(port);
        }
    }
    match (trace_port, flux_port) {
        (Some(trace), Some(flux)) if trace == flux => Ok(Some(trace)),
        (None, None) => Ok(None),
        _ => Err(lowering_error(
            relation,
            "mechanical interface must pair the exact trace Field and matching displacement-derived outward traction with one Port",
        )),
    }
}

fn require_matching_stress(
    program: &KernelProgram,
    expression: &ExprDag,
    stress: ExprId,
    displacement: RawId,
    relation: RawId,
    volume_two_mu: &ScalarSpatialExpression,
    volume_lambda: &ScalarSpatialExpression,
) -> Result<(), Diagnostic> {
    let typed = typed_relation(program, relation)?;
    debug_assert_eq!(typed.expression(), expression);
    let (two_mu, lambda) =
        lower_isotropic_stress_coefficients(program, &typed, stress, displacement, relation)?;
    if !two_mu.is_same_coefficient_as(volume_two_mu)
        || !lambda.is_same_coefficient_as(volume_lambda)
    {
        return Err(lowering_error(
            relation,
            "boundary and volume isotropic stress coefficients differ",
        ));
    }
    Ok(())
}

fn port_trace(expression: &ExprDag, value: ExprId) -> Option<RawId> {
    match expression.node(value) {
        Some(ExprNode::Symbol(SymbolRef::PortTrace(port))) => Some(port.erase()),
        _ => None,
    }
}

fn port_flux(expression: &ExprDag, value: ExprId) -> Option<RawId> {
    match expression.node(value) {
        Some(ExprNode::Symbol(SymbolRef::PortFlux(port))) => Some(port.erase()),
        _ => None,
    }
}

fn is_field(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    matches!(
        expression.node(value),
        Some(ExprNode::Symbol(SymbolRef::Field(id))) if id.erase() == field
    )
}
