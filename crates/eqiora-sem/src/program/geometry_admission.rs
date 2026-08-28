//! Closed-bundle admission of canonical geometry facts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, RawId};
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_graph::{Edge, EdgeKind};
use eqiora_schema::kernel::typing::SpatialSupport;
use eqiora_schema::kernel::{ConnectionSemantics, DomainKind, KernelNode, ValueFrame};

use super::{edge_targets, kernel_error, kernel_path};

pub(super) struct GeometryAdmission {
    pub(super) supports: BTreeMap<RawId, SpatialSupport<RawId>>,
    pub(super) boundary_embeddings: BTreeMap<RawId, GeometryBoundaryEmbedding>,
    pub(super) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeometryBoundaryEmbedding {
    pub(crate) geometry: [u8; 32],
    pub(crate) entity_set: String,
    pub(crate) parent_entity_set: String,
    pub(crate) parent: RawId,
    pub(crate) dimensions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeometryBoundaryJunction {
    pub(crate) dimensions: usize,
}

/// Index one exact closed artifact bundle.
///
/// Bundle faults are returned separately because they stop entity-set and
/// consumer validation: without exact closure there is no sound artifact
/// against which those checks could run.
pub(super) fn index_closed_bundle<'a>(
    nodes: &BTreeMap<RawId, KernelNode>,
    supplied: &[&'a CanonicalGeometryV1],
) -> Result<BTreeMap<[u8; 32], &'a CanonicalGeometryV1>, Vec<Diagnostic>> {
    let mut required = BTreeMap::<[u8; 32], RawId>::new();
    for (&id, node) in nodes {
        let KernelNode::Domain(domain) = node else {
            continue;
        };
        if let DomainKind::GeometryRegion { geometry, .. } = domain.kind() {
            required
                .entry(geometry.bytes())
                .and_modify(|first| *first = (*first).min(id))
                .or_insert(id);
        }
    }

    let mut indexed = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for geometry in supplied {
        let digest = geometry.digest_bytes();
        if indexed.insert(digest, *geometry).is_some() {
            duplicates.insert(digest);
        }
    }

    let mut faults = BTreeMap::<[u8; 32], Vec<Diagnostic>>::new();
    for digest in duplicates {
        faults.entry(digest).or_default().push(invalid_artifact(
            format!(
                "duplicate canonical geometry artifact {}",
                digest_hex(digest)
            ),
            None,
        ));
    }
    for (&digest, &region) in &required {
        if !indexed.contains_key(&digest) {
            faults.entry(digest).or_default().push(invalid_artifact(
                format!("missing canonical geometry artifact {}", digest_hex(digest)),
                Some(region),
            ));
        }
    }
    for &digest in indexed.keys() {
        if !required.contains_key(&digest) {
            faults.entry(digest).or_default().push(invalid_artifact(
                format!(
                    "unreferenced canonical geometry artifact {}",
                    digest_hex(digest)
                ),
                None,
            ));
        }
    }

    if faults.is_empty() {
        Ok(indexed)
    } else {
        Err(faults.into_values().flatten().collect())
    }
}

pub(super) fn admit_entity_sets(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    invalid_domains: &BTreeSet<RawId>,
    artifacts: &BTreeMap<[u8; 32], &CanonicalGeometryV1>,
) -> GeometryAdmission {
    let mut supports = BTreeMap::new();
    let mut boundary_embeddings = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for (&id, node) in nodes {
        let KernelNode::Domain(domain) = node else {
            continue;
        };
        if invalid_domains.contains(&id) {
            continue;
        }

        match domain.kind() {
            DomainKind::GeometryRegion {
                geometry,
                entity_set,
            } => {
                let artifact = artifacts
                    .get(&geometry.bytes())
                    .expect("closed bundle contains every referenced geometry");
                let ambient = artifact.ambient_dimension();
                let topological = artifact.topological_dimension();
                if ambient != topological || topological == 0 {
                    diagnostics.push(kernel_error(
                        id,
                        format!(
                            "geometry region requires equal positive ambient and topological dimensions, found {ambient} and {topological}"
                        ),
                    ));
                    continue;
                }
                match artifact.entity_set_dimension(entity_set) {
                    None => diagnostics.push(kernel_error(
                        id,
                        format!(
                            "geometry region entity set `{entity_set}` is absent from its referenced artifact"
                        ),
                    )),
                    Some(dimension) if dimension != topological => diagnostics.push(kernel_error(
                        id,
                        format!(
                            "geometry region entity set `{entity_set}` has dimension {dimension}, expected {topological}"
                        ),
                    )),
                    Some(_) => {
                        supports.insert(
                            id,
                            SpatialSupport::Volume {
                                domain: id,
                                dimensions: ambient,
                            },
                        );
                    }
                }
            }
            DomainKind::GeometryBoundary { entity_set } => {
                let parents = edge_targets(edges, id, EdgeKind::BoundaryOf);
                if parents.len() != 1 {
                    // `validate_domains` owns this topology diagnostic.
                    continue;
                }
                let parent = *parents.first().expect("one parent was checked");
                if invalid_domains.contains(&parent) {
                    continue;
                }
                let Some(KernelNode::Domain(parent_definition)) = nodes.get(&parent) else {
                    continue;
                };
                let DomainKind::GeometryRegion {
                    geometry,
                    entity_set: parent_entity_set,
                } = parent_definition.kind()
                else {
                    continue;
                };
                let artifact = artifacts
                    .get(&geometry.bytes())
                    .expect("closed bundle contains the parent geometry");
                let ambient = artifact.ambient_dimension();
                let topological = artifact.topological_dimension();
                if ambient != topological || topological == 0 {
                    // The parent owns the dimensional diagnostic and cannot
                    // derive a coherent boundary support.
                    continue;
                }
                let expected = topological - 1;
                match artifact.entity_set_dimension(entity_set) {
                    None => diagnostics.push(kernel_error(
                        id,
                        format!(
                            "geometry boundary entity set `{entity_set}` is absent from its parent artifact"
                        ),
                    )),
                    Some(dimension) if dimension != expected => diagnostics.push(kernel_error(
                        id,
                        format!(
                            "geometry boundary entity set `{entity_set}` has dimension {dimension}, expected {expected}"
                        ),
                    )),
                    Some(_) => {
                        supports.insert(
                            id,
                            SpatialSupport::Boundary {
                                domain: id,
                                parent,
                                dimensions: ambient,
                            },
                        );
                        boundary_embeddings.insert(
                            id,
                            GeometryBoundaryEmbedding {
                                geometry: geometry.bytes(),
                                entity_set: entity_set.clone(),
                                parent_entity_set: parent_entity_set.clone(),
                                parent,
                                dimensions: ambient,
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    GeometryAdmission {
        supports,
        boundary_embeddings,
        diagnostics,
    }
}

/// Admit only the construction-owned, opposite-parent internal interface of
/// the exact adjacent-partition Geometry family. This is intentionally not a
/// generic non-Cartesian Connection fallback.
pub(super) fn admit_geometry_boundary_junctions(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    artifacts: &BTreeMap<[u8; 32], &CanonicalGeometryV1>,
    embeddings: &BTreeMap<RawId, GeometryBoundaryEmbedding>,
) -> (
    BTreeMap<RawId, GeometryBoundaryJunction>,
    BTreeSet<RawId>,
    Vec<Diagnostic>,
) {
    let mut junctions = BTreeMap::new();
    let mut accepted_ports = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for (&connection_id, node) in nodes {
        let KernelNode::Connection(connection) = node else {
            continue;
        };
        let ports = edge_targets(edges, connection_id, EdgeKind::Connects);
        let geometry_ports = ports
            .iter()
            .filter_map(|port| {
                let KernelNode::Port(definition) = nodes.get(port)? else {
                    return None;
                };
                let (connector, boundary) = definition.boundary_physical_contract()?;
                embeddings
                    .get(&boundary.erase())
                    .map(|embedding| (*port, connector.erase(), boundary.erase(), embedding))
            })
            .collect::<Vec<_>>();
        if geometry_ports.is_empty() {
            continue;
        }
        let valid = (|| {
            if connection.semantics() != ConnectionSemantics::Conserving
                || ports.len() != 2
                || geometry_ports.len() != 2
            {
                return false;
            }
            let (first_port, first_connector, first_boundary, first) = geometry_ports[0];
            let (second_port, second_connector, second_boundary, second) = geometry_ports[1];
            if first_port == second_port
                || first_connector != second_connector
                || first_boundary == second_boundary
                || first.parent == second.parent
                || first.geometry != second.geometry
                || first.dimensions != second.dimensions
            {
                return false;
            }
            let Some(KernelNode::Domain(connector)) = nodes.get(&first_connector) else {
                return false;
            };
            let DomainKind::BoundaryPhysical { connector } = connector.kind() else {
                return false;
            };
            if connector.frame() == ValueFrame::SpatialCartesian
                && connector
                    .shape()
                    .extents()
                    .iter()
                    .any(|extent| usize::try_from(extent.get()).ok() != Some(first.dimensions))
            {
                return false;
            }
            artifacts.get(&first.geometry).is_some_and(|artifact| {
                artifact.selections_form_opposite_parent_interface(
                    &first.entity_set,
                    &first.parent_entity_set,
                    &second.entity_set,
                    &second.parent_entity_set,
                )
            })
        })();
        if valid {
            accepted_ports.extend(ports.iter().copied());
            junctions.insert(
                connection_id,
                GeometryBoundaryJunction {
                    dimensions: geometry_ports[0].3.dimensions,
                },
            );
        } else {
            diagnostics.push(kernel_error(
                connection_id,
                "geometry boundary-physical Connection must be the exact two-sided opposite-parent interface of one admitted adjacent-partition Geometry",
            ));
        }
    }
    (junctions, accepted_ports, diagnostics)
}

fn invalid_artifact(message: impl Into<String>, subject: Option<RawId>) -> Diagnostic {
    let diagnostic = Diagnostic::error(codes::INVALID_ARTIFACT, message);
    match subject {
        Some(id) => diagnostic.with_graph_path(kernel_path(id)),
        None => diagnostic,
    }
}

fn digest_hex(digest: [u8; 32]) -> String {
    let mut text = String::with_capacity(64);
    for byte in digest {
        write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}
