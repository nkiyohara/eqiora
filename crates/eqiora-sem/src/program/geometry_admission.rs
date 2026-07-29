//! Closed-bundle admission of canonical geometry facts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, RawId};
use eqiora_geometry::CanonicalGeometryRef;
use eqiora_graph::{Edge, EdgeKind};
use eqiora_schema::kernel::typing::SpatialSupport;
use eqiora_schema::kernel::{DomainKind, KernelNode};

use super::{edge_targets, kernel_error, kernel_path};

pub(super) struct GeometryAdmission {
    pub(super) supports: BTreeMap<RawId, SpatialSupport<RawId>>,
    pub(super) diagnostics: Vec<Diagnostic>,
}

/// Index one exact closed artifact bundle.
///
/// Bundle faults are returned separately because they stop entity-set and
/// consumer validation: without exact closure there is no sound artifact
/// against which those checks could run.
pub(super) fn index_closed_bundle<'a>(
    nodes: &BTreeMap<RawId, KernelNode>,
    supplied: &[CanonicalGeometryRef<'a>],
) -> Result<BTreeMap<[u8; 32], CanonicalGeometryRef<'a>>, Vec<Diagnostic>> {
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
    artifacts: &BTreeMap<[u8; 32], CanonicalGeometryRef<'_>>,
) -> GeometryAdmission {
    let mut supports = BTreeMap::new();
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
                let DomainKind::GeometryRegion { geometry, .. } = parent_definition.kind() else {
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
                    }
                }
            }
            _ => {}
        }
    }

    GeometryAdmission {
        supports,
        diagnostics,
    }
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
