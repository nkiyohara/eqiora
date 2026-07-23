//! Studio transport for the shared compile/check control contract.
//!
//! Canonical compilation is owned by `eqiora::control`. This module only
//! decodes the closed control request, projects an accepted immutable Model
//! for Studio, and admits that Model to the bounded local cache.

use eqiora::control::{
    CompileOutcomeV1, CompileRequestV1, CompileResponseV1, ControlDiagnosticSourceV1,
    ControlDiagnosticV1, ControlSeverityV1, execute_compile_v1,
};
use serde::Serialize;
use tauri::State;

use super::{
    AppState, DiagnosticDto, DocumentProjection, PROTOCOL, SourceSpanDto, project_document,
};

/// Native adapter response. The shared control response remains intact; the
/// Studio projection is an explicitly separate, read-only adapter result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompileCommandEnvelope {
    protocol: &'static str,
    control: Option<CompileResponseV1>,
    projection: Option<DocumentProjection>,
    diagnostics: Vec<DiagnosticDto>,
}

impl CompileCommandEnvelope {
    fn control_rejection(diagnostic: ControlDiagnosticV1) -> Self {
        Self {
            protocol: PROTOCOL,
            control: None,
            projection: None,
            diagnostics: vec![control_diagnostic(diagnostic)],
        }
    }

    fn adapter_rejection(control: CompileResponseV1, diagnostic: DiagnosticDto) -> Self {
        Self {
            protocol: PROTOCOL,
            control: Some(control),
            projection: None,
            diagnostics: vec![diagnostic],
        }
    }
}

#[tauri::command]
pub(crate) fn compile_model(
    request_json: String,
    state: State<'_, AppState>,
) -> CompileCommandEnvelope {
    compile_request(&request_json, state.inner())
}

fn compile_request(request_json: &str, state: &AppState) -> CompileCommandEnvelope {
    // Tauri deliberately admits encoded JSON here. The shared decoder owns
    // encoded-size and unknown-field checks before allocating source content
    // or entering compilation.
    let request = match CompileRequestV1::from_json(request_json.as_bytes()) {
        Ok(request) => request,
        Err(diagnostic) => return CompileCommandEnvelope::control_rejection(diagnostic),
    };

    let (control, document) = execute_compile_v1(&request).into_parts();
    let Some(document) = document else {
        debug_assert!(matches!(
            control.outcome(),
            CompileOutcomeV1::Rejected { .. }
        ));
        return CompileCommandEnvelope {
            protocol: PROTOCOL,
            control: Some(control),
            projection: None,
            diagnostics: Vec::new(),
        };
    };

    let digest = match control.outcome() {
        CompileOutcomeV1::Accepted { model } => model.digest().to_owned(),
        CompileOutcomeV1::Rejected { .. } => {
            unreachable!("a rejected control execution cannot expose a Model document")
        }
    };
    let projection = match project_document(&document, digest.clone(), state.host_worker_budget) {
        Ok(projection) => projection,
        Err(diagnostic) => return CompileCommandEnvelope::adapter_rejection(control, *diagnostic),
    };

    // Cache admission is last. Invalid requests, compiler rejection, and
    // projection rejection leave the prior immutable lineage untouched.
    match state.documents.lock() {
        Ok(mut documents) => documents.reset(digest, document),
        Err(_) => {
            return CompileCommandEnvelope::adapter_rejection(
                control,
                super::studio_error("ST0001", "native document cache is unavailable"),
            );
        }
    }
    CompileCommandEnvelope {
        protocol: PROTOCOL,
        control: Some(control),
        projection: Some(projection),
        diagnostics: Vec::new(),
    }
}

