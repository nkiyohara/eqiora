//! Least-privilege native adapter for Eqiora Studio.
//!
//! This crate contains transport projection only. Canonical compilation,
//! transaction replay, graph commit, artifact reconstruction, and reference
//! execution remain in the public `eqiora` facade.

mod cad;
mod cad_authored;
mod cad_authored_export;
mod compile;
mod dc_motor_demo;

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use eqiora::api::{ModelDocument, ValueEditPlan};
use eqiora::graph::EdgeKind;
use eqiora::kernel::{
    ActivationKind, ClockKind, ConnectionSemantics, DomainKind, KernelNode, PortPayload,
    RepresentationKind, SignalDirection,
};
use eqiora::{Diagnostic, RawId, Severity};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

const PROTOCOL: &str = "eqiora.studio.bridge/v5";
const MAX_DOCUMENTS: usize = 32;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueEditPreviewRequest {
    protocol: String,
    digest: String,
    target_id: String,
    value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueEditCommitRequest {
    protocol: String,
    digest: String,
    target_id: String,
    value: f64,
    plan_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CadProjectionRequest {
    protocol: String,
    model_digest: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeEnvelope<T> {
    protocol: &'static str,
    result: Option<T>,
    diagnostics: Vec<DiagnosticDto>,
}

impl<T> BridgeEnvelope<T> {
    fn success(result: T) -> Self {
        Self {
            protocol: PROTOCOL,
            result: Some(result),
            diagnostics: Vec::new(),
        }
    }

    fn failure(diagnostics: Vec<DiagnosticDto>) -> Self {
        Self {
            protocol: PROTOCOL,
            result: None,
            diagnostics,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDto {
    source: &'static str,
    severity: &'static str,
    code: String,
    message: String,
    graph_path: Option<String>,
    span: Option<SourceSpanDto>,
}

type ProjectionError = Box<DiagnosticDto>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSpanDto {
    file: String,
    start: u32,
    end: u32,
}

impl From<Diagnostic> for DiagnosticDto {
    fn from(diagnostic: Diagnostic) -> Self {
        let severity = match diagnostic.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        Self {
            source: "kernel",
            severity,
            code: diagnostic.code().to_string(),
            message: diagnostic.message().to_owned(),
            graph_path: diagnostic.graph_path().map(ToString::to_string),
            span: diagnostic.source_span().map(|span| SourceSpanDto {
                file: span.file.clone(),
                start: span.start,
                end: span.end,
            }),
        }
    }
}

fn studio_error(code: &str, message: impl Into<String>) -> DiagnosticDto {
    DiagnosticDto {
        source: "studio",
        severity: "error",
        code: code.to_owned(),
        message: message.into(),
        graph_path: None,
        span: None,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentProjection {
    protocol: &'static str,
    digest: String,
    revision: u64,
    model_id: String,
    nodes: Vec<NodeDto>,
    edges: Vec<EdgeDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeDto {
    id: String,
    name: String,
    kind: &'static str,
    summary: String,
    dimension: Option<String>,
    value: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EdgeDto {
    id: String,
    source: String,
    target: String,
    kind: &'static str,
    label: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValueEditPlanDto {
    protocol: &'static str,
    key: String,
    base_digest: String,
    base_revision: u64,
    target_id: String,
    before: QuantityDto,
    after: QuantityDto,
    transaction_digest: String,
}

#[derive(Debug, Serialize)]
struct QuantityDto {
    value: f64,
    dimension: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValueEditEvidenceDto {
    plan: ValueEditPlanDto,
    result_digest: String,
    result_revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValueEditResultDto {
    protocol: &'static str,
    document: DocumentProjection,
    evidence: ValueEditEvidenceDto,
}

impl From<&ValueEditPlan> for ValueEditPlanDto {
    fn from(plan: &ValueEditPlan) -> Self {
        let before = plan.before();
        let after = plan.after();
        Self {
            protocol: PROTOCOL,
            key: plan.key(),
            base_digest: plan.base_digest().to_owned(),
            base_revision: plan.base_revision().0,
            target_id: plan.target().to_string(),
            before: QuantityDto {
                value: before.value(),
                dimension: before.dim().to_string(),
            },
            after: QuantityDto {
                value: after.value(),
                dimension: after.dim().to_string(),
            },
            transaction_digest: plan.transaction_digest().to_owned(),
        }
    }
}

#[derive(Debug, Default)]
struct DocumentCache {
    documents: BTreeMap<String, ModelDocument>,
    active_lineage: VecDeque<String>,
}

impl DocumentCache {
    fn reset(&mut self, digest: String, document: ModelDocument) {
        self.documents.clear();
        self.active_lineage.clear();
        self.active_lineage.push_back(digest.clone());
        self.documents.insert(digest, document);
    }

    fn insert_child(
        &mut self,
        base_digest: &str,
        child_digest: String,
        child: ModelDocument,
    ) -> bool {
        let Some(base_index) = self
            .active_lineage
            .iter()
            .position(|digest| digest == base_digest)
        else {
            return false;
        };
        while self.active_lineage.len() > base_index + 1 {
            if let Some(abandoned) = self.active_lineage.pop_back() {
                self.documents.remove(&abandoned);
            }
        }
        self.active_lineage.push_back(child_digest.clone());
        self.documents.insert(child_digest, child);
        while self.active_lineage.len() > MAX_DOCUMENTS {
            if let Some(oldest) = self.active_lineage.pop_front() {
                self.documents.remove(&oldest);
            }
        }
        true
    }

    fn get(&self, digest: &str) -> Option<ModelDocument> {
        self.documents.get(digest).cloned()
    }

    #[cfg(test)]
    fn contains(&self, digest: &str) -> bool {
        self.documents.contains_key(digest)
    }
}

#[derive(Debug)]
struct AppState {
    documents: Mutex<DocumentCache>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            documents: Mutex::new(DocumentCache::default()),
        }
    }
}

#[tauri::command]
fn preview_cad_box(
    request: CadProjectionRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<cad::CadProjectionDto> {
    if request.protocol != cad::CAD_PROTOCOL {
        return BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "unsupported Studio CAD payload protocol",
        )]);
    }
    let document = match load_document(&state, &request.model_digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    match cad::project(&document) {
        Ok(projection) => BridgeEnvelope::success(projection),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn select_cad_entity(
    request: cad::CadSelectionRequestDto,
    state: State<'_, AppState>,
) -> BridgeEnvelope<cad::CadSelectionDto> {
    let document = match load_document(&state, &request.model_digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    match cad::select(&document, &request) {
        Ok(selection) => BridgeEnvelope::success(selection),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn build_cad_authored_graph(
    request: cad_authored::CadAuthoredBuildRequestDto,
) -> BridgeEnvelope<cad_authored::CadAuthoredProjectionDto> {
    match cad_authored::build_graph(&request) {
        Ok(projection) => BridgeEnvelope::success(projection),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn resolve_cad_authored_face(
    request: cad_authored::CadAuthoredSelectionRequestDto,
) -> BridgeEnvelope<cad_authored::CadAuthoredSelectionDto> {
    match cad_authored::resolve_selection(&request) {
        Ok(selection) => BridgeEnvelope::success(selection),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn render_cad_authored_python(
    request: cad_authored_export::CadAuthoredExportRequestDto,
) -> BridgeEnvelope<cad_authored_export::CadAuthoredExportRenderDto> {
    match cad_authored_export::render_export(&request) {
        Ok(rendering) => BridgeEnvelope::success(rendering),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

/// Validate and render before exposing the native dialog. The client sends
/// neither source nor a path; only the dialog-selected filesystem path ever
/// reaches the private write helper.
#[tauri::command]
async fn save_cad_authored_python(
    request: cad_authored_export::CadAuthoredExportRequestDto,
    app: AppHandle,
) -> BridgeEnvelope<cad_authored_export::CadAuthoredExportSaveDto> {
    if let Err(diagnostic) = cad_authored_export::render_export(&request) {
        return BridgeEnvelope::failure(vec![diagnostic.into()]);
    }
    let (filter_name, extensions) = cad_authored_export::CAD_AUTHORED_EXPORT_DIALOG_FILTER;
    let dialog_path = app
        .dialog()
        .file()
        .set_file_name(cad_authored_export::CAD_AUTHORED_EXPORT_FILE_NAME)
        .add_filter(filter_name, extensions)
        .blocking_save_file();
    let dialog_path = match dialog_path.map(|path| path.into_path()).transpose() {
        Ok(path) => path,
        Err(_) => {
            return BridgeEnvelope::failure(vec![studio_error(
                "ST0003",
                "the native save dialog returned a non-filesystem location",
            )]);
        }
    };
    match cad_authored_export::save_export(&request, dialog_path.as_deref()) {
        Ok(outcome) => BridgeEnvelope::success(outcome),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn preview_value_edit(
    request: ValueEditPreviewRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<ValueEditPlanDto> {
    if let Err(diagnostic) = validate_value_edit_controls(
        &request.protocol,
        &request.digest,
        &request.target_id,
        request.value,
    ) {
        return BridgeEnvelope::failure(vec![*diagnostic]);
    }
    let document = match load_document(&state, &request.digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let target = match resolve_target(&document, &request.target_id) {
        Ok(target) => target,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    match document.preview_value_edit(target, request.value) {
        Ok(plan) => BridgeEnvelope::success((&plan).into()),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn commit_value_edit(
    request: ValueEditCommitRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<ValueEditResultDto> {
    if let Err(diagnostic) = validate_value_edit_controls(
        &request.protocol,
        &request.digest,
        &request.target_id,
        request.value,
    ) {
        return BridgeEnvelope::failure(vec![*diagnostic]);
    }
    if request.plan_key.is_empty() || request.plan_key.len() > 256 {
        return BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "value-edit plan key must contain 1 to 256 UTF-8 bytes",
        )]);
    }
    let document = match load_document(&state, &request.digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let target = match resolve_target(&document, &request.target_id) {
        Ok(target) => target,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let plan = match document.preview_value_edit(target, request.value) {
        Ok(plan) => plan,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![diagnostic.into()]),
    };
    if request.plan_key != plan.key() {
        return BridgeEnvelope::failure(vec![studio_error(
            "ST0006",
            "value edit no longer matches the accepted transaction preview; preview it again",
        )]);
    }
    let result = match document.commit_value_edit(plan) {
        Ok(result) => result,
        Err(diagnostics) => {
            return BridgeEnvelope::failure(diagnostics.into_iter().map(Into::into).collect());
        }
    };
    let result_digest = result.result_digest().to_owned();
    let evidence = ValueEditEvidenceDto {
        plan: result.plan().into(),
        result_digest: result_digest.clone(),
        result_revision: result.result_revision().0,
    };
    let projection = match project_document(result.document(), result_digest.clone()) {
        Ok(projection) => projection,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let child = result.into_document();
    match state.documents.lock() {
        Ok(mut documents) => {
            if !documents.insert_child(&request.digest, result_digest, child) {
                return BridgeEnvelope::failure(vec![studio_error(
                    "ST0004",
                    "value-edit base revision left the active Studio lineage",
                )]);
            }
        }
        Err(_) => {
            return BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native document cache is unavailable",
            )]);
        }
    }
    BridgeEnvelope::success(ValueEditResultDto {
        protocol: PROTOCOL,
        document: projection,
        evidence,
    })
}

fn load_document(
    state: &State<'_, AppState>,
    digest: &str,
) -> Result<ModelDocument, ProjectionError> {
    let document = state
        .documents
        .lock()
        .map_err(|_| {
            Box::new(studio_error(
                "ST0001",
                "native document cache is unavailable",
            ))
        })?
        .get(digest);
    document.ok_or_else(|| {
        Box::new(studio_error(
            "ST0004",
            "the requested canonical revision is not loaded; compile it again",
        ))
    })
}

fn validate_value_edit_controls(
    protocol: &str,
    digest: &str,
    target_id: &str,
    value: f64,
) -> Result<(), ProjectionError> {
    validate_protocol_and_digest(protocol, digest)?;
    if target_id.is_empty() || target_id.len() > 128 {
        return Err(Box::new(studio_error(
            "ST0002",
            "value-edit target ID must contain 1 to 128 UTF-8 bytes",
        )));
    }
    if !value.is_finite() {
        return Err(Box::new(studio_error(
            "ST0002",
            "value edit requires one finite coherent-SI scalar",
        )));
    }
    Ok(())
}

fn validate_protocol_and_digest(protocol: &str, digest: &str) -> Result<(), ProjectionError> {
    if protocol != PROTOCOL {
        return Err(Box::new(studio_error(
            "ST0002",
            "unsupported Studio bridge protocol",
        )));
    }
    if digest.len() < 16 || digest.len() > 128 {
        return Err(Box::new(studio_error(
            "ST0002",
            "model digest must contain 16 to 128 UTF-8 bytes",
        )));
    }
    Ok(())
}

fn resolve_target(document: &ModelDocument, target_id: &str) -> Result<RawId, ProjectionError> {
    document
        .program()
        .nodes()
        .map(KernelNode::id)
        .find(|id| id.to_string() == target_id)
        .ok_or_else(|| {
            Box::new(studio_error(
                "ST0004",
                "value-edit target is outside the requested canonical revision",
            ))
        })
}

fn project_document(
    document: &ModelDocument,
    digest: String,
) -> Result<DocumentProjection, ProjectionError> {
    let mut preferred_names = BTreeMap::<RawId, String>::new();
    for (name, &id) in document.aliases() {
        preferred_names.entry(id).or_insert_with(|| name.clone());
    }
    let nodes = document
        .program()
        .nodes()
        .map(|node| project_node(document, node, &preferred_names))
        .collect::<Result<Vec<_>, _>>()?;
    let edges = document
        .program()
        .edges()
        .iter()
        .map(|edge| {
            let (kind, label) = edge_contract(edge.kind())?;
            let source = edge.from().to_string();
            let target = edge.to().to_string();
            Ok(EdgeDto {
                id: format!("{source}→{target}:{kind}"),
                source,
                target,
                kind,
                label,
            })
        })
        .collect::<Result<Vec<_>, ProjectionError>>()?;
    Ok(DocumentProjection {
        protocol: PROTOCOL,
        digest,
        revision: document.program().revision().0,
        model_id: document.program().model().erase().to_string(),
        nodes,
        edges,
    })
}

fn project_node(
    document: &ModelDocument,
    node: &KernelNode,
    names: &BTreeMap<RawId, String>,
) -> Result<NodeDto, ProjectionError> {
    let id = node.id();
    let name = names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("{} {}", kind_title(node), id.ulid()));
    let (kind, summary, dimension, value) = match node {
        KernelNode::Domain(definition) => (
            "domain",
            match definition.kind() {
                DomainKind::Abstract => "Abstract continuous domain".to_owned(),
                DomainKind::CartesianBox { .. } => {
                    let bounds = document
                        .program()
                        .resolved_cartesian_bounds(definition.id())
                        .map_err(|diagnostic| Box::new(diagnostic.into()))?;
                    format!("{}D Cartesian continuous domain", bounds.len())
                }
                DomainKind::CartesianBoundary { axis, side } => {
                    format!("Cartesian boundary · axis {axis} · {side:?}")
                }
                DomainKind::ScalarPhysical {
                    across_dimension,
                    through_dimension,
                } => format!(
                    "Scalar physical domain · across {across_dimension} · through {through_dimension}"
                ),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::Representation(definition) => (
            "representation",
            match definition.kind() {
                RepresentationKind::Abstract => "Abstract field representation".to_owned(),
                RepresentationKind::Continuum => "Continuous field representation".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::Field(definition) => (
            "field",
            if definition.initial().is_some() {
                "Scalar field with an initial value".to_owned()
            } else {
                "Scalar field requiring execution input".to_owned()
            },
            Some(definition.dimension().to_string()),
            document
                .program()
                .value(id)
                .map(|quantity| quantity.value()),
        ),
        KernelNode::Parameter(definition) => (
            "parameter",
            "Canonical model parameter".to_owned(),
            Some(definition.value().dim().to_string()),
            document.program().value(id).map_or_else(
                || Some(definition.value().value()),
                |quantity| Some(quantity.value()),
            ),
        ),
        KernelNode::Port(definition) => {
            let (summary, dimension) = match definition.payload() {
                PortPayload::Signal {
                    direction: SignalDirection::Input,
                    dimension,
                } => (
                    "Causal signal input".to_owned(),
                    Some(dimension.to_string()),
                ),
                PortPayload::Signal {
                    direction: SignalDirection::Output,
                    dimension,
                } => (
                    "Causal signal output".to_owned(),
                    Some(dimension.to_string()),
                ),
                PortPayload::ConservingMarker { dimension } => (
                    "Structural conserving marker".to_owned(),
                    Some(dimension.to_string()),
                ),
                PortPayload::ScalarPhysical { domain } => {
                    let domain = domain.erase();
                    let domain_name = names
                        .get(&domain)
                        .cloned()
                        .unwrap_or_else(|| format!("domain {}", domain.ulid()));
                    (
                        format!("Scalar physical conserving port · {domain_name}"),
                        None,
                    )
                }
                _ => return Err(unsupported_node_contract()),
            };
            ("port", summary, dimension, None)
        }
        KernelNode::Relation(definition) => (
            "relation",
            format!(
                "{} implicit residual{} · {} expression operations",
                definition.residuals().roots().len(),
                if definition.residuals().roots().len() == 1 {
                    ""
                } else {
                    "s"
                },
                definition.residuals().nodes().len()
            ),
            None,
            None,
        ),
        KernelNode::Activation(definition) => (
            "activation",
            match definition.kind() {
                ActivationKind::Continuous => "Continuous activation".to_owned(),
                ActivationKind::Periodic => "Periodic activation".to_owned(),
                ActivationKind::Event { direction, .. } => {
                    format!("Zero-crossing event · {direction:?}")
                }
                ActivationKind::Guard { .. } => "Guarded activation".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::Connection(definition) => (
            "connection",
            match definition.semantics() {
                ConnectionSemantics::Signal => "Causal signal connection".to_owned(),
                ConnectionSemantics::Conserving => "Acausal conserving connection".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::ClockDomain(definition) => (
            "clock-domain",
            match definition.kind() {
                ClockKind::Continuous => "Continuous model time".to_owned(),
                ClockKind::Periodic { period, phase } => format!(
                    "Periodic model time · {}/{} s · phase {}/{} s",
                    period.numerator(),
                    period.denominator(),
                    phase.numerator(),
                    phase.denominator()
                ),
                ClockKind::Aperiodic => "Aperiodic semantic clock".to_owned(),
                ClockKind::Inherited => "Inherited semantic clock".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        _ => {
            return Err(unsupported_node_contract());
        }
    };
    Ok(NodeDto {
        id: id.to_string(),
        name,
        kind,
        summary,
        dimension,
        value,
    })
}

fn unsupported_node_contract() -> ProjectionError {
    Box::new(studio_error(
        "ST0003",
        "the native adapter does not support a new Semantic Kernel node contract",
    ))
}

fn kind_title(node: &KernelNode) -> &'static str {
    match node {
        KernelNode::Domain(_) => "Domain",
        KernelNode::Representation(_) => "Representation",
        KernelNode::Field(_) => "Field",
        KernelNode::Parameter(_) => "Parameter",
        KernelNode::Port(_) => "Port",
        KernelNode::Relation(_) => "Relation",
        KernelNode::Activation(_) => "Activation",
        KernelNode::Connection(_) => "Connection",
        KernelNode::ClockDomain(_) => "Clock domain",
        _ => "Entity",
    }
}

fn edge_contract(kind: EdgeKind) -> Result<(&'static str, &'static str), ProjectionError> {
    match kind {
        EdgeKind::DefinedOn => Ok(("defined-on", "defined on")),
        EdgeKind::AppliesOn => Ok(("applies-on", "applies on")),
        EdgeKind::BoundaryOf => Ok(("boundary-of", "boundary of")),
        EdgeKind::DependsOn => Ok(("depends-on", "depends on")),
        EdgeKind::HasPort => Ok(("has-port", "has port")),
        EdgeKind::Activates => Ok(("activates", "activates")),
        EdgeKind::Connects => Ok(("connects", "connects")),
        EdgeKind::ClockedBy => Ok(("clocked-by", "clocked by")),
        _ => Err(Box::new(studio_error(
            "ST0003",
            "the native adapter received an unsupported model edge kind",
        ))),
    }
}

/// Launch the native Studio shell.
pub fn run() {
    use compile::compile_model;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            compile_model,
            preview_cad_box,
            select_cad_entity,
            build_cad_authored_graph,
            resolve_cad_authored_face,
            render_cad_authored_python,
            save_cad_authored_python,
            preview_value_edit,
            commit_value_edit,
            dc_motor_demo::run_dc_motor_demo,
        ])
        .run(tauri::generate_context!())
        .expect("failed to launch Eqiora Studio");
}

#[cfg(test)]
mod tests {
    use super::{
        DocumentCache, MAX_DOCUMENTS, ModelDocument, ValueEditPlanDto, project_document,
        project_node,
    };
    use eqiora::entity::kinds;
    use eqiora::kernel::{DomainDef, KernelNode, PortDef};
    use eqiora::{DimExponents, Id};
    use std::collections::BTreeMap;

    const SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

    #[test]
    fn projection_is_deterministic_and_semantically_read_only() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let digest = document.digest().unwrap();
        let projection = project_document(&document, digest.clone()).unwrap();
        assert_eq!(projection.digest, digest);
        assert_eq!(projection.nodes.len(), 4);
        assert_eq!(projection.edges.len(), 3);
        assert!(projection.nodes.iter().any(|node| node.name == "x"));
        assert_eq!(document.digest().unwrap(), projection.digest);
    }

    #[test]
    fn projection_preserves_nominal_scalar_physical_port_meaning() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let domain = Id::<kinds::Domain>::new();
        let port = Id::<kinds::Port>::new();
        let across_dimension = DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -1,
            ..DimExponents::DIMENSIONLESS
        };
        let through_dimension = DimExponents {
            current: 1,
            ..DimExponents::DIMENSIONLESS
        };
        let domain_node = KernelNode::from(DomainDef::scalar_physical(
            domain,
            across_dimension,
            through_dimension,
        ));
        let port_node = KernelNode::from(PortDef::scalar_physical(port, domain));
        let names = BTreeMap::from([
            (domain.erase(), "electrical".to_owned()),
            (port.erase(), "positive".to_owned()),
        ]);

        let domain_dto = project_node(&document, &domain_node, &names).unwrap();
        assert!(domain_dto.summary.contains(&across_dimension.to_string()));
        assert!(domain_dto.summary.contains(&through_dimension.to_string()));
        let port_dto = project_node(&document, &port_node, &names).unwrap();
        assert_eq!(port_dto.name, "positive");
        assert_eq!(
            port_dto.summary,
            "Scalar physical conserving port · electrical"
        );
        assert_eq!(port_dto.dimension, None);
    }

    #[test]
    fn document_cache_is_a_bounded_active_lineage() {
        let root = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let mut cache = DocumentCache::default();
        cache.reset("root".to_owned(), root.clone());

        let mut base = "root".to_owned();
        for index in 0..MAX_DOCUMENTS + 4 {
            let child = format!("child-{index}");
            assert!(cache.insert_child(&base, child.clone(), root.clone()));
            base = child;
        }
        assert_eq!(cache.documents.len(), MAX_DOCUMENTS);
        assert!(!cache.contains("root"));
        assert!(cache.contains(&base));

        assert!(cache.insert_child("child-4", "branch".to_owned(), root));
        assert!(cache.contains("child-4"));
        assert!(cache.contains("branch"));
        assert!(!cache.contains(&base));
        assert_eq!(
            cache.active_lineage.back().map(String::as_str),
            Some("branch")
        );
    }

    #[test]
    fn value_edit_projection_retains_transaction_identity_and_revision_lineage() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let rate = document.aliases()["rate"];
        let plan = document.preview_value_edit(rate, 2.0).unwrap();
        let dto = ValueEditPlanDto::from(&plan);

        assert_eq!(dto.key, plan.key());
        assert_eq!(dto.base_digest, document.digest().unwrap());
        assert_eq!(dto.base_revision, 1);
        assert_eq!(dto.target_id, rate.to_string());
        assert_eq!(dto.before.value, 1.0);
        assert_eq!(dto.after.value, 2.0);
        assert_eq!(dto.before.dimension, dto.after.dimension);
        assert_eq!(dto.transaction_digest, plan.transaction_digest());

        let result = document.commit_value_edit(plan).unwrap();
        let child = project_document(result.document(), result.result_digest().to_owned()).unwrap();
        assert_eq!(child.revision, 2);
        assert_eq!(
            child
                .nodes
                .iter()
                .find(|node| node.name == "rate")
                .and_then(|node| node.value),
            Some(2.0)
        );
        assert_eq!(document.program().value(rate).unwrap().value(), 1.0);
    }
}
