//! Bounded canonical replay retains facets and parent-relative incidences.

use super::{ConvexPolyhedra, SCHEMA, invalid, rotate, undirected};
use crate::canonical::{CANONICAL_ENCODING, WireEntitySet, WireLengthUnit};
use crate::{CanonicalGeometryLimits, NamedEntitySet};
use eqiora_core::Diagnostic;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema: String,
    encoding: String,
    length_unit: WireLengthUnit,
    tolerance_m: f64,
    vertices: Vec<[f64; 3]>,
    facets: Vec<Vec<usize>>,
    volumes: Vec<Vec<Incidence>>,
    entity_sets: Vec<WireEntitySet>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Incidence {
    facet: usize,
    orientation: Orientation,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Orientation {
    Forward,
    Reverse,
}

pub(super) fn encode(geometry: &ConvexPolyhedra) -> Result<Vec<u8>, Diagnostic> {
    let volumes = geometry
        .shells
        .iter()
        .map(|shell| {
            let mut incidences = shell
                .iter()
                .map(|polygon| {
                    let facet = geometry
                        .facets
                        .binary_search(&undirected(polygon))
                        .expect("collected facet");
                    Incidence {
                        facet,
                        orientation: if *polygon == geometry.facets[facet] {
                            Orientation::Forward
                        } else {
                            Orientation::Reverse
                        },
                    }
                })
                .collect::<Vec<_>>();
            incidences.sort_by_key(|i| i.facet);
            incidences
        })
        .collect();
    serde_json::to_vec(&Wire {
        schema: SCHEMA.to_owned(),
        encoding: CANONICAL_ENCODING.to_owned(),
        length_unit: WireLengthUnit::Metre,
        tolerance_m: geometry.tolerance_m,
        vertices: geometry.vertices.clone(),
        facets: geometry.facets.clone(),
        volumes,
        entity_sets: geometry
            .entity_sets
            .iter()
            .map(WireEntitySet::from_set)
            .collect(),
    })
    .map_err(|e| invalid(format!("cannot encode polyhedral geometry: {e}")))
}
pub(super) fn decode(
    bytes: &[u8],
    limits: CanonicalGeometryLimits,
) -> Result<ConvexPolyhedra, Diagnostic> {
    if bytes.len() > limits.max_bytes {
        return Err(invalid("polyhedral geometry exceeds decoder byte limit"));
    }
    let wire: Wire = serde_json::from_slice(bytes)
        .map_err(|e| invalid(format!("invalid polyhedral geometry JSON: {e}")))?;
    if wire.schema != SCHEMA
        || wire.encoding != CANONICAL_ENCODING
        || wire.length_unit != WireLengthUnit::Metre
    {
        return Err(invalid("unsupported polyhedral geometry contract"));
    }
    if wire.vertices.len() > limits.max_vertices
        || wire.facets.len() > limits.max_faces
        || wire.volumes.len() > limits.max_faces
        || wire.entity_sets.len() > limits.max_entity_sets
        || wire
            .facets
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.len()))
            .is_none_or(|n| n > limits.max_loop_indices)
        || wire
            .entity_sets
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.members.len()))
            .is_none_or(|n| n > limits.max_entity_set_members)
    {
        return Err(invalid(
            "polyhedral geometry exceeds decoder topology limits",
        ));
    }
    // Bound total expanded incidence references before cloning any facet loops.
    let mut indices = 0usize;
    for volume in &wire.volumes {
        for incidence in volume {
            let facet = wire
                .facets
                .get(incidence.facet)
                .ok_or_else(|| invalid("polyhedral volume names an absent facet"))?;
            indices = indices
                .checked_add(facet.len())
                .filter(|&n| n <= limits.max_loop_indices)
                .ok_or_else(|| invalid("polyhedral facet incidences exceed decoder loop limit"))?;
        }
    }
    let shells = wire
        .volumes
        .into_iter()
        .map(|volume| {
            volume
                .into_iter()
                .map(|i| {
                    let mut polygon = wire.facets[i.facet].clone();
                    if matches!(i.orientation, Orientation::Reverse) {
                        polygon.reverse();
                        rotate(&mut polygon);
                    }
                    polygon
                })
                .collect()
        })
        .collect();
    let geometry = ConvexPolyhedra::new(
        wire.vertices,
        shells,
        wire.entity_sets
            .into_iter()
            .map(|s| NamedEntitySet::new(s.name, s.dimension, s.members))
            .collect(),
        wire.tolerance_m,
        limits,
    )?;
    if geometry.canonical_bytes() != bytes {
        return Err(invalid("polyhedral geometry JSON is not canonical"));
    }
    Ok(geometry)
}
