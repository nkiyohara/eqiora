//! Studio projection for the bounded CAD semantic-selection slice.
//!
//! The native bridge does not invent CAD meaning. It rebuilds the public
//! Eqiora CAD plan from one exact Model revision and projects only stable
//! semantic identities to the client. In particular, neither Truck topology
//! objects nor face/renderer indices can enter a selection request.

use eqiora::api::{
    CadBoxIntentV1, CadBoxPlanV1, CadSelectionRequestV1, CadSemanticEntityKindV1,
    CadSemanticEntityV1, ModelDocument,
};
use eqiora::artifact::ArtifactDigest;
use eqiora::diagnostic::codes;
use eqiora::geometry::truck::TruckCadAdapterV1;
use eqiora::geometry::{
    AxisAlignedBox3, CadKernelAdapter, CadRepairDispositionV1, ConstrainedRectangleV1,
    StepLengthUnitV1,
};
use eqiora::kernel::{BoundarySide, DomainKind, KernelNode};
use eqiora::{Diagnostic, kinds};
use serde::{Deserialize, Serialize};

/// Protocol for the independently versioned CAD payload nested in the Studio
/// bridge envelope.
pub(super) const CAD_PROTOCOL: &str = "eqiora.studio.cad/v1";

const STEP_SOURCE: &[u8] =
    include_bytes!("../../../verify/geometry/cad-semantic-selection-box/models/outer-box-mm.step");
const BODY_ALIAS: &str = "body";
const ADMITTED_BODY_BOUNDS_M: [(f64, f64); 3] = [(-0.5, 0.5), (-0.5, 0.5), (-0.5, 0.5)];
const IMPORTED_STOCK_BOUNDS_M: [(f64, f64); 3] = [(-1.0, 1.0), (-1.0, 1.0), (-1.0, 1.0)];

/// Selection request emitted identically by the viewport and semantic table.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CadSelectionRequestDto {
    pub(super) protocol: String,
    pub(super) model_digest: String,
    pub(super) plan_key: String,
    pub(super) geometry_digest: String,
    pub(super) domain_id: String,
}

