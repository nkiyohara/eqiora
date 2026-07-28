//! Spatial Domain validation and support reconstruction.

use std::collections::BTreeMap;

use eqiora_core::{Diagnostic, RawId};
use eqiora_graph::{Edge, EdgeKind};
use eqiora_schema::kernel::typing::SpatialSupport;
use eqiora_schema::kernel::{DomainKind, KernelNode, RepresentationKind, ValueFrame};

use super::{edge_targets, kernel_error};

pub(super) fn validate_domains(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&id, node) in nodes {
        let KernelNode::Domain(domain) = node else {
            continue;
        };
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
                    Some(KernelNode::Domain(parent)) => match parent.kind() {
                        DomainKind::CartesianBox { bounds } if *axis < bounds.len() => {}
                        DomainKind::CartesianBox { bounds } => diagnostics.push(kernel_error(
                            id,
                            format!(
                                "boundary axis {axis} is outside parent dimension {}",
                                bounds.len()
                            ),
                        )),
                        _ => diagnostics.push(kernel_error(
                            id,
                            "Cartesian boundary parent must be a Cartesian box Domain",
                        )),
                    },
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
    }
}

pub(super) fn validate_geometry_support_uses(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&id, node) in nodes {
        let (support, subject) = match node {
            KernelNode::Field(_) => (
                edge_targets(edges, id, EdgeKind::DefinedOn)
                    .into_iter()
                    .find(|target| is_geometry_domain(*target, nodes)),
                "Field spatial support",
            ),
            KernelNode::Relation(_) => (
                edge_targets(edges, id, EdgeKind::AppliesOn)
                    .into_iter()
                    .find(|target| is_geometry_domain(*target, nodes)),
                "Relation spatial scope",
            ),
            KernelNode::Port(port) => (
                port.boundary_physical_contract()
                    .map(|(_, boundary)| boundary.erase())
                    .filter(|boundary| is_geometry_domain(*boundary, nodes)),
                "boundary-physical Port support",
            ),
            _ => continue,
        };
        if support.is_some() {
            diagnostics.push(kernel_error(
                id,
                format!("{subject} from a geometry Domain requires artifact admission"),
            ));
        }
    }
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
        if !matches!(
            nodes.get(&domains[0]),
            Some(KernelNode::Domain(domain))
                if matches!(domain.kind(), DomainKind::CartesianBox { .. })
        ) {
            diagnostics.push(kernel_error(
                id,
                "v0 spatial Field Domain must be a Cartesian box",
            ));
        }
        if let Some(KernelNode::Domain(domain)) = nodes.get(&domains[0])
            && let DomainKind::CartesianBox { bounds } = domain.kind()
            && field.frame() == ValueFrame::SpatialCartesian
            && field
                .shape()
                .extents()
                .iter()
                .any(|extent| usize::try_from(extent.get()).ok() != Some(bounds.len()))
        {
            diagnostics.push(kernel_error(
                id,
                "Cartesian spatial Field extents must equal its Domain ambient dimension",
            ));
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
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
) -> Option<SpatialSupport<RawId>> {
    let supports = edge_targets(edges, field, EdgeKind::DefinedOn)
        .into_iter()
        .filter_map(|target| spatial_support(target, nodes, edges))
        .filter(|support| matches!(support, SpatialSupport::Volume { .. }))
        .collect::<Vec<_>>();
    (supports.len() == 1).then(|| supports[0].clone())
}

pub(super) fn spatial_support(
    domain: RawId,
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
) -> Option<SpatialSupport<RawId>> {
    let Some(KernelNode::Domain(definition)) = nodes.get(&domain) else {
        return None;
    };
    match definition.kind() {
        DomainKind::CartesianBox { bounds } => Some(SpatialSupport::Volume {
            domain,
            dimensions: bounds.len(),
        }),
        DomainKind::CartesianBoundary { .. } => {
            let parents = edge_targets(edges, domain, EdgeKind::BoundaryOf);
            if parents.len() != 1 {
                return None;
            }
            let parent = *parents.first().expect("one boundary parent was checked");
            let Some(KernelNode::Domain(parent_definition)) = nodes.get(&parent) else {
                return None;
            };
            let DomainKind::CartesianBox { bounds } = parent_definition.kind() else {
                return None;
            };
            Some(SpatialSupport::Boundary {
                domain,
                parent,
                dimensions: bounds.len(),
            })
        }
        _ => None,
    }
}
