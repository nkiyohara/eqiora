//! Spatial Domain validation and support reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::{Diagnostic, DimExponents, DynQuantity, RawId};
use eqiora_graph::{Edge, EdgeKind};
use eqiora_schema::kernel::typing::SpatialSupport;
use eqiora_schema::kernel::{
    AxisBounds, CartesianCoordinateSource, DomainKind, KernelNode, RepresentationKind, ValueFrame,
};

use super::{edge_targets, kernel_error};

pub(super) fn resolve_cartesian_bounds(
    nodes: &BTreeMap<RawId, KernelNode>,
    values: &BTreeMap<RawId, DynQuantity>,
    edges: &[Edge],
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<RawId, Vec<AxisBounds>> {
    let mut resolved = BTreeMap::new();
    let mut parameter_owners = BTreeMap::new();
    for (&domain, node) in nodes {
        let KernelNode::Domain(definition) = node else {
            continue;
        };
        let dependencies = edge_targets(edges, domain, EdgeKind::DependsOn);
        let DomainKind::CartesianBox { coordinates } = definition.kind() else {
            if !dependencies.is_empty() {
                diagnostics.push(kernel_error(
                    domain,
                    format!(
                        "non-Cartesian Domain must not depend on Parameters, found targets {dependencies:?}"
                    ),
                ));
            }
            continue;
        };

        let mut references = BTreeSet::new();
        let mut bounds = Vec::with_capacity(coordinates.len());
        for (axis, coordinate) in coordinates.iter().copied().enumerate() {
            let lower = resolve_coordinate(
                domain,
                axis,
                "lower",
                coordinate.lower(),
                nodes,
                values,
                &mut references,
                diagnostics,
            );
            let upper = resolve_coordinate(
                domain,
                axis,
                "upper",
                coordinate.upper(),
                nodes,
                values,
                &mut references,
                diagnostics,
            );
            if let (Some(lower), Some(upper)) = (lower, upper) {
                match AxisBounds::new(lower, upper) {
                    Ok(axis_bounds) => bounds.push(axis_bounds),
                    Err(_) => diagnostics.push(kernel_error(
                        domain,
                        format!(
                            "Cartesian axis {axis} resolves to non-finite, equal, or reversed bounds"
                        ),
                    )),
                }
            }
        }

        if references != dependencies {
            diagnostics.push(kernel_error(
                domain,
                format!(
                    "Cartesian coordinate Parameter set {references:?} differs from DependsOn targets {dependencies:?}"
                ),
            ));
        }
        for parameter in references {
            if let Some(previous) = parameter_owners.insert(parameter, domain)
                && previous != domain
            {
                diagnostics.push(kernel_error(
                    domain,
                    format!(
                        "Cartesian coordinate Parameter {parameter} is already owned by Domain {previous}"
                    ),
                ));
            }
        }
        if bounds.len() == coordinates.len() {
            resolved.insert(domain, bounds);
        }
    }
    resolved
}

#[allow(clippy::too_many_arguments)]
fn resolve_coordinate(
    domain: RawId,
    axis: usize,
    role: &str,
    source: CartesianCoordinateSource,
    nodes: &BTreeMap<RawId, KernelNode>,
    values: &BTreeMap<RawId, DynQuantity>,
    references: &mut BTreeSet<RawId>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<DynQuantity> {
    match source {
        CartesianCoordinateSource::Fixed(value) => Some(value),
        CartesianCoordinateSource::Parameter(parameter) => {
            let parameter = parameter.erase();
            references.insert(parameter);
            let length = DimExponents {
                length: 1,
                ..DimExponents::DIMENSIONLESS
            };
            let Some(KernelNode::Parameter(definition)) = nodes.get(&parameter) else {
                diagnostics.push(kernel_error(
                    domain,
                    format!("Cartesian axis {axis} {role} references absent Parameter {parameter}"),
                ));
                return None;
            };
            if definition.value().dim() != length {
                diagnostics.push(kernel_error(
                    domain,
                    format!("Cartesian axis {axis} {role} Parameter {parameter} is not a length"),
                ));
                return None;
            }
            let Some(value) = values.get(&parameter).copied() else {
                diagnostics.push(kernel_error(
                    domain,
                    format!(
                        "Cartesian axis {axis} {role} Parameter {parameter} has no revision-local value"
                    ),
                ));
                return None;
            };
            if value.dim() != length || !value.value().is_finite() {
                diagnostics.push(kernel_error(
                    domain,
                    format!(
                        "Cartesian axis {axis} {role} Parameter {parameter} has no finite length value"
                    ),
                ));
                return None;
            }
            Some(value)
        }
    }
}

pub(super) fn validate_domains(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    cartesian_bounds: &BTreeMap<RawId, Vec<AxisBounds>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeSet<RawId> {
    let mut invalid = BTreeSet::new();
    for (&id, node) in nodes {
        let KernelNode::Domain(domain) = node else {
            continue;
        };
        let diagnostics_before = diagnostics.len();
        let parents = edge_targets(edges, id, EdgeKind::BoundaryOf);
        match domain.kind() {
            DomainKind::Abstract
            | DomainKind::CartesianBox { .. }
            | DomainKind::GeometryRegion { .. }
            | DomainKind::ScalarPhysical { .. }
            | DomainKind::BoundaryPhysical { .. } => {
                if !parents.is_empty() {
                    diagnostics.push(kernel_error(
                        id,
                        "only a boundary Domain may have a BoundaryOf edge",
                    ));
                }
            }
            DomainKind::CartesianBoundary { axis, .. } => {
                if parents.len() != 1 {
                    diagnostics.push(kernel_error(
                        id,
                        format!(
                            "Cartesian boundary Domain requires exactly one BoundaryOf parent, found {}",
                            parents.len()
                        ),
                    ));
                    continue;
                }
                let parent = *parents.first().expect("one boundary parent was checked");
                match nodes.get(&parent) {
                    Some(KernelNode::Domain(parent_definition))
                        if matches!(parent_definition.kind(), DomainKind::CartesianBox { .. }) =>
                    {
                        match cartesian_bounds.get(&parent) {
                            Some(bounds) if *axis < bounds.len() => {}
                            Some(bounds) => diagnostics.push(kernel_error(
                                id,
                                format!(
                                    "boundary axis {axis} is outside parent dimension {}",
                                    bounds.len()
                                ),
                            )),
                            None => diagnostics.push(kernel_error(
                                id,
                                "Cartesian boundary parent has no valid resolved bounds",
                            )),
                        }
                    }
                    Some(KernelNode::Domain(_)) => diagnostics.push(kernel_error(
                        id,
                        "Cartesian boundary parent must be a Cartesian box Domain",
                    )),
                    _ => diagnostics.push(kernel_error(
                        id,
                        "BoundaryOf target has no Domain definition",
                    )),
                }
            }
            DomainKind::GeometryBoundary { .. } => {
                if parents.len() != 1 {
                    diagnostics.push(kernel_error(
                        id,
                        format!(
                            "geometry boundary Domain requires exactly one BoundaryOf parent, found {}",
                            parents.len()
                        ),
                    ));
                    continue;
                }
                let parent = *parents.first().expect("one boundary parent was checked");
                match nodes.get(&parent) {
                    Some(KernelNode::Domain(parent))
                        if matches!(parent.kind(), DomainKind::GeometryRegion { .. }) => {}
                    Some(KernelNode::Domain(_)) => diagnostics.push(kernel_error(
                        id,
                        "geometry boundary parent must be a geometry region Domain",
                    )),
                    _ => diagnostics.push(kernel_error(
                        id,
                        "BoundaryOf target has no Domain definition",
                    )),
                }
            }
            _ => diagnostics.push(kernel_error(
                id,
                "Domain kind is newer than this semantic validator",
            )),
        }
        if diagnostics.len() != diagnostics_before {
            invalid.insert(id);
        }
    }
    invalid
}

pub(super) fn validate_geometry_support_uses(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    artifacts_admitted: bool,
    admitted_geometry_ports: &BTreeSet<RawId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&id, node) in nodes {
        let (support, subject) = match node {
            KernelNode::Field(_) => (
                edge_targets(edges, id, EdgeKind::DefinedOn)
                    .into_iter()
                    .find(|target| {
                        geometry_support_requires_admission(*target, nodes, artifacts_admitted)
                    }),
                "Field spatial support",
            ),
            KernelNode::Relation(_) => (
                edge_targets(edges, id, EdgeKind::AppliesOn)
                    .into_iter()
                    .find(|target| {
                        geometry_relation_support_requires_admission(
                            *target,
                            nodes,
                            artifacts_admitted,
                        )
                    }),
                "Relation spatial scope",
            ),
            KernelNode::Port(port) if !admitted_geometry_ports.contains(&id) => (
                port.boundary_physical_contract()
                    .map(|(_, boundary)| boundary.erase())
                    .filter(|boundary| is_geometry_domain(*boundary, nodes)),
                "boundary-physical Port support",
            ),
            _ => continue,
        };
        if support.is_some() {
            let message = if artifacts_admitted {
                if matches!(node, KernelNode::Port(_)) {
                    format!(
                        "{subject} on a geometry Domain requires a non-Cartesian boundary embedding contract"
                    )
                } else {
                    format!(
                        "{subject} on a geometry boundary Domain requires a non-Cartesian boundary embedding contract"
                    )
                }
            } else {
                format!("{subject} from a geometry Domain requires artifact admission")
            };
            diagnostics.push(kernel_error(id, message));
        }
    }
}

fn geometry_relation_support_requires_admission(
    domain: RawId,
    nodes: &BTreeMap<RawId, KernelNode>,
    artifacts_admitted: bool,
) -> bool {
    !artifacts_admitted && is_geometry_domain(domain, nodes)
}

fn geometry_support_requires_admission(
    domain: RawId,
    nodes: &BTreeMap<RawId, KernelNode>,
    artifacts_admitted: bool,
) -> bool {
    matches!(
        nodes.get(&domain),
        Some(KernelNode::Domain(domain))
            if if artifacts_admitted {
                matches!(domain.kind(), DomainKind::GeometryBoundary { .. })
            } else {
                matches!(
                    domain.kind(),
                    DomainKind::GeometryRegion { .. } | DomainKind::GeometryBoundary { .. }
                )
            }
    )
}

fn is_geometry_domain(domain: RawId, nodes: &BTreeMap<RawId, KernelNode>) -> bool {
    matches!(
        nodes.get(&domain),
        Some(KernelNode::Domain(domain))
            if matches!(
                domain.kind(),
                DomainKind::GeometryRegion { .. } | DomainKind::GeometryBoundary { .. }
            )
    )
}

pub(super) fn validate_fields(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    spatial_supports: &BTreeMap<RawId, SpatialSupport<RawId>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&id, node) in nodes {
        let KernelNode::Field(field) = node else {
            continue;
        };
        let targets = edge_targets(edges, id, EdgeKind::DefinedOn);
        if targets.is_empty() {
            continue;
        }
        let domains = targets
            .iter()
            .filter(|target| target.kind() == eqiora_core::EntityKind::Domain)
            .copied()
            .collect::<Vec<_>>();
        let representations = targets
            .iter()
            .filter(|target| target.kind() == eqiora_core::EntityKind::Representation)
            .copied()
            .collect::<Vec<_>>();
        let spatial = domains.iter().any(|domain| {
            matches!(
                nodes.get(domain),
                Some(KernelNode::Domain(domain))
                    if matches!(domain.kind(), DomainKind::CartesianBox { .. })
            ) || matches!(
                spatial_supports.get(domain),
                Some(SpatialSupport::Volume { .. })
            )
        }) || representations.iter().any(|representation| {
            matches!(
                nodes.get(representation),
                Some(KernelNode::Representation(representation))
                    if representation.kind() == RepresentationKind::Continuum
            )
        });
        if !spatial {
            continue;
        }
        if domains.len() != 1 || representations.len() != 1 || targets.len() != 2 {
            diagnostics.push(kernel_error(
                id,
                format!(
                    "spatial Field requires exactly one Domain and one Representation, found {} and {}",
                    domains.len(),
                    representations.len()
                ),
            ));
            continue;
        }
        let admitted_volume = spatial_supports.get(&domains[0]).and_then(|support| {
            if let SpatialSupport::Volume { dimensions, .. } = support {
                Some(*dimensions)
            } else {
                None
            }
        });
        if admitted_volume.is_none() {
            diagnostics.push(kernel_error(
                id,
                "v0 spatial Field Domain must be a Cartesian box",
            ));
        }
        if let Some(dimensions) = admitted_volume
            && field.frame() == ValueFrame::SpatialCartesian
            && field
                .shape()
                .extents()
                .iter()
                .any(|extent| usize::try_from(extent.get()).ok() != Some(dimensions))
        {
            let message = if matches!(
                nodes.get(&domains[0]),
                Some(KernelNode::Domain(domain))
                    if matches!(domain.kind(), DomainKind::CartesianBox { .. })
            ) {
                "Cartesian spatial Field extents must equal its Domain ambient dimension"
            } else {
                "SpatialCartesian Field extents must equal its admitted Domain ambient dimension"
            };
            diagnostics.push(kernel_error(id, message));
        }
        if !matches!(
            nodes.get(&representations[0]),
            Some(KernelNode::Representation(representation))
                if representation.kind() == RepresentationKind::Continuum
        ) {
            diagnostics.push(kernel_error(
                id,
                "v0 spatial Field Representation must be continuum",
            ));
        }
    }
}

pub(super) fn field_support(
    field: RawId,
    edges: &[Edge],
    spatial_supports: &BTreeMap<RawId, SpatialSupport<RawId>>,
) -> Option<SpatialSupport<RawId>> {
    let supports = edge_targets(edges, field, EdgeKind::DefinedOn)
        .into_iter()
        .filter_map(|target| spatial_supports.get(&target).cloned())
        .filter(|support| matches!(support, SpatialSupport::Volume { .. }))
        .collect::<Vec<_>>();
    (supports.len() == 1).then(|| supports[0].clone())
}

pub(super) fn cartesian_spatial_supports(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    cartesian_bounds: &BTreeMap<RawId, Vec<AxisBounds>>,
) -> BTreeMap<RawId, SpatialSupport<RawId>> {
    let mut supports = BTreeMap::new();
    for (&domain, node) in nodes {
        let KernelNode::Domain(definition) = node else {
            continue;
        };
        match definition.kind() {
            DomainKind::CartesianBox { .. } => {
                if let Some(bounds) = cartesian_bounds.get(&domain) {
                    supports.insert(
                        domain,
                        SpatialSupport::Volume {
                            domain,
                            dimensions: bounds.len(),
                        },
                    );
                }
            }
            DomainKind::CartesianBoundary { .. } => {
                let parents = edge_targets(edges, domain, EdgeKind::BoundaryOf);
                if parents.len() != 1 {
                    continue;
                }
                let parent = *parents.first().expect("one boundary parent was checked");
                let Some(KernelNode::Domain(parent_definition)) = nodes.get(&parent) else {
                    continue;
                };
                let DomainKind::CartesianBox { .. } = parent_definition.kind() else {
                    continue;
                };
                let Some(bounds) = cartesian_bounds.get(&parent) else {
                    continue;
                };
                supports.insert(
                    domain,
                    SpatialSupport::Boundary {
                        domain,
                        parent,
                        dimensions: bounds.len(),
                    },
                );
            }
            _ => {}
        }
    }
    supports
}
