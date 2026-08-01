//! Studio projection for the authored-CAD operation-history slice.
//!
//! The native bridge never invents CAD meaning. It forwards bounded ergonomic
//! scalars to the accepted Rust owner (`eqiora::geometry::CadAuthoredGraph`
//! and friends), then projects the owner's exact observations, canonical
//! bytes, graph identity, opaque graph-bound face handles, and complete
//! analytic build receipt. Canonical bytes and handles cross this boundary as
//! opaque bounded lowercase-hex strings; the client can replay them but never
//! reinterpret, recompute, or rebind them. The projected digest is the
//! authored *graph* identity — it is never presented as a Geometry identity.

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use eqiora::geometry::{
    CadAuthoredBuild, CadAuthoredFaceHandle, CadAuthoredFaceSelection, CadAuthoredGraph,
    CadRepairDispositionV1, ConstrainedRectangleV1,
};
use serde::{Deserialize, Serialize};

/// Protocol for the independently versioned authored-CAD payload nested in
/// the Studio bridge envelope. Distinct from `eqiora.studio.cad/v1`.
pub(super) const CAD_AUTHORED_PROTOCOL: &str = "eqiora.studio.cad-authored/v1";

/// Owner decoder admits at most 4096 canonical graph bytes → 8192 hex digits.
const MAX_GRAPH_HEX_DIGITS: usize = 8_192;
/// Owner decoder admits at most 512 canonical handle bytes → 1024 hex digits.
const MAX_HANDLE_HEX_DIGITS: usize = 1_024;
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// The one closed authored-history request: rectangle-extrusion scalars plus
/// at most one circular through-cut. There is no operations array, node enum,
/// or provider selector, and no client-authored canonical form.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CadAuthoredBuildRequestDto {
    pub(super) protocol: String,
    pub(super) sketch: CadAuthoredSketchRequestDto,
    pub(super) extrusion_depth_m: f64,
    pub(super) requested_modeling_tolerance_m: f64,
    pub(super) cut: Option<CadAuthoredCutRequestDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CadAuthoredSketchRequestDto {
    pub(super) x_bounds_m: [f64; 2],
    pub(super) y_bounds_m: [f64; 2],
    pub(super) plane_z_m: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CadAuthoredCutRequestDto {
    pub(super) center_m: [f64; 2],
    pub(super) radius_m: f64,
    pub(super) requested_boolean_tolerance_m: f64,
}

/// Face-selection replay request. The graph bytes and handle bytes are the
/// exact opaque hex values a previous projection returned; the digest binds
/// the request to one authored graph identity before any resolution.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CadAuthoredSelectionRequestDto {
    pub(super) protocol: String,
    pub(super) graph_digest: String,
    pub(super) canonical_graph_hex: String,
    pub(super) handle_hex: String,
}

/// Complete presentation projection of one accepted authored history.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredProjectionDto {
    protocol: &'static str,
    /// Domain-separated authored-graph identity. Not a Geometry identity.
    graph_digest: String,
    canonical_graph_hex: String,
    canonical_byte_count: usize,
    history: Vec<CadAuthoredOperationDto>,
    tolerances: CadAuthoredToleranceDto,
    observations: CadAuthoredObservationsDto,
    faces: Vec<CadAuthoredFaceDto>,
    build: CadAuthoredBuildReceiptDto,
}

/// Ordered semantic history projected from the owner's closed vocabulary.
/// Identifiers repeat the frozen wire ids so the inspector shows the exact
/// dependency chain the canonical bytes persist.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum CadAuthoredOperationDto {
    #[serde(rename_all = "camelCase")]
    SketchPlane {
        id: &'static str,
        plane: &'static str,
        z_m: f64,
    },
    #[serde(rename_all = "camelCase")]
    RectangleProfile {
        id: &'static str,
        sketch_plane: &'static str,
        constraint: &'static str,
        x_bounds_m: [f64; 2],
        y_bounds_m: [f64; 2],
    },
    #[serde(rename_all = "camelCase")]
    ClosedFace {
        id: &'static str,
        profile: &'static str,
        region_count: usize,
    },
    #[serde(rename_all = "camelCase")]
    PositiveZExtrusion {
        id: &'static str,
        face: &'static str,
        depth_m: f64,
        repair: &'static str,
    },
    #[serde(rename_all = "camelCase")]
    CutSketchPlane {
        id: &'static str,
        face: &'static str,
    },
    #[serde(rename_all = "camelCase")]
    CircleProfile {
        id: &'static str,
        sketch_plane: &'static str,
        constraint: &'static str,
        center_m: [f64; 2],
        radius_m: f64,
    },
    #[serde(rename_all = "camelCase")]
    ClosedCutFace {
        id: &'static str,
        profile: &'static str,
        region_count: usize,
    },
    #[serde(rename_all = "camelCase")]
    CircularThroughCut {
        id: &'static str,
        target: &'static str,
        tool_face: &'static str,
        requested_boolean_tolerance_m: f64,
        repair: &'static str,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredToleranceDto {
    /// Identity-only: replaying with a different value keeps every
    /// observation below but changes the graph digest.
    requested_modeling_tolerance_m: f64,
    requested_boolean_tolerance_m: Option<f64>,
    repair: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredObservationsDto {
    bounds_m: [(f64, f64); 3],
    outer_vertices_m: [[f64; 3]; 8],
    vertex_count: Option<usize>,
    edge_count: Option<usize>,
    face_count: usize,
    closed_shell_count: usize,
    body_count: usize,
    genus: usize,
    volume_m3: f64,
    surface_area_m2: f64,
}

/// One admitted provenance face with its opaque graph-bound handle bytes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredFaceDto {
    provenance_key: &'static str,
    handle_hex: String,
    area_m2: f64,
    boundary_loop_count: usize,
    centroid_m: Option<[f64; 3]>,
    outward_unit_normal: Option<[f64; 3]>,
    vertices_m: Option<[[f64; 3]; 4]>,
}

