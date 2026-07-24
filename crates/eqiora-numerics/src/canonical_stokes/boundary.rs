//! Package-neutral normalization of exact steady-Stokes boundary meaning.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, RawId};
use eqiora_ir::{OperatorApplicationProof, StandardPureOperator};
use eqiora_schema::kernel::typing::TypedResidual;
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef};
use eqiora_sem::KernelProgram;

use crate::canonical_boundary::{
    BoundaryRelationBinding, CartesianBoundaryEntry, CartesianBoundaryInventory,
    PhysicalBoundaryDisposition, PhysicalBoundaryQuantity, PrescribedBoundaryLaw,
    exact_cartesian_boundaries, normalize_field_physical_interface,
};
use crate::spatial_expression::ScalarSpatialExpression;

use super::expression::lower_newtonian_stress_viscosity;
use super::support::{
    is_field, lowering_error, relation_expression, relations_on, require_continuous_relation,
    typed_relation,
};

#[derive(Debug)]
pub(crate) struct LoweredStokesBoundary<const D: usize> {
    pub(crate) inventory: CartesianBoundaryInventory<D>,
    pub(crate) normal_pressure_sources:
        BTreeMap<(usize, eqiora_schema::kernel::BoundarySide), NormalPressureSource2d>,
    pub(crate) relations: BTreeSet<RawId>,
    pub(crate) boundary_relations: Vec<BoundaryRelationBinding>,
    pub(crate) ports: BTreeSet<RawId>,
    pub(crate) connections: BTreeSet<RawId>,
    pub(crate) connector_domains: BTreeSet<RawId>,
    pub(crate) uninterpreted_live_relations: BTreeSet<RawId>,
}

pub(crate) type LoweredStokesBoundary2d = LoweredStokesBoundary<2>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NormalPressureSource2d {
    Zero,
    Field { field: RawId, law_relation: RawId },
}

#[derive(Debug, Clone, Copy)]
struct BoundaryCandidate {
    disposition: PhysicalBoundaryDisposition,
    normal_pressure: Option<NormalPressureSource2d>,
}

#[derive(Clone, Copy)]
struct NewtonianBoundaryContext<'a> {
    program: &'a KernelProgram,
    velocity: RawId,
    pressure: RawId,
    volume_viscosity: &'a ScalarSpatialExpression,
}

pub(super) fn lower(
    program: &KernelProgram,
    domain: RawId,
    velocity: RawId,
    pressure: RawId,
    volume_viscosity: &ScalarSpatialExpression,
) -> Result<LoweredStokesBoundary2d, Diagnostic> {
    lower_dimension::<2>(program, domain, velocity, pressure, volume_viscosity)
}

