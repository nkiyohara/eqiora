use std::f64::consts::TAU;

use eqiora::Diagnostic;
use eqiora::geometry::{CanonicalGeometryV1, PlanarRegion};

use crate::geometry::digest_to_hex;

use super::scene::{GeometryLayer, LayerMetadata, SceneBuilder, SelectionLayer};

const CIRCLE_PRESENTATION_SEGMENTS: usize = 64;

pub(super) fn add_geometry(
    builder: &mut SceneBuilder,
    geometry: &CanonicalGeometryV1,
) -> Result<(), Diagnostic> {
    let owner_digest = digest_to_hex(&geometry.digest_bytes());
    let layer_id = format!("geometry:{owner_digest}");
    let projection = geometry_projection(geometry)?;
    let positions = builder.push_f64(
        format!("{layer_id}:positions"),
        vec![projection.positions.len() / 2, 2],
        projection.positions,
    )?;
    let segments = builder.push_u32(
        format!("{layer_id}:segments"),
        vec![projection.segments.len() / 2, 2],
        projection.segments,
    )?;
    let source_entities = builder.push_u32(
        format!("{layer_id}:source-entities"),
        vec![projection.source_entities.len()],
        projection.source_entities.clone(),
    )?;
    builder.push_layer(LayerMetadata::Geometry(GeometryLayer {
        id: layer_id.clone(),
        owner_digest: owner_digest.clone(),
        dimension: 2,
        projection: projection.policy,
        positions,
        segments,
        source_entities,
    }))?;

    for selection in geometry.entity_sets() {
        let selection_id = format!("selection:{layer_id}:{}", selection.name());
        if selection.dimension() == 1 {
            let primitive_indices = projection
                .source_entities
                .iter()
                .enumerate()
                .filter_map(|(primitive, source)| {
                    selection
                        .members()
                        .binary_search(&(*source as usize))
                        .is_ok()
                        .then_some(primitive as u32)
                })
                .collect::<Vec<_>>();
            let mapping = builder.push_u32(
                format!("{selection_id}:primitive-indices"),
                vec![primitive_indices.len()],
                primitive_indices,
            )?;
            builder.push_layer(LayerMetadata::Selection(SelectionLayer {
                id: selection_id,
                target_layer: layer_id.clone(),
                owner_digest: owner_digest.clone(),
                correspondence_digest: None,
                name: selection.name().to_owned(),
                dimension: selection.dimension(),
                available: true,
                unavailable_reason: None,
                entity_indices: Some(mapping),
                connectivity: None,
            }))?;
        } else {
            let unavailable_reason = if selection.dimension() == 0 {
                "the Geometry line projection does not expose vertex selection interaction"
            } else {
                "the Geometry line projection exposes no exact face primitive; compose its exact Mesh to inspect this selection"
            };
            builder.push_layer(LayerMetadata::Selection(SelectionLayer {
                id: selection_id,
                target_layer: layer_id.clone(),
                owner_digest: owner_digest.clone(),
                correspondence_digest: None,
                name: selection.name().to_owned(),
                dimension: selection.dimension(),
                available: false,
                unavailable_reason: Some(unavailable_reason.to_owned()),
                entity_indices: None,
                connectivity: None,
            }))?;
        }
    }
    Ok(())
}

struct GeometryProjection {
    positions: Vec<f64>,
    segments: Vec<u32>,
    source_entities: Vec<u32>,
    policy: String,
}

fn geometry_projection(geometry: &CanonicalGeometryV1) -> Result<GeometryProjection, Diagnostic> {
    if let Some(region) = geometry.region() {
        return Ok(region_projection(region));
    }
    if let Some(bounds) = geometry.planar_rectangle_bounds() {
        return Ok(rectangle_projection(*bounds));
    }
    if let (Some(bounds), Some(center), Some(radius)) = (
        geometry.circular_hole_bounds(),
        geometry.circular_hole_center(),
        geometry.circular_hole_radius_m(),
    ) {
        return Ok(circular_hole_projection(*bounds, center, radius));
    }
    Err(Diagnostic::error(
        eqiora::diagnostic::codes::NOT_IMPLEMENTED,
        "Geometry has no owner-provided private v0 render projection",
    ))
}

fn rectangle_projection(bounds: [[f64; 2]; 2]) -> GeometryProjection {
    let [[x0, x1], [y0, y1]] = bounds;
    GeometryProjection {
        positions: vec![x0, y0, x0, y1, x1, y0, x1, y1],
        segments: vec![0, 1, 2, 3, 0, 2, 1, 3],
        source_entities: vec![0, 1, 2, 3],
        policy: "exact-axis-aligned-segments/v0".to_owned(),
    }
}

fn circular_hole_projection(
    bounds: [[f64; 2]; 2],
    center: [f64; 2],
    radius: f64,
) -> GeometryProjection {
    let mut projection = rectangle_projection(bounds);
    let first = projection.positions.len() / 2;
    for index in 0..CIRCLE_PRESENTATION_SEGMENTS {
        let angle = TAU * index as f64 / CIRCLE_PRESENTATION_SEGMENTS as f64;
        projection.positions.extend([
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ]);
        projection.segments.extend([
            (first + index) as u32,
            (first + (index + 1) % CIRCLE_PRESENTATION_SEGMENTS) as u32,
        ]);
        projection.source_entities.push(4);
    }
    projection.policy =
        "exact-rectangle+analytic-circle-uniform-64-chords/presentation-only/v0".to_owned();
    projection
}

fn region_projection(region: &PlanarRegion) -> GeometryProjection {
    let positions = region.vertices().iter().flatten().copied().collect();
    let mut segments = Vec::new();
    let mut source_entities = Vec::new();
    let mut source = 0_u32;
    for face in region.faces() {
        for loop_indices in
            std::iter::once(face.outer()).chain(face.holes().iter().map(Vec::as_slice))
        {
            for index in 0..loop_indices.len() {
                segments.extend([
                    loop_indices[index] as u32,
                    loop_indices[(index + 1) % loop_indices.len()] as u32,
                ]);
                source_entities.push(source);
                source += 1;
            }
        }
    }
    GeometryProjection {
        positions,
        segments,
        source_entities,
        policy: "exact-straight-edge-topology/v0".to_owned(),
    }
}