/// Complete accepted analytic build receipt, including topology lineage.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredBuildReceiptDto {
    graph_digest: String,
    provider_profile: &'static str,
    requested_modeling_tolerance_m: f64,
    requested_boolean_tolerance_m: Option<f64>,
    effective_boolean_tolerance_m: Option<f64>,
    maximum_position_discrepancy_m: f64,
    maximum_area_discrepancy_m2: f64,
    maximum_volume_discrepancy_m3: f64,
    repair: &'static str,
    lineage: CadAuthoredLineageDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredLineageDto {
    retained_unchanged: Vec<CadAuthoredLineageHandleDto>,
    retained_modified: Vec<CadAuthoredLineageHandleDto>,
    created: Vec<CadAuthoredLineageHandleDto>,
    deleted: Vec<CadAuthoredLineageHandleDto>,
    split: Vec<CadAuthoredLineageHandleDto>,
    merged: Vec<CadAuthoredLineageHandleDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredLineageHandleDto {
    provenance_key: &'static str,
    handle_hex: String,
}

/// Accepted selection returned only after the owner replayed the exact graph
/// and validated the graph-bound handle against it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredSelectionDto {
    protocol: &'static str,
    graph_digest: String,
    provenance_key: &'static str,
    handle_hex: String,
    area_m2: f64,
    boundary_loop_count: usize,
    centroid_m: Option<[f64; 3]>,
    outward_unit_normal: Option<[f64; 3]>,
}

/// Construct one admitted authored history in the Rust owner and project it.
///
/// Finiteness, positivity, ordering, and cut-clearance admission stay with
/// the owner; this function adds only protocol and transport checks.
pub(super) fn build_graph(
    request: &CadAuthoredBuildRequestDto,
) -> Result<CadAuthoredProjectionDto, Diagnostic> {
    if request.protocol != CAD_AUTHORED_PROTOCOL {
        return Err(invalid("unsupported Studio authored-CAD payload protocol"));
    }
    let sketch = ConstrainedRectangleV1::new(
        (request.sketch.x_bounds_m[0], request.sketch.x_bounds_m[1]),
        (request.sketch.y_bounds_m[0], request.sketch.y_bounds_m[1]),
        request.sketch.plane_z_m,
    )?;
    let base = CadAuthoredGraph::new(
        sketch,
        request.extrusion_depth_m,
        request.requested_modeling_tolerance_m,
    )?;
    let graph = match &request.cut {
        None => base,
        Some(cut) => base.circular_through_cut(
            cut.center_m,
            cut.radius_m,
            cut.requested_boolean_tolerance_m,
        )?,
    };
    project_graph(&graph)
}

/// Replay the exact opaque canonical graph and resolve one graph-bound face
/// handle. Fails before returning a selection for a stale, foreign, mutated,
/// or cross-version digest, graph, or handle.
pub(super) fn resolve_selection(
    request: &CadAuthoredSelectionRequestDto,
) -> Result<CadAuthoredSelectionDto, Diagnostic> {
    if request.protocol != CAD_AUTHORED_PROTOCOL {
        return Err(invalid("unsupported Studio authored-CAD payload protocol"));
    }
    let (graph, digest_hex) = replay_canonical_graph(&request.canonical_graph_hex)?;
    if request.graph_digest != digest_hex {
        return Err(invalid(
            "authored-CAD selection references a stale or foreign graph identity",
        ));
    }
    let handle_bytes = decode_bounded_hex(
        &request.handle_hex,
        MAX_HANDLE_HEX_DIGITS,
        "authored-CAD face handle",
    )?;
    let handle = CadAuthoredFaceHandle::decode_canonical(&handle_bytes)?;
    let selection = graph.resolve_face(&handle)?;
    Ok(CadAuthoredSelectionDto {
        protocol: CAD_AUTHORED_PROTOCOL,
        graph_digest: digest_hex,
        provenance_key: selection.provenance_key(),
        handle_hex: encode_hex(handle.canonical_bytes()),
        area_m2: graph.face_area_m2(&handle)?,
        boundary_loop_count: graph.face_boundary_loop_count(&handle)?,
        centroid_m: graph.rectangular_face_centroid_m(&handle)?,
        outward_unit_normal: graph.planar_face_outward_normal(&handle)?,
    })
}

