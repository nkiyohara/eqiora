use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, DimExponents, RawId, ValueShape};
use eqiora_geometry::CanonicalCircularHoleGeometryV1;
use eqiora_graph::EdgeKind;
use eqiora_schema::kernel::{DomainKind, ExprNode, KernelNode, SymbolRef, ValueFrame};
use eqiora_sem::KernelProgram;

use crate::canonical_boundary::CartesianBoundaryEntry;
use crate::spatial_expression;

use super::api::{
    SteadyIncompressibleStokesCartesianModel2d, SteadyIncompressibleStokesModel2d,
    SteadyStokesBoundaryEntry2d, SteadyStokesNormalPressure2d, StokesBoundaryKey2d,
};
use super::boundary::{self, NormalPressureSource2d};
use super::expression::{
    is_divergence_of_field, load_definition_root, lower_exact_twice_viscosity,
    momentum_viscous_root,
};
use super::support::{
    continuum_representation, has_edge, lowering_error, model_lowering_error, relation_expression,
    relations_on, require_continuous_relation, typed_relation, unique_root,
};

const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};

enum BoundarySource2d {
    Cartesian,
    Named(BTreeMap<String, RawId>),
}

struct LoweredBoundaryProjection2d {
    entries: BTreeMap<StokesBoundaryKey2d, SteadyStokesBoundaryEntry2d>,
    cartesian_entries:
        Option<BTreeMap<(usize, eqiora_schema::kernel::BoundarySide), CartesianBoundaryEntry>>,
    normal_pressure_sources: BTreeMap<StokesBoundaryKey2d, NormalPressureSource2d>,
    normal_velocity_expressions:
        BTreeMap<StokesBoundaryKey2d, crate::spatial_expression::ScalarSpatialExpression>,
    normal_velocity_coefficients: BTreeMap<StokesBoundaryKey2d, (RawId, RawId)>,
    normal_velocity_fields: BTreeSet<RawId>,
    normal_velocity_definitions: BTreeSet<RawId>,
    boundary_relations: Vec<crate::canonical_boundary::BoundaryRelationBinding2d>,
    ports: BTreeSet<RawId>,
    connections: BTreeSet<RawId>,
    connector_domains: BTreeSet<RawId>,
    uninterpreted_live_relations: BTreeSet<RawId>,
}

struct LoweredSteadyStokes2d {
    model: SteadyIncompressibleStokesModel2d,
    cartesian_entries:
        Option<BTreeMap<(usize, eqiora_schema::kernel::BoundarySide), CartesianBoundaryEntry>>,
}

/// Lower the exact canonical 2D steady incompressible Stokes subset.
///
/// Recognition is identity-parametric and whole-model fail-closed. Source
/// names, package provenance, and declaration order are not part of the
/// contract.
///
/// # Errors
/// Returns `EQ0703` when the admitted Model is ambiguous, incomplete, or
/// contains meaning outside this deliberately narrow subset.
pub fn lower_steady_incompressible_stokes_cartesian_2d(
    program: &KernelProgram,
) -> Result<SteadyIncompressibleStokesCartesianModel2d, Diagnostic> {
    let (domain, bounds) = unique_box_2d(program)?;
    let lowered = lower_steady_incompressible_stokes_2d_on(
        program,
        domain,
        bounds,
        BoundarySource2d::Cartesian,
        None,
    )?;
    Ok(SteadyIncompressibleStokesCartesianModel2d::from_common(
        lowered.model,
        lowered
            .cartesian_entries
            .expect("Cartesian lowering retains its exact side inventory"),
    ))
}

/// Lower the exact geometry-backed 2D steady incompressible Stokes subset.
///
/// The supplied exact circular-hole geometry authenticates the Model's
/// `GeometryRegion` digest and supplies only exact bounds and fixed-side
/// parent-outward normals. Mesh entity membership is deliberately absent from
/// this method-neutral result and is resolved later through correspondence.
///
/// # Errors
/// Returns `EQ0703` for semantic ambiguity or unsupported boundary meaning,
/// and rejects a source digest or named-set inventory that differs from the
/// admitted Model before numerical realization.
pub(super) fn lower_steady_incompressible_stokes_geometry_2d(
    program: &KernelProgram,
    geometry: &CanonicalCircularHoleGeometryV1,
) -> Result<SteadyIncompressibleStokesModel2d, Diagnostic> {
    let (domain, boundaries) = unique_circular_hole_domain(program, geometry)?;
    lower_steady_incompressible_stokes_2d_on(
        program,
        domain,
        *geometry.bounds(),
        BoundarySource2d::Named(boundaries),
        Some(geometry.digest_bytes()),
    )
    .map(|lowered| lowered.model)
}