/// Complete presentation projection of one exact bounded CAD plan.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadProjectionDto {
    protocol: &'static str,
    plan_key: String,
    model_digest: String,
    geometry_digest: String,
    mesh_digest: String,
    design: CadDesignDto,
    build: CadBuildDto,
    vertices_m: Vec<[f64; 3]>,
    triangles: Vec<CadTriangleDto>,
    entities: Vec<CadEntityDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CadDesignDto {
    source_unit: &'static str,
    imported_stock_bounds_m: [(f64, f64); 3],
    sketch: CadSketchDto,
    extrusion: CadExtrusionDto,
    boolean: &'static str,
    result_bounds_m: [(f64, f64); 3],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CadSketchDto {
    x_bounds_m: (f64, f64),
    y_bounds_m: (f64, f64),
    plane_z_m: f64,
    remaining_degrees_of_freedom: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CadExtrusionDto {
    direction: &'static str,
    depth_m: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CadBuildDto {
    adapter: &'static str,
    adapter_version: &'static str,
    kernel: &'static str,
    kernel_version: &'static str,
    repair: &'static str,
    imported_stock: CadObservationDto,
    extruded_tool: CadObservationDto,
    intersection: CadObservationDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CadObservationDto {
    solid_count: usize,
    closed_shell_count: usize,
    planar_face_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CadTriangleDto {
    domain_id: String,
    vertex_indices: [usize; 3],
}

/// Presentation-only projection of a selectable semantic entity.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadEntityDto {
    domain_id: String,
    name: Option<String>,
    kind: &'static str,
    parent_domain_id: Option<String>,
    axis: Option<usize>,
    side: Option<&'static str>,
    mesh_entity_count: usize,
    relation_ids: Vec<String>,
    port_ids: Vec<String>,
}

/// Accepted selection returned only after exact plan, geometry, and Domain
/// identity have all been replayed by the native adapter.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadSelectionDto {
    protocol: &'static str,
    model_digest: String,
    plan_key: String,
    geometry_digest: String,
    domain_id: String,
    entity: CadEntityDto,
}

/// Rebuild and project the first bounded CAD plan for a loaded Model revision.
///
/// The workflow is deliberately applicable only to the registered fixed
/// reference fixture shape: alias `body`, exact `[-0.5, 0.5]^3` bounds, the
/// complete six-boundary family, and the embedded millimetre STEP stock.
pub(super) fn project(document: &ModelDocument) -> Result<CadProjectionDto, Diagnostic> {
    let plan = build_plan(document)?;
    project_plan(&plan)
}

/// Rebuild the same exact plan and resolve one modality-neutral semantic
/// selection request.
pub(super) fn select(
    document: &ModelDocument,
    request: &CadSelectionRequestDto,
) -> Result<CadSelectionDto, Diagnostic> {
    let plan = build_plan(document)?;
    if request.protocol != CAD_PROTOCOL {
        return Err(invalid_cad("unsupported Studio CAD payload protocol"));
    }
    if request.model_digest != plan.model_digest() {
        return Err(invalid_cad(
            "Studio CAD selection references a stale or foreign Model",
        ));
    }
    if request.plan_key != plan.key() {
        return Err(invalid_cad(
            "Studio CAD selection references a stale or foreign plan",
        ));
    }

    let geometry = ArtifactDigest::from_hex(request.geometry_digest.clone())?;
    let domain = plan
        .entities()
        .iter()
        .find(|entity| entity.domain().to_string() == request.domain_id)
        .map(CadSemanticEntityV1::domain)
        .ok_or_else(|| invalid_cad("Studio CAD selection Domain is absent from the exact plan"))?;
    let selection = plan.resolve_selection(&CadSelectionRequestV1::new(geometry, domain))?;
    Ok(CadSelectionDto {
        protocol: CAD_PROTOCOL,
        model_digest: plan.model_digest().to_owned(),
        plan_key: plan.key().to_owned(),
        geometry_digest: selection.geometry().as_str().to_owned(),
        domain_id: selection.entity().domain().to_string(),
        entity: project_entity(selection.entity()),
    })
}

fn build_plan(document: &ModelDocument) -> Result<CadBoxPlanV1, Diagnostic> {
    let body = document
        .aliases()
        .get(BODY_ALIAS)
        .copied()
        .and_then(|id| id.downcast::<kinds::Domain>())
        .ok_or_else(|| invalid_cad("Studio CAD v1 requires the exact Domain alias `body`"))?;
    let node = document
        .program()
        .node(body.erase())
        .ok_or_else(|| invalid_cad("Studio CAD body Domain is absent from the Model"))?;
    let KernelNode::Domain(definition) = node else {
        return Err(invalid_cad("Studio CAD alias `body` is not a Domain"));
    };
    let DomainKind::CartesianBox { .. } = definition.kind() else {
        return Err(invalid_cad("Studio CAD v1 requires one Cartesian box body"));
    };
    let bounds = document.program().resolved_cartesian_bounds(body)?;
    let actual_bounds = bounds
        .iter()
        .map(|bounds| (bounds.lower().value(), bounds.upper().value()))
        .collect::<Vec<_>>();
    if actual_bounds.as_slice() != ADMITTED_BODY_BOUNDS_M {
        return Err(invalid_cad(
            "Studio CAD v1 admits only the registered [-0.5, 0.5]^3 body",
        ));
    }

    let intent = CadBoxIntentV1::new(
        body,
        StepLengthUnitV1::Millimetre,
        AxisAlignedBox3::new(IMPORTED_STOCK_BOUNDS_M)?,
        ConstrainedRectangleV1::new(
            ADMITTED_BODY_BOUNDS_M[0],
            ADMITTED_BODY_BOUNDS_M[1],
            ADMITTED_BODY_BOUNDS_M[2].0,
        )?,
        ADMITTED_BODY_BOUNDS_M[2].1 - ADMITTED_BODY_BOUNDS_M[2].0,
        1.0e-12,
        1.0e-10,
        1.0e-10,
        0.05,
    );
    document.preview_cad_box(intent, &TruckCadAdapterV1, STEP_SOURCE)
}

fn project_plan(plan: &CadBoxPlanV1) -> Result<CadProjectionDto, Diagnostic> {
    let design = plan.design().design()?;
    let adapter = TruckCadAdapterV1;
    let adapter_identity = adapter.identity();
    let realization = adapter.realize_box_design(&design, STEP_SOURCE)?;
    let sketch = design.sketch();
    let intersection = realization.intersection();
    let repair = match intersection.repair() {
        CadRepairDispositionV1::None => "none",
    };

    Ok(CadProjectionDto {
        protocol: CAD_PROTOCOL,
        plan_key: plan.key().to_owned(),
        model_digest: plan.model_digest().to_owned(),
        geometry_digest: plan.geometry().digest()?.as_str().to_owned(),
        mesh_digest: plan.mesh().digest()?.as_str().to_owned(),
        design: CadDesignDto {
            source_unit: match design.source_length_unit() {
                StepLengthUnitV1::Metre => "metre",
                StepLengthUnitV1::Millimetre => "millimetre",
            },
            imported_stock_bounds_m: design.imported_stock().bounds_m(),
            sketch: CadSketchDto {
                x_bounds_m: sketch.x_bounds_m(),
                y_bounds_m: sketch.y_bounds_m(),
                plane_z_m: sketch.plane_z_m(),
                remaining_degrees_of_freedom: sketch.remaining_degrees_of_freedom(),
            },
            extrusion: CadExtrusionDto {
                direction: "positive-z",
                depth_m: design.extrusion_depth_m(),
            },
            boolean: "intersection",
            result_bounds_m: design.output().bounds_m(),
        },
        build: CadBuildDto {
            adapter: adapter_identity.adapter(),
            adapter_version: adapter_identity.adapter_version(),
            kernel: adapter_identity.kernel(),
            kernel_version: adapter_identity.kernel_version(),
            repair,
            imported_stock: project_observation(realization.imported_stock()),
            extruded_tool: project_observation(realization.extruded_tool()),
            intersection: project_observation(intersection),
        },
        vertices_m: plan.render().vertices_m().to_vec(),
        triangles: plan
            .render()
            .boundary_triangles()
            .iter()
            .map(|triangle| CadTriangleDto {
                domain_id: triangle.domain().to_string(),
                vertex_indices: triangle.vertex_indices(),
            })
            .collect(),
        entities: plan.entities().iter().map(project_entity).collect(),
    })
}

fn project_observation(observation: eqiora::geometry::CadBoxObservationV1) -> CadObservationDto {
    CadObservationDto {
        solid_count: observation.solid_count(),
        closed_shell_count: observation.closed_shell_count(),
        planar_face_count: observation.planar_face_count(),
    }
}

fn project_entity(entity: &CadSemanticEntityV1) -> CadEntityDto {
    let (axis, side) = entity.axis_side().map_or((None, None), |(axis, side)| {
        (
            Some(axis),
            Some(match side {
                BoundarySide::Lower => "lower",
                BoundarySide::Upper => "upper",
            }),
        )
    });
    CadEntityDto {
        domain_id: entity.domain().to_string(),
        name: entity.display_name().map(ToOwned::to_owned),
        kind: match entity.kind() {
            CadSemanticEntityKindV1::Body => "body",
            CadSemanticEntityKindV1::Boundary => "boundary",
        },
        parent_domain_id: entity.parent().map(|parent| parent.to_string()),
        axis,
        side,
        mesh_entity_count: entity.mesh_entities().len(),
        relation_ids: entity.relations().iter().map(ToString::to_string).collect(),
        port_ids: entity.ports().iter().map(ToString::to_string).collect(),
    }
}

fn invalid_cad(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[cfg(test)]
mod tests {
    use eqiora::api::ModelDocument;

    use super::*;

    const MODEL: &str = r#"model cad_semantic_selection {
  domain body = box(-0.5, 0.5, -0.5, 0.5, -0.5, 0.5);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  domain z_lower = boundary(body, axis = 2, side = lower);
  domain z_upper = boundary(body, axis = 2, side = upper);
  representation geometry_space = continuum;
  field marker on body as geometry_space: 1 = 0;
  relation selected_boundary continuous on x_upper { trace(marker) = 0; }
}"#;

    #[test]
    fn projection_and_selection_share_exact_semantic_identity() {
        let document = ModelDocument::compile("cad.eqi", MODEL).unwrap();
        let projection = project(&document).unwrap();
        let boundary = projection
            .entities
            .iter()
            .find(|entity| entity.name.as_deref() == Some("x_upper"))
            .unwrap();
        let request = CadSelectionRequestDto {
            protocol: CAD_PROTOCOL.to_owned(),
            model_digest: projection.model_digest.clone(),
            plan_key: projection.plan_key.clone(),
            geometry_digest: projection.geometry_digest.clone(),
            domain_id: boundary.domain_id.clone(),
        };
        let selection = select(&document, &request).unwrap();

        assert_eq!(selection.geometry_digest, projection.geometry_digest);
        assert_eq!(selection.domain_id, boundary.domain_id);
        assert_eq!(selection.entity.name.as_deref(), Some("x_upper"));
        assert_eq!(selection.entity.mesh_entity_count, 2);
        assert_eq!(selection.entity.relation_ids.len(), 1);
        assert!(selection.entity.port_ids.is_empty());
    }

    #[test]
    fn selection_fails_closed_for_each_substituted_identity() {
        let document = ModelDocument::compile("cad.eqi", MODEL).unwrap();
        let projection = project(&document).unwrap();
        let domain_id = projection.entities[0].domain_id.clone();
        let mut request = CadSelectionRequestDto {
            protocol: CAD_PROTOCOL.to_owned(),
            model_digest: projection.model_digest,
            plan_key: projection.plan_key,
            geometry_digest: projection.geometry_digest,
            domain_id,
        };

        request.protocol = "eqiora.studio.cad/v2".to_owned();
        assert!(select(&document, &request).is_err());
        request.protocol = CAD_PROTOCOL.to_owned();
        request.model_digest = "0".repeat(64);
        assert!(select(&document, &request).is_err());
        request.model_digest = document.digest().unwrap();
        request.plan_key = "0".repeat(64);
        assert!(select(&document, &request).is_err());
        request.plan_key = project(&document).unwrap().plan_key;
        request.geometry_digest = "0".repeat(64);
        assert!(select(&document, &request).is_err());
        request.geometry_digest = project(&document).unwrap().geometry_digest;
        request.domain_id = "01AAAAAAAAAAAAAAAAAAAAAAAA".to_owned();
        assert!(select(&document, &request).is_err());
    }

    #[test]
    fn projection_does_not_widen_beyond_the_registered_body() {
        let source = MODEL.replacen(
            "box(-0.5, 0.5, -0.5, 0.5, -0.5, 0.5)",
            "box(-0.6, 0.6, -0.5, 0.5, -0.5, 0.5)",
            1,
        );
        let document = ModelDocument::compile("unsupported-cad.eqi", &source).unwrap();
        assert!(project(&document).is_err());
    }
}