/// Replay one opaque bounded canonical-graph payload through the owner and
/// return the graph with its lowercase-hex authored-graph digest. Shared by
/// selection replay and the Python-export adapter; digest-binding comparison
/// stays with each caller.
pub(super) fn replay_canonical_graph(
    canonical_graph_hex: &str,
) -> Result<(CadAuthoredGraph, String), Diagnostic> {
    let graph_bytes = decode_bounded_hex(
        canonical_graph_hex,
        MAX_GRAPH_HEX_DIGITS,
        "authored-CAD canonical graph",
    )?;
    let graph = CadAuthoredGraph::decode_canonical(&graph_bytes)?;
    let digest_hex = encode_hex(&graph.digest_bytes());
    Ok((graph, digest_hex))
}

fn project_graph(graph: &CadAuthoredGraph) -> Result<CadAuthoredProjectionDto, Diagnostic> {
    let build = graph.build_analytic()?;
    let faces = graph
        .selection_inventory()
        .iter()
        .copied()
        .map(|selection| project_face(graph, selection))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CadAuthoredProjectionDto {
        protocol: CAD_AUTHORED_PROTOCOL,
        graph_digest: encode_hex(&graph.digest_bytes()),
        canonical_graph_hex: encode_hex(graph.canonical_bytes()),
        canonical_byte_count: graph.canonical_bytes().len(),
        history: project_history(graph),
        tolerances: CadAuthoredToleranceDto {
            requested_modeling_tolerance_m: graph.requested_modeling_tolerance_m(),
            requested_boolean_tolerance_m: graph.requested_boolean_tolerance_m(),
            repair: repair_key(graph.repair_disposition()),
        },
        observations: CadAuthoredObservationsDto {
            bounds_m: graph.output().bounds_m(),
            outer_vertices_m: graph.vertices_m(),
            vertex_count: graph.vertex_count(),
            edge_count: graph.edge_count(),
            face_count: graph.face_count(),
            closed_shell_count: graph.closed_shell_count(),
            body_count: graph.body_count(),
            genus: graph.genus(),
            volume_m3: graph.volume_m3(),
            surface_area_m2: graph.surface_area_m2(),
        },
        faces,
        build: project_build(graph, &build)?,
    })
}

/// Frozen wire ids of the closed dependency chain, repeated for inspection.
fn project_history(graph: &CadAuthoredGraph) -> Vec<CadAuthoredOperationDto> {
    let sketch = graph.sketch();
    let mut history = vec![
        CadAuthoredOperationDto::SketchPlane {
            id: "sketch-plane",
            plane: "xy",
            z_m: sketch.plane_z_m(),
        },
        CadAuthoredOperationDto::RectangleProfile {
            id: "rectangle-profile",
            sketch_plane: "sketch-plane",
            constraint: "closed-by-construction",
            x_bounds_m: [sketch.x_bounds_m().0, sketch.x_bounds_m().1],
            y_bounds_m: [sketch.y_bounds_m().0, sketch.y_bounds_m().1],
        },
        CadAuthoredOperationDto::ClosedFace {
            id: "profile-face",
            profile: "rectangle-profile",
            region_count: 1,
        },
        CadAuthoredOperationDto::PositiveZExtrusion {
            id: "positive-z-extrusion",
            face: "profile-face",
            depth_m: graph.extrusion_depth_m(),
            repair: repair_key(graph.repair_disposition()),
        },
    ];
    if let (Some(center_m), Some(radius_m), Some(tolerance_m)) = (
        graph.cut_center_m(),
        graph.cut_radius_m(),
        graph.requested_boolean_tolerance_m(),
    ) {
        history.extend([
            CadAuthoredOperationDto::CutSketchPlane {
                id: "cut-sketch-plane",
                face: "end-cap",
            },
            CadAuthoredOperationDto::CircleProfile {
                id: "circle-profile",
                sketch_plane: "cut-sketch-plane",
                constraint: "closed-by-construction",
                center_m,
                radius_m,
            },
            CadAuthoredOperationDto::ClosedCutFace {
                id: "cut-profile-face",
                profile: "circle-profile",
                region_count: 1,
            },
            CadAuthoredOperationDto::CircularThroughCut {
                id: "circular-through-cut",
                target: "positive-z-extrusion",
                tool_face: "cut-profile-face",
                requested_boolean_tolerance_m: tolerance_m,
                repair: repair_key(graph.repair_disposition()),
            },
        ]);
    }
    history
}

fn project_face(
    graph: &CadAuthoredGraph,
    selection: CadAuthoredFaceSelection,
) -> Result<CadAuthoredFaceDto, Diagnostic> {
    let handle = graph.face_handle(selection)?;
    Ok(CadAuthoredFaceDto {
        provenance_key: selection.provenance_key(),
        handle_hex: encode_hex(handle.canonical_bytes()),
        area_m2: graph.face_area_m2(&handle)?,
        boundary_loop_count: graph.face_boundary_loop_count(&handle)?,
        centroid_m: graph.rectangular_face_centroid_m(&handle)?,
        outward_unit_normal: graph.planar_face_outward_normal(&handle)?,
        vertices_m: graph.rectangular_face_vertices_m(&handle)?,
    })
}

