//! Presentation-only command for the verified mixed-boundary elasticity example.

mod core;

use serde::Deserialize;

use self::core::{StructuralDemoResult, prepare_demo};
use super::{BridgeEnvelope, PROTOCOL, studio_error};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StructuralDemoRequest {
    protocol: String,
}

#[tauri::command]
pub(super) async fn run_structural_demo(
    request: StructuralDemoRequest,
) -> Result<BridgeEnvelope<StructuralDemoResult>, ()> {
    if request.protocol != PROTOCOL {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "unsupported Studio structural-demo request protocol",
        )]));
    }
    match tauri::async_runtime::spawn_blocking(prepare_demo).await {
        Ok(Ok(result)) => Ok(BridgeEnvelope::success(result)),
        Ok(Err(message)) => Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0003", message,
        )])),
        Err(error) => Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0001",
            format!("native structural-demo worker failed: {error}"),
        )])),
    }
}
