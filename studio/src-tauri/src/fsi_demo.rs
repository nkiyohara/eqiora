//! Presentation-only command for the verified fixed-reference FSI example.

mod composition;
mod core;

use serde::Deserialize;

use self::core::{FsiDemoResult, prepare_demo};
use super::{BridgeEnvelope, PROTOCOL, studio_error};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FsiDemoRequest {
    protocol: String,
}

#[tauri::command]
pub(super) async fn run_fsi_demo(
    request: FsiDemoRequest,
) -> Result<BridgeEnvelope<FsiDemoResult>, ()> {
    if request.protocol != PROTOCOL {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "unsupported Studio FSI-demo request protocol",
        )]));
    }
    match tauri::async_runtime::spawn_blocking(prepare_demo).await {
        Ok(Ok(result)) => Ok(BridgeEnvelope::success(result)),
        Ok(Err(message)) => Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0003", message,
        )])),
        Err(error) => Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0001",
            format!("native FSI-demo worker failed: {error}"),
        )])),
    }
}