fn control_diagnostic(diagnostic: ControlDiagnosticV1) -> DiagnosticDto {
    DiagnosticDto {
        source: match diagnostic.source() {
            ControlDiagnosticSourceV1::Control => "control",
            ControlDiagnosticSourceV1::Kernel => "kernel",
        },
        severity: match diagnostic.severity() {
            ControlSeverityV1::Error => "error",
            ControlSeverityV1::Warning => "warning",
            ControlSeverityV1::Note => "note",
        },
        code: diagnostic.code().to_owned(),
        message: diagnostic.message().to_owned(),
        graph_path: diagnostic.graph_path().map(|path| path.join("/")),
        span: diagnostic.span().map(|span| SourceSpanDto {
            file: span.file().to_owned(),
            start: span.start(),
            end: span.end(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use eqiora::compatibility::ExactModelCodec;
    use eqiora::control::{COMPILE_COMMAND_V1, COMPILE_FEATURE_V1, CONTROL_PROTOCOL_V1};
    use serde_json::json;

    use super::{AppState, compile_request};

    const SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;
    const ACCEPTED_REQUEST: &str = include_str!(
        "../../../verify/interfaces/control-plane-compile-check/models/accepted-v1.json"
    );
    const REJECTED_SOURCE_REQUEST: &str = include_str!(
        "../../../verify/interfaces/control-plane-compile-check/models/rejected-source-v1.json"
    );
    const UNSUPPORTED_PROTOCOL_REQUEST: &str = include_str!(
        "../../../verify/interfaces/control-plane-compile-check/models/unsupported-protocol-v1.json"
    );

    fn request(request_id: &str, source: &str) -> String {
        let wire = ExactModelCodec::CURRENT.as_str();
        json!({
            "protocol": CONTROL_PROTOCOL_V1,
            "command": COMPILE_COMMAND_V1,
            "requestId": request_id,
            "requiredFeatures": [COMPILE_FEATURE_V1, format!("model-wire/{wire}")],
            "modelWire": wire,
            "filename": "model.eqi",
            "source": source,
        })
        .to_string()
    }

    #[test]
    fn accepted_control_execution_is_projected_and_cached_once() {
        let state = AppState::default();
        let response = compile_request(ACCEPTED_REQUEST, &state);
        let projection = response.projection.expect("Studio projection");
        let control = response.control.expect("shared control response");

        assert_eq!(control.request_id(), "shared-accepted-v1");
        assert!(response.diagnostics.is_empty());
        assert!(state.documents.lock().unwrap().contains(&projection.digest));
    }

    #[test]
    fn every_rejection_preserves_the_previously_admitted_lineage() {
        let state = AppState::default();
        let accepted = compile_request(&request("studio.compile:1", SOURCE), &state);
        let control = accepted
            .control
            .as_ref()
            .expect("accepted control response");
        assert_eq!(control.model_codec(), ExactModelCodec::CURRENT);
        let digest = accepted.projection.unwrap().digest;

        let mut unknown: serde_json::Value =
            serde_json::from_str(&request("studio.compile:2", SOURCE)).unwrap();
        unknown["unexpected"] = json!(true);
        let rejected = compile_request(&unknown.to_string(), &state);
        assert!(rejected.control.is_none());
        assert_eq!(rejected.diagnostics[0].source, "control");
        assert!(state.documents.lock().unwrap().contains(&digest));

        let rejected = compile_request(REJECTED_SOURCE_REQUEST, &state);
        assert!(matches!(
            rejected.control.as_ref().map(|response| response.outcome()),
            Some(eqiora::control::CompileOutcomeV1::Rejected { .. })
        ));
        assert!(rejected.projection.is_none());
        assert!(state.documents.lock().unwrap().contains(&digest));
    }

    #[test]
    fn unsupported_features_fail_before_cache_mutation() {
        let state = AppState::default();
        let accepted = compile_request(&request("studio.compile:1", SOURCE), &state);
        let digest = accepted.projection.unwrap().digest;

        let mut unsupported: serde_json::Value =
            serde_json::from_str(&request("studio.compile:2", SOURCE)).unwrap();
        unsupported["requiredFeatures"] = json!([COMPILE_FEATURE_V1, "model-wire/v3"]);
        let rejected = compile_request(&unsupported.to_string(), &state);

        assert!(rejected.control.is_none());
        assert_eq!(rejected.diagnostics[0].code, "EQ0001");
        assert!(state.documents.lock().unwrap().contains(&digest));

        let rejected = compile_request(UNSUPPORTED_PROTOCOL_REQUEST, &state);
        assert!(rejected.control.is_none());
        assert_eq!(rejected.diagnostics[0].code, "EQ0001");
        assert!(state.documents.lock().unwrap().contains(&digest));
    }
}