fn project_build(
    graph: &CadAuthoredGraph,
    build: &CadAuthoredBuild,
) -> Result<CadAuthoredBuildReceiptDto, Diagnostic> {
    Ok(CadAuthoredBuildReceiptDto {
        graph_digest: encode_hex(&build.graph_digest_bytes()),
        provider_profile: build.provider_profile(),
        requested_modeling_tolerance_m: build.requested_modeling_tolerance_m(),
        requested_boolean_tolerance_m: build.requested_boolean_tolerance_m(),
        effective_boolean_tolerance_m: build.effective_boolean_tolerance_m(),
        maximum_position_discrepancy_m: build.maximum_position_discrepancy_m(),
        maximum_area_discrepancy_m2: build.maximum_area_discrepancy_m2(),
        maximum_volume_discrepancy_m3: build.maximum_volume_discrepancy_m3(),
        repair: repair_key(build.repair_disposition()),
        lineage: CadAuthoredLineageDto {
            retained_unchanged: project_lineage(graph, build.retained_unchanged())?,
            retained_modified: project_lineage(graph, build.retained_modified())?,
            created: project_lineage(graph, build.created())?,
            deleted: project_lineage(graph, build.deleted())?,
            split: project_lineage(graph, build.split())?,
            merged: project_lineage(graph, build.merged())?,
        },
    })
}

fn project_lineage(
    graph: &CadAuthoredGraph,
    handles: &[CadAuthoredFaceHandle],
) -> Result<Vec<CadAuthoredLineageHandleDto>, Diagnostic> {
    handles
        .iter()
        .map(|handle| {
            Ok(CadAuthoredLineageHandleDto {
                provenance_key: graph.resolve_face(handle)?.provenance_key(),
                handle_hex: encode_hex(handle.canonical_bytes()),
            })
        })
        .collect()
}