fn lower_steady_incompressible_stokes_2d_on(
    program: &KernelProgram,
    domain: RawId,
    bounds: [[f64; 2]; 2],
    boundary_source: BoundarySource2d,
    geometry_source_digest: Option<[u8; 32]>,
) -> Result<LoweredSteadyStokes2d, Diagnostic> {
    let (velocity, scalar_fields, normal_velocity_fields, representation) =
        exact_steady_fields(program, domain)?;
    let volume_relations = relations_on(program, domain);
    if volume_relations.len() < 3 {
        return Err(lowering_error(
            domain,
            format!(
                "2D steady Stokes requires at least three volume Relations, found {}",
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
                "2D steady Stokes requires exactly one `div(u) = 0` Relation, found {}",
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
                if let Some(viscous) = momentum_viscous_root(
                    typed,
                    root,
                    velocity,
                    pressure,
                    force_potential,
                    *relation,
                )? {
                    momenta.push((*relation, viscous));
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
                "pressure, force-potential definition, and exact Stokes momentum balance must have one unique identity assignment, found {}",
                candidates.len()
            ),
        ));
    }
    let (
        pressure,
        force_potential,
        (force_potential_definition, source),
        (momentum_relation, viscous),
    ) = candidates.remove(0);
    let force_potential_expression = spatial_expression::lower(
        program,
        relation_expression(program, force_potential_definition)?,
        source,
        force_potential_definition,
        2,
    )?;
    let dynamic_viscosity = lower_exact_twice_viscosity(
        program,
        &typed_relations[&momentum_relation],
        viscous,
        velocity,
        momentum_relation,
    )?
    .ok_or_else(|| {
        lowering_error(
            momentum_relation,
            "Stokes viscous stress must be exactly `2 * mu * symmetric_part(grad(u))` with one typed scalar coefficient",
        )
    })?;
    let Some(viscosity) = dynamic_viscosity.constant_value() else {
        return Err(lowering_error(
            momentum_relation,
            "dynamic viscosity must be a spatially constant scalar expression",
        ));
    };
    if !viscosity.is_finite() || viscosity <= 0.0 {
        return Err(lowering_error(
            momentum_relation,
            "steady Stokes requires finite positive dynamic viscosity",
        ));
    }

    let boundary = lower_boundary_projection(
        program,
        domain,
        velocity,
        pressure,
        &dynamic_viscosity,
        boundary_source,
    )?;
    if boundary.normal_velocity_fields != normal_velocity_fields {
        return Err(lowering_error(
            domain,
            "every velocity-valued scalar Field must define one prescribed normal-velocity trace",
        ));
    }
    let volume_roles = StokesVolumeRoles {
        domain,
        representation,
        pressure,
        force_potential,
        scalar_fields: &scalar_fields,
        relations: &volume_relations,
        force_potential_definition,
        momentum_relation,
        incompressibility_relation,
    };
    let normal_pressures = resolve_normal_pressures(
        program,
        &volume_roles,
        &boundary.normal_pressure_sources,
        &boundary.normal_velocity_definitions,
    )?;
    let model = SteadyIncompressibleStokesModel2d {
        domain,
        velocity,
        pressure,
        force_potential,
        bounds,
        dynamic_viscosity,
        force_potential_expression,
        force_potential_definition,
        momentum_relation,
        incompressibility_relation,
        boundary_entries: boundary.entries.clone(),
        boundary_relations: boundary.boundary_relations.clone(),
        normal_pressures: normal_pressures.by_key,
        normal_velocity_expressions: boundary.normal_velocity_expressions.clone(),
        normal_velocity_coefficients: boundary.normal_velocity_coefficients.clone(),
        geometry_source_digest,
    };
    require_closed_model(
        program,
        &model,
        representation,
        &boundary,
        &normal_pressures.fields,
        &normal_pressures.definitions,
    )?;
    Ok(LoweredSteadyStokes2d {
        model,
        cartesian_entries: boundary.cartesian_entries,
    })
}

fn lower_boundary_projection(
    program: &KernelProgram,
    domain: RawId,
    velocity: RawId,
    pressure: RawId,
    dynamic_viscosity: &crate::spatial_expression::ScalarSpatialExpression,
    source: BoundarySource2d,
) -> Result<LoweredBoundaryProjection2d, Diagnostic> {
    match source {
        BoundarySource2d::Cartesian => {
            let lowered = boundary::lower(program, domain, velocity, pressure, dynamic_viscosity)?;
            let cartesian_entries = lowered
                .inventory
                .entries()
                .map(|(key, entry)| (*key, *entry))
                .collect::<BTreeMap<_, _>>();
            Ok(LoweredBoundaryProjection2d {
                entries: cartesian_entries
                    .iter()
                    .map(|(&(axis, side), entry)| {
                        (
                            StokesBoundaryKey2d::CartesianSide { axis, side },
                            SteadyStokesBoundaryEntry2d {
                                boundary: entry.boundary(),
                                disposition: entry.disposition(),
                            },
                        )
                    })
                    .collect(),
                cartesian_entries: Some(cartesian_entries),
                normal_pressure_sources: lowered
                    .normal_pressure_sources
                    .into_iter()
                    .map(|((axis, side), source)| {
                        (StokesBoundaryKey2d::CartesianSide { axis, side }, source)
                    })
                    .collect(),
                normal_velocity_expressions: lowered
                    .normal_velocity_expressions
                    .into_iter()
                    .map(|((axis, side), expression)| {
                        (
                            StokesBoundaryKey2d::CartesianSide { axis, side },
                            expression,
                        )
                    })
                    .collect(),
                normal_velocity_coefficients: lowered
                    .normal_velocity_coefficients
                    .into_iter()
                    .map(|((axis, side), coefficient)| {
                        (
                            StokesBoundaryKey2d::CartesianSide { axis, side },
                            coefficient,
                        )
                    })
                    .collect(),
                normal_velocity_fields: lowered.normal_velocity_fields,
                normal_velocity_definitions: lowered.normal_velocity_definitions,
                boundary_relations: lowered.boundary_relations,
                ports: lowered.ports,
                connections: lowered.connections,
                connector_domains: lowered.connector_domains,
                uninterpreted_live_relations: lowered.uninterpreted_live_relations,
            })
        }
        BoundarySource2d::Named(boundaries) => {
            let lowered = boundary::lower_named(
                program,
                domain,
                velocity,
                pressure,
                dynamic_viscosity,
                boundaries,
            )?;
            Ok(LoweredBoundaryProjection2d {
                entries: lowered
                    .entries
                    .into_iter()
                    .map(|(name, entry)| {
                        (
                            StokesBoundaryKey2d::NamedEntitySet(name),
                            SteadyStokesBoundaryEntry2d {
                                boundary: entry.boundary(),
                                disposition: entry.disposition(),
                            },
                        )
                    })
                    .collect(),
                cartesian_entries: None,
                normal_pressure_sources: lowered
                    .normal_pressure_sources
                    .into_iter()
                    .map(|(name, source)| (StokesBoundaryKey2d::NamedEntitySet(name), source))
                    .collect(),
                normal_velocity_expressions: lowered
                    .normal_velocity_expressions
                    .into_iter()
                    .map(|(name, expression)| {
                        (StokesBoundaryKey2d::NamedEntitySet(name), expression)
                    })
                    .collect(),
                normal_velocity_coefficients: lowered
                    .normal_velocity_coefficients
                    .into_iter()
                    .map(|(name, coefficient)| {
                        (StokesBoundaryKey2d::NamedEntitySet(name), coefficient)
                    })
                    .collect(),
                normal_velocity_fields: lowered.normal_velocity_fields,
                normal_velocity_definitions: lowered.normal_velocity_definitions,
                boundary_relations: lowered.boundary_relations,
                ports: lowered.ports,
                connections: lowered.connections,
                connector_domains: lowered.connector_domains,
                uninterpreted_live_relations: lowered.uninterpreted_live_relations,
            })
        }
    }
}

fn unique_circular_hole_domain(
    program: &KernelProgram,
    geometry: &CanonicalCircularHoleGeometryV1,
) -> Result<(RawId, BTreeMap<String, RawId>), Diagnostic> {
    let regions = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::GeometryRegion { .. }) =>
            {
                Some(domain)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if regions.len() != 1 {
        return Err(model_lowering_error(
            program,
            format!(
                "geometry-backed 2D Stokes requires exactly one GeometryRegion, found {}",
                regions.len()
            ),
        ));
    }
    let region = regions[0];
    let DomainKind::GeometryRegion {
        geometry: model_geometry,
        entity_set,
    } = region.kind()
    else {
        unreachable!("GeometryRegion filter is exact");
    };
    if model_geometry.bytes() != geometry.digest_bytes() {
        return Err(lowering_error(
            region.id().erase(),
            "exact circular-hole source revision differs from the Model GeometryRegion digest",
        ));
    }
    if entity_set != "fluid" {
        return Err(lowering_error(
            region.id().erase(),
            "geometry-backed steady Stokes requires the exact `fluid` region entity set",
        ));
    }
    let domain = region.id().erase();
    let boundaries = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(boundary)
                if has_edge(program, boundary.id().erase(), domain, EdgeKind::BoundaryOf) =>
            {
                match boundary.kind() {
                    DomainKind::GeometryBoundary { entity_set } => {
                        Some((entity_set.clone(), boundary.id().erase()))
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let required = BTreeSet::from([
        "cylinder".to_owned(),
        "inlet".to_owned(),
        "outlet".to_owned(),
        "walls".to_owned(),
    ]);
    if boundaries.keys().cloned().collect::<BTreeSet<_>>() != required {
        return Err(lowering_error(
            domain,
            "geometry-backed steady Stokes requires exactly `inlet`, `outlet`, `walls`, and `cylinder` boundary entity sets",
        ));
    }
    Ok((domain, boundaries))
}

pub(super) fn unique_box_2d(program: &KernelProgram) -> Result<(RawId, [[f64; 2]; 2]), Diagnostic> {
    unique_box::<2>(program)
}

pub(super) fn unique_box<const D: usize>(
    program: &KernelProgram,
) -> Result<(RawId, [[f64; 2]; D]), Diagnostic> {
    if !matches!(D, 2 | 3) {
        return Err(model_lowering_error(
            program,
            format!(
                "canonical Cartesian fluid lowering supports dimension two or three, received {D}"
            ),
        ));
    }
    let boxes = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::CartesianBox { .. }) =>
            {
                Some(domain)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if boxes.len() != 1 {
        return Err(model_lowering_error(
            program,
            format!(
                "canonical {D}D fluid lowering requires exactly one Cartesian box, found {}",
                boxes.len()
            ),
        ));
    }
    let domain = boxes[0];
    let bounds = program.resolved_cartesian_bounds(domain.id())?;
    if bounds.len() != D {
        return Err(lowering_error(
            domain.id().erase(),
            format!(
                "canonical Cartesian fluid lowering requires dimension {D}, received {}",
                bounds.len()
            ),
        ));
    }
    let bounds = bounds
        .iter()
        .map(|bound| [bound.lower().value(), bound.upper().value()])
        .collect::<Vec<_>>()
        .try_into()
        .expect("dimension equality establishes Cartesian bound count");
    Ok((domain.id().erase(), bounds))
}

fn exact_steady_fields(
    program: &KernelProgram,
    domain: RawId,
) -> Result<(RawId, Vec<RawId>, BTreeSet<RawId>, RawId), Diagnostic> {
    let fields = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Field(field)
                if has_edge(program, field.id().erase(), domain, EdgeKind::DefinedOn)
                    && continuum_representation(program, field.id().erase()).is_some() =>
            {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let velocity_shape = ValueShape::new([2]).expect("two components are representable");
    let velocity = fields
        .iter()
        .filter(|field| {
            field.shape() == &velocity_shape
                && field.frame() == ValueFrame::SpatialCartesian
                && field.dimension() == VELOCITY
        })
        .map(|field| field.id().erase())
        .collect::<Vec<_>>();
    let pressure_scalars = fields
        .iter()
        .filter(|field| {
            field.shape().is_scalar()
                && field.frame() == ValueFrame::Invariant
                && field.dimension() == PRESSURE
        })
        .map(|field| field.id().erase())
        .collect::<Vec<_>>();
    let normal_velocity_scalars = fields
        .iter()
        .filter(|field| {
            field.shape().is_scalar()
                && field.frame() == ValueFrame::Invariant
                && field.dimension() == VELOCITY
        })
        .map(|field| field.id().erase())
        .collect::<BTreeSet<_>>();
    if fields.len() != velocity.len() + pressure_scalars.len() + normal_velocity_scalars.len()
        || velocity.len() != 1
        || pressure_scalars.len() < 2
    {
        return Err(lowering_error(
            domain,
            "steady Stokes Fields must be exactly one spatial velocity vector, at least two pressure-valued invariant scalars, and zero or more invariant normal-velocity scalars",
        ));
    }
    let representation = continuum_representation(program, velocity[0])
        .expect("field filter establishes one continuum Representation");
    if pressure_scalars
        .iter()
        .chain(normal_velocity_scalars.iter())
        .any(|field| continuum_representation(program, *field) != Some(representation))
    {
        return Err(lowering_error(
            domain,
            "steady Stokes solution and boundary-coefficient Fields must share one continuum Representation",
        ));
    }
    Ok((
        velocity[0],
        pressure_scalars,
        normal_velocity_scalars,
        representation,
    ))
}

pub(super) fn exact_fields(
    program: &KernelProgram,
    domain: RawId,
) -> Result<(RawId, Vec<RawId>, RawId), Diagnostic> {
    exact_fields_for_dimension::<2>(program, domain)
}

pub(super) fn exact_fields_for_dimension<const D: usize>(
    program: &KernelProgram,
    domain: RawId,
) -> Result<(RawId, Vec<RawId>, RawId), Diagnostic> {
    if !matches!(D, 2 | 3) {
        return Err(lowering_error(
            domain,
            format!("canonical fluid Fields support dimension two or three, received {D}"),
        ));
    }
    let fields = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Field(field)
                if has_edge(program, field.id().erase(), domain, EdgeKind::DefinedOn)
                    && continuum_representation(program, field.id().erase()).is_some() =>
            {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let components = u32::try_from(D).expect("supported dimensions fit portable component count");
    let vector_shape =
        ValueShape::new([components]).expect("supported dimensions are representable");
    let velocity = fields
        .iter()
        .filter(|field| {
            field.shape() == &vector_shape
                && field.frame() == ValueFrame::SpatialCartesian
                && field.dimension() == VELOCITY
        })
        .map(|field| field.id().erase())
        .collect::<Vec<_>>();
    let scalars = fields
        .iter()
        .filter(|field| {
            field.shape().is_scalar()
                && field.frame() == ValueFrame::Invariant
                && field.dimension() == PRESSURE
        })
        .map(|field| field.id().erase())
        .collect::<Vec<_>>();
    if fields.len() != velocity.len() + scalars.len() || velocity.len() != 1 || scalars.len() < 2 {
        return Err(lowering_error(
            domain,
            format!(
                "Fields must be exactly one velocity-valued spatial Cartesian `[{D}]` Field and at least two pressure-valued invariant scalar Fields"
            ),
        ));
    }
    let representation = continuum_representation(program, velocity[0])
        .expect("field filter establishes one continuum Representation");
    if scalars
        .iter()
        .any(|field| continuum_representation(program, *field) != Some(representation))
    {
        return Err(lowering_error(
            domain,
            "velocity, pressure, and force potential must share one continuum Representation",
        ));
    }
    Ok((velocity[0], scalars, representation))
}

struct ResolvedNormalPressures2d {
    by_key: BTreeMap<StokesBoundaryKey2d, SteadyStokesNormalPressure2d>,
    fields: BTreeSet<RawId>,
    definitions: BTreeSet<RawId>,
}

struct StokesVolumeRoles<'a> {
    domain: RawId,
    representation: RawId,
    pressure: RawId,
    force_potential: RawId,
    scalar_fields: &'a [RawId],
    relations: &'a [RawId],
    force_potential_definition: RawId,
    momentum_relation: RawId,
    incompressibility_relation: RawId,
}

fn resolve_normal_pressures(
    program: &KernelProgram,
    volume: &StokesVolumeRoles<'_>,
    sources: &BTreeMap<StokesBoundaryKey2d, NormalPressureSource2d>,
    normal_velocity_definitions: &BTreeSet<RawId>,
) -> Result<ResolvedNormalPressures2d, Diagnostic> {
    let scalar_fields = volume
        .scalar_fields
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut coefficient_fields = BTreeSet::new();
    let mut definitions = BTreeSet::new();
    let mut lowered_fields: BTreeMap<
        RawId,
        (RawId, crate::spatial_expression::ScalarSpatialExpression),
    > = BTreeMap::new();
    let mut by_key = BTreeMap::new();

    for (key, source) in sources {
        let pressure_law = match *source {
            NormalPressureSource2d::Zero => SteadyStokesNormalPressure2d::zero(
                crate::spatial_expression::ScalarSpatialExpression::constant(2, 0.0),
            ),
            NormalPressureSource2d::Field {
                field,
                law_relation,
            } => {
                if field == volume.pressure || field == volume.force_potential {
                    return Err(lowering_error(
                        law_relation,
                        "normal-pressure coefficient Field must be distinct from solution pressure and force potential",
                    ));
                }
                if !scalar_fields.contains(&field)
                    || continuum_representation(program, field) != Some(volume.representation)
                {
                    return Err(lowering_error(
                        field,
                        "normal-pressure coefficient must be an invariant pressure-valued continuum Field on the exact Stokes volume",
                    ));
                }
                coefficient_fields.insert(field);
                let (definition, expression) = if let Some(lowered) = lowered_fields.get(&field) {
                    lowered.clone()
                } else {
                    let candidates = volume
                        .relations
                        .iter()
                        .copied()
                        .filter_map(|relation| {
                            let expression = relation_expression(program, relation).ok()?;
                            let root = unique_root(expression, relation).ok()?;
                            load_definition_root(expression, root, field)
                                .map(|source| (relation, source))
                        })
                        .collect::<Vec<_>>();
                    if candidates.len() != 1 {
                        return Err(lowering_error(
                            field,
                            format!(
                                "normal-pressure coefficient Field requires exactly one scalar definition Relation, found {}",
                                candidates.len()
                            ),
                        ));
                    }
                    let (definition, root) = candidates[0];
                    let expression = spatial_expression::lower(
                        program,
                        relation_expression(program, definition)?,
                        root,
                        definition,
                        2,
                    )?;
                    lowered_fields.insert(field, (definition, expression.clone()));
                    (definition, expression)
                };
                definitions.insert(definition);
                SteadyStokesNormalPressure2d::field(field, definition, expression)
            }
        };
        by_key.insert(key.clone(), pressure_law);
    }

    let expected_coefficient_fields = scalar_fields
        .iter()
        .copied()
        .filter(|field| *field != volume.pressure && *field != volume.force_potential)
        .collect::<BTreeSet<_>>();
    if coefficient_fields != expected_coefficient_fields {
        return Err(lowering_error(
            volume.domain,
            "every additional pressure-valued Field must be used by an exact normal-pressure boundary law",
        ));
    }

    let mut expected_relations = BTreeSet::from([
        volume.force_potential_definition,
        volume.momentum_relation,
        volume.incompressibility_relation,
    ]);
    expected_relations.extend(definitions.iter().copied());
    expected_relations.extend(normal_velocity_definitions.iter().copied());
    if volume.relations.iter().copied().collect::<BTreeSet<_>>() != expected_relations {
        return Err(lowering_error(
            volume.domain,
            "steady Stokes volume contains a Relation outside the force, momentum, incompressibility, and normal-pressure coefficient definitions",
        ));
    }

    Ok(ResolvedNormalPressures2d {
        by_key,
        fields: coefficient_fields,
        definitions,
    })
}

fn require_closed_model(
    program: &KernelProgram,
    model: &SteadyIncompressibleStokesModel2d,
    representation: RawId,
    boundary: &LoweredBoundaryProjection2d,
    coefficient_fields: &BTreeSet<RawId>,
    coefficient_definitions: &BTreeSet<RawId>,
) -> Result<(), Diagnostic> {
    let mut domains = BTreeSet::from([model.domain()]);
    let mut relations = BTreeSet::from([
        model.force_potential_definition(),
        model.momentum_relation(),
        model.incompressibility_relation(),
    ]);
    domains.extend(
        model
            .boundary_relations()
            .iter()
            .map(|binding| binding.boundary()),
    );
    domains.extend(boundary.connector_domains.iter().copied());
    relations.extend(
        model
            .boundary_relations()
            .iter()
            .map(|binding| binding.relation()),
    );
    relations.extend(coefficient_definitions.iter().copied());
    relations.extend(boundary.normal_velocity_definitions.iter().copied());
    debug_assert!(boundary.uninterpreted_live_relations.is_subset(&relations));
    let activations = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Activates && relations.contains(&edge.to()))
        .map(|edge| edge.from())
        .collect::<BTreeSet<_>>();
    let parameters = relations
        .iter()
        .copied()
        .flat_map(|relation| {
            relation_expression(program, relation)
                .expect("admitted Relations were already inspected")
                .nodes()
                .iter()
        })
        .filter_map(|node| match node {
            ExprNode::Symbol(SymbolRef::Parameter(parameter)) => Some(parameter.erase()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut fields = BTreeSet::from([model.velocity(), model.pressure(), model.force_potential()]);
    fields.extend(coefficient_fields.iter().copied());
    fields.extend(boundary.normal_velocity_fields.iter().copied());
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
                    "closed 2D steady Stokes lowering would ignore unexpected {:?} node {}",
                    node.kind(),
                    node.id()
                ),
            ));
        }
    }
    Ok(())
}