pub(super) fn lower_dimension<const D: usize>(
    program: &KernelProgram,
    domain: RawId,
    velocity: RawId,
    pressure: RawId,
    volume_viscosity: &ScalarSpatialExpression,
) -> Result<LoweredStokesBoundary<D>, Diagnostic> {
    let exact_boundaries = exact_cartesian_boundaries::<D>(program, domain)?;
    let mut entries = BTreeMap::new();
    let mut normal_pressure_sources = BTreeMap::new();
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
            if let Some(disposition) =
                direct_disposition(program, *relation, velocity, pressure, volume_viscosity)?
            {
                direct.push((*relation, disposition));
            }
        }
        let (candidate, side_relations) = if direct.len() == 1 && relations.len() == 1 {
            (direct[0].1, BTreeSet::from([direct[0].0]))
        } else {
            if !direct.is_empty() {
                return Err(lowering_error(
                    boundary,
                    "direct steady-Stokes boundary meaning is ambiguous with additional Relations",
                ));
            }
            let normalized = normalize_physical_interface(
                program,
                boundary,
                velocity,
                pressure,
                volume_viscosity,
                &relations,
            )?;
            let side_relations = normalized.relations.clone();
            admitted_ports.extend(normalized.ports);
            admitted_connections.insert(normalized.connection);
            connector_domains.extend(normalized.connector_domains);
            uninterpreted_live_relations.extend(normalized.uninterpreted_live_relations);
            let normal_pressure = normalized_pressure_source(
                program,
                normalized.disposition,
                normalized.interface_port,
            )?;
            (
                BoundaryCandidate {
                    disposition: normalized.disposition,
                    normal_pressure,
                },
                side_relations,
            )
        };
        admitted_relations.extend(side_relations.iter().copied());
        boundary_relations.extend(
            side_relations
                .into_iter()
                .map(|relation| BoundaryRelationBinding::new(boundary, relation)),
        );
        if let Some(normal_pressure) = candidate.normal_pressure {
            normal_pressure_sources.insert((axis, side), normal_pressure);
        }
        entries.insert(
            (axis, side),
            CartesianBoundaryEntry::new(boundary, candidate.disposition),
        );
    }

    Ok(LoweredStokesBoundary {
        inventory: CartesianBoundaryInventory::new(entries),
        normal_pressure_sources,
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
    velocity: RawId,
    pressure: RawId,
    volume_viscosity: &ScalarSpatialExpression,
) -> Result<Option<BoundaryCandidate>, Diagnostic> {
    let expression = relation_expression(program, relation)?;
    let [root] = expression.roots() else {
        return Ok(None);
    };
    match expression.node(*root) {
        Some(ExprNode::Trace(value)) if is_field(expression, *value, velocity) => {
            Ok(Some(BoundaryCandidate {
                disposition: PhysicalBoundaryDisposition::TraceZero,
                normal_pressure: None,
            }))
        }
        Some(ExprNode::NormalComponent(stress)) => {
            require_matching_stress(
                program,
                expression,
                *stress,
                velocity,
                pressure,
                relation,
                volume_viscosity,
            )?;
            Ok(Some(BoundaryCandidate {
                disposition: PhysicalBoundaryDisposition::FluxZero,
                normal_pressure: Some(NormalPressureSource2d::Zero),
            }))
        }
        Some(ExprNode::Add(left, right)) => {
            let Some(field) = direct_normal_pressure_field(
                expression,
                *left,
                *right,
                relation,
                NewtonianBoundaryContext {
                    program,
                    velocity,
                    pressure,
                    volume_viscosity,
                },
            )?
            else {
                return Ok(None);
            };
            Ok(Some(BoundaryCandidate {
                disposition: PhysicalBoundaryDisposition::Prescribed(PrescribedBoundaryLaw::new(
                    PhysicalBoundaryQuantity::Flux,
                    relation,
                )),
                normal_pressure: Some(NormalPressureSource2d::Field {
                    field,
                    law_relation: relation,
                }),
            }))
        }
        _ => Ok(None),
    }
}

fn direct_normal_pressure_field(
    expression: &ExprDag,
    first: ExprId,
    second: ExprId,
    relation: RawId,
    context: NewtonianBoundaryContext<'_>,
) -> Result<Option<RawId>, Diagnostic> {
    let typed = typed_relation(context.program, relation)?;
    debug_assert_eq!(typed.expression(), expression);
    for (traction, pressure_value) in [(first, second), (second, first)] {
        let Some(field) = normal_pressure_field(&typed, pressure_value, relation)? else {
            continue;
        };
        let Some(ExprNode::NormalComponent(stress)) = expression.node(traction) else {
            continue;
        };
        require_matching_stress(
            context.program,
            expression,
            *stress,
            context.velocity,
            context.pressure,
            relation,
            context.volume_viscosity,
        )?;
        return Ok(Some(field));
    }
    Ok(None)
}

fn normalized_pressure_source(
    program: &KernelProgram,
    disposition: PhysicalBoundaryDisposition,
    interface_port: RawId,
) -> Result<Option<NormalPressureSource2d>, Diagnostic> {
    match disposition {
        PhysicalBoundaryDisposition::TraceZero => Ok(None),
        PhysicalBoundaryDisposition::FluxZero => Ok(Some(NormalPressureSource2d::Zero)),
        PhysicalBoundaryDisposition::Prescribed(law) => {
            if law.quantity() != PhysicalBoundaryQuantity::Flux {
                return Err(lowering_error(
                    law.relation(),
                    "steady Stokes does not yet admit a nonzero prescribed velocity trace",
                ));
            }
            let expression = relation_expression(program, law.relation())?;
            let typed = typed_relation(program, law.relation())?;
            debug_assert_eq!(typed.expression(), expression);
            let [root] = expression.roots() else {
                return Err(lowering_error(
                    law.relation(),
                    "normal-pressure terminal requires exactly one residual root",
                ));
            };
            let Some(ExprNode::Sub(first, second)) = expression.node(*root) else {
                return Err(lowering_error(
                    law.relation(),
                    "normal-pressure terminal must prescribe positive terminal flux as `flux(port) - normal(isotropic_lift(pressure)) = 0`",
                ));
            };
            let (terminal_port, pressure_value) = if let Some(port) =
                port_flux(expression, *first).filter(|port| *port != interface_port)
            {
                (port, *second)
            } else if let Some(port) =
                port_flux(expression, *second).filter(|port| *port != interface_port)
            {
                (port, *first)
            } else {
                return Err(lowering_error(
                    law.relation(),
                    "normal-pressure terminal must prescribe its peer Port flux",
                ));
            };
            let Some(field) = normal_pressure_field(&typed, pressure_value, law.relation())? else {
                return Err(lowering_error(
                    law.relation(),
                    "normal-pressure terminal requires the parent-outward normal of one isotropic pressure Field",
                ));
            };
            debug_assert_ne!(terminal_port, interface_port);
            Ok(Some(NormalPressureSource2d::Field {
                field,
                law_relation: law.relation(),
            }))
        }
        PhysicalBoundaryDisposition::PortBinding { .. } => Ok(None),
    }
}

fn normal_pressure_field(
    residual: &TypedResidual<RawId>,
    value: ExprId,
    owner: RawId,
) -> Result<Option<RawId>, Diagnostic> {
    let expression = residual.expression();
    let Some(ExprNode::NormalComponent(tensor)) = expression.node(value) else {
        return Ok(None);
    };
    let Some(proof) =
        OperatorApplicationProof::classify(residual, *tensor, StandardPureOperator::IsotropicLift)
            .map_err(|error| {
                lowering_error(
                    owner,
                    format!(
                        "isotropic_lift calculus proof failed at expression node {}: {error}",
                        tensor.index()
                    ),
                )
            })?
    else {
        return Ok(None);
    };
    Ok(match expression.node(proof.operand()) {
        Some(ExprNode::Symbol(SymbolRef::Field(field))) => Some(field.erase()),
        _ => None,
    })
}

fn normalize_physical_interface(
    program: &KernelProgram,
    boundary: RawId,
    velocity: RawId,
    pressure: RawId,
    volume_viscosity: &ScalarSpatialExpression,
    boundary_relations: &[RawId],
) -> Result<crate::canonical_boundary::NormalizedFieldPhysicalInterface, Diagnostic> {
    let mut interfaces = Vec::new();
    for relation in boundary_relations {
        if let Some(port) =
            interface_port(program, *relation, velocity, pressure, volume_viscosity)?
        {
            interfaces.push((*relation, port));
        }
    }
    if interfaces.len() != 1 {
        return Err(lowering_error(
            boundary,
            format!(
                "steady-Stokes boundary requires one direct law or one exact field-physical interface, found {} interface candidates",
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
    velocity: RawId,
    pressure: RawId,
    volume_viscosity: &ScalarSpatialExpression,
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
        if matches!(expression.node(*left), Some(ExprNode::Trace(value)) if is_field(expression, *value, velocity))
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
                velocity,
                pressure,
                relation,
                volume_viscosity,
            )?;
            flux_port = Some(port);
        }
    }
    match (trace_port, flux_port) {
        (Some(trace), Some(flux)) if trace == flux => Ok(Some(trace)),
        (None, None) => Ok(None),
        _ => Err(lowering_error(
            relation,
            "steady-Stokes interface must pair exact velocity trace and Newtonian outward traction with one Port",
        )),
    }
}

fn require_matching_stress(
    program: &KernelProgram,
    expression: &ExprDag,
    stress: ExprId,
    velocity: RawId,
    pressure: RawId,
    relation: RawId,
    volume_viscosity: &ScalarSpatialExpression,
) -> Result<(), Diagnostic> {
    let typed = typed_relation(program, relation)?;
    debug_assert_eq!(typed.expression(), expression);
    let viscosity =
        lower_newtonian_stress_viscosity(program, &typed, stress, velocity, pressure, relation)?
            .ok_or_else(|| {
                lowering_error(
                    relation,
                    "boundary traction must use the exact incompressible Newtonian stress",
                )
            })?;
    if !viscosity.is_same_coefficient_as(volume_viscosity) {
        return Err(lowering_error(
            relation,
            "boundary and volume dynamic viscosity coefficients differ",
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