const fn repair_key(disposition: CadRepairDispositionV1) -> &'static str {
    match disposition {
        CadRepairDispositionV1::None => "none",
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

fn decode_bounded_hex(
    text: &str,
    maximum_digits: usize,
    subject: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.len() > maximum_digits || !bytes.len().is_multiple_of(2) {
        return Err(invalid(format!(
            "{subject} must be 2 to {maximum_digits} lowercase hexadecimal digits of even length",
        )));
    }
    bytes
        .chunks_exact(2)
        .map(
            |pair| match (decode_hex_digit(pair[0]), decode_hex_digit(pair[1])) {
                (Some(high), Some(low)) => Ok((high << 4) | low),
                _ => Err(invalid(format!(
                    "{subject} contains a non-lowercase-hexadecimal digit",
                ))),
            },
        )
        .collect()
}

const fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Frozen v1 witness scalars, digest, canonical wire, and observations are
    // copied verbatim from the accepted case
    // `crates/eqiora-geometry/tests/cad_authored_rectangle_extrusion.rs`.
    const V1_SKETCH_X_M: [f64; 2] = [-2.0, 3.0];
    const V1_SKETCH_Y_M: [f64; 2] = [-1.0, 2.0];
    const V1_PLANE_Z_M: f64 = 0.5;
    const V1_DEPTH_M: f64 = 4.0;
    const V1_TOLERANCE_M: f64 = 1.0e-9;
    const V1_DIGEST_HEX: &str = "919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36";
    const V1_CANONICAL_BYTE_COUNT: usize = 731;
    const V1_VOLUME_M3: f64 = 60.0;
    const V1_SURFACE_AREA_M2: f64 = 94.0;
    const V1_END_CAP_AREA_M2: f64 = 15.0;

    // Frozen v2 witness scalars, digest, and byte count are copied verbatim
    // from the accepted case
    // `crates/eqiora-geometry/tests/cad_authored_circular_through_cut.rs`.
    const V2_SKETCH_X_M: [f64; 2] = [-0.04, 0.04];
    const V2_SKETCH_Y_M: [f64; 2] = [-0.025, 0.025];
    const V2_PLANE_Z_M: f64 = 0.0;
    const V2_DEPTH_M: f64 = 0.02;
    const V2_TOLERANCE_M: f64 = 1.0e-10;
    const V2_CUT_CENTER_M: [f64; 2] = [0.02, 0.0];
    const V2_CUT_RADIUS_M: f64 = 0.008;
    const V2_CUT_TOLERANCE_M: f64 = 1.0e-9;
    const V2_DIGEST_HEX: &str = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47";
    const V2_CANONICAL_BYTE_COUNT: usize = 1_292;

    fn v1_request() -> CadAuthoredBuildRequestDto {
        CadAuthoredBuildRequestDto {
            protocol: CAD_AUTHORED_PROTOCOL.to_owned(),
            sketch: CadAuthoredSketchRequestDto {
                x_bounds_m: V1_SKETCH_X_M,
                y_bounds_m: V1_SKETCH_Y_M,
                plane_z_m: V1_PLANE_Z_M,
            },
            extrusion_depth_m: V1_DEPTH_M,
            requested_modeling_tolerance_m: V1_TOLERANCE_M,
            cut: None,
        }
    }

    fn v2_request() -> CadAuthoredBuildRequestDto {
        CadAuthoredBuildRequestDto {
            protocol: CAD_AUTHORED_PROTOCOL.to_owned(),
            sketch: CadAuthoredSketchRequestDto {
                x_bounds_m: V2_SKETCH_X_M,
                y_bounds_m: V2_SKETCH_Y_M,
                plane_z_m: V2_PLANE_Z_M,
            },
            extrusion_depth_m: V2_DEPTH_M,
            requested_modeling_tolerance_m: V2_TOLERANCE_M,
            cut: Some(CadAuthoredCutRequestDto {
                center_m: V2_CUT_CENTER_M,
                radius_m: V2_CUT_RADIUS_M,
                requested_boolean_tolerance_m: V2_CUT_TOLERANCE_M,
            }),
        }
    }

    fn lineage_keys(handles: &[CadAuthoredLineageHandleDto]) -> Vec<&'static str> {
        handles.iter().map(|handle| handle.provenance_key).collect()
    }

    /// Public owner constructed independently of `build_graph`, from the same
    /// frozen witness scalars, to compare every projected field against.
    fn v1_owner() -> CadAuthoredGraph {
        let sketch = ConstrainedRectangleV1::new(
            (V1_SKETCH_X_M[0], V1_SKETCH_X_M[1]),
            (V1_SKETCH_Y_M[0], V1_SKETCH_Y_M[1]),
            V1_PLANE_Z_M,
        )
        .unwrap();
        CadAuthoredGraph::new(sketch, V1_DEPTH_M, V1_TOLERANCE_M).unwrap()
    }

    fn v2_owner() -> CadAuthoredGraph {
        let sketch = ConstrainedRectangleV1::new(
            (V2_SKETCH_X_M[0], V2_SKETCH_X_M[1]),
            (V2_SKETCH_Y_M[0], V2_SKETCH_Y_M[1]),
            V2_PLANE_Z_M,
        )
        .unwrap();
        CadAuthoredGraph::new(sketch, V2_DEPTH_M, V2_TOLERANCE_M)
            .unwrap()
            .circular_through_cut(V2_CUT_CENTER_M, V2_CUT_RADIUS_M, V2_CUT_TOLERANCE_M)
            .unwrap()
    }

    fn assert_lineage_matches_owner(
        owner: &CadAuthoredGraph,
        handles: &[CadAuthoredFaceHandle],
        projected: &[CadAuthoredLineageHandleDto],
    ) {
        assert_eq!(projected.len(), handles.len());
        for (dto, handle) in projected.iter().zip(handles) {
            assert_eq!(
                dto.provenance_key,
                owner.resolve_face(handle).unwrap().provenance_key()
            );
            assert_eq!(dto.handle_hex, encode_hex(handle.canonical_bytes()));
        }
    }

    /// Every projected identity, observation, face, and build-receipt field
    /// must equal the value the separately constructed public owner reports.
    fn assert_projection_matches_owner(
        projection: &CadAuthoredProjectionDto,
        owner: &CadAuthoredGraph,
    ) {
        assert_eq!(projection.graph_digest, encode_hex(&owner.digest_bytes()));
        assert_eq!(
            projection.canonical_graph_hex,
            encode_hex(owner.canonical_bytes())
        );
        assert_eq!(
            projection.canonical_byte_count,
            owner.canonical_bytes().len()
        );
        assert_eq!(
            projection.tolerances.requested_modeling_tolerance_m,
            owner.requested_modeling_tolerance_m()
        );
        assert_eq!(
            projection.tolerances.requested_boolean_tolerance_m,
            owner.requested_boolean_tolerance_m()
        );
        assert_eq!(
            projection.tolerances.repair,
            repair_key(owner.repair_disposition())
        );

        let observations = &projection.observations;
        assert_eq!(observations.bounds_m, owner.output().bounds_m());
        assert_eq!(observations.outer_vertices_m, owner.vertices_m());
        assert_eq!(observations.vertex_count, owner.vertex_count());
        assert_eq!(observations.edge_count, owner.edge_count());
        assert_eq!(observations.face_count, owner.face_count());
        assert_eq!(observations.closed_shell_count, owner.closed_shell_count());
        assert_eq!(observations.body_count, owner.body_count());
        assert_eq!(observations.genus, owner.genus());
        assert_eq!(observations.volume_m3, owner.volume_m3());
        assert_eq!(observations.surface_area_m2, owner.surface_area_m2());

        let inventory = owner.selection_inventory();
        assert_eq!(projection.faces.len(), inventory.len());
        for (face, selection) in projection.faces.iter().zip(inventory.iter().copied()) {
            let handle = owner.face_handle(selection).unwrap();
            assert_eq!(face.provenance_key, selection.provenance_key());
            assert_eq!(face.handle_hex, encode_hex(handle.canonical_bytes()));
            assert_eq!(face.area_m2, owner.face_area_m2(&handle).unwrap());
            assert_eq!(
                face.boundary_loop_count,
                owner.face_boundary_loop_count(&handle).unwrap()
            );
            assert_eq!(
                face.centroid_m,
                owner.rectangular_face_centroid_m(&handle).unwrap()
            );
            assert_eq!(
                face.outward_unit_normal,
                owner.planar_face_outward_normal(&handle).unwrap()
            );
            assert_eq!(
                face.vertices_m,
                owner.rectangular_face_vertices_m(&handle).unwrap()
            );
        }

        let build = owner.build_analytic().unwrap();
        let receipt = &projection.build;
        assert_eq!(
            receipt.graph_digest,
            encode_hex(&build.graph_digest_bytes())
        );
        assert_eq!(receipt.provider_profile, build.provider_profile());
        assert_eq!(
            receipt.requested_modeling_tolerance_m,
            build.requested_modeling_tolerance_m()
        );
        assert_eq!(
            receipt.requested_boolean_tolerance_m,
            build.requested_boolean_tolerance_m()
        );
        assert_eq!(
            receipt.effective_boolean_tolerance_m,
            build.effective_boolean_tolerance_m()
        );
        assert_eq!(
            receipt.maximum_position_discrepancy_m,
            build.maximum_position_discrepancy_m()
        );
        assert_eq!(
            receipt.maximum_area_discrepancy_m2,
            build.maximum_area_discrepancy_m2()
        );
        assert_eq!(
            receipt.maximum_volume_discrepancy_m3,
            build.maximum_volume_discrepancy_m3()
        );
        assert_eq!(receipt.repair, repair_key(build.repair_disposition()));
        assert_lineage_matches_owner(
            owner,
            build.retained_unchanged(),
            &receipt.lineage.retained_unchanged,
        );
        assert_lineage_matches_owner(
            owner,
            build.retained_modified(),
            &receipt.lineage.retained_modified,
        );
        assert_lineage_matches_owner(owner, build.created(), &receipt.lineage.created);
        assert_lineage_matches_owner(owner, build.deleted(), &receipt.lineage.deleted);
        assert_lineage_matches_owner(owner, build.split(), &receipt.lineage.split);
        assert_lineage_matches_owner(owner, build.merged(), &receipt.lineage.merged);
    }

    #[test]
    fn v1_projection_repeats_the_frozen_owner_identity_and_observations() {
        let projection = build_graph(&v1_request()).unwrap();
        assert_eq!(projection.protocol, CAD_AUTHORED_PROTOCOL);
        assert_eq!(projection.graph_digest, V1_DIGEST_HEX);
        assert_eq!(projection.canonical_byte_count, V1_CANONICAL_BYTE_COUNT);
        assert_eq!(
            projection.canonical_graph_hex.len(),
            2 * V1_CANONICAL_BYTE_COUNT
        );
        assert_eq!(projection.history.len(), 4);
        assert_eq!(projection.observations.vertex_count, Some(8));
        assert_eq!(projection.observations.edge_count, Some(12));
        assert_eq!(projection.observations.face_count, 6);
        assert_eq!(projection.observations.genus, 0);
        assert_eq!(projection.observations.volume_m3, V1_VOLUME_M3);
        assert_eq!(projection.observations.surface_area_m2, V1_SURFACE_AREA_M2);
        assert_eq!(projection.faces.len(), 6);
        let end_cap = projection
            .faces
            .iter()
            .find(|face| face.provenance_key == "end-cap")
            .unwrap();
        assert_eq!(end_cap.area_m2, V1_END_CAP_AREA_M2);
        assert_eq!(end_cap.boundary_loop_count, 1);
        assert_eq!(projection.build.graph_digest, V1_DIGEST_HEX);
        assert_eq!(projection.build.requested_boolean_tolerance_m, None);
        assert_eq!(lineage_keys(&projection.build.lineage.created).len(), 6);
        assert!(projection.build.lineage.retained_unchanged.is_empty());
    }

    #[test]
    fn v1_projection_equals_the_separately_constructed_public_owner() {
        assert_projection_matches_owner(&build_graph(&v1_request()).unwrap(), &v1_owner());
    }

    #[test]
    fn v2_projection_equals_the_separately_constructed_public_owner() {
        assert_projection_matches_owner(&build_graph(&v2_request()).unwrap(), &v2_owner());
    }

    #[test]
    fn v2_history_projects_the_complete_frozen_dependency_chain() {
        let projection = build_graph(&v2_request()).unwrap();
        assert_eq!(projection.history.len(), 8);
        let CadAuthoredOperationDto::CutSketchPlane { id, face } = &projection.history[4] else {
            panic!("history step 5 must be the cut sketch plane");
        };
        assert_eq!((*id, *face), ("cut-sketch-plane", "end-cap"));
        let CadAuthoredOperationDto::CircleProfile {
            id,
            sketch_plane,
            constraint,
            center_m,
            radius_m,
        } = &projection.history[5]
        else {
            panic!("history step 6 must be the circle profile");
        };
        assert_eq!(
            (*id, *sketch_plane, *constraint),
            (
                "circle-profile",
                "cut-sketch-plane",
                "closed-by-construction"
            )
        );
        assert_eq!(*center_m, V2_CUT_CENTER_M);
        assert_eq!(*radius_m, V2_CUT_RADIUS_M);
        let CadAuthoredOperationDto::ClosedCutFace {
            id,
            profile,
            region_count,
        } = &projection.history[6]
        else {
            panic!("history step 7 must be the closed cut face");
        };
        assert_eq!(
            (*id, *profile, *region_count),
            ("cut-profile-face", "circle-profile", 1)
        );
        let CadAuthoredOperationDto::CircularThroughCut {
            id,
            target,
            tool_face,
            requested_boolean_tolerance_m,
            repair,
        } = &projection.history[7]
        else {
            panic!("history step 8 must be the circular through-cut");
        };
        assert_eq!(
            (*id, *target, *tool_face, *repair),
            (
                "circular-through-cut",
                "positive-z-extrusion",
                "cut-profile-face",
                "none"
            )
        );
        assert_eq!(*requested_boolean_tolerance_m, V2_CUT_TOLERANCE_M);
    }

    #[test]
    fn v2_projection_carries_cut_history_receipt_and_lineage() {
        let projection = build_graph(&v2_request()).unwrap();
        assert_eq!(projection.graph_digest, V2_DIGEST_HEX);
        assert_eq!(projection.canonical_byte_count, V2_CANONICAL_BYTE_COUNT);
        assert_eq!(projection.history.len(), 8);
        assert_eq!(projection.observations.vertex_count, None);
        assert_eq!(projection.observations.face_count, 7);
        assert_eq!(projection.observations.genus, 1);
        assert_eq!(projection.faces.len(), 7);
        assert_eq!(
            projection.tolerances.requested_boolean_tolerance_m,
            Some(V2_CUT_TOLERANCE_M)
        );
        assert_eq!(
            projection.build.provider_profile,
            "eqiora.cad.analytic-circular-through-cut-v1"
        );
        assert_eq!(
            projection.build.effective_boolean_tolerance_m,
            Some(V2_CUT_TOLERANCE_M)
        );
        assert_eq!(projection.build.maximum_volume_discrepancy_m3, 0.0);
        assert_eq!(projection.build.repair, "none");
        assert_eq!(
            lineage_keys(&projection.build.lineage.retained_unchanged),
            [
                "profile-x-lower",
                "profile-x-upper",
                "profile-y-lower",
                "profile-y-upper"
            ]
        );
        assert_eq!(
            lineage_keys(&projection.build.lineage.retained_modified),
            ["start-cap", "end-cap"]
        );
        assert_eq!(
            lineage_keys(&projection.build.lineage.created),
            ["cut-wall"]
        );
        assert!(projection.build.lineage.deleted.is_empty());
        let cut_wall = projection
            .faces
            .iter()
            .find(|face| face.provenance_key == "cut-wall")
            .unwrap();
        assert_eq!(cut_wall.boundary_loop_count, 2);
        assert_eq!(cut_wall.centroid_m, None);
        assert_eq!(cut_wall.outward_unit_normal, None);
        assert_eq!(cut_wall.vertices_m, None);
    }

    #[test]
    fn tau_only_witness_changes_identity_and_nothing_observable() {
        // τ-only witness pair copied from the frozen rectangle case:
        // tolerance 1e-9 versus 2e-9 with every other scalar identical.
        let first = build_graph(&v1_request()).unwrap();
        let mut request = v1_request();
        request.requested_modeling_tolerance_m = 2.0e-9;
        let witness = build_graph(&request).unwrap();

        assert_ne!(witness.graph_digest, first.graph_digest);
        assert_eq!(witness.observations.volume_m3, first.observations.volume_m3);
        assert_eq!(
            witness.observations.surface_area_m2,
            first.observations.surface_area_m2
        );
        assert_eq!(witness.observations.bounds_m, first.observations.bounds_m);
        assert_eq!(
            witness.observations.outer_vertices_m,
            first.observations.outer_vertices_m
        );
    }

    #[test]
    fn selection_replays_only_the_exact_bound_graph_and_handle() {
        let projection = build_graph(&v2_request()).unwrap();
        let face = projection
            .faces
            .iter()
            .find(|face| face.provenance_key == "cut-wall")
            .unwrap();
        let request = CadAuthoredSelectionRequestDto {
            protocol: CAD_AUTHORED_PROTOCOL.to_owned(),
            graph_digest: projection.graph_digest.clone(),
            canonical_graph_hex: projection.canonical_graph_hex.clone(),
            handle_hex: face.handle_hex.clone(),
        };
        let selection = resolve_selection(&request).unwrap();
        assert_eq!(selection.provenance_key, "cut-wall");
        assert_eq!(selection.graph_digest, projection.graph_digest);
        assert_eq!(selection.handle_hex, face.handle_hex);
        assert_eq!(selection.area_m2, face.area_m2);
        assert_eq!(selection.boundary_loop_count, 2);
    }

    #[test]
    fn selection_fails_closed_for_each_substituted_identity() {
        let v2 = build_graph(&v2_request()).unwrap();
        let v1 = build_graph(&v1_request()).unwrap();
        let cut_wall = v2
            .faces
            .iter()
            .find(|face| face.provenance_key == "cut-wall")
            .unwrap();
        let template = || CadAuthoredSelectionRequestDto {
            protocol: CAD_AUTHORED_PROTOCOL.to_owned(),
            graph_digest: v2.graph_digest.clone(),
            canonical_graph_hex: v2.canonical_graph_hex.clone(),
            handle_hex: cut_wall.handle_hex.clone(),
        };
        assert!(resolve_selection(&template()).is_ok());

        let mut request = template();
        request.protocol = "eqiora.studio.cad-authored/v2".to_owned();
        assert!(resolve_selection(&request).is_err());

        // Digest that does not match the replayed canonical bytes is stale.
        let mut request = template();
        request.graph_digest = v1.graph_digest.clone();
        assert!(resolve_selection(&request).is_err());

        // Foreign graph: v1 bytes and v1 digest reject a v2-bound handle.
        let mut request = template();
        request.graph_digest = v1.graph_digest.clone();
        request.canonical_graph_hex = v1.canonical_graph_hex.clone();
        assert!(resolve_selection(&request).is_err());

        // Cross-version: a v1 handle never resolves on the v2 graph.
        let mut request = template();
        request.handle_hex = v1.faces[0].handle_hex.clone();
        assert!(resolve_selection(&request).is_err());

        // Mutated, odd-length, oversized, or non-hex opaque bytes reject.
        let mut request = template();
        request.handle_hex.replace_range(0..2, "ff");
        assert!(resolve_selection(&request).is_err());
        let mut request = template();
        request.handle_hex.pop();
        assert!(resolve_selection(&request).is_err());
        let mut request = template();
        request.canonical_graph_hex = "ab".repeat(MAX_GRAPH_HEX_DIGITS / 2 + 1);
        assert!(resolve_selection(&request).is_err());
        let mut request = template();
        request.canonical_graph_hex.replace_range(0..2, "ZZ");
        assert!(resolve_selection(&request).is_err());
    }

    #[test]
    fn selection_rejects_valid_hex_replays_of_altered_canonical_wires() {
        let v2 = build_graph(&v2_request()).unwrap();
        let cut_wall = v2
            .faces
            .iter()
            .find(|face| face.provenance_key == "cut-wall")
            .unwrap();
        let wire_bytes = decode_bounded_hex(
            &v2.canonical_graph_hex,
            MAX_GRAPH_HEX_DIGITS,
            "test canonical graph",
        )
        .unwrap();
        let wire = String::from_utf8(wire_bytes).unwrap();

        // Each altered wire is valid even-length lowercase hex on the
        // transport; the owner decode or digest binding must reject it.
        let unknown_member = wire.replacen('{', "{\"unknown_member\":0,", 1);
        let duplicate_member = wire.replacen(
            "\"length_unit\":\"metre\"",
            "\"length_unit\":\"metre\",\"length_unit\":\"metre\"",
            1,
        );
        // Decodes to a different admitted graph, so the recomputed identity
        // no longer matches the request's digest binding.
        let mutated_scalar = wire.replacen("\"radius_m\":0.008", "\"radius_m\":0.009", 1);
        for altered in [unknown_member, duplicate_member, mutated_scalar] {
            assert_ne!(altered, wire, "the falsifier must alter the wire");
            let request = CadAuthoredSelectionRequestDto {
                protocol: CAD_AUTHORED_PROTOCOL.to_owned(),
                graph_digest: v2.graph_digest.clone(),
                canonical_graph_hex: encode_hex(altered.as_bytes()),
                handle_hex: cut_wall.handle_hex.clone(),
            };
            assert!(resolve_selection(&request).is_err());
        }
    }

    #[test]
    fn request_wire_rejects_unknown_fields_and_owner_rejects_bad_scalars() {
        let accepted = serde_json::json!({
            "protocol": CAD_AUTHORED_PROTOCOL,
            "sketch": {"xBoundsM": [0.0, 1.0], "yBoundsM": [0.0, 1.0], "planeZM": 0.0},
            "extrusionDepthM": 1.0,
            "requestedModelingToleranceM": 1.0e-9,
            "cut": null,
        });
        assert!(serde_json::from_value::<CadAuthoredBuildRequestDto>(accepted).is_ok());

        let widened = serde_json::json!({
            "protocol": CAD_AUTHORED_PROTOCOL,
            "sketch": {"xBoundsM": [0.0, 1.0], "yBoundsM": [0.0, 1.0], "planeZM": 0.0},
            "extrusionDepthM": 1.0,
            "requestedModelingToleranceM": 1.0e-9,
            "cut": null,
            "operations": [],
        });
        assert!(serde_json::from_value::<CadAuthoredBuildRequestDto>(widened).is_err());

        // Invalid-scalar spellings copied from the frozen rectangle case; the
        // owner, not this module, rejects each one.
        for (depth, tolerance) in [(0.0, 1.0e-9), (-1.0, 1.0e-9), (1.0, 0.0), (1.0, -1.0)] {
            let mut request = v1_request();
            request.extrusion_depth_m = depth;
            request.requested_modeling_tolerance_m = tolerance;
            assert!(build_graph(&request).is_err());
        }
        // Degenerate rectangle spelling copied from the frozen rectangle case.
        let mut request = v1_request();
        request.sketch.x_bounds_m = [1.0, 1.0];
        assert!(build_graph(&request).is_err());
        // Cut outside the rectangle, copied from the frozen cut case.
        let mut request = v2_request();
        request.cut = Some(CadAuthoredCutRequestDto {
            center_m: [0.10, 0.0],
            radius_m: V2_CUT_RADIUS_M,
            requested_boolean_tolerance_m: V2_CUT_TOLERANCE_M,
        });
        assert!(build_graph(&request).is_err());
    }
}
